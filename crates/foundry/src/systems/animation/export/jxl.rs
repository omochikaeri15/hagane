mod ans;
mod bits;
mod entropy;
mod frame;
mod learn;
mod lossless;
mod modular;
mod prefix;
mod tables;
mod vardct;
mod weighted;

use std::{fs, path::Path};

use tracing::error;

use bits::BitWriter;
use frame::{CLEAR, Header, KEY, PAINT, TRANSFORMED};

use super::frames::{self, Chain, Intake, Still};

const TILE: usize = 8;
const SAVE: u8 = 8;
const LEAD: usize = 21;
const KEPT_SHARE: usize = 20;
const SMALL: usize = 1 << 16;
const SHORTEST: usize = 30;
const LONGEST: usize = 120;

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
    effort: usize,
    tuning: Option<vardct::Tuning>,
    grain: u16,
    previous: Vec<u8>,
    started: bool,
}

struct Patch {
    effort: usize,
    tuning: Option<vardct::Tuning>,
    clear: Option<(Bounds, Vec<u8>)>,
    kind: u8,
    bounds: Bounds,
    image: Vec<u8>,
}

enum Draft {
    Fresh(Patch),
    Done(Vec<u8>),
}

impl Draft {
    fn render(self) -> Self {
        match self {
            Self::Fresh(patch) => Self::Done(patch.render()),
            done => done,
        }
    }

    fn payload(&self) -> Option<&[u8]> {
        match self {
            Self::Done(payload) => Some(payload),
            Self::Fresh(_) => None,
        }
    }
}

impl Patch {
    fn render(&self) -> Vec<u8> {
        let mut payload = Vec::new();
        if let Some((bounds, keep)) = &self.clear {
            part(&mut payload, CLEAR | SAVE, *bounds, &lossless::mask(keep, bounds.width as usize, bounds.height as usize));
        }
        let (kind, body) = encode(&self.image, self.bounds.width as usize, self.bounds.height as usize, self.effort, self.tuning.as_ref());
        part(&mut payload, kind | self.kind, self.bounds, &body);
        payload
    }
}

fn encode(rgba: &[u8], width: usize, height: usize, effort: usize, tuning: Option<&vardct::Tuning>) -> (u8, Vec<u8>) {
    let mut learner = learn::Learner::new();
    let Some(tuning) = tuning else { return (0, lossless::encode(rgba, width, height, effort, &mut learner)) };
    let lossy = vardct::encode(rgba, width, height, tuning, &mut learner);
    if tuning.mask <= 1 && width * height <= SMALL {
        let exact = lossless::encode(rgba, width, height, effort, &mut learner);
        if exact.len() < lossy.len() {
            return (0, exact);
        }
    }
    (TRANSFORMED, lossy)
}

struct Placed<'a> {
    ticks: u32,
    payload: &'a [u8],
}

fn run_length(options: Options) -> usize {
    SHORTEST + (LONGEST - SHORTEST) * usize::from(options.effort.clamp(30, 100) - 30) / 70
}

impl Run {
    fn new(width: u32, height: u32, options: Options, _before: Option<Bounds>) -> Self {
        let tuning = (options.quality < 100).then(|| vardct::Tuning::new(options.quality, options.effort));
        let grain = tuning.as_ref().map_or(1, |tuning| tuning.mask.max(1) as u16);
        Self { width: width.max(1) as usize, height: height.max(1) as usize, effort: usize::from(options.effort.min(100)), tuning, grain, previous: Vec::new(), started: false }
    }

    fn plan(&mut self, frame: &Frame<'_>, last: bool) -> Draft {
        let needed = self.width * self.height * 4;
        let padded;
        let rgba = if frame.rgba.len() >= needed {
            &frame.rgba[..needed]
        } else {
            padded = frame.rgba.iter().copied().chain(std::iter::repeat(0)).take(needed).collect::<Vec<u8>>();
            &padded
        };
        let save = if last { 0 } else { SAVE };
        let plan = if self.started && frame.content.is_some() { self.tiling(rgba) } else { None };
        let patch = match plan {
            Some(plan) => self.delta(rgba, &plan, save),
            None => self.key(rgba, frame.content, save),
        };
        if !last {
            self.previous.clear();
            self.previous.extend_from_slice(rgba);
        }
        self.started = true;
        Draft::Fresh(patch)
    }

    fn patch(&self, clear: Option<(Bounds, Vec<u8>)>, kind: u8, bounds: Bounds, image: Vec<u8>) -> Patch {
        Patch { effort: self.effort, tuning: self.tuning, clear, kind, bounds, image }
    }

    fn key(&self, rgba: &[u8], content: Option<Bounds>, save: u8) -> Patch {
        let bounds = content.map_or(Bounds { x: 0, y: 0, width: 1, height: 1 }, |bounds| {
            let x = (bounds.x as usize).min(self.width - 1);
            let y = (bounds.y as usize).min(self.height - 1);
            Bounds { x: x as u32, y: y as u32, width: (bounds.width as usize).clamp(1, self.width - x) as u32, height: (bounds.height as usize).clamp(1, self.height - y) as u32 }
        });
        let (left, top, wide, tall) = (bounds.x as usize, bounds.y as usize, bounds.width as usize, bounds.height as usize);
        let mut crop = Vec::with_capacity(wide * tall * 4);
        for row in top..top + tall {
            crop.extend_from_slice(&rgba[(row * self.width + left) * 4..(row * self.width + left + wide) * 4]);
        }
        self.patch(None, KEY | save, bounds, crop)
    }

    fn level(&self, alpha: u8) -> u16 {
        (u16::from(alpha) + self.grain / 2) / self.grain * self.grain
    }

    fn tiling(&self, rgba: &[u8]) -> Option<Plan> {
        let (across, down) = (self.width.div_ceil(TILE), self.height.div_ceil(TILE));
        let mut painted = vec![false; across * down];
        let (mut paint, mut clear) = (Span::new(), Span::new());
        let (mut visible, mut kept) = (0usize, 0usize);
        let stride = self.width * 4;
        for tile_y in 0..down {
            let (top, bottom) = (tile_y * TILE, ((tile_y + 1) * TILE).min(self.height));
            let rows = || top..bottom;
            for tile_x in 0..across {
                let (start, end) = (tile_x * TILE * 4, ((tile_x + 1) * TILE).min(self.width) * 4);
                let changed = rows().any(|row| rgba[row * stride + start..row * stride + end] != self.previous[row * stride + start..row * stride + end]);
                if !changed {
                    let shown = rows().any(|row| rgba[row * stride + start..row * stride + end].as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 0));
                    visible += usize::from(shown);
                    kept += usize::from(shown);
                    continue;
                }
                visible += 1;
                painted[tile_y * across + tile_x] = true;
                paint.add(tile_x, tile_y);
                let blocked = rows().any(|row| {
                    let (new, old) = (&rgba[row * stride + start..row * stride + end], &self.previous[row * stride + start..row * stride + end]);
                    new.as_chunks::<4>().0.iter().zip(old.as_chunks::<4>().0).any(|(new, old)| self.level(new[3]) < 255 && self.level(old[3]) > 0)
                });
                if blocked {
                    clear.add(tile_x, tile_y);
                }
            }
        }
        (kept * KEPT_SHARE >= visible).then_some(Plan { painted, paint, clear })
    }

    fn delta(&self, rgba: &[u8], plan: &Plan, save: u8) -> Patch {
        let Plan { painted, paint, clear } = plan;
        let clear = clear.bounds(self.width, self.height).map(|bounds| (bounds, self.tiles(&bounds, painted, |_, inside| [u8::from(!inside)]).concat()));
        let bounds = paint.bounds(self.width, self.height).unwrap_or(Bounds { x: 0, y: 0, width: 1, height: 1 });
        let image = self.tiles(&bounds, painted, |at, inside| if inside { [rgba[at], rgba[at + 1], rgba[at + 2], rgba[at + 3]] } else { [0; 4] }).concat();
        self.patch(clear, PAINT | save, bounds, image)
    }

    fn tiles<const N: usize>(&self, bounds: &Bounds, painted: &[bool], pick: impl Fn(usize, bool) -> [u8; N]) -> Vec<[u8; N]> {
        let across = self.width.div_ceil(TILE);
        let (left, top, wide, tall) = (bounds.x as usize, bounds.y as usize, bounds.width as usize, bounds.height as usize);
        let mut image = Vec::with_capacity(wide * tall);
        for row in top..top + tall {
            for column in left..left + wide {
                image.push(pick((row * self.width + column) * 4, painted[row / TILE * across + column / TILE]));
            }
        }
        image
    }
}

struct Plan {
    painted: Vec<bool>,
    paint: Span,
    clear: Span,
}

struct Span {
    low: (usize, usize),
    high: (usize, usize),
}

impl Span {
    fn new() -> Self {
        Self { low: (usize::MAX, usize::MAX), high: (0, 0) }
    }

    fn add(&mut self, x: usize, y: usize) {
        self.low = (self.low.0.min(x), self.low.1.min(y));
        self.high = (self.high.0.max(x), self.high.1.max(y));
    }

    fn bounds(&self, width: usize, height: usize) -> Option<Bounds> {
        if self.low.0 > self.high.0 {
            return None;
        }
        let (left, top) = (self.low.0 * TILE, self.low.1 * TILE);
        let (right, bottom) = (((self.high.0 + 1) * TILE).min(width), ((self.high.1 + 1) * TILE).min(height));
        Some(Bounds { x: left as u32, y: top as u32, width: (right - left) as u32, height: (bottom - top) as u32 })
    }
}

fn part(payload: &mut Vec<u8>, kind: u8, bounds: Bounds, body: &[u8]) {
    payload.push(kind);
    for value in [bounds.x, bounds.y, bounds.width, bounds.height, body.len() as u32] {
        payload.extend_from_slice(&value.to_le_bytes());
    }
    payload.extend_from_slice(body);
}

fn assemble(width: u32, height: u32, ticks_per_second: u32, frames: &[Placed<'_>]) -> Vec<u8> {
    let mut writer = BitWriter::with_capacity(frames.iter().map(|placed| placed.payload.len()).sum::<usize>() + 64);
    let canvas = (width.max(1), height.max(1));
    frame::file_header(&mut writer, canvas.0, canvas.1, ticks_per_second);
    for (index, placed) in frames.iter().enumerate() {
        let mut rest = placed.payload;
        while let Some((lead, tail)) = rest.split_at_checked(LEAD) {
            let field = |at: usize| u32::from_le_bytes([lead[1 + at * 4], lead[2 + at * 4], lead[3 + at * 4], lead[4 + at * 4]]);
            let Some((body, after)) = tail.split_at_checked(field(4) as usize) else { break };
            let shown = after.len() < LEAD;
            let header = Header {
                kind: lead[0],
                x: field(0),
                y: field(1),
                width: field(2),
                height: field(3),
                ticks: if shown { placed.ticks.max(1) } else { 0 },
                save: lead[0] & SAVE != 0,
                last: shown && index + 1 == frames.len(),
            };
            frame::frame_header(&mut writer, canvas, &header);
            writer.extend(body);
            rest = after;
        }
    }
    writer.finish()
}

pub(crate) fn export(intake: Intake<'_>, path: &Path) -> bool {
    let config = intake.config;
    let options = Options { quality: config.quality_percent.clamp(1, 100) as u8, effort: config.compression_percent.min(100) as u8 };
    let (width, height) = (config.width, config.height);
    let deferred = rayon::current_num_threads() > 1;
    let Some(takes) = frames::runs(intake, run_length(options), |before, _| Planner { run: Run::new(width, height, options, before.map(bounds)), deferred }, |draft: Draft| Some(draft.render()), None) else {
        return false;
    };
    let Some(placed) = takes.iter().map(|take| take.payload.payload().map(|payload| Placed { ticks: take.ticks, payload })).collect::<Option<Vec<Placed<'_>>>>() else {
        error!("A JXL frame was left without a payload.");
        return false;
    };

    fs::write(path, assemble(width, height, config.fps, &placed)).inspect_err(|err| error!("Failed to write the JXL: {err}")).is_ok()
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
    use super::{Bounds, Frame, KEY, LEAD, Options, PAINT, Placed, Run, assemble};

    fn push(run: &mut Run, frame: &Frame<'_>, last: bool) -> Vec<u8> {
        run.plan(frame, last).render().payload().unwrap().to_vec()
    }

    fn canvas(shift: u32) -> Vec<u8> {
        let mut rgba = vec![0u8; 40 * 30 * 4];
        for y in 5..25 {
            for x in (5 + shift)..(30 + shift).min(40) {
                let at = ((y * 40 + x) * 4) as usize;
                rgba[at..at + 4].copy_from_slice(&[(x * 7) as u8, (y * 5) as u8, 90, if x == 5 + shift { 128 } else { 255 }]);
            }
        }
        rgba
    }

    fn kinds(payload: &[u8]) -> Vec<u8> {
        let mut kinds = Vec::new();
        let mut rest = payload;
        while rest.len() >= LEAD {
            let length = u32::from_le_bytes([rest[17], rest[18], rest[19], rest[20]]) as usize;
            kinds.push(rest[0] & 6);
            rest = &rest[LEAD + length..];
        }
        kinds
    }

    // A run opens with a key frame and continues with paint frames for small changes.
    #[test]
    fn runs_open_with_a_key_and_then_paint() {
        for quality in [80, 100] {
            let options = Options { quality, effort: 30 };
            let first = canvas(0);
            let mut second = first.clone();
            second[(10 * 40 + 12) * 4] ^= 0x55;
            let bounds = Some(Bounds { x: 5, y: 5, width: 25, height: 20 });
            let mut run = Run::new(40, 30, options, None);
            let opening = push(&mut run, &Frame { rgba: &first, content: bounds }, false);
            let follow = push(&mut run, &Frame { rgba: &second, content: bounds }, true);
            assert_eq!(kinds(&opening), vec![KEY]);
            assert_eq!(kinds(&follow).last(), Some(&PAINT));
        }
    }

    // The same pushes must give the same bytes, whatever ran before.
    #[test]
    fn same_pushes_give_same_payloads() {
        let frames = [canvas(0), canvas(3), canvas(6)];
        let encode = || {
            let mut run = Run::new(40, 30, Options { quality: 70, effort: 40 }, Some(Bounds { x: 1, y: 1, width: 2, height: 2 }));
            frames.iter().enumerate().map(|(index, rgba)| push(&mut run, &Frame { rgba, content: Some(Bounds { x: 5, y: 5, width: 35, height: 20 }) }, index == 2)).collect::<Vec<_>>()
        };
        assert_eq!(encode(), encode());
    }

    #[test]
    fn file_holds_every_part_and_survives_blank_frames() {
        let blank = vec![0u8; 40 * 30 * 4];
        let picture = canvas(2);
        let mut run = Run::new(40, 30, Options { quality: 100, effort: 0 }, None);
        let payloads = [push(&mut run, &Frame { rgba: &blank, content: None }, false), push(&mut run, &Frame { rgba: &picture, content: Some(Bounds { x: 7, y: 5, width: 25, height: 20 }) }, true)];
        let placed: Vec<Placed<'_>> = payloads.iter().map(|payload| Placed { ticks: 2, payload }).collect();
        let file = assemble(40, 30, 30, &placed);
        assert_eq!(file[..2], [0xff, 0x0a]);
        for payload in &payloads {
            let body = &payload[LEAD..];
            assert!(file.windows(body.len()).any(|window| window == body));
        }
    }
}
