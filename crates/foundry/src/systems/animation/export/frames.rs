use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::Receiver;

use rayon::prelude::*;
use tracing::{error, info};

use crate::systems::animation::alpha;

use super::{EncoderMessage, EncoderStatus, ExportConfig};

const PIXEL: usize = 4;
const SEED_LOW: u64 = 0x9E37_79B9_7F4A_7C15;
const SEED_HIGH: u64 = 0xC2B2_AE3D_27D4_EB4F;
const MIX_LOW: u64 = 0xFF51_AFD7_ED55_8CCD;
const MIX_HIGH: u64 = 0xC4CE_B9FE_1A85_EC53;
const HELD_BYTES: usize = 512 << 20;
const SHORT_REPEAT: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

pub(crate) struct Intake<'a> {
    pub config: &'a ExportConfig,
    pub receiver: Receiver<EncoderMessage>,
    pub emit: &'a (dyn Fn(EncoderStatus) + Sync),
    pub abort: &'a AtomicBool,
}

struct Ready {
    pixels: Vec<u8>,
    content: Option<Rect>,
    hash: u128,
}

struct Digest {
    low: u64,
    high: u64,
}

impl Digest {
    fn new(length: usize) -> Self {
        Self { low: SEED_LOW ^ length as u64, high: SEED_HIGH }
    }

    fn mark(&mut self, row: usize, column: usize) {
        self.low = (self.low ^ row as u64).wrapping_mul(MIX_LOW).rotate_left(29);
        self.high = (self.high.rotate_left(31) ^ column as u64).wrapping_mul(MIX_HIGH);
    }

    fn absorb(&mut self, bytes: &[u8]) {
        let (pairs, tail) = bytes.as_chunks::<16>();
        let (mut low, mut high) = (self.low, self.high);

        for pair in pairs {
            let (near, far) = pair.split_at(8);
            let near = near.first_chunk::<8>().map_or(0, |word| u64::from_le_bytes(*word));
            let far = far.first_chunk::<8>().map_or(0, |word| u64::from_le_bytes(*word));

            low = (low ^ near).wrapping_mul(MIX_LOW).rotate_left(29) ^ far;
            high = (high.rotate_left(31) ^ far).wrapping_mul(MIX_HIGH) ^ near;
        }

        for byte in tail {
            low = (low ^ u64::from(*byte)).wrapping_mul(MIX_LOW).rotate_left(29);
            high = (high.rotate_left(31) ^ u64::from(*byte)).wrapping_mul(MIX_HIGH);
        }

        self.low = (low ^ bytes.len() as u64).wrapping_mul(MIX_LOW);
        self.high = high;
    }

    fn finish(self) -> u128 {
        let (mut low, mut high) = (self.low, self.high);

        low ^= low >> 33;
        low = low.wrapping_mul(MIX_HIGH);
        low ^= low >> 29;
        high ^= high >> 31;
        high = high.wrapping_mul(MIX_LOW);
        high ^= high >> 32;

        (u128::from(high) << 64) | u128::from(low)
    }
}

fn prepare(raw: &[u8], width: u32, height: u32, opaque: bool) -> Ready {
    let stride = width as usize * PIXEL;
    let wanted = stride * height as usize;
    let mut pixels = Vec::with_capacity(wanted);
    let mut digest = Digest::new(wanted);
    let mut rows: Option<(usize, usize)> = None;
    let mut left = usize::MAX;
    let mut right = 0;

    for (y, source) in raw[..wanted].chunks_exact(stride).rev().enumerate() {
        let start = pixels.len();

        pixels.extend_from_slice(source);

        let target = &mut pixels[start..];
        let texels = target.as_chunks_mut::<PIXEL>().0;

        if opaque {
            for texel in texels.iter_mut() {
                texel[3] = u8::MAX;
            }
        }

        let Some(first) = texels.iter().position(|texel| *texel != [0; PIXEL]) else {
            continue;
        };
        let last = texels.iter().rposition(|texel| *texel != [0; PIXEL]).unwrap_or(first);
        let span = &mut target[first * PIXEL..(last + 1) * PIXEL];

        if !opaque {
            alpha::straighten(span);
        }

        digest.mark(y, first);
        digest.absorb(span);
        left = left.min(first);
        right = right.max(last);
        rows = Some(rows.map_or((y, y), |(top, _)| (top, y)));
    }

    let content = rows.map(|(top, bottom)| Rect { x: left as u32, y: top as u32, width: (right - left + 1) as u32, height: (bottom - top + 1) as u32 });

    Ready { pixels, content, hash: digest.finish() }
}

fn gather(receiver: &Receiver<EncoderMessage>, limit: usize) -> (Vec<(Vec<u8>, u32, u32)>, bool) {
    let mut batch = Vec::with_capacity(limit);

    match receiver.recv() {
        Ok(EncoderMessage::Frame(raw, width, height, _)) => batch.push((raw, width, height)),
        Ok(EncoderMessage::Finish) => return (batch, true),
        Err(_) => return (batch, false),
    }

    while batch.len() < limit {
        match receiver.try_recv() {
            Ok(EncoderMessage::Frame(raw, width, height, _)) => batch.push((raw, width, height)),
            Ok(EncoderMessage::Finish) => return (batch, true),
            Err(_) => break,
        }
    }

    (batch, false)
}

pub(crate) struct Still<'a> {
    pub pixels: &'a [u8],
    pub content: Option<Rect>,
}

pub(crate) trait Chain {
    type Payload: Send;

    fn push(&mut self, before: Option<&Still<'_>>, now: &Still<'_>, next: Option<&Still<'_>>) -> Option<Self::Payload>;
}

pub(crate) trait Weighed {
    fn weight(&self) -> usize;
}

impl Weighed for Vec<u8> {
    fn weight(&self) -> usize {
        self.len()
    }
}

pub(crate) struct Take<P> {
    pub payload: P,
    pub ticks: u32,
}

#[derive(Default)]
struct Stretch {
    before: Option<Rect>,
    first: usize,
    frames: Vec<Ready>,
}

struct Mark {
    hash: u128,
    opens: bool,
    origin: Option<usize>,
}

struct Tally<'a> {
    emit: &'a (dyn Fn(EncoderStatus) + Sync),
    done: AtomicU32,
    shown: AtomicU32,
}

impl Tally<'_> {
    fn advance(&self, by: u32) {
        let total = self.done.fetch_add(by, Ordering::Relaxed) + by;

        if self.shown.fetch_max(total, Ordering::Relaxed) < total {
            (self.emit)(EncoderStatus::Progress(total));
        }
    }
}

fn stretch<C: Chain>(stretch: &Stretch, weights: &[u32], tally: &Tally<'_>, start: &(impl Fn(Option<Rect>, bool) -> C + Sync)) -> Option<Vec<C::Payload>> {
    let stills: Vec<Still<'_>> = stretch.frames.iter().map(|ready| Still { pixels: &ready.pixels, content: ready.content }).collect();
    let mut chain = start(stretch.before, stretch.first == 0);
    let mut out = Vec::with_capacity(stills.len());

    for (at, now) in stills.iter().enumerate() {
        let payload = chain.push(at.checked_sub(1).and_then(|back| stills.get(back)), now, stills.get(at + 1));

        tally.advance(weights.get(stretch.first + at).copied().unwrap_or(1));
        out.push(payload?);
    }

    Some(out)
}

pub(crate) fn runs<C: Chain, P: Send + Weighed>(intake: Intake<'_>, length: usize, start: impl Fn(Option<Rect>, bool) -> C + Sync, finish: impl Fn(C::Payload) -> Option<P> + Sync, reuse: Option<fn(&P) -> P>) -> Option<Vec<Take<P>>> {
    let Intake { config, receiver, emit, abort } = intake;
    let (width, height) = (config.width, config.height);
    let wanted = width as usize * height as usize * PIXEL;
    let length = length.max(1);
    let threads = rayon::current_num_threads().max(1);
    let flight = threads.min((HELD_BYTES / wanted.max(1) / length).max(1));
    let tally = Tally { emit, done: AtomicU32::new(0), shown: AtomicU32::new(0) };
    let mut ticks: Vec<u32> = Vec::new();
    let mut payloads: Vec<Option<P>> = Vec::new();
    let mut open = Stretch::default();
    let mut closed: Vec<Stretch> = Vec::new();
    let mut last: Option<u128> = None;
    let mut trail: Option<Rect> = None;
    let mut seen = 0u32;
    let mut marks: Vec<Mark> = Vec::new();
    let mut anchors: HashMap<u128, usize> = HashMap::new();
    let mut mirror: Option<usize> = None;
    let mut pending: Vec<usize> = Vec::new();
    let mut detached = false;
    let mut written = 0u64;

    loop {
        let (batch, finished) = gather(&receiver, threads * 2);

        if abort.load(Ordering::Relaxed) {
            info!("Export encoding aborted after {seen} frames.");
            return None;
        }

        if batch.is_empty() && !finished {
            error!("The frame stream closed before the export was finished.");
            return None;
        }

        if batch.iter().any(|(raw, frame_width, frame_height)| *frame_width != width || *frame_height != height || raw.len() < wanted) {
            error!("A frame did not match the export size {width}x{height}.");
            return None;
        }

        let ready: Vec<Ready> = batch.par_iter().map(|(raw, _, _)| prepare(raw, width, height, config.background)).collect();

        for item in ready {
            seen += 1;

            if last == Some(item.hash) {
                if let Some(held) = ticks.last_mut() {
                    *held += 1;
                }

                continue;
            }

            last = Some(item.hash);

            let index = marks.len();
            let follow = mirror.map(|at| at + 1).filter(|&next| next < index && marks.get(next).is_some_and(|mark| mark.origin.is_none() && !mark.opens && mark.hash == item.hash));
            let anchor = follow.is_none().then(|| anchors.get(&item.hash).copied()).flatten();
            let origin = follow.or(anchor);

            if anchor.is_some() {
                if !open.frames.is_empty() {
                    closed.push(std::mem::take(&mut open));
                }

                pending.clear();
                detached = false;
            }

            if origin.is_none() {
                pending.clear();
                detached = false;
            }

            mirror = origin;

            if detached {
                marks.push(Mark { hash: item.hash, opens: false, origin });
                trail = item.content;
                ticks.push(1);
                tally.advance(1);
                continue;
            }

            let opens = open.frames.is_empty();

            if opens {
                open.before = trail;
                open.first = index;

                if reuse.is_some() && origin.is_none() {
                    anchors.entry(item.hash).or_insert(index);
                }
            }

            marks.push(Mark { hash: item.hash, opens, origin: None });
            trail = item.content;
            ticks.push(1);
            open.frames.push(item);

            if let Some(origin) = origin {
                pending.push(origin);

                if pending.len() > SHORT_REPEAT || open.frames.len() == length {
                    for (mark, from) in marks.iter_mut().skip(open.first).zip(pending.drain(..)) {
                        mark.origin = Some(from);
                    }

                    tally.advance(open.frames.len() as u32);
                    open = Stretch::default();
                    detached = true;
                    continue;
                }
            }

            if open.frames.len() == length {
                closed.push(std::mem::take(&mut open));
            }
        }

        if finished && !open.frames.is_empty() {
            closed.push(std::mem::take(&mut open));
        }

        if closed.len() >= flight || (finished && !closed.is_empty()) {
            let weights = ticks.clone();
            let done: Vec<(usize, Option<Vec<C::Payload>>)> = closed.par_drain(..).map(|held| (held.first, stretch(&held, &weights, &tally, &start))).collect();

            payloads.resize_with(ticks.len(), || None);

            let mut drafts: Vec<(usize, C::Payload)> = Vec::new();

            for (first, encoded) in done {
                let Some(encoded) = encoded else {
                    error!("A run of frames from {first} could not be encoded.");
                    return None;
                };

                drafts.extend(encoded.into_iter().enumerate().map(|(at, draft)| (first + at, draft)));
            }

            let finished: Vec<(usize, Option<P>)> = drafts.into_par_iter().map(|(at, draft)| (at, finish(draft))).collect();

            for (at, payload) in finished {
                let Some(payload) = payload else {
                    error!("Frame {at} could not be encoded.");
                    return None;
                };

                written += payload.weight() as u64;

                if let Some(slot) = payloads.get_mut(at) {
                    *slot = Some(payload);
                }
            }

            emit(EncoderStatus::Size(written));
        }

        if finished {
            break;
        }
    }

    emit(EncoderStatus::Progress(seen));
    payloads.resize_with(ticks.len(), || None);

    let reused = marks.iter().filter(|mark| mark.origin.is_some()).count();

    if let Some(copy) = reuse {
        let copies: Vec<(usize, Option<P>)> = marks.iter().enumerate().filter_map(|(at, mark)| mark.origin.map(|from| (at, payloads.get(from).and_then(Option::as_ref).map(copy)))).collect();

        for (at, payload) in copies {
            if let Some(slot) = payloads.get_mut(at) {
                *slot = payload;
            }
        }
    }

    let takes: Option<Vec<Take<P>>> = payloads.into_iter().zip(ticks).map(|(payload, ticks)| payload.map(|payload| Take { payload, ticks })).collect();
    let takes = takes.filter(|takes| !takes.is_empty());

    if takes.is_none() {
        error!("No frames reached the encoder.");
    }

    takes.inspect(|takes| info!("Encoded {} shown frames out of {seen} in runs of {length}, {reused} reused from earlier runs.", takes.len()))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::{Mutex, mpsc};

    use crate::systems::animation::export::ExportFormat;

    use super::*;

    const WIDTH: u32 = 8;
    const HEIGHT: u32 = 6;
    const WEIGHT: usize = 7;

    fn frame(dots: &[(u32, u32)]) -> Vec<u8> {
        let mut pixels = vec![0u8; (WIDTH * HEIGHT) as usize * PIXEL];

        for (x, y) in dots {
            let at = ((HEIGHT - 1 - y) * WIDTH + x) as usize * PIXEL;
            pixels[at..at + PIXEL].copy_from_slice(&[10, 20, 30, 255]);
        }

        pixels
    }

    struct Echo {
        before: Option<Rect>,
        opening: bool,
        pushed: usize,
    }

    impl Chain for Echo {
        type Payload = (Option<Rect>, bool, usize, bool, bool, Option<Rect>);

        fn push(&mut self, before: Option<&Still<'_>>, now: &Still<'_>, next: Option<&Still<'_>>) -> Option<Self::Payload> {
            self.pushed += 1;

            Some((self.before, self.opening, self.pushed, before.is_some(), next.is_none(), now.content))
        }
    }

    type Seen = (Option<Rect>, bool, usize, bool, bool, Option<Rect>);

    impl Weighed for Seen {
        fn weight(&self) -> usize {
            WEIGHT
        }
    }

    fn pump(sequence: &[&Vec<u8>], length: usize, reuse: Option<fn(&Seen) -> Seen>) -> Vec<Take<Seen>> {
        watched(sequence, length, reuse, &|_| {})
    }

    fn watched(sequence: &[&Vec<u8>], length: usize, reuse: Option<fn(&Seen) -> Seen>, emit: &(dyn Fn(EncoderStatus) + Sync)) -> Vec<Take<Seen>> {
        let config = ExportConfig { width: WIDTH, height: HEIGHT, format: ExportFormat::Png, quality_percent: 80, compression_percent: 30, fps: 30, start_frame: 0, end_frame: 0, output_path: PathBuf::new(), base_name: String::new(), background: false };
        let (tx, rx) = mpsc::channel();

        for pixels in sequence {
            tx.send(EncoderMessage::Frame((*pixels).clone(), WIDTH, HEIGHT, 33)).unwrap();
        }
        tx.send(EncoderMessage::Finish).unwrap();

        let abort = AtomicBool::new(false);
        runs(Intake { config: &config, receiver: rx, emit, abort: &abort }, length, |before, opening| Echo { before, opening, pushed: 0 }, Some, reuse).unwrap()
    }

    fn spot(x: u32, y: u32) -> Option<Rect> {
        Some(Rect { x, y, width: 1, height: 1 })
    }

    // The size so far only grows, and a frame merged into the one before it adds nothing.
    #[test]
    fn the_size_so_far_counts_each_encoded_frame_once() {
        let frames: Vec<Vec<u8>> = (0..5).map(|x| frame(&[(x, 1)])).collect();
        let sequence = [&frames[0], &frames[0], &frames[1], &frames[2], &frames[3], &frames[4]];
        let sizes = Mutex::new(Vec::new());
        let takes = watched(&sequence, 2, None, &|status| {
            if let EncoderStatus::Size(bytes) = status {
                sizes.lock().unwrap().push(bytes);
            }
        });
        let sizes = sizes.into_inner().unwrap();

        assert_eq!(takes.len(), 5);
        assert_eq!(sizes.last().copied(), Some(5 * WEIGHT as u64));
        assert!(sizes.windows(2).all(|pair| pair[0] <= pair[1]));
    }

    #[test]
    fn runs_cut_after_merging_repeats_and_know_what_came_before() {
        let a = frame(&[(2, 1)]);
        let b = frame(&[(3, 3)]);
        let c = frame(&[(5, 4)]);
        let takes = pump(&[&a, &a, &b, &c, &c, &c, &a], 2, None);

        // a a b c c c a: four shown frames, cut into runs of two
        assert_eq!(takes.iter().map(|take| take.ticks).collect::<Vec<_>>(), [2, 1, 3, 1]);
        assert_eq!(takes[0].payload, (None, true, 1, false, false, spot(2, 1)));
        assert_eq!(takes[1].payload, (None, true, 2, true, true, spot(3, 3)));
        assert_eq!(takes[2].payload, (spot(3, 3), false, 1, false, false, spot(5, 4)));
        assert_eq!(takes[3].payload, (spot(3, 3), false, 2, true, true, spot(2, 1)));
    }

    #[test]
    fn a_long_repeat_reuses_the_earlier_run_and_a_short_one_carries_on() {
        let loop_frames: Vec<Vec<u8>> = (0..7).map(|x| frame(&[(x, 1)])).collect();
        let order = [0, 1, 2, 3, 4, 5, 0, 1, 2, 3, 4, 5, 6];
        let takes = pump(&order.map(|at| &loop_frames[at]), 16, Some(|seen| *seen));

        // the second pass of the loop is never pushed, it copies the first
        for at in 0..6 {
            assert_eq!(takes[at + 6].payload, takes[at].payload);
        }
        assert_eq!(takes[12].payload, (spot(5, 1), false, 1, false, true, spot(6, 1)));

        let a = frame(&[(2, 1)]);
        let b = frame(&[(3, 3)]);
        let c = frame(&[(5, 4)]);
        let takes = pump(&[&a, &b, &a, &c], 16, Some(|seen| *seen));

        // a single repeated frame starts a run that goes on as usual
        assert_eq!(takes[1].payload, (None, true, 2, true, true, spot(3, 3)));
        assert_eq!(takes[2].payload, (spot(3, 3), false, 1, false, false, spot(2, 1)));
        assert_eq!(takes[3].payload, (spot(3, 3), false, 2, true, true, spot(5, 4)));
    }
}
