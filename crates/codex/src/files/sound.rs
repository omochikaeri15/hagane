use std::fmt;

pub const MUSIC: &str = "ogg";
pub const EFFECT: &str = "caf";
pub const LINEAR_PCM: [u8; 4] = *b"lpcm";
pub const FLOAT: u32 = 1;
pub const LITTLE_ENDIAN: u32 = 2;

const MAGIC: &[u8; 4] = b"caff";
const HEADER: usize = 8;
const CHUNK_HEADER: usize = 12;
const DESCRIPTION: &[u8; 4] = b"desc";
const DESCRIPTION_SIZE: usize = 32;
const DATA: &[u8; 4] = b"data";
const EDIT_COUNT: usize = 4;
const SAMPLE_BITS: u32 = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CafError {
    NotCaf,
    NoDescription,
    NoData,
}

impl fmt::Display for CafError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CafError::NotCaf => write!(f, "The provided file bytes do not start a CAF file."),
            CafError::NoDescription => write!(f, "The CAF file has no complete audio description before its audio data."),
            CafError::NoData => write!(f, "The CAF file has no audio data."),
        }
    }
}

impl std::error::Error for CafError {}

pub fn names(id: u32, extension: &str) -> [String; 2] {
    [format!("snd{id:03}.{extension}"), format!("{id:03}.{extension}")]
}

#[derive(Debug, Clone, PartialEq)]
pub struct Caf {
    pub sample_rate: f64,
    pub format_id: [u8; 4],
    pub format_flags: u32,
    pub bytes_per_packet: u32,
    pub frames_per_packet: u32,
    pub channels_per_frame: u32,
    pub bits_per_channel: u32,
    pub edit_count: u32,
    pub data: Vec<u8>,
}

fn word(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

impl Caf {
    pub fn parse(bytes: impl AsRef<[u8]>) -> Result<Self, CafError> {
        Self::parse_inner(bytes.as_ref())
    }

    fn parse_inner(bytes: &[u8]) -> Result<Self, CafError> {
        if bytes.get(..MAGIC.len()) != Some(MAGIC.as_slice()) {
            return Err(CafError::NotCaf);
        }

        let mut at = HEADER;
        let mut described: Option<&[u8]> = None;

        while let Some(header) = bytes.get(at..at + CHUNK_HEADER) {
            let size = header.get(4..).and_then(|size| size.try_into().ok()).map_or(0, i64::from_be_bytes);
            let body = at + CHUNK_HEADER;
            let end = usize::try_from(size).ok().and_then(|size| body.checked_add(size)).map_or(bytes.len(), |end| end.min(bytes.len()));

            if header.starts_with(DESCRIPTION) {
                described = bytes.get(body..body + DESCRIPTION_SIZE);
            }

            if header.starts_with(DATA) {
                let description = described.ok_or(CafError::NoDescription)?;
                let field = |at: usize| word(description, at).ok_or(CafError::NoDescription);
                let rate = description.get(..8).and_then(|rate| rate.try_into().ok()).map(f64::from_be_bytes).ok_or(CafError::NoDescription)?;
                let format_id = description.get(8..12).and_then(|id| id.try_into().ok()).ok_or(CafError::NoDescription)?;

                return Ok(Self {
                    sample_rate: rate,
                    format_id,
                    format_flags: field(12)?,
                    bytes_per_packet: field(16)?,
                    frames_per_packet: field(20)?,
                    channels_per_frame: field(24)?,
                    bits_per_channel: field(28)?,
                    edit_count: word(bytes, body).ok_or(CafError::NoData)?,
                    data: bytes.get(body + EDIT_COUNT..end).ok_or(CafError::NoData)?.to_vec(),
                });
            }

            if size < 0 {
                break;
            }

            at = end;
        }

        Err(CafError::NoData)
    }

    pub fn samples(&self) -> Option<Vec<i16>> {
        if self.format_id != LINEAR_PCM || self.format_flags & FLOAT != 0 || self.bits_per_channel != SAMPLE_BITS {
            return None;
        }

        let little = self.format_flags & LITTLE_ENDIAN != 0;

        Some(self.data.as_chunks::<2>().0.iter().map(|word| if little { i16::from_le_bytes(*word) } else { i16::from_be_bytes(*word) }).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(flags: u32, size: i64, samples: &[u8]) -> Vec<u8> {
        let mut bytes = b"caff\x00\x01\x00\x00".to_vec();

        bytes.extend(b"desc");
        bytes.extend(32i64.to_be_bytes());
        bytes.extend(44100f64.to_be_bytes());
        bytes.extend(b"lpcm");

        for value in [flags, 2, 1, 1, 16] {
            bytes.extend(value.to_be_bytes());
        }

        bytes.extend(b"free");
        bytes.extend(3i64.to_be_bytes());
        bytes.extend([0; 3]);
        bytes.extend(b"data");
        bytes.extend(size.to_be_bytes());
        bytes.extend(0u32.to_be_bytes());
        bytes.extend(samples);
        bytes
    }

    #[test]
    fn the_game_asks_for_the_snd_name_first() {
        assert_eq!(names(5, MUSIC), ["snd005.ogg", "005.ogg"]);
        assert_eq!(names(1234, EFFECT), ["snd1234.caf", "1234.caf"]);
    }

    // A data chunk may give its size or say -1 for "the rest of the file", and either byte order has to read right.
    #[test]
    fn samples_come_out_in_either_byte_order() {
        let little = Caf::parse(file(LITTLE_ENDIAN, 8, &[1, 0, 0xff, 0xff, 9, 9, 9, 9])).unwrap();
        let big = Caf::parse(file(0, -1, &[0, 1, 0xff, 0xff])).unwrap();

        assert_eq!((little.sample_rate, little.channels_per_frame, little.bits_per_channel), (44100.0, 1, 16));
        assert_eq!(little.samples(), Some(vec![1, -1]));
        assert_eq!(big.samples(), Some(vec![1, -1]));
        assert_eq!(Caf::parse(file(FLOAT, -1, &[0; 4])).unwrap().samples(), None);
        assert_eq!(Caf::parse(b"OggS"), Err(CafError::NotCaf));
        assert_eq!(Caf::parse(b"caff\x00\x01\x00\x00"), Err(CafError::NoData));
    }
}
