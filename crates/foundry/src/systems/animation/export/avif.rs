mod bits;
mod container;
mod fan;
mod header;
mod model;
mod motion;
mod picture;
mod predict;
mod range;
mod survey;
mod tables;
mod tile;
mod transform;

use std::{fs, num::NonZero, panic, path::Path, thread};

use tracing::error;

use header::{Controls, Stream};
use model::Model;
use motion::{Phases, Plane};
use picture::{Prepared, Veil};
use survey::Survey;
use tables::AC_QUANT;
use tile::{Area, Canvas, Filters, Guide, Past, Scene, Source, Tuning};

use super::frames::{self, Chain, Intake, Still};

const EXACT_FROM: u8 = 95;
const DEADZONE: u32 = 26;
const INTER_DEADZONE: u32 = 18;
const MERGE: u32 = 16;
const SMALL_PICTURE: f32 = 456.0;
const SMALL_POWERS: [[f32; 2]; 2] = [[50.0, 0.47], [80.0, 0.40]];
const FINEST: f32 = 0.28;
const TINT_OFFSET: f32 = 20.0;
const SMOOTHING: [f32; 2] = [172.0, 230.0];
const RINGING: [f32; 2] = [40.0, 27.0];
const SHORTEST_RUN: usize = 40;
const CONTEST_FROM: usize = 20;
const SLOWEST_FROM: usize = 90;
const CLOSE_SEARCH: usize = 2;
const PRUNE: [f32; 2] = [8.0, 4.0];
const LOOSE_PRUNE: [f32; 2] = [16.0, 8.0];
const INTER_STEP: f32 = 1.6;
const BALANCE: f32 = 0.55;
const QUALITY_STEPS: [[f32; 2]; 3] = [[20.0, 0.794], [50.0, 0.721], [80.0, 0.683]];
const EFFORT_STEPS: [[f32; 2]; 4] = [[0.0, 1.0], [30.0, 1.0], [70.0, 0.3127], [100.0, 0.444]];

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
}

struct Kept {
    source: Vec<Vec<u8>>,
    recon: Vec<Vec<u8>>,
    model: Model,
    lean: [f32; 5],
    count: usize,
}

struct Memory {
    color: Kept,
    alpha: Kept,
}

struct Run {
    width: u32,
    height: u32,
    options: Options,
    threads: usize,
    memory: Option<Memory>,
    ahead: Option<(Vec<u8>, Staged)>,
}

struct Staged {
    prepared: Prepared,
    veil: Option<Veil>,
}

fn stage(rgba: &[u8], width: usize, height: usize, exact: bool) -> Staged {
    let smoothed = (!exact).then(|| picture::soften(rgba, width, height));
    let prepared = picture::prepare(smoothed.as_deref().unwrap_or(rgba), width, height, exact);
    let veil = (!exact).then(|| picture::veil(&prepared.alpha[0], prepared.stride, prepared.columns4, prepared.rows4));
    Staged { prepared, veil }
}

struct Placed<'a> {
    ticks: u32,
    payload: &'a [u8],
}

fn run_length(options: Options) -> usize {
    SHORTEST_RUN + usize::from(options.effort.min(100)) * 4 / 5
}

fn stream(planes: &[Vec<u8>], prepared: &Prepared, empty: &[bool], shape: &Stream, tuning: &Tuning, kept: Option<&Kept>, veil: Option<&Veil>) -> (Vec<u8>, Vec<Vec<u8>>, Model, [f32; 5]) {
    let layout = header::tiling(shape);
    let mut recon: Vec<Vec<u8>> = if tuning.base_q == 0 { planes.to_vec() } else { planes.iter().map(|plane| vec![0u8; plane.len()]).collect() };
    let frame = Source { planes, stride: prepared.stride, columns4: prepared.columns4, rows4: prepared.rows4, width: shape.width as usize, height: shape.height as usize };
    let survey = Survey::new(planes, prepared.stride, empty, kept.map(|kept| &kept.source[..]));
    let region = survey.active();
    let past = kept.map(|kept| Past { recon: &kept.recon, phases: Phases::new(&Plane { data: &kept.recon[0], stride: prepared.stride, width: frame.width, height: frame.height }, region, tuning.sharp, tuning.threads) });
    let mut grid = tile::grid(&frame);
    let mut aimed = *tuning;
    if let Some(veil) = veil.filter(|_| tuning.base_q > 0) {
        let wanted = (f32::from(AC_QUANT[usize::from(tuning.base_q)]) / veil.mean.powf(BALANCE)) as u16;
        aimed.base_q = AC_QUANT.partition_point(|&step| step < wanted).clamp(1, 255) as u8;
    }
    let tuning = &aimed;
    let start = kept.map_or_else(|| Model::new(tuning.base_q), |kept| kept.model.carried());
    let scene = Scene { frame: &frame, veil, tuning, survey: &survey, past: past.as_ref() };
    let mut tiles = Vec::new();
    let mut saved = None;
    for rows in layout.rows.windows(2) {
        for columns in layout.columns.windows(2) {
            let area = Area { left4: columns[0], right4: columns[1], top4: rows[0], bottom4: rows[1] };
            let (bytes, model) = tile::encode(&scene, &mut recon, &mut grid, area, start.clone());
            saved.get_or_insert(model);
            tiles.push(bytes);
        }
    }
    let step = i32::from(AC_QUANT[usize::from(tuning.base_q)]);
    let smooth = tuning.base_q > 0;
    let level = if smooth { (tuning.smoothing * step as f32 / 1024.0).clamp(0.0, 63.0) as u8 } else { 0 };
    let main = (step as f32 / tuning.ringing).min(15.0) as u8;
    let side = [0, 0, 1, 1, 2, 2, 2, 2, 3][usize::from(main.min(8))];
    let ring = smooth.then_some([3 + (main / 4).min(3), main, side, main, side]);
    let count = kept.map_or(0, |kept| kept.count);
    let guide = match kept {
        _ if !smooth || tuning.search == 0 => Guide::Fixed,
        Some(kept) if count > 1 && count.is_multiple_of(tuning.search) => Guide::Near(kept.lean),
        Some(kept) if count > 1 => Guide::Reuse(kept.lean),
        _ => Guide::Full,
    };
    let weight = veil.is_some().then_some(&prepared.alpha[0][..]);
    let canvas = Canvas { frame: &frame, grid: &grid, hollow: empty, weight };
    let (Filters { levels, ring }, lean) = tile::settle(&mut recon, &canvas, Filters { levels: [level; 4], ring }, guide, tuning.threads);
    let controls = Controls { base_q: tuning.base_q, filter: levels, sharpness: 0, ring, shade: tuning.shade, inter: kept.is_some(), reduced: kept.is_some() && !tuning.bend, adaptive: veil.is_some() && tuning.base_q > 0 };
    let mut out = if kept.is_some() { Vec::new() } else { header::sequence(shape, smooth) };
    out.extend_from_slice(&header::frame(shape, &controls, &layout, &tiles));
    (out, recon, saved.unwrap_or(start), lean)
}

fn along(points: &[[f32; 2]], at: f32) -> f32 {
    let (first, last) = (points[0], points[points.len() - 1]);
    if at <= first[0] {
        return first[1];
    }
    points.windows(2).find(|pair| at <= pair[1][0]).map_or(last[1], |pair| pair[0][1] + (pair[1][1] - pair[0][1]) * (at - pair[0][0]) / (pair[1][0] - pair[0][0]))
}

fn tune(options: Options, busy: usize, inter: bool) -> Tuning {
    let level = usize::from(options.effort.min(100));
    let effort = level as f32;
    let quality = f32::from(options.quality.clamp(1, EXACT_FROM - 1));
    let nominal = (218.67 - quality * 0.7667 - quality * quality / 75.0).clamp(1.0, 255.0) as usize;
    let scale = (busy.max(1) as f32 / SMALL_PICTURE).powf(along(&SMALL_POWERS, quality)).clamp(0.4, 1.0);
    let lift = along(&EFFORT_STEPS, effort);
    let firm = ((quality - 50.0) / 20.0).clamp(0.0, 1.0);
    let coarse = if inter { INTER_STEP } else { 1.0 };
    let wanted = (f32::from(AC_QUANT[nominal]) * along(&QUALITY_STEPS, quality) * (scale * lift).max(FINEST) * coarse) as u16;
    let base_q = if options.quality >= EXACT_FROM { 0 } else { AC_QUANT.partition_point(|&step| step < wanted).clamp(1, 255) as u8 };
    let step = u32::from(AC_QUANT[usize::from(base_q)]);
    let fine = if level < 40 { u32::MAX } else { ((32.0 * 2f32.powf((100.0 - effort.min(70.0)) / 10.0)) as u32 * step / 64).max(1) };
    Tuning {
        base_q,
        bias: if inter { INTER_DEADZONE } else { DEADZONE },
        calm: [fine, MERGE * step / 64, MERGE * step / 128, MERGE * step / 256],
        plain: false,
        trial: level >= 50,
        modes: 3 + (level / 15).min(4),
        turns: [0, 1, 3][usize::from(level >= 10) + usize::from(level >= 60)],
        tints: [0, 1, 3][usize::from(level >= 20) + usize::from(level >= 60)],
        shade: if base_q == 0 { 0 } else { ((TINT_OFFSET - 12.0 * ((effort - 30.0) / 40.0).clamp(0.0, 1.0)) as i32).min(255 - i32::from(base_q)) as i8 },
        smoothing: SMOOTHING[0] + (SMOOTHING[1] - SMOOTHING[0]) * firm,
        ringing: RINGING[0] + (RINGING[1] - RINGING[0]) * firm,
        reach: (1 + level / 25) as i32,
        fractions: 1 + usize::from(level >= 10),
        eager: level >= 60,
        sharp: level >= 60,
        contest: level >= CONTEST_FROM,
        prune: if level >= 60 { LOOSE_PRUNE } else { PRUNE },
        threads: 1,
        bend: level >= SLOWEST_FROM,
        search: if level >= SLOWEST_FROM { CLOSE_SEARCH } else { 0 },
    }
}

impl Run {
    fn new(width: u32, height: u32, options: Options, _before: Option<Bounds>) -> Self {
        Self { width, height, options, threads: thread::available_parallelism().map_or(1, NonZero::get), memory: None, ahead: None }
    }

    fn push(&mut self, frame: &Frame<'_>, next: Option<&Frame<'_>>) -> Vec<u8> {
        let (width, height) = (self.width as usize, self.height as usize);
        let area = width * height * 4;
        if area == 0 || frame.rgba.len() < area {
            return Vec::new();
        }
        let exact = self.options.quality >= EXACT_FROM;
        let Staged { prepared, veil } = match self.ahead.take() {
            Some((rgba, staged)) if rgba[..] == frame.rgba[..area] => staged,
            _ => stage(frame.rgba, width, height, exact),
        };
        let upcoming = next.map(|next| next.rgba).filter(|rgba| self.threads > 1 && rgba.len() >= area);
        let tuning = Tuning { threads: self.threads, ..tune(self.options, prepared.empty.iter().filter(|&&hollow| !hollow).count(), self.memory.is_some()) };
        let faint = Tuning { shade: 0, plain: true, ..tuning };
        let solid = vec![false; prepared.empty.len()];
        let shape = Stream { width: self.width, height: self.height, color: true, identity: exact };
        let memory = self.memory.take();
        let alpha_shape = Stream { color: false, ..shape };
        let alpha_stream = || stream(&prepared.alpha, &prepared, &solid, &alpha_shape, &faint, memory.as_ref().map(|memory| &memory.alpha), None);
        let ((color, color_recon, color_model, color_lean), (alpha, alpha_recon, alpha_model, alpha_lean)) = thread::scope(|scope| {
            let early = upcoming.and_then(|rgba| thread::Builder::new().spawn_scoped(scope, move || (rgba[..area].to_vec(), stage(rgba, width, height, exact))).ok());
            let side = (self.threads > 1).then(|| thread::Builder::new().spawn_scoped(scope, alpha_stream).ok()).flatten();
            let color = stream(&prepared.color, &prepared, if exact { &solid } else { &prepared.empty }, &shape, &tuning, memory.as_ref().map(|memory| &memory.color), veil.as_ref());
            let alpha = side.map_or_else(alpha_stream, |handle| handle.join().unwrap_or_else(|payload| panic::resume_unwind(payload)));
            self.ahead = early.map(|handle| handle.join().unwrap_or_else(|payload| panic::resume_unwind(payload)));
            (color, alpha)
        });
        let payload = container::join(exact, memory.is_none(), &color, &alpha);
        if next.is_some() {
            let Prepared { color: color_source, alpha: alpha_source, .. } = prepared;
            let count = memory.as_ref().map_or(1, |memory| memory.color.count + 1);
            self.memory = Some(Memory { color: Kept { source: color_source, recon: color_recon, model: color_model, lean: color_lean, count }, alpha: Kept { source: alpha_source, recon: alpha_recon, model: alpha_model, lean: alpha_lean, count } });
        }
        payload
    }
}

fn assemble(width: u32, height: u32, ticks_per_second: u32, frames: &[Placed<'_>]) -> Vec<u8> {
    container::assemble(width, height, ticks_per_second, frames)
}

pub(crate) fn export(intake: Intake<'_>, path: &Path) -> bool {
    let config = intake.config;
    let options = Options { quality: config.quality_percent.clamp(1, 100) as u8, effort: config.compression_percent.min(100) as u8 };
    let (width, height) = (config.width, config.height);
    let Some(takes) = frames::runs(intake, run_length(options), |before, _| Run::new(width, height, options, before.map(bounds)), Some, Some(Vec::clone)) else {
        return false;
    };
    let placed: Vec<Placed<'_>> = takes.iter().map(|take| Placed { ticks: take.ticks, payload: &take.payload }).collect();

    fs::write(path, assemble(width, height, config.fps, &placed)).inspect_err(|err| error!("Failed to write the AVIF: {err}")).is_ok()
}

fn bounds(rect: frames::Rect) -> Bounds {
    Bounds { x: rect.x, y: rect.y, width: rect.width, height: rect.height }
}

impl Chain for Run {
    type Payload = Vec<u8>;

    fn push(&mut self, _: Option<&Still<'_>>, now: &Still<'_>, next: Option<&Still<'_>>) -> Option<Vec<u8>> {
        Some(Run::push(self, &Frame { rgba: now.pixels }, next.map(|next| Frame { rgba: next.pixels }).as_ref()))
    }
}

#[cfg(test)]
mod tests {
    use super::{Frame, Options, Placed, Run, assemble, tune};

    #[test]
    fn exact_settings_leave_every_quantizer_offset_at_zero() {
        // A chroma offset next to base index 0 makes a decoder treat the frame as lossy.
        for quality in 95..=100 {
            for effort in [0, 30, 70, 100] {
                let tuning = tune(Options { quality, effort }, 1000, false);
                assert_eq!((tuning.base_q, tuning.shade), (0, 0));
            }
        }
    }

    #[test]
    fn quantizer_falls_as_quality_rises_and_never_reaches_zero_when_lossy() {
        for effort in [0, 30, 70, 100] {
            for busy in [1, 100, 5000] {
                let steps: Vec<u8> = (1..95).map(|quality| tune(Options { quality, effort }, busy, false).base_q).collect();
                assert!(steps.windows(2).all(|pair| pair[0] >= pair[1]));
                assert!(steps.iter().all(|&step| step > 0));
            }
        }
    }

    #[test]
    fn later_frames_of_a_run_are_far_smaller_than_the_key_frame() {
        let mut rgba = vec![0u8; 48 * 32 * 4];
        for (index, pixel) in rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let inside = (index % 48) > 5 && (index / 48) > 3;
            pixel.copy_from_slice(&if inside { [200, (index % 48 * 9) as u8, 40, 255] } else { [0; 4] });
        }
        let mut moved = rgba.clone();
        moved[(10 * 48 + 20) * 4..(10 * 48 + 20) * 4 + 4].copy_from_slice(&[0, 0, 255, 255]);
        let mut run = Run::new(48, 32, Options { quality: 80, effort: 30 }, None);
        let first = run.push(&Frame { rgba: &rgba }, Some(&Frame { rgba: &moved }));
        let second = run.push(&Frame { rgba: &moved }, None);
        assert_eq!(first[0] & 2, 2);
        assert_eq!(second[0] & 2, 0);
        assert!(second.len() * 3 < first.len());
        let file = assemble(48, 32, 30, &[Placed { ticks: 1, payload: &first }, Placed { ticks: 2, payload: &second }]);
        assert!(file.starts_with(&[0, 0, 0, 44, b'f', b't', b'y', b'p', b'a', b'v', b'i', b's']));
        assert!(file.windows(4).any(|name| name == b"stss"));
    }

    #[test]
    fn a_frame_prepared_ahead_is_only_used_when_it_arrives() {
        let frames: Vec<Vec<u8>> = (0..4u8).map(|shift| (0..40 * 24).flat_map(|at| if (at % 40) as u8 > 6 + shift * 3 { [shift * 50, 90, (at % 40 * 6) as u8, 255] } else { [0; 4] }).collect()).collect();
        let options = Options { quality: 70, effort: 30 };
        let mut single = Run::new(40, 24, options, None);
        single.threads = 1;
        let plain: Vec<Vec<u8>> = (0..3).map(|at| single.push(&Frame { rgba: &frames[at] }, (at < 2).then(|| Frame { rgba: &frames[at + 1] }).as_ref())).collect();
        let mut shared = Run::new(40, 24, options, None);
        shared.threads = 4;
        // the first push is told the wrong next frame, so what it prepared ahead must be thrown away
        let ahead = [
            shared.push(&Frame { rgba: &frames[0] }, Some(&Frame { rgba: &frames[3] })),
            shared.push(&Frame { rgba: &frames[1] }, Some(&Frame { rgba: &frames[2] })),
            shared.push(&Frame { rgba: &frames[2] }, None),
        ];
        assert_eq!(plain, ahead);
    }

    #[test]
    fn empty_input_gives_empty_output() {
        let mut run = Run::new(0, 0, Options { quality: 80, effort: 30 }, None);
        assert!(run.push(&Frame { rgba: &[] }, None).is_empty());
        assert!(assemble(8, 8, 30, &[]).is_empty());
    }
}
