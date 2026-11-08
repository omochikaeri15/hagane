use std::fs::File;
use std::io::{self, BufWriter, Seek, SeekFrom, Write};
use std::path::Path;

use flate2::write::ZlibEncoder;
use flate2::{Compression, Crc};

const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
const RGBA_8_BIT: [u8; 5] = [8, 6, 0, 0, 0];
const CONTROL_OFFSET: u64 = 33;
const DELAY_DENOMINATOR: u16 = 1000;
const BYTES_PER_PIXEL: usize = 4;

pub(super) struct Apng {
    file: BufWriter<File>,
    width: usize,
    height: usize,
    frames: u32,
    sequence: u32,
    previous: Vec<u8>,
}

struct Region {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

impl Apng {
    pub(super) fn create(path: &Path, width: u32, height: u32) -> io::Result<Self> {
        let mut file = BufWriter::new(File::create(path)?);
        file.write_all(&SIGNATURE)?;
        write_chunk(&mut file, b"IHDR", &[&width.to_be_bytes(), &height.to_be_bytes(), &RGBA_8_BIT])?;
        write_chunk(&mut file, b"acTL", &[&0u32.to_be_bytes(), &0u32.to_be_bytes()])?;

        Ok(Self {
            file,
            width: width as usize,
            height: height as usize,
            frames: 0,
            sequence: 0,
            previous: Vec::new(),
        })
    }

    pub(super) fn push(&mut self, pixels: Vec<u8>, delay_ms: u32) -> io::Result<()> {
        if pixels.len() != self.width * self.height * BYTES_PER_PIXEL {
            return Err(io::Error::other("frame size differs from the first frame"));
        }

        let region = if self.previous.is_empty() {
            Region { x: 0, y: 0, width: self.width, height: self.height }
        } else {
            changed_region(&self.previous, &pixels, self.width)
        };

        let delay = u16::try_from(delay_ms).unwrap_or(u16::MAX);
        write_chunk(&mut self.file, b"fcTL", &[
            &self.sequence.to_be_bytes(),
            &(region.width as u32).to_be_bytes(),
            &(region.height as u32).to_be_bytes(),
            &(region.x as u32).to_be_bytes(),
            &(region.y as u32).to_be_bytes(),
            &delay.to_be_bytes(),
            &DELAY_DENOMINATOR.to_be_bytes(),
            &[0, 0],
        ])?;
        self.sequence += 1;

        let compressed = compress(&pixels, self.width, &region)?;

        if self.frames == 0 {
            write_chunk(&mut self.file, b"IDAT", &[&compressed])?;
        } else {
            write_chunk(&mut self.file, b"fdAT", &[&self.sequence.to_be_bytes(), &compressed])?;
            self.sequence += 1;
        }

        self.frames += 1;
        self.previous = pixels;
        Ok(())
    }

    pub(super) fn finish(mut self) -> io::Result<()> {
        write_chunk(&mut self.file, b"IEND", &[])?;

        let mut file = self.file.into_inner().map_err(io::IntoInnerError::into_error)?;
        file.seek(SeekFrom::Start(CONTROL_OFFSET))?;
        write_chunk(&mut file, b"acTL", &[&self.frames.to_be_bytes(), &0u32.to_be_bytes()])
    }
}

fn changed_region(previous: &[u8], current: &[u8], width: usize) -> Region {
    let stride = width * BYTES_PER_PIXEL;
    let mut rows = None;
    let mut left = width;
    let mut right = 0;

    for (y, (old, new)) in previous.chunks_exact(stride).zip(current.chunks_exact(stride)).enumerate() {
        if old == new {
            continue;
        }

        let pairs = || old.as_chunks::<BYTES_PER_PIXEL>().0.iter().zip(new.as_chunks::<BYTES_PER_PIXEL>().0);
        left = left.min(pairs().position(|(a, b)| a != b).unwrap_or(0));
        right = right.max(pairs().rposition(|(a, b)| a != b).unwrap_or(0));
        rows = Some(rows.map_or((y, y), |(top, _)| (top, y)));
    }

    rows.map_or(
        Region { x: 0, y: 0, width: 1, height: 1 },
        |(top, bottom)| Region { x: left, y: top, width: right - left + 1, height: bottom - top + 1 },
    )
}

fn compress(pixels: &[u8], width: usize, region: &Region) -> io::Result<Vec<u8>> {
    let stride = width * BYTES_PER_PIXEL;
    let start = region.x * BYTES_PER_PIXEL;
    let end = start + region.width * BYTES_PER_PIXEL;
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());

    for row in pixels.chunks_exact(stride).skip(region.y).take(region.height) {
        encoder.write_all(&[0])?;
        encoder.write_all(&row[start..end])?;
    }

    encoder.finish()
}

fn write_chunk(out: &mut impl Write, kind: &[u8; 4], parts: &[&[u8]]) -> io::Result<()> {
    let length = u32::try_from(parts.iter().map(|part| part.len()).sum::<usize>())
        .map_err(|_| io::Error::other("PNG chunk is too large"))?;
    let mut crc = Crc::new();
    crc.update(kind);

    out.write_all(&length.to_be_bytes())?;
    out.write_all(kind)?;
    for part in parts {
        crc.update(part);
        out.write_all(part)?;
    }
    out.write_all(&crc.sum().to_be_bytes())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::BufReader;

    use image::AnimationDecoder;
    use image::codecs::png::PngDecoder;

    use super::*;

    const WIDTH: u32 = 6;
    const HEIGHT: u32 = 5;

    fn paint(frame: &mut [u8], x: usize, y: usize, rgba: [u8; 4]) {
        let at = (y * WIDTH as usize + x) * BYTES_PER_PIXEL;
        frame[at..at + BYTES_PER_PIXEL].copy_from_slice(&rgba);
    }

    #[test]
    fn cropped_frames_decode_back_to_every_full_frame() {
        let first: Vec<u8> = (0..WIDTH * HEIGHT * 4).map(|i| (i * 37 % 251) as u8).collect();

        let mut moved = first.clone();
        paint(&mut moved, 2, 1, [255, 0, 0, 128]);
        paint(&mut moved, 3, 2, [0, 0, 0, 0]);

        // nothing changes here, which still has to be a valid frame
        let held = moved.clone();

        let mut corners = held.clone();
        paint(&mut corners, 0, 0, [1, 2, 3, 4]);
        paint(&mut corners, 5, 4, [5, 6, 7, 8]);

        let frames = [first, moved, held, corners];
        let path = std::env::temp_dir().join(format!("hagane-apng-{}.png", std::process::id()));

        let mut apng = Apng::create(&path, WIDTH, HEIGHT).expect("it creates");
        for frame in &frames {
            apng.push(frame.clone(), 33).expect("it writes");
        }
        apng.finish().expect("it finishes");

        let file = BufReader::new(fs::File::open(&path).expect("it opens"));
        let decoded = PngDecoder::new(file).expect("it is a PNG")
            .apng().expect("it is animated")
            .into_frames()
            .collect_frames()
            .expect("every frame decodes");
        let _ = fs::remove_file(&path);

        assert_eq!(decoded.len(), frames.len());
        for (index, (frame, expected)) in decoded.iter().zip(&frames).enumerate() {
            let (numer, denom) = frame.delay().numer_denom_ms();
            assert_eq!(numer, 33 * denom, "frame {index} lost its delay");
            assert_eq!(frame.buffer().as_raw(), expected, "frame {index} differs");
        }
    }
}
