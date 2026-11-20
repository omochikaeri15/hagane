use super::{
    bits::BitWriter,
    entropy::{Coder, Hybrid, RUN_FLOOR, RUN_SYMBOL, SYMBOLS, run_token, store_tokens},
};

pub(super) const GRADIENT: u8 = 5;
pub(super) const LITERALS: usize = 96;
pub(super) const WIDTH: usize = LITERALS + SYMBOLS - RUN_SYMBOL;
pub(super) const HYBRID: Hybrid = Hybrid::new(4, 2, 0);
const RUN_FLAG: u32 = 1 << 31;
const CELL_SHIFT: u32 = 20;
const CELL_MASK: u32 = (1 << 11) - 1;
const VALUE_MASK: u32 = (1 << CELL_SHIFT) - 1;
const RUN_LIMIT: u32 = (1 << 20) - 1;
const NEVER: u32 = u32::MAX;

pub(super) enum Node {
    Split { property: u32, value: i32, above: usize, below: usize },
    Leaf { cell: u16, predictor: u8, multiplier: u32 },
}

pub(super) struct Tree {
    nodes: Vec<Node>,
}

impl Tree {
    pub(super) fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    pub(super) fn leaf(&mut self, cell: usize, predictor: u8, multiplier: u32) -> usize {
        self.nodes.push(Node::Leaf { cell: cell as u16, predictor, multiplier });
        self.nodes.len() - 1
    }

    pub(super) fn split(&mut self, property: u32, value: i32, above: usize, below: usize) -> usize {
        self.nodes.push(Node::Split { property, value, above, below });
        self.nodes.len() - 1
    }

    pub(super) fn ladder(&mut self, property: u32, cuts: &[i32], rungs: &[usize]) -> usize {
        if cuts.is_empty() {
            return rungs.first().copied().unwrap_or(0);
        }
        let middle = cuts.len() / 2;
        let below = self.ladder(property, &cuts[..middle], &rungs[..=middle]);
        let above = self.ladder(property, &cuts[middle + 1..], &rungs[middle + 1..]);
        self.split(property, cuts[middle], above, below)
    }

    pub(super) fn store(&self, writer: &mut BitWriter, root: usize) -> Vec<u16> {
        let mut tokens: Vec<(u8, u32)> = Vec::with_capacity(self.nodes.len() * 3);
        let mut cells = Vec::with_capacity(self.nodes.len() / 2 + 1);
        let mut queue = std::collections::VecDeque::with_capacity(self.nodes.len());
        queue.push_back(root);
        while let Some(at) = queue.pop_front() {
            match &self.nodes[at] {
                Node::Split { property, value, above, below } => {
                    tokens.push((1, property + 1));
                    tokens.push((0, pack(*value)));
                    queue.push_back(*above);
                    queue.push_back(*below);
                }
                Node::Leaf { cell, predictor, multiplier } => {
                    let shift = multiplier.trailing_zeros();
                    tokens.push((1, 0));
                    tokens.push((2, u32::from(*predictor)));
                    tokens.push((3, 0));
                    tokens.push((4, shift));
                    tokens.push((5, (multiplier >> shift) - 1));
                    cells.push(*cell);
                }
            }
        }
        store_tokens(writer, 6, &tokens);
        cells
    }
}

#[inline]
pub(super) fn pack(value: i32) -> u32 {
    ((value << 1) ^ (value >> 31)) as u32
}

pub(super) struct Tally {
    pub(super) cells: Vec<[u32; WIDTH]>,
    literal: Vec<u8>,
}

impl Tally {
    pub(super) fn new(cells: usize) -> Self {
        let literal = (0..1024u32).map(|value| HYBRID.token(value).0 as u8).collect();
        Self { cells: vec![[0; WIDTH]; cells], literal }
    }

    #[inline]
    fn symbol(&self, value: u32) -> usize {
        self.literal.get(value as usize).map_or_else(|| HYBRID.token(value).0 as usize, |token| usize::from(*token))
    }
}

pub(super) struct Stream {
    tokens: Vec<u32>,
    last: u32,
    run: u32,
    held: [u16; RUN_FLOOR as usize],
}

impl Stream {
    pub(super) fn new(capacity: usize) -> Self {
        Self { tokens: Vec::with_capacity(capacity), last: NEVER, run: 0, held: [0; RUN_FLOOR as usize] }
    }

    #[inline]
    pub(super) fn push(&mut self, tally: &mut Tally, cell: usize, value: u32) {
        if value == self.last {
            if self.run < RUN_FLOOR {
                self.held[self.run as usize] = cell as u16;
            }
            self.run += 1;
            if self.run == RUN_LIMIT {
                self.flush(tally);
            }
        } else {
            if self.run != 0 {
                self.flush(tally);
            }
            self.literal(tally, cell, value);
            self.last = value;
        }
    }

    #[inline]
    pub(super) fn repeat(&mut self, tally: &mut Tally, cell: usize, count: u32) {
        let mut left = count;
        while left != 0 {
            let room = RUN_LIMIT - 1 - self.run;
            let step = left.min(room);
            for slot in self.run..(self.run + step).min(RUN_FLOOR) {
                self.held[slot as usize] = cell as u16;
            }
            self.run += step;
            left -= step;
            if self.run >= RUN_LIMIT - 1 {
                self.flush(tally);
            }
        }
    }

    #[inline]
    pub(super) fn last(&self) -> u32 {
        self.last
    }

    #[inline]
    fn literal(&mut self, tally: &mut Tally, cell: usize, value: u32) {
        let symbol = tally.symbol(value);
        tally.cells[cell][symbol] += 1;
        self.tokens.push((cell as u32) << CELL_SHIFT | (value & VALUE_MASK));
    }

    pub(super) fn flush(&mut self, tally: &mut Tally) {
        if self.run >= RUN_FLOOR {
            let cell = usize::from(self.held[0]);
            let extra = self.run - RUN_FLOOR;
            tally.cells[cell][LITERALS + run_token(extra) - RUN_SYMBOL] += 1;
            self.tokens.push(RUN_FLAG | (cell as u32) << CELL_SHIFT | extra);
        } else {
            for slot in 0..self.run as usize {
                self.literal(tally, usize::from(self.held[slot]), self.last);
            }
        }
        self.run = 0;
    }

    pub(super) fn store(&self, writer: &mut BitWriter, coder: &Coder, clusters: &[u8]) {
        coder.emit(writer, self.tokens.iter().map(|token| (usize::from(clusters[((token >> CELL_SHIFT) & CELL_MASK) as usize]), token & RUN_FLAG != 0, token & VALUE_MASK)));
    }
}

pub(super) struct Model {
    pub(super) coder: Coder,
    pub(super) clusters: Vec<u8>,
}

impl Model {
    pub(super) fn fit(tally: &Tally, effort: usize) -> Self {
        let limit = (16 + effort * 2).clamp(1, 255);
        let rows: Vec<&[u32]> = tally.cells.iter().map(|counts| &counts[..]).collect();
        let (tables, clusters) = cluster(&rows, limit);
        let merged: Vec<Vec<u32>> = tables.iter().map(|table| {
            let mut full = vec![0u32; SYMBOLS];
            full[..LITERALS].copy_from_slice(&table[..LITERALS]);
            full[RUN_SYMBOL..].copy_from_slice(&table[LITERALS..]);
            full
        }).collect();
        Self { coder: Coder::new(HYBRID, &merged, true, true), clusters }
    }

    pub(super) fn store(&self, writer: &mut BitWriter, leaves: &[u16]) {
        let map: Vec<u8> = leaves.iter().map(|cell| self.clusters[usize::from(*cell)]).collect();
        self.coder.store(writer, &map);
    }
}

const SPENT: usize = 1 << 17;

static SPENT_TABLE: std::sync::OnceLock<Vec<f64>> = std::sync::OnceLock::new();

pub(super) fn table() -> &'static [f64] {
    SPENT_TABLE.get_or_init(|| (0..SPENT).map(|count| if count < 2 { 0.0 } else { count as f64 * (count as f64).log2() }).collect())
}

#[inline]
pub(super) fn spend(count: u32) -> f64 {
    table().get(count as usize).copied().unwrap_or_else(|| f64::from(count) * f64::from(count).log2())
}

pub(super) fn cluster(rows: &[&[u32]], limit: usize) -> (Vec<Vec<u32>>, Vec<u8>) {
    let width = rows.first().map_or(1, |row| row.len());
    let mut order: Vec<(u64, usize)> = rows.iter().enumerate().map(|(at, row)| (row.iter().map(|count| u64::from(*count)).sum::<u64>(), at)).filter(|entry| entry.0 > 0).collect();
    order.sort_unstable_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut tables: Vec<Vec<u32>> = Vec::new();
    let mut totals: Vec<u64> = Vec::new();
    let mut owner = vec![0u8; rows.len()];
    let sparse: Vec<Vec<(usize, u32)>> = rows.iter().map(|row| row.iter().enumerate().filter(|(_, count)| **count != 0).map(|(symbol, count)| (symbol, *count)).collect()).collect();
    let added = |table: &[u32], total: u64, row: &[(usize, u32)], sum: u64| -> f64 {
        let mut bits = spend((total + sum) as u32) - spend(total as u32);
        for (symbol, count) in row {
            let held = table[*symbol];
            bits -= spend(held + count) - spend(held);
            if held == 0 {
                bits += 5.0;
            }
        }
        bits
    };
    let alone = |row: &[u32], sum: u64| -> f64 {
        let mut bits = spend(sum as u32) + 30.0;
        for count in row {
            if *count != 0 {
                bits += 5.0 - spend(*count);
            }
        }
        bits
    };
    for (sum, at) in &order {
        let row = rows[*at];
        let mut best = if tables.len() < limit { (alone(row, *sum), tables.len()) } else { (f64::MAX, 0) };
        for (index, table) in tables.iter().enumerate() {
            let bits = added(table, totals[index], &sparse[*at], *sum);
            if bits < best.0 {
                best = (bits, index);
            }
        }
        if best.1 == tables.len() {
            tables.push(vec![0; width]);
            totals.push(0);
        }
        for (held, count) in tables[best.1].iter_mut().zip(row) {
            *held += count;
        }
        totals[best.1] += sum;
        owner[*at] = best.1 as u8;
    }
    for (sum, at) in &order {
        let row = rows[*at];
        let home = usize::from(owner[*at]);
        for (held, count) in tables[home].iter_mut().zip(row) {
            *held -= count;
        }
        totals[home] -= sum;
        let mut best = (added(&tables[home], totals[home], &sparse[*at], *sum), home);
        for (index, table) in tables.iter().enumerate() {
            if index != home && totals[index] > 0 {
                let bits = added(table, totals[index], &sparse[*at], *sum);
                if bits < best.0 {
                    best = (bits, index);
                }
            }
        }
        for (held, count) in tables[best.1].iter_mut().zip(row) {
            *held += count;
        }
        totals[best.1] += sum;
        owner[*at] = best.1 as u8;
    }
    let mut renumber = vec![u8::MAX; tables.len()];
    let mut kept = Vec::with_capacity(tables.len());
    for (index, table) in tables.into_iter().enumerate() {
        if totals[index] > 0 {
            renumber[index] = kept.len() as u8;
            kept.push(table);
        }
    }
    if kept.is_empty() {
        kept.push(vec![0; width]);
    }
    for slot in &mut owner {
        *slot = renumber.get(usize::from(*slot)).copied().filter(|value| *value != u8::MAX).unwrap_or(0);
    }
    (kept, owner)
}

fn price(counts: &[u32]) -> f32 {
    let (mut total, mut used) = (0u32, 0u32);
    for count in counts {
        total += count;
        used += u32::from(*count != 0);
    }
    let whole = total as f32;
    let spent: f32 = counts.iter().filter(|count| **count != 0).map(|count| *count as f32 * (whole / *count as f32).log2()).sum();
    spent + 30.0 + 5.0 * used as f32
}

fn joint(first: &[u32], second: &[u32]) -> f32 {
    let both: Vec<u32> = first.iter().zip(second).map(|(a, b)| a + b).collect();
    price(&both)
}

pub(super) fn condense(tables: &mut Vec<Vec<u32>>) -> Vec<u8> {
    let mut owners: Vec<usize> = (0..tables.len()).collect();
    let mut prices: Vec<f32> = tables.iter().map(|counts| price(counts)).collect();
    let mut gains: Vec<f32> = (1..tables.len()).map(|at| joint(&tables[at - 1], &tables[at]) - prices[at - 1] - prices[at]).collect();
    while let Some((at, gain)) = gains.iter().copied().enumerate().min_by(|a, b| a.1.total_cmp(&b.1)) {
        if gain >= 0.0 {
            break;
        }
        let absorbed = tables.remove(at + 1);
        for (sum, count) in tables[at].iter_mut().zip(&absorbed) {
            *sum += count;
        }
        prices.remove(at + 1);
        prices[at] = price(&tables[at]);
        gains.remove(at);
        for owner in &mut owners {
            *owner -= usize::from(*owner > at);
        }
        if at > 0 {
            gains[at - 1] = joint(&tables[at - 1], &tables[at]) - prices[at - 1] - prices[at];
        }
        if at < gains.len() {
            gains[at] = joint(&tables[at], &tables[at + 1]) - prices[at] - prices[at + 1];
        }
    }
    owners.iter().map(|owner| *owner as u8).collect()
}

pub(super) fn store_header(writer: &mut BitWriter, transform: Option<u32>) {
    writer.flag(true);
    writer.flag(true);
    match transform {
        Some(kind) => {
            writer.write(2, 1);
            writer.write(2, 0);
            writer.select(0, [(3, 0), (6, 8), (10, 72), (13, 1096)]);
            writer.select(kind, [(0, 6), (2, 0), (4, 2), (6, 10)]);
        }
        None => writer.write(2, 0),
    }
}

#[cfg(test)]
mod tests {
    use super::{BitWriter, CELL_MASK, CELL_SHIFT, RUN_FLAG, RUN_FLOOR, Stream, Tally, Tree, VALUE_MASK, condense, pack};

    // Expands the tokens the way the decoder does: a run repeats the value before it.
    fn expand(stream: &Stream) -> Vec<u32> {
        let mut out: Vec<u32> = Vec::new();
        for token in &stream.tokens {
            if token & RUN_FLAG == 0 {
                out.push(token & VALUE_MASK);
            } else {
                let last = *out.last().unwrap();
                out.extend(std::iter::repeat_n(last, ((token & VALUE_MASK) + RUN_FLOOR) as usize));
            }
        }
        out
    }

    #[test]
    fn runs_and_literals_expand_to_what_went_in() {
        let mut values = vec![0u32; 5];
        values.extend([7, 7, 7, 3]);
        values.extend(std::iter::repeat_n(0, 40));
        values.extend([9; 9]);
        values.push(1);
        let mut tally = Tally::new(4);
        let mut stream = Stream::new(16);
        for (index, value) in values.iter().enumerate() {
            stream.push(&mut tally, index % 4, *value);
        }
        stream.repeat(&mut tally, 0, 100);
        stream.flush(&mut tally);
        values.extend([1; 100]);
        assert_eq!(expand(&stream), values);
        assert!(stream.tokens.len() < 30);
        let counted: u32 = tally.cells.iter().flatten().sum();
        assert_eq!(counted as usize, stream.tokens.len());
        assert!(stream.tokens.iter().all(|token| (token >> CELL_SHIFT) & CELL_MASK < 4));
    }

    #[test]
    fn signed_values_pack_like_the_format() {
        assert_eq!([0, -1, 1, -2, 2, -255, 255].map(pack), [0, 1, 2, 3, 4, 509, 510]);
    }

    #[test]
    fn ladder_keeps_every_rung_as_a_leaf() {
        let mut tree = Tree::new();
        let rungs: Vec<usize> = (0..5).map(|cell| tree.leaf(cell, 5, 1)).collect();
        let root = tree.ladder(10, &[-3, -1, 0, 2], &rungs);
        let mut writer = BitWriter::new();
        let mut leaves = tree.store(&mut writer, root);
        leaves.sort_unstable();
        assert_eq!(leaves, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn alike_tables_merge_and_unlike_ones_stay() {
        let mut flat = vec![0u32; 32];
        flat[..8].fill(500);
        let mut sharp = vec![0u32; 32];
        sharp[0] = 4000;
        sharp[20] = 3;
        let mut tables = vec![flat.clone(), flat, sharp];
        let moved = condense(&mut tables);
        assert_eq!(moved, vec![0, 0, 1]);
        assert_eq!(tables.len(), 2);
        assert_eq!(tables[0][0], 1000);
    }
}
