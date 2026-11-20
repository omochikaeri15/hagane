use super::{
    bits::BitWriter,
    entropy::{Coder, Hybrid, store_map},
    frame,
    learn::{Budget, Learner, Plane},
    lossless::{Area, Rule, gradient, levels, scan},
    modular::{GRADIENT, Model, Stream, Tally, Tree, condense, pack, store_header},
    tables::{BLOCK, COUNT_BUCKETS, DENSITY, FREQUENCY, ORDER, REMAINING, REMAINING_STARTS},
};

const GROUP: usize = 256;
const GROUP_BLOCKS: usize = GROUP / 8;
const FIELD: usize = 2048 / 8;
const BASE_FIELD: f32 = 64.0;
const SCALE_UNIT: f32 = 2_088_960.0;
const COEFFICIENT_UNIT: f32 = 261_120.0;
const MEAN_UNIT: f32 = 130_560.0;
const MEAN_DIVIDER: u32 = 16;
const ONE: u64 = 0x3c00;
const SNAP: f32 = 12_582_912.0;
const DARK: f32 = -127.5;
const BRIGHT: f32 = 126.5;
const BANDS: usize = 7;
const SPREAD: usize = 8 * BANDS * 2;
const CHANNELS: usize = 3;
const COUNT_CELLS: usize = COUNT_BUCKETS * CHANNELS;
const CELLS: usize = COUNT_CELLS + SPREAD * CHANNELS;
const SYMBOLS: usize = 64;
const HYBRID: Hybrid = Hybrid::new(4, 2, 0);
const CELL_SHIFT: u32 = 20;
const ZERO: u8 = 0;
const WEST: u8 = 1;
const ORDERED: [usize; CHANNELS] = [1, 0, 2];

#[derive(Clone, Copy)]
pub(super) struct Tuning {
    pub(super) step: f32,
    pub(super) dead: f32,
    pub(super) chroma: f32,
    pub(super) mean: f32,
    pub(super) mask: u32,
    pub(super) effort: usize,
}

const CHROMA: f32 = 0.30;
const FINEST: f32 = 0.62;
const LAST: f32 = 0.75;
const STEPS: [(f32, f32); 6] = [(0.1, 0.8), (0.55, 3.7), (1.9, 12.0), (4.6, 20.0), (9.07, 38.0), (25.0, 80.0)];

impl Tuning {
    pub(super) fn new(quality: u8, effort: u8) -> Self {
        let quality = f32::from(quality.clamp(1, 99));
        let distance = if quality >= 30.0 { 0.1 + (100.0 - quality) * 0.09 } else { 53.0 / 3000.0 * quality * quality - 23.0 / 20.0 * quality + 25.0 };
        let upper = STEPS.iter().position(|point| point.0 >= distance).unwrap_or(STEPS.len() - 1).max(1);
        let (low, high) = (STEPS[upper - 1], STEPS[upper]);
        let sharpen = if effort <= 70 { 1.0 - (1.0 - FINEST) * f32::from(effort.max(30) - 30) / 40.0 } else { FINEST + (LAST - FINEST) * f32::from(effort.min(100) - 70) / 30.0 };
        let step = (low.1 + (high.1 - low.1) * ((distance - low.0) / (high.0 - low.0)).clamp(0.0, 1.0)) * sharpen;
        let mask = match distance {
            _ if distance < 1.0 => 1,
            _ if distance < 3.5 => 3,
            _ => 5,
        };
        Self { step, dead: 0.58, chroma: CHROMA * sharpen, mean: 0.5, mask, effort: usize::from(effort.min(100)) }
    }
}

fn half(value: f32) -> (u64, f32) {
    let exponent = value.max(1e-4).log2().floor().clamp(-14.0, 15.0) as i32;
    let unit = 2f32.powi(exponent);
    let mantissa = ((value / unit - 1.0) * 1024.0).round().clamp(0.0, 1023.0);
    (((exponent + 15) as u64) << 10 | mantissa as u64, (1.0 + mantissa / 1024.0) * unit)
}

type Square = [f32; BLOCK];

#[inline]
fn fold(input: &Square, output: &mut Square) {
    const SCALE: f32 = std::f32::consts::SQRT_2 / 8.0;
    const C1: f32 = 0.980_785_3 * SCALE;
    const C2: f32 = 0.923_879_5 * SCALE;
    const C3: f32 = 0.831_469_6 * SCALE;
    const C4: f32 = std::f32::consts::FRAC_1_SQRT_2 * SCALE;
    const C5: f32 = 0.555_570_2 * SCALE;
    const C6: f32 = 0.382_683_43 * SCALE;
    const C7: f32 = 0.195_090_32 * SCALE;
    for lane in 0..8 {
        let (sum0, gap0) = (input[lane] + input[56 + lane], input[lane] - input[56 + lane]);
        let (sum1, gap1) = (input[8 + lane] + input[48 + lane], input[8 + lane] - input[48 + lane]);
        let (sum2, gap2) = (input[16 + lane] + input[40 + lane], input[16 + lane] - input[40 + lane]);
        let (sum3, gap3) = (input[24 + lane] + input[32 + lane], input[24 + lane] - input[32 + lane]);
        let (outer, wide) = (sum0 + sum3, sum0 - sum3);
        let (inner, near) = (sum1 + sum2, sum1 - sum2);
        output[lane] = (outer + inner) * 0.125;
        output[8 + lane] = gap0 * C1 + gap1 * C3 + gap2 * C5 + gap3 * C7;
        output[16 + lane] = wide * C2 + near * C6;
        output[24 + lane] = gap0 * C3 - gap1 * C7 - gap2 * C1 - gap3 * C5;
        output[32 + lane] = (outer - inner) * C4;
        output[40 + lane] = gap0 * C5 - gap1 * C1 + gap2 * C7 + gap3 * C3;
        output[48 + lane] = wide * C6 - near * C2;
        output[56 + lane] = gap0 * C7 - gap1 * C5 + gap2 * C3 - gap3 * C1;
    }
}

#[inline]
fn span(values: &Square) -> (f32, f32) {
    let (mut low, mut high) = ([f32::MAX; 8], [f32::MIN; 8]);
    for row in values.as_chunks::<8>().0 {
        for lane in 0..8 {
            low[lane] = if row[lane] < low[lane] { row[lane] } else { low[lane] };
            high[lane] = if row[lane] > high[lane] { row[lane] } else { high[lane] };
        }
    }
    (low.iter().fold(f32::MAX, |least, value| least.min(*value)), high.iter().fold(f32::MIN, |most, value| most.max(*value)))
}

#[inline]
fn turn(input: &Square, output: &mut Square) {
    for row in 0..8 {
        for lane in 0..8 {
            output[lane * 8 + row] = input[row * 8 + lane];
        }
    }
}

#[inline]
pub(super) fn transform(pixels: &Square) -> Square {
    let (mut first, mut second) = ([0f32; BLOCK], [0f32; BLOCK]);
    fold(pixels, &mut first);
    turn(&first, &mut second);
    fold(&second, &mut first);
    first
}

struct Grid {
    wide: usize,
    tall: usize,
    means: [Vec<i16>; CHANNELS],
    field: Vec<u8>,
    blank: Vec<bool>,
}

struct Scales {
    coefficient: [f32; CHANNELS],
    mean: f32,
    dead: f32,
}

struct Lookup {
    remaining: [u16; BLOCK],
    band: [u8; BLOCK],
    symbol: [u8; 256],
}

impl Lookup {
    #[inline]
    fn symbol(&self, value: u32) -> usize {
        self.symbol.get(value as usize).map_or_else(|| HYBRID.token(value).0 as usize, |symbol| usize::from(*symbol))
    }
}

impl Lookup {
    fn new() -> Self {
        let mut remaining = [0u16; BLOCK];
        let mut band = [0u8; BLOCK];
        for at in 1..BLOCK {
            let group = REMAINING_STARTS.iter().rposition(|start| *start <= REMAINING[at]).unwrap_or(0);
            remaining[at] = (group * BANDS * 2) as u16;
            band[at] = band_of(usize::from(FREQUENCY[at])) * 2;
        }
        Self { remaining, band, symbol: std::array::from_fn(|value| HYBRID.token(value as u32).0 as u8) }
    }
}

fn band_of(frequency: usize) -> u8 {
    match frequency {
        0..2 => 0,
        2..5 => 1,
        5..9 => 2,
        9..14 => 3,
        14..18 => 4,
        18..23 => 5,
        _ => 6,
    }
}

fn cell_of(context: usize) -> usize {
    if context < COUNT_CELLS {
        return context;
    }
    let (channel, inner) = ((context - COUNT_CELLS) / DENSITY, (context - COUNT_CELLS) % DENSITY);
    let (level, previous) = (inner / 2, inner % 2);
    let group = REMAINING_STARTS.iter().rposition(|start| usize::from(*start) <= level).unwrap_or(0);
    let frequency = (level - usize::from(REMAINING_STARTS[group])).min(30);
    COUNT_CELLS + channel * SPREAD + (group * BANDS + usize::from(band_of(frequency))) * 2 + previous
}

struct Counts {
    cells: Vec<[u32; SYMBOLS]>,
}

struct Pass<'a> {
    rgba: &'a [u8],
    width: usize,
    height: usize,
    scales: Scales,
    lookup: Lookup,
}

impl Pass<'_> {
    fn load(&self, left: usize, top: usize) -> ([Square; CHANNELS], u32) {
        if left + 8 <= self.width && top + 8 <= self.height {
            let mut pixels = [0i32; BLOCK];
            for row in 0..8 {
                let start = ((top + row) * self.width + left) * 4;
                for (slot, pixel) in pixels[row * 8..row * 8 + 8].iter_mut().zip(self.rgba[start..start + 32].as_chunks::<4>().0) {
                    *slot = i32::from_le_bytes([pixel[0], pixel[1], pixel[2], pixel[3]]);
                }
            }
            let (mut clear, mut high) = (0i32, 0i32);
            for pixel in &pixels {
                let alpha = (pixel >> 24) & 255;
                clear |= i32::from(alpha == 0);
                high = if alpha > high { alpha } else { high };
            }
            if high == 0 {
                return ([[0f32; BLOCK]; CHANNELS], 0);
            }
            if clear == 0 {
                let mut planes = [[0f32; BLOCK]; CHANNELS];
                for at in 0..BLOCK {
                    let (r, g, b) = ((pixels[at] & 255) as f32, ((pixels[at] >> 8) & 255) as f32, ((pixels[at] >> 16) & 255) as f32);
                    planes[1][at] = 0.299 * r + 0.587 * g + 0.114 * b - 128.0;
                    planes[0][at] = -0.168_736 * r - 0.331_264 * g + 0.5 * b;
                    planes[2][at] = 0.5 * r - 0.418_688 * g - 0.081_312 * b;
                }
                return (planes, high as u32);
            }
        }
        let mut red = [[0f32; 8]; 8];
        let mut green = [[0f32; 8]; 8];
        let mut blue = [[0f32; 8]; 8];
        let mut seen = [[false; 8]; 8];
        let (mut sums, mut solid, mut peak) = ([0f32; 3], 0u32, 0u32);
        for row in 0..8.min(self.height - top) {
            let start = ((top + row) * self.width + left) * 4;
            let line = &self.rgba[start..start + 8.min(self.width - left) * 4];
            for (at, pixel) in line.as_chunks::<4>().0.iter().enumerate() {
                if pixel[3] == 0 {
                    continue;
                }
                let color = [f32::from(pixel[0]), f32::from(pixel[1]), f32::from(pixel[2])];
                red[row][at] = color[0];
                green[row][at] = color[1];
                blue[row][at] = color[2];
                seen[row][at] = true;
                sums = [sums[0] + color[0], sums[1] + color[1], sums[2] + color[2]];
                solid += 1;
                peak = peak.max(u32::from(pixel[3]));
            }
        }
        if solid != 0 && solid != 64 {
            let fill = [sums[0] / solid as f32, sums[1] / solid as f32, sums[2] / solid as f32];
            for row in 0..8 {
                for at in 0..8 {
                    if !seen[row][at] {
                        red[row][at] = fill[0];
                        green[row][at] = fill[1];
                        blue[row][at] = fill[2];
                    }
                }
            }
        }
        let mut planes = [[0f32; BLOCK]; CHANNELS];
        for row in 0..8 {
            for at in 0..8 {
                let (r, g, b) = (red[row][at], green[row][at], blue[row][at]);
                planes[1][row * 8 + at] = 0.299 * r + 0.587 * g + 0.114 * b - 128.0;
                planes[0][row * 8 + at] = -0.168_736 * r - 0.331_264 * g + 0.5 * b;
                planes[2][row * 8 + at] = 0.5 * r - 0.418_688 * g - 0.081_312 * b;
            }
        }
        (planes, peak)
    }

    fn group(&self, grid: &mut Grid, counts: &mut Counts, left: usize, top: usize) -> Vec<u32> {
        let wide = (grid.wide - left).min(GROUP_BLOCKS);
        let tall = (grid.tall - top).min(GROUP_BLOCKS);
        let mut tokens = Vec::with_capacity(wide * tall * 24);
        let mut tallies = vec![[0u8; CHANNELS]; wide * tall];
        for row in 0..tall {
            for column in 0..wide {
                let at = (top + row) * grid.wide + left + column;
                let (planes, peak) = self.load((left + column) * 8, (top + row) * 8);
                grid.blank[at] = peak == 0;
                let spectra = if peak == 0 { [[0f32; BLOCK]; CHANNELS] } else { [transform(&planes[0]), transform(&planes[1]), transform(&planes[2])] };
                let field = (BASE_FIELD as u32 * peak).div_ceil(255).clamp(1, 255) as u8;
                grid.field[at] = field;
                for channel in ORDERED {
                    let scale = self.scales.coefficient[channel] * f32::from(field);
                    let mut levels = [0f32; BLOCK];
                    let mut total = 0u32;
                    if peak != 0 {
                        let spectrum = &spectra[channel];
                        let mean = spectrum[0] * self.scales.mean;
                        let settled = if channel == 1 {
                            let (low, high) = span(&planes[1]);
                            if high <= DARK {
                                mean.floor()
                            } else if low >= BRIGHT {
                                mean.ceil()
                            } else {
                                mean.round()
                            }
                        } else {
                            mean.round()
                        };
                        grid.means[channel][at] = settled as i16;
                        let mut busy = 0i32;
                        for (slot, value) in levels.iter_mut().zip(spectrum) {
                            let scaled = value * scale;
                            let near = (scaled + SNAP) - SNAP;
                            let kept = near != 0.0 && scaled.abs() >= self.scales.dead;
                            *slot = if kept { near } else { 0.0 };
                            busy += i32::from(kept);
                        }
                        total = (busy - i32::from(levels[0] != 0.0)) as u32;
                        levels[0] = 0.0;
                    }
                    let here = row * wide + column;
                    let guess = match (row, column) {
                        (0, 0) => 32,
                        (0, _) => u32::from(tallies[here - 1][channel]),
                        (_, 0) => u32::from(tallies[here - wide][channel]),
                        _ => (u32::from(tallies[here - wide][channel]) + u32::from(tallies[here - 1][channel])).div_ceil(2),
                    };
                    tallies[here][channel] = total as u8;
                    let bucket = if guess < 8 { guess } else { 4 + guess / 2 } as usize;
                    let plane = if channel == 1 { 0 } else { channel / 2 + 1 };
                    let cell = bucket * CHANNELS + plane;
                    counts.cells[cell][self.lookup.symbol(total)] += 1;
                    tokens.push((cell as u32) << CELL_SHIFT | total);
                    if total == 0 {
                        continue;
                    }
                    let base = COUNT_CELLS + plane * SPREAD;
                    let mut left_over = total as usize;
                    let mut previous = usize::from(total <= 4);
                    for step in 1..BLOCK {
                        let level = levels[usize::from(ORDER[step])] as i32;
                        let cell = base + usize::from(self.lookup.remaining[left_over]) + usize::from(self.lookup.band[step]) + previous;
                        let value = pack(level);
                        counts.cells[cell][self.lookup.symbol(value)] += 1;
                        tokens.push((cell as u32) << CELL_SHIFT | value);
                        previous = usize::from(level != 0);
                        left_over -= previous;
                        if left_over == 0 {
                            break;
                        }
                    }
                }
            }
        }
        tokens
    }
}

fn settle(grid: &mut Grid, area: &Area) {
    for plane in &mut grid.means {
        for row in 0..area.height {
            for column in 0..area.width {
                let at = (area.top + row) * grid.wide + area.left + column;
                if !grid.blank[at] {
                    continue;
                }
                let west = if column > 0 { plane[at - 1] } else if row > 0 { plane[at - grid.wide] } else { 0 };
                let north = if row > 0 { plane[at - grid.wide] } else { west };
                let corner = if row > 0 && column > 0 { plane[at - grid.wide - 1] } else { west };
                plane[at] = gradient(i32::from(west), i32::from(north), i32::from(corner)) as i16;
            }
        }
    }
}

fn fit(counts: &Counts, effort: usize) -> (Coder, Vec<u8>) {
    let mut clusters = vec![0u8; CELLS];
    let mut ranks: Vec<(u32, usize)> = Vec::with_capacity(CELLS);
    for (cell, tally) in counts.cells.iter().enumerate() {
        let total: u64 = tally.iter().map(|count| u64::from(*count)).sum();
        let weight: u64 = tally.iter().enumerate().map(|(symbol, count)| symbol as u64 * u64::from(*count)).sum();
        if let Some(rank) = (weight * 64).checked_div(total) {
            ranks.push((rank as u32, cell));
        }
    }
    ranks.sort_unstable();
    let mut merged: Vec<Vec<u32>> = Vec::new();
    let mut start = 0u32;
    for (rank, cell) in &ranks {
        if merged.is_empty() || (*rank >= start + 4 + start / 12 && merged.len() < 12 + effort / 4) {
            merged.push(vec![0u32; SYMBOLS]);
            start = *rank;
        }
        let table = merged.len() - 1;
        clusters[*cell] = table as u8;
        for (sum, count) in merged[table].iter_mut().zip(&counts.cells[*cell]) {
            *sum += count;
        }
    }
    if merged.is_empty() {
        merged.push(vec![0u32; SYMBOLS]);
    }
    let moved = condense(&mut merged);
    for cluster in &mut clusters {
        *cluster = moved[usize::from(*cluster)];
    }
    for cell in 0..CELLS {
        if counts.cells[cell].iter().all(|count| *count == 0) {
            clusters[cell] = if cell >= COUNT_CELLS + 2 { clusters[cell - 2] } else if cell > 0 { clusters[cell - 1] } else { 0 };
        }
    }
    (Coder::new(HYBRID, &merged, false, true), clusters)
}

fn pour(writer: &mut BitWriter, coder: &Coder, clusters: &[u8], tokens: &[u32]) {
    coder.emit(writer, tokens.iter().map(|token| (usize::from(clusters[(token >> CELL_SHIFT) as usize]), false, token & ((1 << CELL_SHIFT) - 1))));
}

pub(super) fn encode(rgba: &[u8], width: usize, height: usize, tuning: &Tuning, learner: &mut Learner) -> Vec<u8> {
    let (wide, tall) = (width.div_ceil(8), height.div_ceil(8));
    let global = (SCALE_UNIT / (tuning.step * BASE_FIELD)).round().clamp(1.0, 73_728.0);
    let (chroma_bits, chroma) = half(tuning.chroma);
    let (mean_bits, mean) = half(tuning.mean);
    let luma = global / COEFFICIENT_UNIT;
    let scales = Scales { coefficient: [luma * chroma, luma, luma * chroma], mean: global * MEAN_DIVIDER as f32 / (MEAN_UNIT * mean), dead: tuning.dead };
    let pass = Pass { rgba, width, height, scales, lookup: Lookup::new() };
    let mut grid = Grid { wide, tall, means: [vec![0; wide * tall], vec![0; wide * tall], vec![0; wide * tall]], field: vec![1; wide * tall], blank: vec![false; wide * tall] };
    let mut counts = Counts { cells: vec![[0; SYMBOLS]; CELLS] };
    let (across, down) = (wide.div_ceil(GROUP_BLOCKS), tall.div_ceil(GROUP_BLOCKS));
    let groups: Vec<Vec<u32>> = (0..across * down).map(|index| pass.group(&mut grid, &mut counts, index % across * GROUP_BLOCKS, index / across * GROUP_BLOCKS)).collect();
    let (field_across, field_down) = (wide.div_ceil(FIELD), tall.div_ceil(FIELD));
    let fields = field_across * field_down;
    let whole = across * down == 1;

    let mean_layout = levels(wide * tall * 16, tuning.effort);
    let grain = tuning.mask.max(1) as i16;
    let mask: Vec<i16> = rgba.as_chunks::<4>().0.iter().map(|pixel| (i16::from(pixel[3]) + grain / 2) / grain).collect();
    let plane = Plane { data: &mask, link: None, stride: width, gain: i32::from(grain) };
    let learned = learner.learn(&plane, &Area { left: 0, top: 0, width, height }, &Budget::new(tuning.effort));
    let mean_cells = mean_layout.cells();
    let zero_cell = CHANNELS * mean_cells;
    let field_cell = zero_cell + 1;
    let mask_cell = field_cell + 1;
    let mut tree = Tree::new();
    let mean_roots: Vec<usize> = (0..CHANNELS).map(|channel| mean_layout.grow(&mut tree, channel * mean_cells, GRADIENT, 1)).collect();
    let mean_root = tree.ladder(0, &[0, 1], &mean_roots);
    let zeros = [tree.leaf(zero_cell, ZERO, 1), tree.leaf(zero_cell, ZERO, 1), tree.leaf(zero_cell, ZERO, 1)];
    let field_leaf = tree.leaf(field_cell, WEST, 1);
    let rows = tree.split(2, 0, field_leaf, zeros[0]);
    let upper = tree.split(0, 2, zeros[1], rows);
    let side_root = tree.split(0, 1, upper, zeros[2]);
    let mask_root = learned.grow(&mut tree, mask_cell, tuning.mask.max(1));
    let inner = tree.split(1, 2 * fields as i32, side_root, mean_root);
    let root = if whole { tree.split(1, 0, inner, mask_root) } else { tree.split(1, 3 * fields as i32, mask_root, inner) };

    let mut tally = Tally::new(mask_cell + learned.leaves);
    let mut mean_streams = Vec::with_capacity(fields);
    let mut side_streams = Vec::with_capacity(fields);
    for index in 0..fields {
        let (left, top) = (index % field_across * FIELD, index / field_across * FIELD);
        let area = Area { left, top, width: (wide - left).min(FIELD), height: (tall - top).min(FIELD) };
        settle(&mut grid, &area);
        let mut stream = Stream::new(area.width * area.height * 2);
        for (channel, plane) in ORDERED.iter().enumerate() {
            scan(&grid.means[*plane], wide, &area, &Rule { layout: &mean_layout, base: channel * mean_cells, gain: 1 }, &mut stream, &mut tally);
        }
        stream.flush(&mut tally);
        mean_streams.push(stream);
        let blocks = area.width * area.height;
        let tiles = area.width.div_ceil(8) * area.height.div_ceil(8);
        let mut stream = Stream::new(blocks + 64);
        for _ in 0..2 * tiles + blocks {
            stream.push(&mut tally, zero_cell, 0);
        }
        let mut before = 0i32;
        for row in 0..area.height {
            for column in 0..area.width {
                let at = (top + row) * wide + left + column;
                let value = if grid.blank[at] { before } else { i32::from(grid.field[at]) - 1 };
                stream.push(&mut tally, field_cell, pack(value - before));
                before = value;
            }
        }
        for _ in 0..blocks {
            stream.push(&mut tally, zero_cell, 0);
        }
        stream.flush(&mut tally);
        side_streams.push((stream, blocks));
    }
    let mut mask_streams = Vec::with_capacity(across * down);
    for index in 0..across * down {
        let (left, top) = (index % across * GROUP, index / across * GROUP);
        let area = Area { left, top, width: (width - left).min(GROUP), height: (height - top).min(GROUP) };
        let mut stream = Stream::new(area.width * area.height / 2 + 16);
        learned.code(learner, &plane, &area, mask_cell, &mut stream, &mut tally);
        stream.flush(&mut tally);
        mask_streams.push(stream);
    }
    let model = Model::fit(&tally, tuning.effort);
    let (coder, clusters) = fit(&counts, tuning.effort);

    let mut parts: Vec<Vec<u8>> = Vec::with_capacity(2 + fields + groups.len());
    let mut writer = BitWriter::with_capacity(width * height / 4 + 1024);
    writer.flag(false);
    for _ in 0..CHANNELS {
        writer.write(16, mean_bits);
    }
    writer.select(global as u32, [(11, 1), (11, 2049), (12, 4097), (16, 8193)]);
    writer.select(MEAN_DIVIDER, [(0, 16), (5, 1), (8, 1), (16, 1)]);
    writer.flag(false);
    writer.write(16, 0);
    let planes: Vec<u8> = (0..39u8).map(|entry| entry / 13).collect();
    store_map(&mut writer, &planes, CHANNELS);
    writer.flag(false);
    writer.write(2, 0);
    writer.write(16, 0);
    writer.write(16, 0);
    writer.write(8, 128);
    writer.write(8, 128);
    writer.flag(true);
    let leaves = tree.store(&mut writer, root);
    model.store(&mut writer, &leaves);
    store_header(&mut writer, None);
    if whole {
        mask_streams[0].store(&mut writer, &model.coder, &model.clusters);
    } else {
        model.coder.idle(&mut writer);
        parts.push(std::mem::replace(&mut writer, BitWriter::new()).finish());
    }
    for (means, (sides, blocks)) in mean_streams.iter().zip(&side_streams) {
        writer.write(2, 0);
        store_header(&mut writer, None);
        means.store(&mut writer, &model.coder, &model.clusters);
        let width = (*blocks as u32).next_power_of_two().ilog2();
        writer.write(width, *blocks as u64 - 1);
        store_header(&mut writer, None);
        sides.store(&mut writer, &model.coder, &model.clusters);
        if !whole {
            parts.push(std::mem::replace(&mut writer, BitWriter::new()).finish());
        }
    }
    writer.flag(false);
    writer.write(3, 6);
    writer.write(4, 0);
    for bits in [chroma_bits, ONE, chroma_bits] {
        writer.write(16, bits);
    }
    for _ in 1..17 {
        writer.write(3, 0);
    }
    writer.write((groups.len() as u32).next_power_of_two().ilog2(), 0);
    writer.write(2, 2);
    let map: Vec<u8> = (0..CHANNELS * (COUNT_BUCKETS + DENSITY)).map(|context| clusters[cell_of(context)]).collect();
    coder.store(&mut writer, &map);
    if !whole {
        parts.push(std::mem::replace(&mut writer, BitWriter::new()).finish());
    }
    for (tokens, mask) in groups.iter().zip(&mask_streams) {
        pour(&mut writer, &coder, &clusters, tokens);
        if !whole {
            store_header(&mut writer, None);
            mask.store(&mut writer, &model.coder, &model.clusters);
            parts.push(std::mem::replace(&mut writer, BitWriter::new()).finish());
        }
    }
    if whole {
        parts.push(writer.finish());
    }
    frame::sections(parts)
}

#[cfg(test)]
mod tests {
    use super::{BLOCK, CELLS, COUNT_BUCKETS, DENSITY, Tuning, cell_of, half, transform};

    #[test]
    fn transform_matches_the_plain_cosine_sum() {
        let pixels: [f32; BLOCK] = std::array::from_fn(|at| ((at * 37 + 11) % 97) as f32 - 40.0);
        let fast = transform(&pixels);
        for across in 0..8 {
            for down in 0..8 {
                let mut sum = 0f64;
                for y in 0..8 {
                    for x in 0..8 {
                        let wave = |index: usize, rate: usize| (std::f64::consts::PI * (2 * index + 1) as f64 * rate as f64 / 16.0).cos();
                        sum += f64::from(pixels[y * 8 + x]) * wave(x, across) * wave(y, down);
                    }
                }
                let scale = |rate: usize| if rate == 0 { 1.0 } else { std::f64::consts::SQRT_2 };
                let slow = sum / 64.0 * scale(across) * scale(down);
                assert!((f64::from(fast[across * 8 + down]) - slow).abs() < 1e-3, "{across} {down}");
            }
        }
    }

    #[test]
    fn half_floats_round_trip() {
        for value in [0.25f32, 0.35, 0.5, 1.0, 3.0, 0.001] {
            let (bits, exact) = half(value);
            let rebuilt = (1.0 + (bits & 1023) as f32 / 1024.0) * 2f32.powi(((bits >> 10) & 31) as i32 - 15);
            assert_eq!(rebuilt, exact);
            assert!((exact - value).abs() <= value * 0.001);
        }
    }

    #[test]
    fn quality_moves_the_step_one_way() {
        let steps: Vec<f32> = (1..=99).map(|quality| Tuning::new(quality, 30).step).collect();
        assert!(steps.windows(2).all(|pair| pair[1] < pair[0]));
        assert!(steps[98] < 2.0 && steps[0] > 60.0);
    }

    #[test]
    fn every_context_lands_in_a_cell() {
        let cells: Vec<usize> = (0..3 * (COUNT_BUCKETS + DENSITY)).map(cell_of).collect();
        assert!(cells.iter().all(|cell| *cell < CELLS));
        assert_eq!(cells[..3 * COUNT_BUCKETS], (0..3 * COUNT_BUCKETS).collect::<Vec<_>>()[..]);
    }
}
