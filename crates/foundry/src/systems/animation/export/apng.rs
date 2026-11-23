use std::fs;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use flate2::write::ZlibEncoder;
use flate2::{Compression, Crc};
use tracing::error;

use super::frames::{self, Chain, Intake, Rect, Still, Take, Weighed};

const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
const RGBA_8_BIT: [u8; 5] = [8, 6, 0, 0, 0];
const PIXEL: usize = 4;
const LOOP_FOREVER: u32 = 0;
const DISPOSE_NONE: u8 = 0;
const DISPOSE_BACKGROUND: u8 = 1;
const BLEND_SOURCE: u8 = 0;
const BLEND_OVER: u8 = 1;
const RUN_LENGTH: usize = 60;
const FILTERS: usize = 5;
const UP: u8 = 2;
const ADAPTIVE_FROM: u32 = 10;
const DEFLATE_FROM: u32 = 20;
const LIGHTEST_LEVEL: u32 = 4;
const DEEPEST_LEVEL: u32 = 8;

#[derive(Clone, Copy)]
struct Plan {
    adaptive: bool,
    level: Option<u32>,
}

impl Plan {
    fn new(compression_percent: u32) -> Self {
        let effort = compression_percent.min(100);
        let span = 100 - DEFLATE_FROM;
        let level = (effort >= DEFLATE_FROM).then(|| LIGHTEST_LEVEL + ((effort - DEFLATE_FROM) * (DEEPEST_LEVEL - LIGHTEST_LEVEL) + span / 2) / span);

        Self { adaptive: effort >= ADAPTIVE_FROM, level }
    }
}

fn paeth(left: u8, above: u8, corner: u8) -> u8 {
    let (a, b, c) = (i16::from(left), i16::from(above), i16::from(corner));
    let estimate = a + b - c;
    let (da, db, dc) = ((estimate - a).abs(), (estimate - b).abs(), (estimate - c).abs());

    if da <= db && da <= dc {
        left
    } else if db <= dc {
        above
    } else {
        corner
    }
}

fn filtered(kind: u8, row: &[u8], above: &[u8], out: &mut Vec<u8>) {
    out.clear();
    out.push(kind);

    match kind {
        0 => out.extend_from_slice(row),
        1 => {
            out.extend_from_slice(&row[..PIXEL.min(row.len())]);
            out.extend(row.iter().skip(PIXEL).zip(row).map(|(value, left)| value.wrapping_sub(*left)));
        }
        UP => out.extend(row.iter().zip(above).map(|(value, up)| value.wrapping_sub(*up))),
        3 => {
            out.extend(row.iter().zip(above).take(PIXEL).map(|(value, up)| value.wrapping_sub(up >> 1)));
            out.extend(row.iter().skip(PIXEL).zip(row).zip(above.iter().skip(PIXEL)).map(|((value, left), up)| value.wrapping_sub(((u16::from(*left) + u16::from(*up)) >> 1) as u8)));
        }
        _ => {
            out.extend(row.iter().zip(above).take(PIXEL).map(|(value, up)| value.wrapping_sub(*up)));
            out.extend(row.iter().skip(PIXEL).zip(row).zip(above.iter().skip(PIXEL).zip(above)).map(|((value, left), (up, corner))| value.wrapping_sub(paeth(*left, *up, *corner))));
        }
    }
}

fn weight(line: &[u8]) -> u64 {
    line.iter().skip(1).map(|value| u64::from((*value as i8).unsigned_abs())).sum()
}

struct Cel {
    data: Vec<u8>,
    rect: Rect,
    dispose: u8,
    blend: u8,
}

impl Weighed for Cel {
    fn weight(&self) -> usize {
        self.data.len()
    }
}

struct Draft {
    pixels: Vec<u8>,
    rect: Rect,
    dispose: u8,
    blend: u8,
}

fn serialize(pixels: &[u8], width: usize, plan: Plan) -> Vec<u8> {
    let stride = width * PIXEL;
    let blank = vec![0u8; stride];
    let mut raw = Vec::with_capacity(pixels.len() + pixels.len() / stride.max(1));
    let mut trial: [Vec<u8>; FILTERS] = std::array::from_fn(|_| Vec::with_capacity(stride + 1));
    let mut above: &[u8] = &blank;

    for row in pixels.chunks_exact(stride.max(1)) {
        if !plan.adaptive {
            filtered(UP, row, above, &mut trial[0]);
            raw.extend_from_slice(&trial[0]);
        } else if row == above {
            raw.push(UP);
            raw.extend_from_slice(&blank);
        } else {
            for (kind, line) in trial.iter_mut().enumerate() {
                filtered(kind as u8, row, above, line);
            }

            let best = trial.iter().min_by_key(|line| weight(line)).unwrap_or(&trial[0]);

            raw.extend_from_slice(best);
        }

        above = row;
    }

    raw
}

fn compress(pixels: &[u8], width: usize, plan: Plan) -> Option<Vec<u8>> {
    let raw = serialize(pixels, width, plan);
    let Some(level) = plan.level else {
        return Some(fdeflate::compress_to_vec(&raw));
    };
    let mut encoder = ZlibEncoder::new(Vec::with_capacity(raw.len() / 4), Compression::new(level));

    encoder.write_all(&raw).and_then(|()| encoder.finish()).inspect_err(|err| error!("Failed to compress an APNG frame: {err}")).ok()
}

fn join(one: Option<Rect>, other: Option<Rect>) -> Option<Rect> {
    match (one, other) {
        (Some(one), Some(other)) => {
            let x = one.x.min(other.x);
            let y = one.y.min(other.y);

            Some(Rect { x, y, width: (one.x + one.width).max(other.x + other.width) - x, height: (one.y + one.height).max(other.y + other.height) - y })
        }
        (one, other) => one.or(other),
    }
}

struct Change {
    bounds: Option<Rect>,
    blendable: bool,
}

fn changes(before: Option<&[u8]>, now: &[u8], width: u32, region: Option<Rect>) -> Change {
    let Some(region) = region else {
        return Change { bounds: None, blendable: true };
    };
    let stride = width as usize;
    let (left, right) = (region.x as usize, (region.x + region.width) as usize);
    let mut bounds: Option<(usize, usize, usize, usize)> = None;
    let mut blendable = true;

    for y in region.y as usize..(region.y + region.height) as usize {
        let row = y * stride;
        let fresh = &now[(row + left) * PIXEL..(row + right) * PIXEL];
        let Some(before) = before else {
            let texels = fresh.as_chunks::<PIXEL>().0;

            if let Some(first) = texels.iter().position(|texel| *texel != [0; PIXEL]) {
                let last = texels.iter().rposition(|texel| *texel != [0; PIXEL]).unwrap_or(first);
                bounds = Some(bounds.map_or((left + first, y, left + last, y), |(x0, y0, x1, _)| (x0.min(left + first), y0, x1.max(left + last), y)));
            }

            continue;
        };
        let old = &before[(row + left) * PIXEL..(row + right) * PIXEL];

        if old == fresh {
            continue;
        }

        let pairs = || old.as_chunks::<PIXEL>().0.iter().zip(fresh.as_chunks::<PIXEL>().0);
        let first = pairs().position(|(was, is)| was != is).unwrap_or(0);
        let last = pairs().rposition(|(was, is)| was != is).unwrap_or(first);

        blendable &= pairs().skip(first).take(last + 1 - first).all(|(was, is)| was == is || is[3] == u8::MAX || was[3] == 0);
        bounds = Some(bounds.map_or((left + first, y, left + last, y), |(x0, y0, x1, _)| (x0.min(left + first), y0, x1.max(left + last), y)));
    }

    let bounds = bounds.map(|(x0, y0, x1, y1)| Rect { x: x0 as u32, y: y0 as u32, width: (x1 - x0 + 1) as u32, height: (y1 - y0 + 1) as u32 });

    Change { bounds, blendable }
}

struct Strip {
    width: u32,
    height: u32,
    opaque: bool,
    opening: bool,
}

impl Chain for Strip {
    type Payload = Draft;

    fn push(&mut self, before: Option<&Still<'_>>, now: &Still<'_>, next: Option<&Still<'_>>) -> Option<Draft> {
        let canvas = Rect { x: 0, y: 0, width: self.width, height: self.height };
        let opening = std::mem::take(&mut self.opening);
        let clears = next.is_none() && !self.opaque;
        let region = join(now.content, before.and_then(|held| held.content));
        let change = changes(before.map(|held| held.pixels), now.pixels, self.width, region);
        let rect = if opening {
            canvas
        } else {
            join(change.bounds, now.content.filter(|_| clears)).unwrap_or(Rect { x: 0, y: 0, width: 1, height: 1 })
        };
        let blend = if before.is_some() && change.blendable && !opening { BLEND_OVER } else { BLEND_SOURCE };
        let stride = self.width as usize * PIXEL;
        let (left, right) = (rect.x as usize * PIXEL, (rect.x + rect.width) as usize * PIXEL);
        let mut pixels = Vec::with_capacity(rect.width as usize * rect.height as usize * PIXEL);

        for y in rect.y as usize..(rect.y + rect.height) as usize {
            let fresh = &now.pixels[y * stride + left..y * stride + right];
            let Some(old) = before.filter(|_| blend == BLEND_OVER).map(|held| &held.pixels[y * stride + left..y * stride + right]) else {
                pixels.extend_from_slice(fresh);
                continue;
            };

            for (was, is) in old.as_chunks::<PIXEL>().0.iter().zip(fresh.as_chunks::<PIXEL>().0) {
                pixels.extend_from_slice(if was == is { &[0; PIXEL] } else { is });
            }
        }

        Some(Draft { pixels, rect, dispose: if clears { DISPOSE_BACKGROUND } else { DISPOSE_NONE }, blend })
    }
}

fn chunk(out: &mut impl Write, kind: &[u8; 4], parts: &[&[u8]]) -> io::Result<()> {
    let length = u32::try_from(parts.iter().map(|part| part.len()).sum::<usize>()).map_err(|_| io::Error::other("PNG chunk is too large"))?;
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

fn write(path: &Path, width: u32, height: u32, fps: u32, takes: &[Take<Cel>]) -> io::Result<()> {
    let mut out = BufWriter::new(fs::File::create(path)?);
    let rate = u16::try_from(fps.max(1)).unwrap_or(u16::MAX);
    let mut sequence = 0u32;

    out.write_all(&SIGNATURE)?;
    chunk(&mut out, b"IHDR", &[&width.to_be_bytes(), &height.to_be_bytes(), &RGBA_8_BIT])?;
    chunk(&mut out, b"acTL", &[&(takes.len() as u32).to_be_bytes(), &LOOP_FOREVER.to_be_bytes()])?;

    for (index, take) in takes.iter().enumerate() {
        let cel = &take.payload;
        let ticks = u16::try_from(take.ticks).unwrap_or(u16::MAX);

        chunk(&mut out, b"fcTL", &[
            &sequence.to_be_bytes(),
            &cel.rect.width.to_be_bytes(),
            &cel.rect.height.to_be_bytes(),
            &cel.rect.x.to_be_bytes(),
            &cel.rect.y.to_be_bytes(),
            &ticks.to_be_bytes(),
            &rate.to_be_bytes(),
            &[cel.dispose, cel.blend],
        ])?;
        sequence += 1;

        if index == 0 {
            chunk(&mut out, b"IDAT", &[cel.data.as_slice()])?;
        } else {
            chunk(&mut out, b"fdAT", &[&sequence.to_be_bytes(), cel.data.as_slice()])?;
            sequence += 1;
        }
    }

    chunk(&mut out, b"IEND", &[])?;
    out.flush()
}

pub(crate) fn export(intake: Intake<'_>, path: &Path) -> bool {
    let config = intake.config;
    let plan = Plan::new(config.compression_percent);
    let (width, height, opaque) = (config.width, config.height, config.background);
    let finish = |draft: Draft| compress(&draft.pixels, draft.rect.width as usize, plan).map(|data| Cel { data, rect: draft.rect, dispose: draft.dispose, blend: draft.blend });
    let Some(takes) = frames::runs(intake, RUN_LENGTH, |_, opening| Strip { width, height, opaque, opening }, finish, None) else {
        return false;
    };

    write(path, width, height, config.fps, &takes).inspect_err(|err| error!("Failed to write the APNG: {err}")).is_ok()
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;
    use std::sync::mpsc;

    use image::AnimationDecoder;
    use image::codecs::png::PngDecoder;

    use crate::systems::animation::export::{EncoderMessage, ExportConfig, ExportFormat};

    use super::*;

    const WIDTH: u32 = 7;
    const HEIGHT: u32 = 6;

    fn canvas(dots: &[(u32, u32, [u8; 4])]) -> Vec<u8> {
        let mut pixels = vec![0u8; (WIDTH * HEIGHT) as usize * PIXEL];

        for (x, y, rgba) in dots {
            let at = (y * WIDTH + x) as usize * PIXEL;
            pixels[at..at + PIXEL].copy_from_slice(rgba);
        }

        pixels
    }

    fn flipped(pixels: &[u8]) -> Vec<u8> {
        pixels.as_chunks::<{ WIDTH as usize * PIXEL }>().0.iter().rev().flatten().copied().collect()
    }

    #[test]
    fn every_frame_decodes_back_exactly_and_repeats_merge() {
        // opaque colors only, so straightening leaves them alone
        let first = canvas(&[(1, 1, [200, 10, 10, 255]), (2, 3, [0, 90, 255, 255])]);
        let second = canvas(&[(5, 4, [9, 8, 7, 255])]);
        let empty = canvas(&[]);
        let shown = [&first, &second, &second, &empty, &first];

        for compression in [0, 30, 100] {
            let path = std::env::temp_dir().join(format!("hagane-apng-{}-{compression}.png", std::process::id()));
            let config = ExportConfig { width: WIDTH, height: HEIGHT, format: ExportFormat::Png, quality_percent: 80, compression_percent: compression, fps: 30, start_frame: 0, end_frame: 4, output_path: path.clone(), base_name: String::new(), background: false };
            let (tx, rx) = mpsc::channel();

            for frame in shown {
                tx.send(EncoderMessage::Frame(flipped(frame), WIDTH, HEIGHT, 33)).unwrap();
            }
            tx.send(EncoderMessage::Finish).unwrap();

            let abort = AtomicBool::new(false);
            assert!(export(Intake { config: &config, receiver: rx, emit: &|_| {}, abort: &abort }, &path));

            let file = io::BufReader::new(fs::File::open(&path).unwrap());
            let decoded = PngDecoder::new(file).unwrap().apng().unwrap().into_frames().collect_frames().unwrap();
            let _ = fs::remove_file(&path);

            // the two identical neighbors become one frame shown twice as long
            let expected = [(&first, 1), (&second, 2), (&empty, 1), (&first, 1)];
            assert_eq!(decoded.len(), expected.len());

            for (frame, (pixels, ticks)) in decoded.iter().zip(expected) {
                let (numer, denom) = frame.delay().numer_denom_ms();
                assert_eq!(numer * 30, ticks * 1000 * denom);
                assert_eq!(frame.buffer().as_raw(), pixels);
            }
        }
    }
}
