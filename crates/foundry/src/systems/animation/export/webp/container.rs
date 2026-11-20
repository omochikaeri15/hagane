use super::Placed;

const ANIMATED: u8 = 0x02;
const ALPHA: u8 = 0x10;
const LIMIT: u32 = 0x00ff_ffff;
const BLEND: u8 = 0x00;
const COVER: u8 = 0x02;
pub(super) const HEADER: usize = 13;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct Rect {
    pub(super) x: usize,
    pub(super) y: usize,
    pub(super) width: usize,
    pub(super) height: usize,
}

pub(super) fn chunk(out: &mut Vec<u8>, tag: [u8; 4], body: &[u8]) {
    out.extend_from_slice(&tag);
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(body);
    if body.len() % 2 == 1 {
        out.push(0);
    }
}

fn triple(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.min(LIMIT).to_le_bytes()[..3]);
}

pub(super) fn header(rect: Rect, blend: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER);
    triple(&mut out, rect.x as u32 / 2);
    triple(&mut out, rect.y as u32 / 2);
    triple(&mut out, (rect.width as u32).saturating_sub(1));
    triple(&mut out, (rect.height as u32).saturating_sub(1));
    out.push(if blend { BLEND } else { COVER });
    out
}

pub(super) fn assemble(canvas_width: u32, canvas_height: u32, ticks_per_second: u32, frames: &[Placed<'_>]) -> Vec<u8> {
    let total: usize = frames.iter().map(|frame| frame.payload.len() + 16).sum();
    let mut out = Vec::with_capacity(total + 64);
    out.extend_from_slice(b"RIFF\0\0\0\0WEBPVP8X");
    out.extend_from_slice(&10u32.to_le_bytes());
    out.extend_from_slice(&[ANIMATED | ALPHA, 0, 0, 0]);
    triple(&mut out, canvas_width.saturating_sub(1));
    triple(&mut out, canvas_height.saturating_sub(1));
    chunk(&mut out, *b"ANIM", &[0; 6]);
    let rate = u64::from(ticks_per_second.max(1));
    let mut ticks = 0u64;
    let mut shown = 0u64;
    for frame in frames {
        let Some((place, body)) = frame.payload.split_at_checked(HEADER) else {
            continue;
        };
        ticks += u64::from(frame.ticks);
        let end = (ticks * 1000 + rate / 2) / rate;
        let duration = (end - shown).min(u64::from(LIMIT)) as u32;
        shown = end;
        out.extend_from_slice(b"ANMF");
        out.extend_from_slice(&(body.len() as u32 + 16).to_le_bytes());
        out.extend_from_slice(&place[..12]);
        triple(&mut out, duration);
        out.push(place[12]);
        out.extend_from_slice(body);
    }
    let size = (out.len() - 8) as u32;
    out[4..8].copy_from_slice(&size.to_le_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::super::Placed;
    use super::{Rect, assemble, header};

    #[test]
    fn frames_keep_total_length() {
        // 33.33 ms ticks cannot be written exactly, so the rounding has to be spread instead of drifting.
        let mut payload = header(Rect { x: 2, y: 4, width: 6, height: 8 }, false);
        payload.extend_from_slice(&[b'V', b'P', b'8', b'L', 2, 0, 0, 0, 0x2f, 0]);
        let ticks: Vec<u32> = (0..100).map(|index| 1 + index % 3).collect();
        let frames: Vec<Placed<'_>> = ticks.iter().map(|&length| Placed { ticks: length, payload: &payload }).collect();
        let file = assemble(64, 48, 30, &frames);
        assert_eq!(&file[..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(file[4..8].try_into().unwrap()) as usize, file.len() - 8);
        assert_eq!(file[20], 0x12);
        assert_eq!((file[24], file[27]), (63, 47));
        let mut at = 30 + 14;
        let mut total = 0u32;
        let mut seen = 0;
        while at < file.len() {
            assert_eq!(&file[at..at + 4], b"ANMF");
            let size = u32::from_le_bytes(file[at + 4..at + 8].try_into().unwrap()) as usize;
            assert_eq!((file[at + 8], file[at + 11], file[at + 14], file[at + 17]), (1, 2, 5, 7));
            total += u32::from_le_bytes([file[at + 20], file[at + 21], file[at + 22], 0]);
            assert_eq!(file[at + 23], 2);
            assert_eq!(&file[at + 24..at + 28], b"VP8L");
            at += 8 + size;
            seen += 1;
        }
        assert_eq!(seen, 100);
        assert_eq!(total, (ticks.iter().sum::<u32>() * 1000 + 15) / 30);
    }
}
