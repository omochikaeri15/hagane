use super::bits::BitWriter;

const ORDER: [usize; 19] = [17, 18, 0, 1, 2, 3, 4, 5, 16, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
const DEPTH_LIMIT: u8 = 15;
const LENGTH_LIMIT: u8 = 7;
const SIMILAR: u64 = 4;
const STRIDE: usize = 4;

pub(super) struct Code {
    lengths: Vec<u8>,
    words: Vec<(u16, u8)>,
}

fn smooth(counts: &[u32]) -> Vec<u32> {
    let mut out = counts.to_vec();
    let end = out.iter().rposition(|&count| count > 0).map_or(0, |last| last + 1);
    let mut start = 0;
    while start < end {
        let mut sum = u64::from(out[start]);
        let mut stop = start + 1;
        while stop < end {
            let mean = sum / (stop - start) as u64;
            if u64::from(out[stop]).abs_diff(mean) >= SIMILAR {
                break;
            }
            sum += u64::from(out[stop]);
            stop += 1;
        }
        let run = stop - start;
        if run >= STRIDE && sum > 0 {
            let mean = ((sum + run as u64 / 2) / run as u64).max(1) as u32;
            out[start..stop].fill(mean);
        }
        start = stop;
    }
    out
}

impl Code {
    pub(super) fn build(counts: &[u32]) -> Self {
        let lengths = measure(counts, DEPTH_LIMIT);
        let words = assign(&lengths);
        let plain = Self { lengths, words };
        let smoothed = smooth(counts);
        if smoothed == counts {
            return plain;
        }
        let lengths = measure(&smoothed, DEPTH_LIMIT);
        let words = assign(&lengths);
        let other = Self { lengths, words };
        if other.spend(counts) < plain.spend(counts) { other } else { plain }
    }

    fn spend(&self, counts: &[u32]) -> u64 {
        let mut scratch = BitWriter::new(64);
        self.describe(&mut scratch);
        scratch.position() + counts.iter().zip(&self.lengths).map(|(&count, &length)| u64::from(count) * u64::from(length)).sum::<u64>()
    }

    #[inline]
    pub(super) fn put(&self, writer: &mut BitWriter, symbol: usize) {
        let (word, size) = self.words[symbol];
        writer.put(u32::from(word), u32::from(size));
    }

    pub(super) fn describe(&self, writer: &mut BitWriter) {
        let used: Vec<usize> = (0..self.lengths.len()).filter(|&symbol| self.lengths[symbol] > 0).collect();
        let simple = used.len() <= 2 && used.iter().all(|&symbol| symbol < 256 && self.lengths[symbol] == 1);
        if simple {
            let first = used.first().copied().unwrap_or(0) as u32;
            writer.put(1, 1);
            writer.put(u32::from(used.len() == 2), 1);
            if first < 2 {
                writer.put(0, 1);
                writer.put(first, 1);
            } else {
                writer.put(1, 1);
                writer.put(first, 8);
            }
            if let Some(&second) = used.get(1) {
                writer.put(second as u32, 8);
            }
            return;
        }
        writer.put(0, 1);
        let mut tokens = compact(&self.lengths);
        let mut counts = [0u32; 19];
        for token in &tokens {
            counts[usize::from(token.0)] += 1;
        }
        let lengths = measure(&counts, LENGTH_LIMIT);
        let words = assign(&lengths);
        let listed = ORDER.iter().rposition(|&symbol| lengths[symbol] > 0).map_or(4, |last| (last + 1).max(4));
        writer.put(listed as u32 - 4, 4);
        for &symbol in &ORDER[..listed] {
            writer.put(u32::from(lengths[symbol]), 3);
        }
        let kept = tokens.iter().rposition(|token| !matches!(token.0, 0 | 17 | 18)).map_or(0, |last| last + 1).max(2);
        if kept < tokens.len() {
            tokens.truncate(kept);
            let pairs = (31 - (kept as u32 - 2).max(1).leading_zeros()) / 2 + 1;
            writer.put(1, 1);
            writer.put(pairs - 1, 3);
            writer.put(kept as u32 - 2, pairs * 2);
        } else {
            writer.put(0, 1);
        }
        for token in &tokens {
            let (word, size) = words[usize::from(token.0)];
            writer.put(u32::from(word), u32::from(size));
            match token.0 {
                16 => writer.put(u32::from(token.1), 2),
                17 => writer.put(u32::from(token.1), 3),
                18 => writer.put(u32::from(token.1), 7),
                _ => {}
            }
        }
    }
}

fn measure(counts: &[u32], limit: u8) -> Vec<u8> {
    let mut lengths = vec![0u8; counts.len()];
    let used: Vec<usize> = (0..counts.len()).filter(|&symbol| counts[symbol] > 0).collect();
    if used.len() < 2 {
        if let Some(&only) = used.first() {
            lengths[only] = 1;
        }
        return lengths;
    }
    let leaves = used.len();
    let total = 2 * leaves - 1;
    let mut floor = 1u32;
    let mut weights = vec![0u64; total];
    let mut parents = vec![0u32; total];
    let mut depths = vec![0u8; total];
    let mut sorted: Vec<(u32, usize)> = Vec::with_capacity(leaves);
    loop {
        sorted.clear();
        sorted.extend(used.iter().map(|&symbol| (counts[symbol].max(floor), symbol)));
        sorted.sort_unstable();
        for (weight, leaf) in weights.iter_mut().zip(&sorted) {
            *weight = u64::from(leaf.0);
        }
        let mut leaf = 0;
        let mut inner = leaves;
        for node in leaves..total {
            let mut sum = 0;
            for _ in 0..2 {
                let take_leaf = leaf < leaves && (inner >= node || weights[leaf] <= weights[inner]);
                let pick = if take_leaf { leaf } else { inner };
                if take_leaf {
                    leaf += 1;
                } else {
                    inner += 1;
                }
                sum += weights[pick];
                parents[pick] = node as u32;
            }
            weights[node] = sum;
        }
        depths[total - 1] = 0;
        let mut deepest = 0;
        for node in (0..total - 1).rev() {
            depths[node] = depths[parents[node] as usize] + 1;
            deepest = deepest.max(depths[node]);
        }
        if deepest <= limit {
            for (entry, depth) in sorted.iter().zip(&depths) {
                lengths[entry.1] = *depth;
            }
            return lengths;
        }
        floor *= 2;
    }
}

fn assign(lengths: &[u8]) -> Vec<(u16, u8)> {
    let mut tally = [0u16; 17];
    for &length in lengths {
        tally[usize::from(length)] += 1;
    }
    let single = lengths.len() - usize::from(tally[0]) < 2;
    tally[0] = 0;
    let mut next = [0u32; 17];
    let mut word = 0u32;
    for length in 1..17 {
        word = (word + u32::from(tally[length - 1])) << 1;
        next[length] = word;
    }
    lengths.iter().map(|&length| {
        if length == 0 || single {
            return (0, 0);
        }
        let word = next[usize::from(length)];
        next[usize::from(length)] += 1;
        ((word.reverse_bits() >> (32 - u32::from(length))) as u16, length)
    }).collect()
}

fn compact(lengths: &[u8]) -> Vec<(u8, u8)> {
    let mut tokens = Vec::with_capacity(lengths.len());
    let mut previous = 8u8;
    let mut at = 0;
    while at < lengths.len() {
        let value = lengths[at];
        let mut run = lengths[at..].iter().take_while(|&&other| other == value).count();
        at += run;
        if value == 0 {
            while run >= 11 {
                let taken = run.min(138);
                tokens.push((18, (taken - 11) as u8));
                run -= taken;
            }
            if run >= 3 {
                tokens.push((17, (run - 3) as u8));
                run = 0;
            }
        } else {
            if value != previous {
                tokens.push((value, 0));
                previous = value;
                run -= 1;
            }
            while run >= 3 {
                let taken = run.min(6);
                tokens.push((16, (taken - 3) as u8));
                run -= taken;
            }
        }
        tokens.extend(std::iter::repeat_n((value, 0), run));
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::{DEPTH_LIMIT, assign, measure};

    #[test]
    fn codes_are_complete_and_bounded() {
        // A decoder rejects a code whose lengths do not fill the tree exactly, so check Kraft equality on skewed counts.
        let mut state = 99u32;
        for round in 0..200 {
            let size = 2 + round % 280;
            let counts: Vec<u32> = (0..size).map(|index| {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                if state >> 28 < 3 { 0 } else { 1 << ((state >> 8) % 24).min(index as u32 % 25) }
            }).collect();
            let lengths = measure(&counts, DEPTH_LIMIT);
            let used = lengths.iter().filter(|&&length| length > 0).count();
            assert!(lengths.iter().all(|&length| length <= DEPTH_LIMIT));
            assert!(counts.iter().zip(&lengths).all(|(&count, &length)| (count > 0) == (length > 0)));
            if used >= 2 {
                let filled: u64 = lengths.iter().filter(|&&length| length > 0).map(|&length| 1u64 << (DEPTH_LIMIT - length)).sum();
                assert_eq!(filled, 1 << DEPTH_LIMIT);
            }
            let words = assign(&lengths);
            assert!(words.iter().all(|&(_, size)| if used < 2 { size == 0 } else { true }));
        }
    }
}
