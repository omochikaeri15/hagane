use super::{
    bits::BitWriter,
    frame,
    learn::{Budget, Learned, Learner, Plane},
    modular::{HYBRID, Model, Stream, Tally, Tree, pack, spend, store_header},
};

pub(super) const GROUP_SHIFT: u32 = 3;
const GROUP: usize = 128 << GROUP_SHIFT;
const SPAN: usize = 1024;
const HALF: i32 = 512;
const CHANNELS: usize = 4;
const PROBES: usize = 32;
const CANDIDATES: [u32; 19] = [6, 0, 5, 10, 26, 40, 12, 19, 8, 4, 9, 15, 16, 17, 32, 33, 2, 1, 3];
const DEFAULT_EFFORT: usize = 30;
const ZERO: u8 = 0;
const NORTH: u8 = 2;

pub(super) struct Layout {
    side: usize,
    bucket: [u8; SPAN],
    cuts: Vec<i32>,
}

impl Layout {
    pub(super) fn new(bounds: &[i32]) -> Self {
        let mut bucket = [0u8; SPAN];
        for (index, slot) in bucket.iter_mut().enumerate() {
            let size = (index as i32 - HALF).abs();
            *slot = if size == 0 { 0 } else { 1 + bounds.iter().filter(|bound| size > **bound).count() as u8 };
        }
        let mut cuts: Vec<i32> = bounds.iter().rev().map(|bound| -bound - 1).collect();
        cuts.extend([-1, 0]);
        cuts.extend(bounds);
        Self { side: bounds.len() + 2, bucket, cuts }
    }

    pub(super) fn flat() -> Self {
        Self { side: 1, bucket: [0; SPAN], cuts: Vec::new() }
    }

    pub(super) fn cells(&self) -> usize {
        self.side * self.side
    }

    fn rung(&self, range: usize) -> usize {
        let middle = self.cuts.len() / 2;
        if self.cuts.is_empty() { 0 } else { range.abs_diff(middle) }
    }

    pub(super) fn grow(&self, tree: &mut Tree, base: usize, predictor: u8, multiplier: u32) -> usize {
        let ranges = self.cuts.len() + 1;
        let mut outer = Vec::with_capacity(ranges);
        for first in 0..ranges {
            let inner: Vec<usize> = (0..ranges).map(|second| tree.leaf(base + self.rung(first) * self.side + self.rung(second), predictor, multiplier)).collect();
            outer.push(tree.ladder(11, &self.cuts, &inner));
        }
        tree.ladder(10, &self.cuts, &outer)
    }

    #[inline]
    pub(super) fn cell(&self, across: i32, down: i32) -> usize {
        let slot = |gap: i32| usize::from(self.bucket[(gap + HALF).clamp(0, SPAN as i32 - 1) as usize]);
        slot(across) * self.side + slot(down)
    }
}

#[inline]
pub(super) fn gradient(west: i32, north: i32, corner: i32) -> i32 {
    (west + north - corner).clamp(west.min(north), west.max(north))
}

struct Planes {
    width: usize,
    height: usize,
    data: [Vec<i16>; CHANNELS],
}

#[inline]
fn forward(kind: u32, colors: [i32; 3]) -> [i32; 3] {
    let order = (kind / 7) as usize;
    let first = colors[order % 3];
    let second = colors[(order + 1 + order / 3) % 3];
    let third = colors[(order + 2 - order / 3) % 3];
    let custom = kind % 7;
    if custom == 6 {
        let orange = first - third;
        let base = third + (orange >> 1);
        let lime = second - base;
        return [base + (lime >> 1), orange, lime];
    }
    let second = match custom >> 1 {
        1 => second - first,
        2 => second - ((first + third) >> 1),
        _ => second,
    };
    [first, second, if custom & 1 == 1 { third - first } else { third }]
}

fn split(rgba: &[u8], width: usize, height: usize, kind: u32) -> Planes {
    let area = width * height;
    let mut data = [vec![0i16; area], vec![0i16; area], vec![0i16; area], vec![0i16; area]];
    let [first, second, third, alpha] = &mut data;
    let pixels = rgba.as_chunks::<4>().0.iter();
    for ((((pixel, first), second), third), alpha) in pixels.zip(first.iter_mut()).zip(second.iter_mut()).zip(third.iter_mut()).zip(alpha.iter_mut()) {
        let mixed = forward(kind, [i32::from(pixel[0]), i32::from(pixel[1]), i32::from(pixel[2])]);
        (*first, *second, *third) = (mixed[0] as i16, mixed[1] as i16, mixed[2] as i16);
        *alpha = i16::from(pixel[3]);
    }
    Planes { width, height, data }
}

fn choose(rgba: &[u8], width: usize, height: usize, effort: usize) -> u32 {
    if width < 2 || height < 2 {
        return 0;
    }
    let count = match effort {
        0..10 => 2,
        10..50 => 7,
        _ => CANDIDATES.len(),
    };
    let stride = width * 4;
    let step = (height - 1).div_ceil(PROBES).max(1);
    let read = |line: &[u8], at: usize| [i32::from(line[at * 4]), i32::from(line[at * 4 + 1]), i32::from(line[at * 4 + 2])];
    let mut best = (f64::MAX, 0);
    let mut above = vec![[0i32; 3]; width];
    let mut here = vec![[0i32; 3]; width];
    for kind in &CANDIDATES[..count] {
        let mut counts = [[0u32; 96]; 3];
        let mut extra = 0u64;
        for row in (1..height).step_by(step) {
            let (upper, lower) = (&rgba[(row - 1) * stride..row * stride], &rgba[row * stride..(row + 1) * stride]);
            for at in 0..width {
                above[at] = forward(*kind, read(upper, at));
                here[at] = forward(*kind, read(lower, at));
            }
            for at in 1..width {
                for channel in 0..3 {
                    let miss = here[at][channel] - gradient(here[at - 1][channel], above[at][channel], above[at - 1][channel]);
                    let (token, bits, _) = HYBRID.token(pack(miss));
                    counts[channel][(token as usize).min(95)] += 1;
                    extra += u64::from(bits);
                }
            }
        }
        let bits: f64 = counts.iter().map(|table| spend(table.iter().sum()) - table.iter().map(|count| spend(*count)).sum::<f64>()).sum::<f64>() + extra as f64;
        if bits < best.0 {
            best = (bits, *kind);
        }
    }
    best.1
}

pub(super) struct Area {
    pub(super) left: usize,
    pub(super) top: usize,
    pub(super) width: usize,
    pub(super) height: usize,
}

pub(super) struct Rule<'a> {
    pub(super) layout: &'a Layout,
    pub(super) base: usize,
    pub(super) gain: i32,
}

pub(super) fn scan(plane: &[i16], stride: usize, area: &Area, rule: &Rule<'_>, stream: &mut Stream, tally: &mut Tally) {
    let (layout, base, gain) = (rule.layout, rule.base, rule.gain);
    let mut misses = vec![0u32; area.width];
    let mut cells = vec![base as u16; area.width];
    for row in 0..area.height {
        let start = (area.top + row) * stride + area.left;
        let here = &plane[start..start + area.width];
        if row == 0 {
            misses[0] = pack(i32::from(here[0]));
            cells[0] = base as u16;
            for ((miss, cell), pair) in misses[1..].iter_mut().zip(cells[1..].iter_mut()).zip(here.windows(2)) {
                *miss = pack(i32::from(pair[1]) - i32::from(pair[0]));
                *cell = base as u16;
            }
        } else {
            let above = &plane[start - stride..start - stride + area.width];
            misses[0] = pack(i32::from(here[0]) - i32::from(above[0]));
            cells[0] = base as u16;
            let quads = here.windows(2).zip(above.windows(2));
            if layout.side == 1 {
                for (miss, (pair, over)) in misses[1..].iter_mut().zip(quads) {
                    *miss = pack(i32::from(pair[1]) - gradient(i32::from(pair[0]), i32::from(over[1]), i32::from(over[0])));
                }
            } else {
                for ((miss, cell), (pair, over)) in misses[1..].iter_mut().zip(cells[1..].iter_mut()).zip(quads) {
                    let (west, north, corner) = (i32::from(pair[0]), i32::from(over[1]), i32::from(over[0]));
                    *miss = pack(i32::from(pair[1]) - gradient(west, north, corner));
                    *cell = (base + layout.cell(gain * (west - corner), gain * (corner - north))) as u16;
                }
            }
        }
        let mut at = 0;
        while at < area.width {
            let value = misses[at];
            if value == stream.last() && cells[at] == base as u16 {
                let same = misses[at..].iter().zip(&cells[at..]).take_while(|(miss, cell)| **miss == value && **cell == base as u16).count();
                stream.repeat(tally, base, same as u32);
                at += same;
            } else {
                stream.push(tally, usize::from(cells[at]), value);
                at += 1;
            }
        }
    }
}

pub(super) fn levels(pixels: usize, effort: usize) -> Layout {
    let grade = effort.min(DEFAULT_EFFORT);
    match (pixels as u64 * (grade * grade) as u64 / (DEFAULT_EFFORT * DEFAULT_EFFORT) as u64) as usize {
        0..2048 => Layout::flat(),
        2048..16384 => Layout::new(&[2]),
        16384..65536 => Layout::new(&[2, 10]),
        _ => Layout::new(&[1, 4, 12, 30]),
    }
}

pub(super) fn encode(rgba: &[u8], width: usize, height: usize, effort: usize, learner: &mut Learner) -> Vec<u8> {
    let kind = choose(rgba, width, height, effort);
    let planes = split(rgba, width, height, kind);
    let budget = Budget::new(effort);
    let full = Area { left: 0, top: 0, width, height };
    let plane = |channel: usize| Plane { data: &planes.data[channel], link: channel.checked_sub(1).map(|link| &planes.data[link][..]), stride: width, gain: 1 };
    let learned: Vec<Learned> = (0..CHANNELS).map(|channel| learner.learn(&plane(channel), &full, &budget)).collect();
    let mut tree = Tree::new();
    let mut bases = Vec::with_capacity(CHANNELS);
    let mut cells = 0;
    let roots: Vec<usize> = learned.iter().map(|learned| {
        bases.push(cells);
        cells += learned.leaves;
        learned.grow(&mut tree, cells - learned.leaves, 1)
    }).collect();
    let root = tree.ladder(0, &[0, 1, 2], &roots);
    let mut tally = Tally::new(cells);
    let whole = width <= GROUP && height <= GROUP;
    let (across, down) = if whole { (1, 1) } else { (width.div_ceil(GROUP), height.div_ceil(GROUP)) };
    let side = if whole { width.max(height) } else { GROUP };
    let mut streams = Vec::with_capacity(across * down);
    for tile in 0..across * down {
        let (left, top) = (tile % across * side, tile / across * side);
        let area = Area { left, top, width: (planes.width - left).min(side), height: (planes.height - top).min(side) };
        let mut stream = Stream::new(area.width * area.height * CHANNELS / 2 + 16);
        for (channel, learned) in learned.iter().enumerate() {
            learned.code(learner, &plane(channel), &area, bases[channel], &mut stream, &mut tally);
        }
        stream.flush(&mut tally);
        streams.push(stream);
    }
    seal(&tree, root, &tally, &streams, (kind != 0).then_some(kind), whole, effort)
}

fn seal(tree: &Tree, root: usize, tally: &Tally, streams: &[Stream], transform: Option<u32>, whole: bool, effort: usize) -> Vec<u8> {
    let model = Model::fit(tally, effort);
    let mut global = BitWriter::with_capacity(streams.len() * 4096 + 1024);
    global.flag(true);
    global.flag(true);
    let leaves = tree.store(&mut global, root);
    model.store(&mut global, &leaves);
    store_header(&mut global, transform);
    if whole {
        for stream in streams {
            stream.store(&mut global, &model.coder, &model.clusters);
        }
        return frame::sections(vec![global.finish()]);
    }
    model.coder.idle(&mut global);
    let mut parts = vec![global.finish(), Vec::new(), Vec::new()];
    for stream in streams {
        let mut writer = BitWriter::with_capacity(GROUP * GROUP / 2);
        store_header(&mut writer, None);
        stream.store(&mut writer, &model.coder, &model.clusters);
        parts.push(writer.finish());
    }
    frame::sections(parts)
}

pub(super) fn mask(keep: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut tree = Tree::new();
    let color = tree.leaf(0, ZERO, 1);
    let alpha = tree.leaf(1, NORTH, 255);
    let root = tree.split(0, 2, alpha, color);
    let mut tally = Tally::new(2);
    let whole = width <= GROUP && height <= GROUP;
    let (across, down) = if whole { (1, 1) } else { (width.div_ceil(GROUP), height.div_ceil(GROUP)) };
    let side = if whole { width.max(height) } else { GROUP };
    let mut streams = Vec::with_capacity(across * down);
    let mut misses = Vec::with_capacity(side.min(width));
    for tile in 0..across * down {
        let (left, top) = (tile % across * side, tile / across * side);
        let (wide, tall) = ((width - left).min(side), (height - top).min(side));
        let mut stream = Stream::new(tall * 4 + 64);
        stream.push(&mut tally, 0, 0);
        stream.repeat(&mut tally, 0, (3 * wide * tall - 1) as u32);
        for row in 0..tall {
            let here = &keep[(top + row) * width + left..(top + row) * width + left + wide];
            misses.clear();
            if row == 0 {
                misses.push(pack(i32::from(here[0])));
                misses.extend(here.windows(2).map(|pair| pack(i32::from(pair[1]) - i32::from(pair[0]))));
            } else {
                let above = &keep[(top + row - 1) * width + left..(top + row - 1) * width + left + wide];
                if here == above {
                    if stream.last() != 0 {
                        stream.push(&mut tally, 1, 0);
                        stream.repeat(&mut tally, 1, (wide - 1) as u32);
                    } else {
                        stream.repeat(&mut tally, 1, wide as u32);
                    }
                    continue;
                }
                misses.extend(here.iter().zip(above).map(|(value, over)| pack(i32::from(*value) - i32::from(*over))));
            }
            for miss in &misses {
                if *miss == stream.last() && *miss == 0 {
                    stream.repeat(&mut tally, 1, 1);
                } else {
                    stream.push(&mut tally, 1, *miss);
                }
            }
        }
        stream.flush(&mut tally);
        streams.push(stream);
    }
    seal(&tree, root, &tally, &streams, None, whole, 0)
}
