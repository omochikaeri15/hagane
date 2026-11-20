mod lzw;
mod quantize;

use std::fs;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use tracing::error;

use super::frames::{self, Chain, Intake, Rect, Still, Take};

const HEADER: &[u8; 6] = b"GIF89a";
const LOOP_FOREVER: [u8; 19] = [0x21, 0xFF, 0x0B, b'N', b'E', b'T', b'S', b'C', b'A', b'P', b'E', b'2', b'.', b'0', 0x03, 0x01, 0x00, 0x00, 0x00];
const TRAILER: u8 = 0x3B;
const OPAQUE_FROM: u8 = 128;
const MOST_COLORS: usize = 256;
const LOOSEST_SHARE: f64 = 0.004;
const FULL_FIDELITY_AT: u32 = 50;
const SMALLEST_SHARED: usize = 16384;
const DISPOSE_KEEP: u8 = 1;
const DISPOSE_CLEAR: u8 = 2;
const SHORTEST_DELAY: u32 = 2;
const DEEPEST_REFINE: u32 = 4;
const RUN_LENGTH: usize = 60;
const PIXEL: usize = 4;

#[derive(Clone, Copy)]
struct Tuning {
    share: f64,
    rounds: u32,
}

impl Tuning {
    fn new(quality_percent: u32, compression_percent: u32) -> Self {
        let below = FULL_FIDELITY_AT.saturating_sub(quality_percent.clamp(1, 100));
        let share = LOOSEST_SHARE * f64::from(below) / f64::from(FULL_FIDELITY_AT - 1);

        Self { share, rounds: 1 + (compression_percent.min(100) * (DEEPEST_REFINE - 1) + 50) / 100 }
    }
}

struct Picture {
    depth: u32,
    colors: Vec<[u8; 3]>,
    clear: Option<u8>,
    data: Vec<u8>,
    rect: Rect,
    dispose: u8,
}

fn solid(texel: &[u8; PIXEL]) -> bool {
    texel[3] >= OPAQUE_FROM
}

fn same(one: &[u8; PIXEL], other: &[u8; PIXEL]) -> bool {
    solid(one) == solid(other) && (!solid(one) || one[..3] == other[..3])
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

struct Span {
    bounds: Option<(usize, usize, usize, usize)>,
}

impl Span {
    fn mark(&mut self, y: usize, first: usize, last: usize) {
        self.bounds = Some(self.bounds.map_or((first, y, last, y), |(x0, y0, x1, _)| (x0.min(first), y0, x1.max(last), y)));
    }

    fn rect(self) -> Option<Rect> {
        self.bounds.map(|(x0, y0, x1, y1)| Rect { x: x0 as u32, y: y0 as u32, width: (x1 - x0 + 1) as u32, height: (y1 - y0 + 1) as u32 })
    }
}

fn row(pixels: &[u8], width: usize, y: usize, left: usize, right: usize) -> &[[u8; PIXEL]] {
    pixels.get((y * width + left) * PIXEL..(y * width + right) * PIXEL).map_or(&[], |slice| slice.as_chunks::<PIXEL>().0)
}

struct Draft {
    pixels: Vec<u8>,
    rect: Rect,
    dispose: u8,
}

struct Strip {
    width: usize,
    opaque: bool,
    cleared: Option<Rect>,
    canvas: Vec<[u8; PIXEL]>,
}

impl Strip {
    fn canvas(&mut self, before: Option<&Still<'_>>, y: usize, left: usize, right: usize) -> &[[u8; PIXEL]] {
        self.canvas.clear();

        let Some(before) = before else {
            self.canvas.resize(right - left, [0; PIXEL]);
            return &self.canvas;
        };

        self.canvas.extend_from_slice(row(before.pixels, self.width, y, left, right));

        if let Some(cleared) = self.cleared.filter(|cleared| (cleared.y as usize..(cleared.y + cleared.height) as usize).contains(&y)) {
            let start = (cleared.x as usize).clamp(left, right) - left;
            let end = ((cleared.x + cleared.width) as usize).clamp(left, right) - left;

            self.canvas[start..end].fill([0; PIXEL]);
        }

        &self.canvas
    }
}

impl Chain for Strip {
    type Payload = Draft;

    fn push(&mut self, before: Option<&Still<'_>>, now: &Still<'_>, next: Option<&Still<'_>>) -> Option<Draft> {
        let width = self.width;
        let mut need = Span { bounds: None };

        if let Some(region) = join(now.content, before.and_then(|held| held.content)) {
            let (left, right) = (region.x as usize, (region.x + region.width) as usize);

            for y in region.y as usize..(region.y + region.height) as usize {
                let fresh = row(now.pixels, width, y, left, right);
                let canvas = self.canvas(before, y, left, right);
                let pairs = || canvas.iter().zip(fresh);

                if let Some(first) = pairs().position(|(was, is)| !same(was, is)) {
                    let last = pairs().rposition(|(was, is)| !same(was, is)).unwrap_or(first);

                    need.mark(y, left + first, left + last);
                }
            }
        }

        let mut vanish = Span { bounds: None };

        if let (Some(after), Some(region), false) = (next, now.content, self.opaque) {
            let (left, right) = (region.x as usize, (region.x + region.width) as usize);

            for y in region.y as usize..(region.y + region.height) as usize {
                let pairs = || row(now.pixels, width, y, left, right).iter().zip(row(after.pixels, width, y, left, right));

                if let Some(first) = pairs().position(|(is, will)| solid(is) && !solid(will)) {
                    let last = pairs().rposition(|(is, will)| solid(is) && !solid(will)).unwrap_or(first);

                    vanish.mark(y, left + first, left + last);
                }
            }
        }

        let vanish = vanish.rect();
        let clears = !self.opaque && (next.is_none() || vanish.is_some());
        let rect = join(join(need.rect(), vanish), now.content.filter(|_| clears && next.is_none())).unwrap_or(Rect { x: 0, y: 0, width: 1, height: 1 });
        let mut pixels = Vec::with_capacity(rect.width as usize * rect.height as usize * PIXEL);

        let (left, right) = (rect.x as usize, (rect.x + rect.width) as usize);

        for y in rect.y as usize..(rect.y + rect.height) as usize {
            let fresh = row(now.pixels, width, y, left, right);
            let canvas = self.canvas(before, y, left, right);

            for (was, is) in canvas.iter().zip(fresh) {
                pixels.extend_from_slice(if same(was, is) { &[0; PIXEL] } else { is });
            }
        }

        self.cleared = clears.then_some(rect);

        Some(Draft { pixels, rect, dispose: if clears { DISPOSE_CLEAR } else { DISPOSE_KEEP } })
    }
}

fn draw(pixels: &[u8], tuning: Tuning, rect: Rect, dispose: u8) -> Picture {
    let texels = || pixels.as_chunks::<PIXEL>().0.iter();
    let holes = texels().any(|texel| !solid(texel));
    let wanted = if holes { MOST_COLORS - 1 } else { MOST_COLORS };
    let share = if texels().filter(|texel| solid(texel)).count() < SMALLEST_SHARED { 0.0 } else { tuning.share };
    let palette = quantize::palette(texels().filter(|texel| solid(texel)).map(|texel| [texel[0], texel[1], texel[2]]), wanted, tuning.rounds, share);
    let clear = holes.then_some(palette.colors.len() as u8);
    let indices: Vec<u8> = texels().map(|texel| clear.filter(|_| !solid(texel)).unwrap_or_else(|| palette.index([texel[0], texel[1], texel[2]]))).collect();
    let used = palette.colors.len() + usize::from(holes);
    let depth = used.next_power_of_two().trailing_zeros().max(1);

    Picture { depth, colors: palette.colors, clear, data: lzw::compress(&indices, depth.max(2)), rect, dispose }
}

fn hundredths(ticks: u32, fps: u32) -> u32 {
    ((u64::from(ticks) * 200 + u64::from(fps)) / (u64::from(fps) * 2)) as u32
}

fn write(path: &Path, width: u32, height: u32, fps: u32, takes: &[Take<Picture>]) -> io::Result<()> {
    let side = |value: u32| u16::try_from(value).map(u16::to_le_bytes).map_err(|_| io::Error::other("the canvas is too large for a GIF"));
    let mut out = BufWriter::new(fs::File::create(path)?);
    let rate = fps.max(1);
    let mut elapsed = 0u32;
    let mut written = 0u32;
    let mut block = Vec::new();

    out.write_all(HEADER)?;
    out.write_all(&side(width)?)?;
    out.write_all(&side(height)?)?;
    out.write_all(&[0x70, 0, 0])?;
    out.write_all(&LOOP_FOREVER)?;

    for take in takes {
        let picture = &take.payload;

        elapsed += take.ticks;

        let delay = hundredths(elapsed, rate).saturating_sub(written).max(SHORTEST_DELAY);

        written += delay;
        out.write_all(&[0x21, 0xF9, 0x04, (picture.dispose << 2) | u8::from(picture.clear.is_some())])?;
        out.write_all(&u16::try_from(delay).unwrap_or(u16::MAX).to_le_bytes())?;
        out.write_all(&[picture.clear.unwrap_or(0), 0, 0x2C])?;
        out.write_all(&side(picture.rect.x)?)?;
        out.write_all(&side(picture.rect.y)?)?;
        out.write_all(&side(picture.rect.width)?)?;
        out.write_all(&side(picture.rect.height)?)?;
        out.write_all(&[0x80 | (picture.depth - 1) as u8])?;

        block.clear();
        block.extend(picture.colors.iter().flatten());
        block.resize(3 << picture.depth, 0);
        block.push(picture.depth.max(2) as u8);
        lzw::blocks(&picture.data, &mut block);
        out.write_all(&block)?;
    }

    out.write_all(&[TRAILER])?;
    out.flush()
}

pub(crate) fn export(intake: Intake<'_>, path: &Path) -> bool {
    let config = intake.config;
    let tuning = Tuning::new(config.quality_percent, config.compression_percent);
    let (width, opaque) = (config.width as usize, config.background);
    let finish = |draft: Draft| Some(draw(&draft.pixels, tuning, draft.rect, draft.dispose));
    let Some(takes) = frames::runs(intake, RUN_LENGTH, |_, _| Strip { width, opaque, cleared: None, canvas: Vec::new() }, finish, None) else {
        return false;
    };

    write(path, config.width, config.height, config.fps, &takes).inspect_err(|err| error!("Failed to write the GIF: {err}")).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delays_keep_the_clip_at_its_real_length() {
        // 33.3 ms frames cannot be written exactly, so the rounding is spread instead of dropped
        let total: u32 = (1..=30).map(|tick| hundredths(tick, 30) - hundredths(tick - 1, 30)).sum();

        assert_eq!(total, 100);
        assert!((1..=30).all(|tick| (3..=4).contains(&(hundredths(tick, 30) - hundredths(tick - 1, 30)))));
    }
}
