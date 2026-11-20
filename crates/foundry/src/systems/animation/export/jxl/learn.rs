use super::{
    lossless::Area,
    modular::{HYBRID, Stream, Tally, Tree, pack, spend, table},
    weighted::{Near, Weighted},
};

const SLOTS: [u32; 16] = [4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19];
const OWN: usize = 12;
const ERROR_SLOT: usize = 11;
const WEIGHTED: u8 = 6;
const CUTS: [i32; 31] = [-255, -127, -63, -31, -15, -7, -3, -2, -1, 0, 1, 2, 3, 4, 6, 8, 12, 16, 24, 32, 48, 64, 96, 128, 160, 192, 224, 240, 248, 252, 254];
const BINS: usize = CUTS.len() + 1;
const FLOOR: i32 = -256;
const TABLE: usize = 512;
const TOKENS: usize = 32;
const CHOICES: [u8; 6] = [5, 1, 2, 4, 3, 6];
const PLAIN: usize = 5;
const WEIGHTED_EFFORT: usize = 20;
const STEADY: usize = 4;
const SPLIT_BITS: f64 = 56.0;
const SMALLEST: usize = 32;

const BIN: [u8; TABLE] = bin_table();

const fn bin_table() -> [u8; TABLE] {
    let mut table = [0u8; TABLE];
    let mut at = 0;
    while at < TABLE {
        let value = at as i32 + FLOOR;
        let mut count = 0;
        while count < CUTS.len() && CUTS[count] < value {
            count += 1;
        }
        table[at] = count as u8;
        at += 1;
    }
    table
}

#[inline]
fn bin(value: i32) -> u8 {
    BIN[(value - FLOOR).clamp(0, TABLE as i32 - 1) as usize]
}

#[inline]
fn token(value: u32) -> u8 {
    (HYBRID.token(value).0 as usize).min(TOKENS - 1) as u8
}

#[inline]
pub(super) fn gradient(west: i32, north: i32, corner: i32) -> i32 {
    (west + north - corner).clamp(west.min(north), west.max(north))
}

#[derive(Default)]
pub(super) struct Row {
    value: Vec<i32>,
    west: Vec<i32>,
    north: Vec<i32>,
    corner: Vec<i32>,
    east: Vec<i32>,
    far_west: Vec<i32>,
    far_north: Vec<i32>,
    link: Vec<i32>,
    link_guess: Vec<i32>,
    weighted_guess: Vec<i32>,
    weighted_error: Vec<i32>,
}

impl Row {
    fn load(&mut self, plane: &[i16], stride: usize, area: &Area, y: usize) {
        let width = area.width;
        let line = |row: usize| &plane[(area.top + row) * stride + area.left..(area.top + row) * stride + area.left + width];
        let here = line(y);
        for list in [&mut self.value, &mut self.west, &mut self.north, &mut self.corner, &mut self.east, &mut self.far_west, &mut self.far_north] {
            list.clear();
        }
        self.value.extend(here.iter().map(|value| i32::from(*value)));
        if y == 0 {
            for x in 0..width {
                let west = if x > 0 { self.value[x - 1] } else { 0 };
                self.west.push(west);
                self.north.push(west);
                self.corner.push(west);
                self.east.push(west);
                self.far_west.push(if x > 1 { self.value[x - 2] } else { west });
                self.far_north.push(west);
            }
            return;
        }
        let above = line(y - 1);
        let top = above.iter().map(|value| i32::from(*value));
        self.north.extend(top);
        for x in 0..width {
            let west = if x > 0 { self.value[x - 1] } else { self.north[x] };
            self.west.push(west);
            self.corner.push(if x > 0 { self.north[x - 1] } else { west });
            self.east.push(if x + 1 < width { self.north[x + 1] } else { self.north[x] });
            self.far_west.push(if x > 1 { self.value[x - 2] } else { west });
        }
        if y > 1 {
            self.far_north.extend(line(y - 2).iter().map(|value| i32::from(*value)));
        } else {
            self.far_north.extend_from_slice(&self.north);
        }
    }

    fn load_link(&mut self, plane: &[i16], stride: usize, area: &Area, y: usize) {
        let width = area.width;
        let start = (area.top + y) * stride + area.left;
        let here = &plane[start..start + width];
        self.link.clear();
        self.link_guess.clear();
        self.link.extend(here.iter().map(|value| i32::from(*value)));
        for x in 0..width {
            let west = if x > 0 { self.link[x - 1] } else { 0 };
            let (north, corner) = if y > 0 {
                let above = &plane[start - stride..start - stride + width];
                (i32::from(above[x]), if x > 0 { i32::from(above[x - 1]) } else { west })
            } else {
                (west, west)
            };
            self.link_guess.push(gradient(west, north, corner));
        }
    }

    fn weigh(&mut self, state: &mut Weighted, y: usize, gain: i32) {
        self.weighted_guess.clear();
        self.weighted_error.clear();
        let gain = i64::from(gain);
        for x in 0..self.value.len() {
            let near = Near { north: i64::from(self.north[x]) * gain, west: i64::from(self.west[x]) * gain, east: i64::from(self.east[x]) * gain };
            let (guess, error) = state.predict(x, y, &near);
            self.weighted_guess.push(guess.clamp(-(1 << 30), 1 << 30) as i32);
            self.weighted_error.push(error.clamp(-(1 << 30), 1 << 30) as i32);
            state.update(i64::from(self.value[x]) * gain, x, y);
        }
    }

    fn property(&self, slot: usize, gain: i32, out: &mut Vec<i32>) {
        out.clear();
        let width = self.value.len();
        match slot {
            0 => out.extend(self.north.iter().map(|value| value.abs() * gain)),
            1 => out.extend(self.west.iter().map(|value| value.abs() * gain)),
            2 => out.extend(self.north.iter().map(|value| value * gain)),
            3 => out.extend(self.west.iter().map(|value| value * gain)),
            4 => {
                let mut before = 0;
                for x in 0..width {
                    out.push((self.west[x] - before) * gain);
                    before = self.west[x] + self.north[x] - self.corner[x];
                }
            }
            5 => out.extend((0..width).map(|x| (self.west[x] + self.north[x] - self.corner[x]) * gain)),
            6 => out.extend((0..width).map(|x| (self.west[x] - self.corner[x]) * gain)),
            7 => out.extend((0..width).map(|x| (self.corner[x] - self.north[x]) * gain)),
            8 => out.extend((0..width).map(|x| (self.north[x] - self.east[x]) * gain)),
            9 => out.extend((0..width).map(|x| (self.north[x] - self.far_north[x]) * gain)),
            10 => out.extend((0..width).map(|x| (self.west[x] - self.far_west[x]) * gain)),
            11 => out.extend_from_slice(&self.weighted_error),
            12 => out.extend(self.link.iter().map(|value| value.abs())),
            13 => out.extend_from_slice(&self.link),
            14 => out.extend(self.link.iter().zip(&self.link_guess).map(|(value, guess)| (value - guess).abs())),
            _ => out.extend(self.link.iter().zip(&self.link_guess).map(|(value, guess)| value - guess)),
        }
    }

    #[inline]
    fn predict(&self, predictor: u8, x: usize) -> i32 {
        let (west, north) = (self.west[x], self.north[x]);
        match predictor {
            1 => west,
            2 => north,
            3 => (west + north) / 2,
            4 => {
                let guess = west + north - self.corner[x];
                if (guess - west).abs() < (guess - north).abs() { west } else { north }
            }
            WEIGHTED => self.weighted_guess[x],
            _ => gradient(west, north, self.corner[x]),
        }
    }
}

const LEAF: u8 = u8::MAX;

#[derive(Clone, Copy)]
struct Node {
    rank: u8,
    cut: i32,
    above: u32,
    below: u32,
}

impl Node {
    const fn leaf(index: usize, predictor: u8) -> Self {
        Self { rank: LEAF, cut: 0, above: index as u32, below: predictor as u32 }
    }
}

pub(super) struct Learned {
    nodes: Vec<Node>,
    used: Vec<usize>,
    pub(super) leaves: usize,
    linked: bool,
    weighted: bool,
    quiet: (u32, u8),
}

pub(super) struct Budget {
    pub(super) samples: usize,
    pub(super) leaves: usize,
    weighted: bool,
}

impl Budget {
    pub(super) fn new(effort: usize) -> Self {
        Self { samples: 8192 + effort * 1024, leaves: 2 + effort * 2, weighted: effort >= WEIGHTED_EFFORT }
    }
}

struct Samples {
    bins: Vec<[u8; SLOTS.len()]>,
    tokens: Vec<[u8; CHOICES.len()]>,
    columns: Vec<Vec<u8>>,
    first: Vec<u8>,
}

struct Open {
    node: usize,
    members: Vec<u32>,
    grid: Vec<u32>,
    split: Option<(f64, usize, usize)>,
}

pub(super) struct Plane<'a> {
    pub(super) data: &'a [i16],
    pub(super) link: Option<&'a [i16]>,
    pub(super) stride: usize,
    pub(super) gain: i32,
}

pub(super) struct Learner {
    state: Weighted,
    row: Row,
    props: Vec<i32>,
    lists: Vec<Vec<i32>>,
}

impl Learner {
    pub(super) fn new() -> Self {
        Self { state: Weighted::new(), row: Row::default(), props: Vec::new(), lists: vec![Vec::new(); SLOTS.len()] }
    }

    pub(super) fn learn(&mut self, plane: &Plane<'_>, area: &Area, budget: &Budget) -> Learned {
        let Plane { data, link, stride, gain } = *plane;
        let flat = Learned { nodes: vec![Node::leaf(0, CHOICES[0])], used: Vec::new(), leaves: 1, linked: false, weighted: false, quiet: (0, CHOICES[0]) };
        let total = area.width * area.height;
        if budget.leaves <= 1 || total < SMALLEST * 4 {
            return flat;
        }
        let wanted = budget.samples.div_ceil(area.width).max(1);
        let step = area.height.div_ceil(wanted).max(1);
        let skip = gain > 1 || !budget.weighted;
        let slots = if link.is_some() { SLOTS.len() } else if skip { ERROR_SLOT } else { OWN };
        let choices = match (gain > 1, skip) {
            (true, _) => STEADY,
            (false, true) => PLAIN,
            (false, false) => CHOICES.len(),
        };
        if skip {
            self.row.weighted_error.clear();
            self.row.weighted_error.resize(area.width, 0);
        }
        let mut samples = Samples { bins: Vec::with_capacity(budget.samples + area.width), tokens: Vec::with_capacity(budget.samples + area.width), columns: Vec::new(), first: Vec::new() };
        let mut lists = std::mem::take(&mut self.lists);
        self.state.reset(area.width);
        for y in 0..area.height {
            self.row.load(data, stride, area, y);
            if !skip {
                self.row.weigh(&mut self.state, y, gain);
            }
            if y % step != 0 {
                continue;
            }
            if let Some(link) = link {
                self.row.load_link(link, stride, area, y);
            }
            for (slot, list) in lists.iter_mut().enumerate().take(slots) {
                self.row.property(slot, gain, list);
            }
            for x in 0..area.width {
                let mut bins = [0u8; SLOTS.len()];
                for (slot, list) in lists.iter().enumerate().take(slots) {
                    bins[slot] = bin(list[x]);
                }
                let mut tokens = [0u8; CHOICES.len()];
                for (choice, predictor) in CHOICES.iter().enumerate().take(choices) {
                    tokens[choice] = token(pack(self.row.value[x] - self.row.predict(*predictor, x)));
                }
                samples.bins.push(bins);
                samples.tokens.push(tokens);
            }
        }
        self.lists = lists;
        samples.columns = (0..slots).map(|slot| samples.bins.iter().map(|bins| bins[slot]).collect()).collect();
        let mut totals = [[0u32; TOKENS]; CHOICES.len()];
        for tokens in &samples.tokens {
            for (choice, counted) in totals.iter_mut().enumerate().take(choices) {
                counted[usize::from(tokens[choice])] += 1;
            }
        }
        let lead = (0..choices).min_by(|a, b| spent(&totals[*a]).total_cmp(&spent(&totals[*b]))).unwrap_or(0);
        samples.first = samples.tokens.iter().map(|tokens| tokens[lead]).collect();
        let scale = total as f64 / samples.bins.len().max(1) as f64;
        let mut nodes = vec![Node::leaf(0, CHOICES[0])];
        let members: Vec<u32> = (0..samples.bins.len() as u32).collect();
        let grid = fill(&samples, &members, slots);
        let split = best(&grid, slots);
        let mut open = vec![Open { node: 0, members, grid, split }];
        let mut leaves = 1;
        while leaves < budget.leaves {
            let Some(pick) = open.iter().enumerate().filter_map(|(at, leaf)| leaf.split.map(|split| (at, split.0))).max_by(|a, b| a.1.total_cmp(&b.1)).map(|(at, _)| at) else { break };
            let Some((gain_bits, slot, cut)) = open[pick].split else { break };
            if gain_bits * scale < SPLIT_BITS {
                break;
            }
            let leaf = open.swap_remove(pick);
            let (above, below): (Vec<u32>, Vec<u32>) = leaf.members.iter().partition(|member| usize::from(samples.columns[slot][**member as usize]) > cut);
            let (high, low) = (nodes.len(), nodes.len() + 1);
            nodes.push(Node::leaf(0, CHOICES[0]));
            nodes.push(Node::leaf(0, CHOICES[0]));
            nodes[leaf.node] = Node { rank: slot as u8, cut: CUTS[cut], above: high as u32, below: low as u32 };
            leaves += 1;
            let (small, large) = if above.len() <= below.len() { ((high, above), (low, below)) } else { ((low, below), (high, above)) };
            let small_grid = fill(&samples, &small.1, slots);
            let mut large_grid = leaf.grid;
            for (held, taken) in large_grid.iter_mut().zip(&small_grid) {
                *held -= taken;
            }
            for ((node, members), grid) in [(small, small_grid), (large, large_grid)] {
                let split = if members.len() >= SMALLEST * 4 { best(&grid, slots) } else { None };
                let grid = if split.is_some() { grid } else { Vec::new() };
                open.push(Open { node, members, grid, split });
            }
        }
        open.sort_unstable_by_key(|leaf| leaf.node);
        for (index, leaf) in open.iter().enumerate() {
            let mut counts = [[0u32; TOKENS]; CHOICES.len()];
            for member in &leaf.members {
                let tokens = &samples.tokens[*member as usize];
                for (choice, counted) in counts.iter_mut().enumerate().take(choices) {
                    counted[usize::from(tokens[choice])] += 1;
                }
            }
            let predictor = (0..choices).min_by(|a, b| spent(&counts[*a]).total_cmp(&spent(&counts[*b]))).map_or(CHOICES[0], |choice| CHOICES[choice]);
            nodes[leaf.node] = Node::leaf(index, predictor);
        }
        let mut used: Vec<usize> = Vec::new();
        for node in &mut nodes {
            if node.rank != LEAF {
                let slot = usize::from(node.rank);
                let rank = used.iter().position(|kept| *kept == slot).unwrap_or_else(|| {
                    used.push(slot);
                    used.len() - 1
                });
                node.rank = rank as u8;
            }
        }
        let linked = used.iter().any(|slot| *slot >= OWN);
        let weighted = used.contains(&ERROR_SLOT) || nodes.iter().any(|node| node.rank == LEAF && node.below == u32::from(WEIGHTED));
        let mut learned = Learned { nodes, used, leaves: open.len(), linked, weighted, quiet: (0, CHOICES[0]) };
        learned.quiet = learned.find(|_| 0);
        learned
    }
}

fn spent(counts: &[u32]) -> f64 {
    let extra: f64 = counts.iter().enumerate().map(|(symbol, count)| f64::from(*count) * f64::from(HYBRID.extra(symbol as u32))).sum();
    spend(counts.iter().sum()) - counts.iter().map(|count| spend(*count)).sum::<f64>() + extra
}

fn fill(samples: &Samples, members: &[u32], slots: usize) -> Vec<u32> {
    let span = BINS * TOKENS;
    let mut grid = vec![0u32; slots * span + slots * BINS];
    let symbols: Vec<u8> = members.iter().map(|member| samples.first[*member as usize]).collect();
    let (cells, totals) = grid.split_at_mut(slots * span);
    for (slot, (block, sums)) in cells.chunks_exact_mut(span).zip(totals.as_chunks_mut::<BINS>().0).enumerate() {
        let column = &samples.columns[slot];
        for (member, symbol) in members.iter().zip(&symbols) {
            block[usize::from(column[*member as usize]) * TOKENS + usize::from(*symbol)] += 1;
        }
        for (sum, cell) in sums.iter_mut().zip(block.as_chunks::<TOKENS>().0) {
            *sum = cell.iter().sum();
        }
    }
    grid
}

fn best(grid: &[u32], slots: usize) -> Option<(f64, usize, usize)> {
    let span = BINS * TOKENS;
    let table = table();
    let spend = |count: u32| table.get(count as usize).copied().unwrap_or_else(|| f64::from(count) * f64::from(count).log2());
    let totals = &grid[slots * span..];
    let mut parent = [0u32; TOKENS];
    for cell in grid[..span].as_chunks::<TOKENS>().0 {
        for (sum, count) in parent.iter_mut().zip(cell) {
            *sum += count;
        }
    }
    let symbols: Vec<usize> = (0..TOKENS).filter(|symbol| parent[*symbol] != 0).collect();
    let parent_spend: f64 = symbols.iter().map(|symbol| spend(parent[*symbol])).sum();
    let count: u32 = parent.iter().sum();
    let whole = spend(count) - parent_spend;
    let mut found: Option<(f64, usize, usize)> = None;
    for slot in 0..slots {
        let mut left = [0u32; TOKENS];
        let (mut left_spend, mut right_spend) = (0f64, parent_spend);
        let mut taken = 0u32;
        for cut in 0..BINS - 1 {
            if totals[slot * BINS + cut] == 0 {
                continue;
            }
            let base = slot * span + cut * TOKENS;
            let mut added = 0u32;
            for symbol in &symbols {
                let moved = grid[base + symbol];
                if moved != 0 {
                    let (held, rest) = (left[*symbol], parent[*symbol] - left[*symbol]);
                    left_spend += spend(held + moved) - spend(held);
                    right_spend += spend(rest - moved) - spend(rest);
                    left[*symbol] = held + moved;
                    added += moved;
                }
            }
            if added == 0 {
                continue;
            }
            taken += added;
            if (taken as usize) < SMALLEST || ((count - taken) as usize) < SMALLEST {
                continue;
            }
            let gain = whole - (spend(taken) - left_spend) - (spend(count - taken) - right_spend);
            if found.is_none_or(|kept| gain > kept.0) {
                found = Some((gain, slot, cut));
            }
        }
    }
    found
}

impl Learned {
    fn find(&self, value: impl Fn(usize) -> i32) -> (u32, u8) {
        let mut at = 0;
        loop {
            let node = self.nodes[at];
            if node.rank == LEAF {
                return (node.above, node.below as u8);
            }
            at = if value(usize::from(node.rank)) > node.cut { node.above } else { node.below } as usize;
        }
    }

    pub(super) fn grow(&self, tree: &mut Tree, base: usize, multiplier: u32) -> usize {
        self.place(tree, 0, base, multiplier)
    }

    fn place(&self, tree: &mut Tree, at: usize, base: usize, multiplier: u32) -> usize {
        let node = self.nodes[at];
        if node.rank == LEAF {
            return tree.leaf(base + node.above as usize, node.below as u8, multiplier);
        }
        let high = self.place(tree, node.above as usize, base, multiplier);
        let low = self.place(tree, node.below as usize, base, multiplier);
        tree.split(SLOTS[self.used[usize::from(node.rank)]], node.cut, high, low)
    }

    pub(super) fn code(&self, coder: &mut Learner, plane: &Plane<'_>, area: &Area, base: usize, stream: &mut Stream, tally: &mut Tally) {
        let Plane { data, link, stride, gain } = *plane;
        let width = area.width;
        let mut lists = std::mem::take(&mut coder.lists);
        let props = &mut coder.props;
        let row = &mut coder.row;
        let quiet_cell = base + self.quiet.0 as usize;
        let state = &mut coder.state;
        state.reset(area.width);
        for y in 0..area.height {
            row.load(data, stride, area, y);
            if self.weighted {
                row.weigh(state, y, gain);
            }
            if self.linked
                && let Some(link) = link
            {
                row.load_link(link, stride, area, y);
            }
            props.clear();
            props.resize(self.used.len() * width + width, 0);
            let (values, busy) = props.split_at_mut(self.used.len() * width);
            for (rank, slot) in self.used.iter().enumerate() {
                row.property(*slot, gain, &mut lists[0]);
                values[rank * width..(rank + 1) * width].copy_from_slice(&lists[0]);
                for (flag, value) in busy.iter_mut().zip(&lists[0]) {
                    *flag |= value;
                }
            }
            for x in 0..width {
                let (index, predictor) = if busy[x] == 0 { self.quiet } else { self.find(|rank| values[rank * width + x]) };
                let cell = if busy[x] == 0 { quiet_cell } else { base + index as usize };
                stream.push(tally, cell, pack(row.value[x] - row.predict(predictor, x)));
            }
        }
        coder.lists = lists;
    }
}
