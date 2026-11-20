use super::bits::BitWriter;

const LIMIT: u8 = 15;
const ORDER: [usize; 18] = [1, 2, 3, 4, 0, 5, 17, 6, 16, 7, 8, 9, 10, 11, 12, 13, 14, 15];
const LENGTH_BITS: [u64; 6] = [0, 7, 3, 2, 1, 15];
const LENGTH_SIZES: [u32; 6] = [2, 4, 3, 2, 2, 4];

pub(super) struct Prefix {
    pub(super) depths: Vec<u8>,
    pub(super) codes: Vec<u16>,
    used: usize,
}

impl Prefix {
    pub(super) fn build(counts: &[u32]) -> Self {
        let size = counts.iter().rposition(|count| *count != 0).map_or(1, |last| last + 1);
        let counts = &counts[..size.min(counts.len())];
        let used = counts.iter().filter(|count| **count != 0).count();
        let depths = if used > 1 { limited(counts, LIMIT) } else { vec![0; size] };
        let codes = canonical(&depths);
        Self { depths, codes, used }
    }

    pub(super) fn size(&self) -> usize {
        self.depths.len()
    }

    pub(super) fn store(&self, writer: &mut BitWriter) {
        let size = self.depths.len();
        if size <= 1 {
            return;
        }
        let width = (size - 1).ilog2() + 1;
        if self.used <= 1 {
            writer.write(4, 1);
            writer.write(width, (size - 1) as u64);
        } else if self.used <= 4 {
            let mut symbols: Vec<usize> = (0..size).filter(|symbol| self.depths[*symbol] != 0).collect();
            symbols.sort_by_key(|symbol| self.depths[*symbol]);
            writer.write(2, 1);
            writer.write(2, (symbols.len() - 1) as u64);
            for symbol in &symbols {
                writer.write(width, *symbol as u64);
            }
            if symbols.len() == 4 {
                writer.flag(self.depths[symbols[0]] == 1);
            }
        } else {
            self.store_lengths(writer);
        }
    }

    fn store_lengths(&self, writer: &mut BitWriter) {
        let (symbols, extras) = run_lengths(&self.depths);
        let mut counts = [0u32; 18];
        for symbol in &symbols {
            counts[usize::from(*symbol)] += 1;
        }
        let distinct = counts.iter().filter(|count| **count != 0).count();
        let mut depths = if distinct > 1 { limited(&counts, 5) } else { vec![0; 18] };
        depths.resize(18, 0);
        let codes = canonical(&depths);
        let mut stored = depths.clone();
        if distinct == 1 {
            for (depth, count) in stored.iter_mut().zip(&counts) {
                *depth = u8::from(*count != 0);
            }
        }
        let kept = if distinct > 1 { ORDER.iter().rposition(|at| stored[*at] != 0).map_or(0, |last| last + 1) } else { 18 };
        let skip = match (stored[ORDER[0]], stored[ORDER[1]], stored[ORDER[2]]) {
            (0, 0, 0) => 3,
            (0, 0, _) => 2,
            _ => 0,
        };
        writer.write(2, skip as u64);
        for at in &ORDER[skip..kept.max(skip)] {
            let depth = usize::from(stored[*at]);
            writer.write(LENGTH_SIZES[depth], LENGTH_BITS[depth]);
        }
        for (symbol, extra) in symbols.iter().zip(&extras) {
            let symbol = usize::from(*symbol);
            writer.write(u32::from(depths[symbol]), u64::from(codes[symbol]));
            match symbol {
                16 => writer.write(2, u64::from(*extra)),
                17 => writer.write(3, u64::from(*extra)),
                _ => {}
            }
        }
    }
}

fn limited(counts: &[u32], limit: u8) -> Vec<u8> {
    let mut floor = 1u32;
    loop {
        let depths = huffman(counts, floor);
        if depths.iter().all(|depth| *depth <= limit) {
            return depths;
        }
        floor = floor.saturating_mul(2);
    }
}

fn huffman(counts: &[u32], floor: u32) -> Vec<u8> {
    let mut leaves: Vec<(u64, usize)> = counts.iter().enumerate().filter(|(_, count)| **count != 0).map(|(symbol, count)| (u64::from((*count).max(floor)), symbol)).collect();
    let mut depths = vec![0u8; counts.len()];
    let total = leaves.len();
    if total < 2 {
        return depths;
    }
    leaves.sort_unstable();
    let mut weights: Vec<u64> = leaves.iter().map(|leaf| leaf.0).collect();
    let mut parents = vec![0usize; 2 * total - 1];
    let (mut leaf, mut inner) = (0usize, total);
    for made in total..2 * total - 1 {
        let mut sum = 0u64;
        for _ in 0..2 {
            let take_leaf = leaf < total && (inner >= made || weights[leaf] <= weights[inner]);
            let picked = if take_leaf { leaf } else { inner };
            if take_leaf {
                leaf += 1;
            } else {
                inner += 1;
            }
            sum += weights[picked];
            parents[picked] = made;
        }
        weights.push(sum);
    }
    let mut levels = vec![0u8; 2 * total - 1];
    for node in (0..2 * total - 2).rev() {
        levels[node] = levels[parents[node]].saturating_add(1);
    }
    for (node, (_, symbol)) in leaves.iter().enumerate() {
        depths[*symbol] = levels[node];
    }
    depths
}

fn canonical(depths: &[u8]) -> Vec<u16> {
    let mut tally = [0u16; 17];
    for depth in depths {
        tally[usize::from(*depth)] += 1;
    }
    tally[0] = 0;
    let mut next = [0u16; 17];
    let mut code = 0u16;
    for depth in 1..17 {
        code = (code + tally[depth - 1]) << 1;
        next[depth] = code;
    }
    depths.iter().map(|depth| {
        let depth = usize::from(*depth);
        if depth == 0 {
            return 0;
        }
        let code = next[depth];
        next[depth] += 1;
        code.reverse_bits() >> (16 - depth)
    }).collect()
}

fn run_lengths(depths: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let length = depths.iter().rposition(|depth| *depth != 0).map_or(0, |last| last + 1);
    let depths = &depths[..length];
    let (dense, sparse) = if depths.len() > 50 { worth_runs(depths) } else { (false, false) };
    let mut symbols = Vec::with_capacity(length);
    let mut extras = Vec::with_capacity(length);
    let mut previous = 8u8;
    let mut at = 0;
    while at < length {
        let value = depths[at];
        let allowed = if value == 0 { sparse } else { dense };
        let reps = if allowed { depths[at..].iter().take_while(|depth| **depth == value).count() } else { 1 };
        if value == 0 {
            zero_run(reps, &mut symbols, &mut extras);
        } else {
            value_run(previous, value, reps, &mut symbols, &mut extras);
            previous = value;
        }
        at += reps;
    }
    (symbols, extras)
}

fn worth_runs(depths: &[u8]) -> (bool, bool) {
    let (mut zeros, mut zero_runs, mut values, mut value_runs) = (0usize, 1usize, 0usize, 1usize);
    let mut at = 0;
    while at < depths.len() {
        let value = depths[at];
        let reps = depths[at..].iter().take_while(|depth| **depth == value).count();
        if reps >= 3 && value == 0 {
            zeros += reps;
            zero_runs += 1;
        }
        if reps >= 4 && value != 0 {
            values += reps;
            value_runs += 1;
        }
        at += reps;
    }
    (values > value_runs * 2, zeros > zero_runs * 2)
}

fn value_run(previous: u8, value: u8, mut reps: usize, symbols: &mut Vec<u8>, extras: &mut Vec<u8>) {
    if previous != value {
        symbols.push(value);
        extras.push(0);
        reps -= 1;
    }
    if reps == 7 {
        symbols.push(value);
        extras.push(0);
        reps -= 1;
    }
    if reps < 3 {
        for _ in 0..reps {
            symbols.push(value);
            extras.push(0);
        }
        return;
    }
    reps -= 3;
    let start = symbols.len();
    loop {
        symbols.push(16);
        extras.push((reps & 3) as u8);
        reps >>= 2;
        if reps == 0 {
            break;
        }
        reps -= 1;
    }
    symbols[start..].reverse();
    extras[start..].reverse();
}

fn zero_run(mut reps: usize, symbols: &mut Vec<u8>, extras: &mut Vec<u8>) {
    if reps == 11 {
        symbols.push(0);
        extras.push(0);
        reps -= 1;
    }
    if reps < 3 {
        for _ in 0..reps {
            symbols.push(0);
            extras.push(0);
        }
        return;
    }
    reps -= 3;
    let start = symbols.len();
    loop {
        symbols.push(17);
        extras.push((reps & 7) as u8);
        reps >>= 3;
        if reps == 0 {
            break;
        }
        reps -= 1;
    }
    symbols[start..].reverse();
    extras[start..].reverse();
}

#[cfg(test)]
mod tests {
    use super::{Prefix, canonical, limited, run_lengths};

    fn kraft(depths: &[u8]) -> u32 {
        depths.iter().filter(|depth| **depth != 0).map(|depth| 1u32 << (15 - u32::from(*depth))).sum()
    }

    #[test]
    fn lengths_stay_complete_under_the_limit() {
        let counts: Vec<u32> = (0..40).map(|index| 1u32 << (index % 31)).collect();
        let depths = limited(&counts, 15);
        assert!(depths.iter().all(|depth| (1..=15).contains(depth)));
        assert_eq!(kraft(&depths), 1 << 15);
        let narrow = limited(&[5, 1, 1, 1, 9, 200, 3, 3, 0, 0, 1, 7, 7, 7, 1, 1, 60, 2], 5);
        assert!(narrow.iter().all(|depth| *depth <= 5));
        assert_eq!(kraft(&narrow), 1 << 15);
    }

    #[test]
    fn codes_are_prefix_free() {
        let prefix = Prefix::build(&[10, 0, 3, 3, 1, 1, 40, 2, 0, 9]);
        let used: Vec<usize> = (0..prefix.size()).filter(|symbol| prefix.depths[*symbol] != 0).collect();
        for a in &used {
            for b in &used {
                if a != b {
                    let short = prefix.depths[*a].min(prefix.depths[*b]);
                    let mask = (1u16 << short) - 1;
                    assert_ne!(prefix.codes[*a] & mask, prefix.codes[*b] & mask);
                }
            }
        }
    }

    #[test]
    fn a_lone_symbol_costs_no_bits() {
        let prefix = Prefix::build(&[0, 0, 7]);
        assert_eq!(prefix.size(), 3);
        assert!(prefix.depths.iter().all(|depth| *depth == 0));
        assert_eq!(canonical(&[0, 0, 0]), vec![0, 0, 0]);
    }

    // Replays the decoder's reading of the run symbols and checks the lengths come back.
    #[test]
    fn run_symbols_expand_back() {
        let mut depths = vec![0u8; 230];
        for (index, depth) in depths.iter_mut().enumerate().take(20) {
            *depth = if index < 9 { 4 } else { 5 };
        }
        depths[224] = 5;
        depths[225] = 5;
        depths[60] = 7;
        let (symbols, extras) = run_lengths(&depths);
        let mut out: Vec<u8> = Vec::new();
        let (mut previous, mut repeat, mut repeat_len) = (8u8, 0usize, 0u8);
        for (symbol, extra) in symbols.iter().zip(&extras) {
            if *symbol < 16 {
                repeat = 0;
                out.push(*symbol);
                if *symbol != 0 {
                    previous = *symbol;
                }
            } else {
                let extra_bits = usize::from(*symbol) - 14;
                let new_len = if *symbol == 16 { previous } else { 0 };
                if repeat_len != new_len {
                    repeat = 0;
                    repeat_len = new_len;
                }
                let old = repeat;
                if repeat > 0 {
                    repeat = (repeat - 2) << extra_bits;
                }
                repeat += usize::from(*extra) + 3;
                out.extend(std::iter::repeat_n(repeat_len, repeat - old));
            }
        }
        assert_eq!(out, depths[..226]);
    }
}
