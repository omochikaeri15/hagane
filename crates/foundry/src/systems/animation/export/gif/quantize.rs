const AXES: usize = 3;
const LEVELS: usize = 256;
const CELL_SHIFT: u32 = 3;
const CELL_SIDE: usize = 1 << (8 - CELL_SHIFT);
const CELLS: usize = CELL_SIDE * CELL_SIDE * CELL_SIDE;
const CELL_REACH: f32 = 6.07;
const FIRST_SLOTS: usize = 1 << 14;
const SPREAD: u32 = 0x9E37_79B1;
const HEAVY_SHARE: u32 = 4096;
const HEAVY_FLOOR: u32 = 8;
const NOWHERE: u32 = u32::MAX;

struct Tally {
    keys: Vec<u32>,
    counts: Vec<u32>,
    bits: u32,
    held: usize,
}

impl Tally {
    fn new(slots: usize) -> Self {
        Self { keys: vec![0; slots], counts: vec![0; slots], bits: slots.trailing_zeros(), held: 0 }
    }

    fn slot(&self, key: u32) -> usize {
        let mask = self.keys.len() - 1;
        let mut slot = (key.wrapping_mul(SPREAD) >> (32 - self.bits)) as usize;

        while self.keys[slot] != 0 && self.keys[slot] != key {
            slot = (slot + 1) & mask;
        }

        slot
    }

    fn add(&mut self, key: u32, count: u32) {
        let slot = self.slot(key);

        if self.keys[slot] == 0 {
            self.keys[slot] = key;
            self.held += 1;
        }

        self.counts[slot] += count;

        if self.held * 2 > self.keys.len() {
            self.grow();
        }
    }

    fn grow(&mut self) {
        let mut wider = Self::new(self.keys.len() * 4);

        for (key, count) in self.keys.iter().zip(&self.counts).filter(|(key, _)| **key != 0) {
            let slot = wider.slot(*key);

            wider.keys[slot] = *key;
            wider.counts[slot] = *count;
        }

        wider.held = self.held;
        *self = wider;
    }
}

fn key(rgb: [u8; 3]) -> u32 {
    (u32::from(rgb[0]) << 16 | u32::from(rgb[1]) << 8 | u32::from(rgb[2])) + 1
}

fn color(key: u32) -> [u8; 3] {
    let packed = key - 1;

    [(packed >> 16) as u8, (packed >> 8) as u8, packed as u8]
}

fn cell(rgb: [u8; 3]) -> usize {
    ((rgb[0] >> CELL_SHIFT) as usize * CELL_SIDE + (rgb[1] >> CELL_SHIFT) as usize) * CELL_SIDE + (rgb[2] >> CELL_SHIFT) as usize
}

pub(super) struct Palette {
    pub colors: Vec<[u8; 3]>,
    tally: Tally,
}

impl Palette {
    pub(super) fn index(&self, rgb: [u8; 3]) -> u8 {
        self.tally.counts[self.tally.slot(key(rgb))] as u8
    }
}

#[derive(Clone, Copy)]
struct Spot {
    weight: f32,
    mean: [f32; AXES],
    owner: u32,
}

#[derive(Clone, Copy, Default)]
struct Sum {
    weight: f64,
    total: [f64; AXES],
}

impl Sum {
    fn add(&mut self, spot: &Spot) {
        self.weight += f64::from(spot.weight);

        for axis in 0..AXES {
            self.total[axis] += f64::from(spot.mean[axis]) * f64::from(spot.weight);
        }
    }

    fn gain(&self) -> f64 {
        if self.weight <= 0.0 {
            return 0.0;
        }

        self.total.iter().map(|total| total * total).sum::<f64>() / self.weight
    }

    fn less(&self, other: &Self) -> Self {
        Self { weight: self.weight - other.weight, total: std::array::from_fn(|axis| self.total[axis] - other.total[axis]) }
    }
}

struct Crate {
    start: usize,
    end: usize,
    sum: Sum,
    spread: f64,
}

fn crated(spots: &[Spot], order: &[u32], start: usize, end: usize) -> Crate {
    let mut sum = Sum::default();
    let mut squares = 0f64;

    for at in &order[start..end] {
        let spot = &spots[*at as usize];

        sum.add(spot);
        squares += spot.mean.iter().map(|value| f64::from(*value) * f64::from(*value)).sum::<f64>() * f64::from(spot.weight);
    }

    Crate { start, end, sum, spread: if end - start > 1 { (squares - sum.gain()).max(0.0) } else { 0.0 } }
}

fn level(value: f32) -> usize {
    (value.round() as usize).min(LEVELS - 1)
}

fn best_cut(spots: &[Spot], members: &[u32], whole: &Sum) -> Option<(usize, usize)> {
    let mut best: Option<(f64, usize, usize)> = None;

    for axis in 0..AXES {
        let mut bins = [Sum::default(); LEVELS];
        let (mut low, mut high) = (LEVELS, 0);

        for at in members {
            let spot = &spots[*at as usize];
            let bin = level(spot.mean[axis]);

            bins[bin].add(spot);
            low = low.min(bin);
            high = high.max(bin);
        }

        let mut near = Sum::default();

        for (cut, bin) in bins.iter().enumerate().take(high).skip(low) {
            near.weight += bin.weight;

            for channel in 0..AXES {
                near.total[channel] += bin.total[channel];
            }

            let far = whole.less(&near);

            if near.weight <= 0.0 || far.weight <= 0.0 {
                continue;
            }

            let score = near.gain() + far.gain();

            if best.is_none_or(|(held, _, _)| score > held) {
                best = Some((score, axis, cut));
            }
        }
    }

    best.map(|(_, axis, cut)| (axis, cut))
}

fn crates(spots: &[Spot], order: &mut [u32], wanted: usize, share: f64) -> Vec<Crate> {
    let mut boxes = vec![crated(spots, order, 0, order.len())];
    let allowed = share * boxes[0].spread;

    while boxes.len() < wanted && boxes.iter().map(|held| held.spread).sum::<f64>() > allowed {
        let Some((pick, _)) = boxes.iter().enumerate().filter(|(_, held)| held.spread > 0.0).max_by(|left, right| left.1.spread.total_cmp(&right.1.spread)) else {
            break;
        };
        let (start, end) = (boxes[pick].start, boxes[pick].end);
        let Some((axis, cut)) = best_cut(spots, &order[start..end], &boxes[pick].sum) else {
            boxes[pick].spread = 0.0;
            continue;
        };
        let members = &mut order[start..end];
        let mut middle = 0;

        for at in 0..members.len() {
            if level(spots[members[at] as usize].mean[axis]) <= cut {
                members.swap(at, middle);
                middle += 1;
            }
        }

        if middle == 0 || middle == members.len() {
            boxes[pick].spread = 0.0;
            continue;
        }

        boxes[pick] = crated(spots, order, start, start + middle);
        boxes.push(crated(spots, order, start + middle, end));
    }

    boxes
}

struct Field {
    planes: [Vec<f32>; AXES],
    gaps: Vec<f32>,
}

impl Field {
    fn new(centers: &[[f32; AXES]]) -> Self {
        Self { planes: std::array::from_fn(|axis| centers.iter().map(|center| center[axis]).collect()), gaps: vec![0.0; centers.len()] }
    }

    fn measure(&mut self, point: &[f32; AXES]) -> f32 {
        let [reds, greens, blues] = &self.planes;

        for (((gap, red), green), blue) in self.gaps.iter_mut().zip(reds).zip(greens).zip(blues) {
            *gap = (red - point[0]) * (red - point[0]) + (green - point[1]) * (green - point[1]) + (blue - point[2]) * (blue - point[2]);
        }

        self.gaps.iter().copied().fold(f32::MAX, f32::min)
    }

    fn nearest(&mut self, point: &[f32; AXES]) -> u32 {
        let closest = self.measure(point);

        self.gaps.iter().position(|gap| *gap == closest).unwrap_or(0) as u32
    }
}

fn refine(spots: &mut [Spot], centers: &mut [[f32; AXES]], rounds: u32) {
    for _ in 0..rounds {
        let mut field = Field::new(centers);
        let mut sums = vec![Sum::default(); centers.len()];
        let mut moved = false;

        for spot in spots.iter_mut() {
            let owner = field.nearest(&spot.mean);

            moved |= owner != spot.owner;
            spot.owner = owner;
            sums[owner as usize].add(spot);
        }

        for (center, sum) in centers.iter_mut().zip(&sums).filter(|(_, sum)| sum.weight > 0.0) {
            *center = std::array::from_fn(|axis| (sum.total[axis] / sum.weight) as f32);
        }

        if !moved {
            break;
        }
    }
}

struct Rivals {
    ranges: Vec<(u32, u32)>,
    pool: Vec<u8>,
}

impl Rivals {
    fn gather(cells: impl Iterator<Item = usize>, colors: &[[u8; 3]]) -> Self {
        let centers: Vec<[f32; AXES]> = colors.iter().map(|color| color.map(f32::from)).collect();
        let mut field = Field::new(&centers);
        let mut ranges = vec![(NOWHERE, 0); CELLS];
        let mut pool: Vec<u8> = Vec::new();
        let span = (1 << CELL_SHIFT) as f32;

        for at in cells {
            if ranges[at].0 != NOWHERE {
                continue;
            }

            let middle = [(at / (CELL_SIDE * CELL_SIDE)) as f32, (at / CELL_SIDE % CELL_SIDE) as f32, (at % CELL_SIDE) as f32].map(|step| step * span + (span - 1.0) / 2.0);
            let limit = (field.measure(&middle).sqrt() + 2.0 * CELL_REACH).powi(2);
            let start = pool.len();

            pool.extend(field.gaps.iter().enumerate().filter(|(_, gap)| **gap <= limit).map(|(index, _)| index as u8));
            ranges[at] = (start as u32, (pool.len() - start) as u32);
        }

        Self { ranges, pool }
    }

    fn pick(&self, rgb: [u8; 3], colors: &[[u8; 3]]) -> u8 {
        let (start, count) = self.ranges[cell(rgb)];
        let mut best = (i32::MAX, 0);

        for rival in &self.pool[start as usize..(start + count) as usize] {
            let held = colors[usize::from(*rival)];
            let gap = (0..AXES).map(|axis| (i32::from(held[axis]) - i32::from(rgb[axis])).pow(2)).sum::<i32>();

            if gap < best.0 {
                best = (gap, *rival);
            }
        }

        best.1
    }
}

fn counted(pixels: impl Iterator<Item = [u8; 3]>) -> (Tally, u32) {
    let mut tally = Tally::new(FIRST_SLOTS);
    let mut total = 0u32;
    let mut run: Option<(u32, u32)> = None;

    for rgb in pixels {
        let next = key(rgb);

        total += 1;

        match &mut run {
            Some((held, count)) if *held == next => *count += 1,
            _ => {
                if let Some((held, count)) = run.replace((next, 1)) {
                    tally.add(held, count);
                }
            }
        }
    }

    if let Some((held, count)) = run {
        tally.add(held, count);
    }

    (tally, total)
}

fn spotted(tally: &Tally, heavy: u32) -> (Vec<Spot>, Vec<u32>) {
    let mut spots: Vec<Spot> = Vec::new();
    let mut sums: Vec<Sum> = Vec::new();
    let mut homes = vec![NOWHERE; tally.keys.len()];
    let mut shared = vec![NOWHERE; CELLS];

    for (slot, (key, count)) in tally.keys.iter().zip(&tally.counts).enumerate().filter(|(_, (key, _))| **key != 0) {
        let rgb = color(*key);
        let exact = Spot { weight: *count as f32, mean: rgb.map(f32::from), owner: 0 };

        if *count >= heavy {
            homes[slot] = spots.len() as u32;
            spots.push(exact);
            sums.push(Sum::default());
            continue;
        }

        let at = cell(rgb);

        if shared[at] == NOWHERE {
            shared[at] = spots.len() as u32;
            spots.push(Spot { weight: 0.0, mean: [0.0; AXES], owner: 0 });
            sums.push(Sum::default());
        }

        sums[shared[at] as usize].add(&exact);
        homes[slot] = shared[at];
    }

    for (spot, sum) in spots.iter_mut().zip(&sums).filter(|(_, sum)| sum.weight > 0.0) {
        *spot = Spot { weight: sum.weight as f32, mean: std::array::from_fn(|axis| (sum.total[axis] / sum.weight) as f32), owner: 0 };
    }

    (spots, homes)
}

pub(super) fn palette(pixels: impl Iterator<Item = [u8; 3]>, wanted: usize, rounds: u32, share: f64) -> Palette {
    let (mut tally, total) = counted(pixels);
    let wanted = wanted.clamp(1, LEVELS);

    if tally.held <= wanted {
        let mut colors = Vec::with_capacity(tally.held);

        for slot in 0..tally.keys.len() {
            if tally.keys[slot] != 0 {
                tally.counts[slot] = colors.len() as u32;
                colors.push(color(tally.keys[slot]));
            }
        }

        if colors.is_empty() {
            colors.push([0; 3]);
        }

        return Palette { colors, tally };
    }

    let heavy = (total / HEAVY_SHARE).max(HEAVY_FLOOR);
    let (mut spots, homes) = spotted(&tally, heavy);
    let mut order: Vec<u32> = (0..spots.len() as u32).collect();
    let boxes = crates(&spots, &mut order, wanted, share);
    let mut centers: Vec<[f32; AXES]> = boxes.iter().map(|held| std::array::from_fn(|axis| (held.sum.total[axis] / held.sum.weight.max(1.0)) as f32)).collect();

    for (index, held) in boxes.iter().enumerate() {
        for at in &order[held.start..held.end] {
            spots[*at as usize].owner = index as u32;
        }
    }

    refine(&mut spots, &mut centers, rounds);

    let colors: Vec<[u8; 3]> = centers.iter().map(|center| center.map(|value| value.round().clamp(0.0, 255.0) as u8)).collect();

    if rounds == 0 {
        for slot in 0..tally.keys.len() {
            if tally.keys[slot] != 0 {
                tally.counts[slot] = spots[homes[slot] as usize].owner;
            }
        }

        return Palette { colors, tally };
    }

    let rivals = Rivals::gather(tally.keys.iter().filter(|key| **key != 0).map(|key| cell(color(*key))), &colors);

    for slot in 0..tally.keys.len() {
        if tally.keys[slot] != 0 {
            tally.counts[slot] = u32::from(rivals.pick(color(tally.keys[slot]), &colors));
        }
    }

    Palette { colors, tally }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_with_few_colors_keeps_every_one_exactly() {
        // two tones a single step apart are still two colors
        let colors = [[255, 0, 0], [254, 0, 0], [12, 34, 56], [250, 250, 250], [0, 0, 0]];
        let pixels: Vec<[u8; 3]> = (0..5_000).map(|n| colors[(n * 7 + n / 11) % colors.len()]).collect();

        for rounds in [0, 3] {
            let palette = palette(pixels.iter().copied(), 255, rounds, 0.0);

            assert_eq!(palette.colors.len(), colors.len());

            for color in colors {
                assert_eq!(palette.colors[usize::from(palette.index(color))], color, "{color:?} with {rounds} rounds");
            }
        }
    }

    #[test]
    fn busy_pictures_stay_within_the_palette_and_close_to_their_colors() {
        let pixels: Vec<[u8; 3]> = (0..=255u8).flat_map(|r| (0..=255u8).step_by(5).map(move |g| [r, g, r ^ g])).collect();

        for wanted in [1, 2, 17, 255, 256] {
            let palette = palette(pixels.iter().copied(), wanted, 1, 0.0);

            assert!(palette.colors.len() <= wanted);
            assert!(pixels.iter().all(|pixel| usize::from(palette.index(*pixel)) < palette.colors.len()));
        }

        // a heavily used flat tone must survive among thousands of gradient colors
        let flat = [40u8, 41, 42];
        let mixed: Vec<[u8; 3]> = pixels.iter().copied().chain(std::iter::repeat_n(flat, 20_000)).collect();
        let palette = palette(mixed.iter().copied(), 255, 2, 0.0);

        assert_eq!(palette.colors[usize::from(palette.index(flat))], flat);
    }
}
