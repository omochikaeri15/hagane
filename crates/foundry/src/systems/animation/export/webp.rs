mod container;
mod lossless;
mod lossy;

use std::{fs, path::Path};

use tracing::error;

use container::Rect;

use super::frames::{self, Chain, Intake, Still};

const LOSSLESS_ALPHA: u8 = 1;
const LOSSLESS_QUALITY: u8 = 100;
const RUN_LENGTH: usize = 64;
const WIDE_EFFORT: u8 = 70;
const SMALL_AREA: usize = 1 << 15;
const SEEDS: [u64; 2] = [0x9e37_79b9_7f4a_7c15, 0xc2b2_ae3d_27d4_eb4f];
const MIXES: [u64; 2] = [0xff51_afd7_ed55_8ccd, 0xc4ce_b9fe_1a85_ec53];

#[derive(Clone, Copy)]
struct Options {
    quality: u8,
    effort: u8,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Bounds {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

struct Frame<'a> {
    rgba: &'a [u8],
    content: Option<Bounds>,
}

struct Run {
    width: usize,
    height: usize,
    options: Options,
    before: Option<Bounds>,
    previous: Vec<u8>,
    previous_key: u128,
    seen: Vec<(u128, u128, usize)>,
    pushed: usize,
}

struct Patch {
    rect: Rect,
    blend: bool,
    canvas: usize,
    options: Options,
    pixels: Vec<u8>,
    masked: Option<Vec<u8>>,
}

enum Draft {
    Again(usize),
    Fresh(Patch),
    Done(Vec<u8>),
}

impl Draft {
    fn render(self) -> Self {
        match self {
            Self::Fresh(patch) => Self::Done(patch.render()),
            other => other,
        }
    }

    fn payload(&self) -> Option<&[u8]> {
        match self {
            Self::Done(payload) => Some(payload),
            _ => None,
        }
    }

    fn back(&self) -> Option<usize> {
        match self {
            Self::Again(back) => Some(*back),
            _ => None,
        }
    }
}

impl Patch {
    fn render(&self) -> Vec<u8> {
        let mut out = container::header(self.rect, self.blend);
        encode(&mut out, &self.pixels, self.rect, self.options, self.canvas);
        if let Some(masked) = &self.masked {
            let mut other = container::header(self.rect, true);
            encode(&mut other, masked, self.rect, self.options, self.canvas);
            if other.len() < out.len() {
                return other;
            }
        }
        out
    }
}

struct Placed<'a> {
    ticks: u32,
    payload: &'a [u8],
}

fn run_length(options: Options) -> usize {
    let Options { quality: _, effort: _ } = options;
    RUN_LENGTH
}

fn assemble(width: u32, height: u32, ticks_per_second: u32, frames: &[Placed<'_>]) -> Vec<u8> {
    container::assemble(width, height, ticks_per_second, frames)
}

impl Run {
    fn new(width: u32, height: u32, options: Options, before: Option<Bounds>) -> Self {
        Self { width: width as usize, height: height as usize, options, before, previous: Vec::new(), previous_key: 0, seen: Vec::new(), pushed: 0 }
    }

    fn plan(&mut self, frame: &Frame<'_>, last: bool) -> Draft {
        let area = self.width * self.height * 4;
        let Some(rgba) = frame.rgba.get(..area).filter(|_| area > 0) else {
            return Draft::Fresh(keep_all(self.options));
        };
        let key = digest(rgba);
        let draft = if self.previous.len() == area { self.recall(rgba, key) } else { Draft::Fresh(self.opening(rgba, frame.content)) };
        self.previous.clear();
        self.previous_key = key;
        self.pushed += 1;
        if !last {
            self.previous.extend_from_slice(rgba);
        }
        draft
    }

    fn recall(&mut self, rgba: &[u8], key: u128) -> Draft {
        if let Some(&(_, _, at)) = self.seen.iter().find(|(before, after, _)| *before == self.previous_key && *after == key) {
            return Draft::Again(self.pushed - at);
        }
        self.seen.push((self.previous_key, key, self.pushed));
        Draft::Fresh(self.delta(rgba))
    }

    fn opening(&self, rgba: &[u8], content: Option<Bounds>) -> Patch {
        let whole = Rect { x: 0, y: 0, width: self.width, height: self.height };
        let rect = self.before.map_or(whole, |before| {
            let joined = content.map_or(before, |content| join(content, before));
            even(clamp(joined, self.width, self.height).unwrap_or(whole))
        });
        Patch { rect, blend: false, canvas: self.width * self.height, options: self.options, pixels: crop(rgba, self.width, rect), masked: None }
    }

    fn delta(&self, rgba: &[u8]) -> Patch {
        let Some(changed) = changes(&self.previous, rgba, self.width) else {
            return keep_all(self.options);
        };
        let rect = even(changed);
        Patch { rect, blend: false, canvas: self.width * self.height, options: self.options, pixels: crop(rgba, self.width, rect), masked: mask(&self.previous, rgba, self.width, rect) }
    }
}

fn digest(rgba: &[u8]) -> u128 {
    let (words, rest) = rgba.as_chunks::<8>();
    let mut state = SEEDS;
    for word in words {
        let value = u64::from_le_bytes(*word);
        for (lane, mix) in state.iter_mut().zip(MIXES) {
            *lane = (*lane ^ value).wrapping_mul(mix).rotate_left(29);
        }
    }
    for &byte in rest {
        for (lane, mix) in state.iter_mut().zip(MIXES) {
            *lane = (*lane ^ u64::from(byte)).wrapping_mul(mix).rotate_left(29);
        }
    }
    u128::from(state[0]) << 64 | u128::from(state[1] ^ rgba.len() as u64)
}

fn keep_all(options: Options) -> Patch {
    Patch { rect: Rect { x: 0, y: 0, width: 1, height: 1 }, blend: true, canvas: 1, options, pixels: vec![0; 4], masked: None }
}

fn join(a: Bounds, b: Bounds) -> Bounds {
    let (x, y) = (a.x.min(b.x), a.y.min(b.y));
    let right = (a.x + a.width).max(b.x + b.width);
    let bottom = (a.y + a.height).max(b.y + b.height);
    Bounds { x, y, width: right - x, height: bottom - y }
}

fn clamp(bounds: Bounds, width: usize, height: usize) -> Option<Rect> {
    let (x, y) = (bounds.x as usize, bounds.y as usize);
    let right = (x + bounds.width as usize).min(width);
    let bottom = (y + bounds.height as usize).min(height);
    (x < right && y < bottom).then(|| Rect { x, y, width: right - x, height: bottom - y })
}

fn even(rect: Rect) -> Rect {
    let (x, y) = (rect.x & !1, rect.y & !1);
    Rect { x, y, width: rect.x + rect.width - x, height: rect.y + rect.height - y }
}

fn changes(previous: &[u8], current: &[u8], width: usize) -> Option<Rect> {
    let stride = width * 4;
    let mut top = usize::MAX;
    let mut bottom = 0;
    let mut left = width;
    let mut right = 0;
    for (y, (old, new)) in previous.chunks_exact(stride).zip(current.chunks_exact(stride)).enumerate() {
        if old == new {
            continue;
        }
        let (old, new) = (old.as_chunks::<4>().0, new.as_chunks::<4>().0);
        let first = old.iter().zip(new).position(|(a, b)| a != b).unwrap_or(0);
        let last = old.iter().zip(new).rposition(|(a, b)| a != b).unwrap_or(first);
        top = top.min(y);
        bottom = y + 1;
        left = left.min(first);
        right = right.max(last + 1);
    }
    (top < bottom).then(|| Rect { x: left, y: top, width: right - left, height: bottom - top })
}

fn crop(rgba: &[u8], width: usize, rect: Rect) -> Vec<u8> {
    let mut out = Vec::with_capacity(rect.width * rect.height * 4);
    for row in rgba.chunks_exact(width * 4).skip(rect.y).take(rect.height) {
        out.extend_from_slice(&row[rect.x * 4..(rect.x + rect.width) * 4]);
    }
    out
}

fn mask(previous: &[u8], current: &[u8], width: usize, rect: Rect) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(rect.width * rect.height * 4);
    let rows = previous.chunks_exact(width * 4).zip(current.chunks_exact(width * 4)).skip(rect.y).take(rect.height);
    let (from, to) = (rect.x * 4, (rect.x + rect.width) * 4);
    for (old, new) in rows {
        for (old, new) in old[from..to].as_chunks::<4>().0.iter().zip(new[from..to].as_chunks::<4>().0) {
            if old == new {
                out.extend_from_slice(&[0; 4]);
            } else if new[3] == 255 {
                out.extend_from_slice(new);
            } else {
                return None;
            }
        }
    }
    Some(out)
}

fn encode(out: &mut Vec<u8>, rgba: &[u8], rect: Rect, options: Options, canvas: usize) {
    let mark = out.len();
    encode_once(out, rgba, rect, options, canvas);
    if options.effort < WIDE_EFFORT || options.quality >= lossy::FLATTENING || rect.width * rect.height >= SMALL_AREA {
        return;
    }
    let mut other = Vec::new();
    encode_once(&mut other, rgba, rect, Options { effort: lossy::THOROUGH_EFFORT, ..options }, canvas);
    if other.len() < out.len() - mark {
        out.truncate(mark);
        out.extend_from_slice(&other);
    }
}

fn encode_once(out: &mut Vec<u8>, rgba: &[u8], rect: Rect, options: Options, canvas: usize) {
    let (width, height) = (rect.width, rect.height);
    let area = width * height;
    if area == 0 || rgba.len() < area * 4 {
        container::chunk(out, *b"VP8L", &lossless::encode_argb(&[0; 4], 1, 1, options.effort));
        return;
    }
    let rgba = &rgba[..area * 4];
    let alpha: Vec<u8> = rgba.as_chunks::<4>().0.iter().map(|pixel| pixel[3]).collect();
    if options.quality >= LOSSLESS_QUALITY || alpha.iter().all(|&value| value == 0) {
        container::chunk(out, *b"VP8L", &lossless::encode_argb(rgba, width, height, options.effort));
        return;
    }
    let Some(color) = lossy::encode(rgba, &alpha, width, height, options.quality, options.effort, canvas) else {
        container::chunk(out, *b"VP8L", &lossless::encode_argb(rgba, width, height, options.effort));
        return;
    };
    let mut plane = vec![LOSSLESS_ALPHA];
    plane.extend_from_slice(&lossless::encode_plane(&alpha, width, height, options.effort));
    container::chunk(out, *b"ALPH", &plane);
    container::chunk(out, *b"VP8 ", &color);
}

pub(crate) fn export(intake: Intake<'_>, path: &Path) -> bool {
    let config = intake.config;
    let options = Options { quality: config.quality_percent.clamp(1, 100) as u8, effort: config.compression_percent.min(100) as u8 };
    let (width, height) = (config.width, config.height);
    let deferred = rayon::current_num_threads() > 1;
    let Some(takes) = frames::runs(intake, run_length(options), |before, _| Planner { run: Run::new(width, height, options, before.map(bounds)), deferred }, |draft: Draft| Some(draft.render()), None) else {
        return false;
    };
    let mut payloads: Vec<&[u8]> = Vec::with_capacity(takes.len());

    for take in &takes {
        let earlier = take.payload.back().and_then(|back| payloads.len().checked_sub(back)).and_then(|from| payloads.get(from).copied());
        let Some(payload) = take.payload.payload().or(earlier) else {
            error!("A WebP frame was left without a payload.");
            return false;
        };

        payloads.push(payload);
    }

    let placed: Vec<Placed<'_>> = takes.iter().zip(payloads).map(|(take, payload)| Placed { ticks: take.ticks, payload }).collect();

    fs::write(path, assemble(width, height, config.fps, &placed)).inspect_err(|err| error!("Failed to write the WebP: {err}")).is_ok()
}

fn bounds(rect: frames::Rect) -> Bounds {
    Bounds { x: rect.x, y: rect.y, width: rect.width, height: rect.height }
}

struct Planner {
    run: Run,
    deferred: bool,
}

impl Chain for Planner {
    type Payload = Draft;

    fn push(&mut self, _: Option<&Still<'_>>, now: &Still<'_>, next: Option<&Still<'_>>) -> Option<Draft> {
        let draft = self.run.plan(&Frame { rgba: now.pixels, content: now.content.map(bounds) }, next.is_none());

        Some(if self.deferred { draft } else { draft.render() })
    }
}

#[cfg(test)]
mod tests {
    use super::{Bounds, Frame, Options, Run, assemble, Placed};

    fn push(run: &mut Run, rgba: &[u8], content: Option<Bounds>, last: bool) -> Vec<u8> {
        run.plan(&Frame { rgba, content }, last).render().payload().unwrap().to_vec()
    }

    fn sample(width: u32, height: u32, shift: u32) -> Vec<u8> {
        let mut rgba = Vec::new();
        for y in 0..height {
            for x in 0..width {
                let inside = (x as i32 - 20 - shift as i32).pow(2) + (y as i32 - 17).pow(2) < 150;
                let alpha = if inside { 255 } else if x > 30 { (x * 5) as u8 } else { 0 };
                let pixel = if alpha == 0 { [0; 4] } else { [(x * 7) as u8, (y * 9) as u8, if inside { 0 } else { 200 }, alpha] };
                rgba.extend_from_slice(&pixel);
            }
        }
        rgba
    }

    fn tags(payload: &[u8]) -> Vec<[u8; 4]> {
        let mut out = Vec::new();
        let mut at = 13;
        while at < payload.len() {
            let size = u32::from_le_bytes(payload[at + 4..at + 8].try_into().unwrap()) as usize;
            out.push(payload[at..at + 4].try_into().unwrap());
            at += 8 + size + size % 2;
        }
        assert_eq!(at, payload.len());
        out
    }

    #[test]
    fn payloads_hold_the_expected_chunks() {
        // Odd sizes on purpose: neither side is a multiple of the 16 pixel macroblock or of 2.
        let frames = [sample(45, 37, 0), sample(45, 37, 3)];
        for effort in [0, 30, 100] {
            for quality in [80, 99, 100] {
                let options = Options { quality, effort };
                let mut run = Run::new(45, 37, options, None);
                let payloads: Vec<Vec<u8>> = frames.iter().enumerate().map(|(index, rgba)| push(&mut run, rgba, None, index == 1)).collect();
                // only the top of the slider is lossless; everything below it is VP8 with an alpha plane
                let expected: &[[u8; 4]] = if quality == 100 { &[*b"VP8L"] } else { &[*b"ALPH", *b"VP8 "] };
                for payload in &payloads {
                    assert_eq!(tags(payload), expected);
                }
                assert_eq!(&payloads[0][..13], &[0, 0, 0, 0, 0, 0, 44, 0, 0, 36, 0, 0, 2]);
                let mut again = Run::new(45, 37, options, None);
                assert_eq!(push(&mut again, &frames[0], None, false), payloads[0]);
                let placed: Vec<Placed<'_>> = payloads.iter().map(|payload| Placed { ticks: 1, payload }).collect();
                assert_eq!(&assemble(45, 37, 30, &placed)[..4], b"RIFF");
            }
        }
        let clear = vec![0u8; 8 * 6 * 4];
        let mut outside = Run::new(8, 6, Options { quality: 50, effort: 30 }, Some(Bounds { x: 50, y: 40, width: 3, height: 3 }));
        assert_eq!(&push(&mut outside, &clear, None, true)[..13], &[0, 0, 0, 0, 0, 0, 7, 0, 0, 5, 0, 0, 2]);
        let mut run = Run::new(8, 6, Options { quality: 50, effort: 30 }, Some(Bounds { x: 1, y: 1, width: 3, height: 3 }));
        let payload = push(&mut run, &clear, None, true);
        assert_eq!(tags(&payload), [*b"VP8L"]);
        assert_eq!(&payload[..13], &[0, 0, 0, 0, 0, 0, 3, 0, 0, 3, 0, 0, 2]);
        assert_eq!(tags(&push(&mut Run::new(0, 0, Options { quality: 50, effort: 30 }, None), &[], None, true)), [*b"VP8L"]);
    }

    #[test]
    fn delta_frames_cover_only_changes() {
        // Opaque left half, translucent right half; a change on either side must land in a small rectangle, and only
        // an all-opaque change may blend over what the canvas already shows.
        let (width, height) = (12usize, 8usize);
        let base: Vec<u8> = (0..width * height).flat_map(|at| if at % width < 6 { [200, 30, 30, 255] } else { [30, 30, 200, 100] }).collect();
        let mut opaque = base.clone();
        opaque[(width + 1) * 4..(width + 1) * 4 + 4].copy_from_slice(&[10, 250, 10, 255]);
        let mut translucent = base.clone();
        translucent[(5 * width + 9) * 4..(5 * width + 9) * 4 + 4].copy_from_slice(&[30, 30, 200, 160]);
        for quality in [70, 100] {
            let options = Options { quality, effort: 30 };
            let mut run = Run::new(width as u32, height as u32, options, None);
            push(&mut run, &base, None, false);
            let first = push(&mut run, &opaque, None, false);
            assert_eq!(&first[..12], &[0, 0, 0, 0, 0, 0, 1, 0, 0, 1, 0, 0]);
            let still = push(&mut run, &opaque, None, false);
            assert_eq!(&still[..13], &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            let second = push(&mut run, &translucent, None, true);
            assert_eq!(&second[..13], &[0, 0, 0, 0, 0, 0, 9, 0, 0, 5, 0, 0, 2]);
        }
    }
}
