use super::bits::BitWriter;

const LOG_RANGE: u32 = 12;
const RANGE: u32 = 1 << LOG_RANGE;
pub(super) const SIGNATURE: u32 = 0x13 << 16;
const RUN_MARK: u32 = LOG_RANGE + 1;
const LOG_CODES: [(u32, u64); 14] = [(5, 17), (4, 11), (4, 15), (4, 3), (4, 9), (4, 7), (3, 4), (3, 2), (3, 5), (3, 6), (3, 0), (6, 33), (7, 1), (7, 65)];

pub(super) struct Table {
    freq: Vec<u32>,
    start: Vec<usize>,
    slots: Vec<u16>,
}

fn alias(counts: &[u32], log_alpha: u32) -> Vec<(u32, u32, u32)> {
    let size = 1usize << log_alpha;
    let entry = RANGE / size as u32;
    let mut table = vec![(0u32, 0u32, 0u32); size];
    if let Some(single) = counts.iter().position(|count| *count == RANGE) {
        for (index, slot) in table.iter_mut().enumerate() {
            *slot = (0, single as u32, entry * index as u32);
        }
        return table;
    }
    let mut cutoffs = vec![0u32; size];
    let (mut under, mut over) = (Vec::new(), Vec::new());
    for (index, count) in counts.iter().enumerate() {
        cutoffs[index] = *count;
        if *count > entry {
            over.push(index);
        } else if *count < entry {
            under.push(index);
        }
    }
    under.extend(counts.len()..size);
    let mut right = vec![0usize; size];
    let mut offsets = vec![0u32; size];
    while let Some(high) = over.pop() {
        let Some(low) = under.pop() else { break };
        let missing = entry - cutoffs[low];
        cutoffs[high] -= missing;
        right[low] = high;
        offsets[low] = cutoffs[high];
        if cutoffs[high] < entry {
            under.push(high);
        } else if cutoffs[high] > entry {
            over.push(high);
        }
    }
    for index in 0..size {
        table[index] = if cutoffs[index] == entry { (0, index as u32, 0) } else { (cutoffs[index], right[index] as u32, offsets[index].wrapping_sub(cutoffs[index])) };
    }
    table
}

impl Table {
    pub(super) fn new(counts: &[u32], log_alpha: u32) -> Self {
        let table = alias(counts, log_alpha);
        let mut start = Vec::with_capacity(counts.len() + 1);
        let mut total = 0;
        for count in counts {
            start.push(total);
            total += *count as usize;
        }
        start.push(total);
        let mut slots = vec![0u16; total];
        let shift = LOG_RANGE - log_alpha;
        let mask = (1 << shift) - 1;
        for value in 0..RANGE {
            let (cutoff, right, offset) = table[(value >> shift) as usize];
            let position = value & mask;
            let (symbol, place) = if position >= cutoff { (right as usize, offset.wrapping_add(position)) } else { ((value >> shift) as usize, position) };
            if let Some(slot) = slots.get_mut(start[symbol] + place as usize) {
                *slot = value as u16;
            }
        }
        Self { freq: counts.to_vec(), start, slots }
    }

    pub(super) fn cost(&self, counts: &[u32]) -> f64 {
        counts.iter().zip(&self.freq).filter(|(count, _)| **count != 0).map(|(count, freq)| f64::from(*count) * (f64::from(LOG_RANGE) - f64::from((*freq).max(1)).log2())).sum()
    }

    #[inline]
    fn put(&self, state: &mut u32, symbol: usize, out: &mut Vec<(u32, u64)>) {
        let freq = self.freq.get(symbol).copied().unwrap_or(0).max(1);
        if (*state >> (32 - LOG_RANGE)) >= freq {
            out.push((16, u64::from(*state & 0xffff)));
            *state >>= 16;
        }
        let slot = self.slots.get(self.start.get(symbol).copied().unwrap_or(0) + (*state % freq) as usize).copied().unwrap_or(0);
        *state = ((*state / freq) << LOG_RANGE) + u32::from(slot);
    }
}

pub(super) fn encode(writer: &mut BitWriter, tables: &[Table], items: &[(u16, u16, u32, u32)]) {
    let mut state = SIGNATURE;
    let mut out: Vec<(u32, u64)> = Vec::with_capacity(items.len() * 2);
    for (table, symbol, count, bits) in items.iter().rev() {
        if *count != 0 {
            out.push((*count, u64::from(*bits)));
        }
        if let Some(table) = tables.get(usize::from(*table)) {
            table.put(&mut state, usize::from(*symbol), &mut out);
        }
    }
    writer.write(32, u64::from(state));
    for (count, bits) in out.iter().rev() {
        writer.write(*count, *bits);
    }
}

fn small(writer: &mut BitWriter, value: u32) {
    if value == 0 {
        writer.flag(false);
    } else {
        let count = value.ilog2();
        writer.flag(true);
        writer.write(3, u64::from(count));
        writer.write(count, u64::from(value - (1 << count)));
    }
}

fn code_of(count: u32) -> u32 {
    if count == 0 { 0 } else { count.ilog2() + 1 }
}

fn precision(code: u32, shift: u32) -> u32 {
    let bits = code - 1;
    (shift as i32 - ((LOG_RANGE - bits) >> 1) as i32).clamp(0, bits as i32) as u32
}

fn quantize(target: f64, shift: u32) -> u32 {
    let rounded = target.round().max(1.0) as u32;
    let code = code_of(rounded);
    if code <= 1 {
        return 1;
    }
    let step = 1u32 << (code - 1 - precision(code, shift));
    let snapped = ((target / f64::from(step)).round() as u32 * step).max(1 << (code - 1));
    snapped.min(RANGE - 1)
}

fn spread(raw: &[u32], shift: u32) -> Option<(Vec<u32>, usize)> {
    let total: f64 = raw.iter().map(|count| f64::from(*count)).sum();
    let largest = raw.iter().enumerate().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(&a.0))).map(|(at, _)| at)?;
    let mut counts: Vec<u32> = raw.iter().map(|count| if *count == 0 { 0 } else { quantize(f64::from(*count) * f64::from(RANGE) / total, shift) }).collect();
    counts[largest] = 0;
    let used: u32 = counts.iter().sum();
    let rest = RANGE.checked_sub(used).filter(|rest| *rest > 0 && counts.iter().all(|count| count <= rest))?;
    counts[largest] = rest;
    Some((counts, largest))
}

fn header(writer: &mut BitWriter, counts: &[u32], omit: usize, shift: u32) {
    let length = counts.iter().rposition(|count| *count != 0).map_or(1, |last| last + 1).max(3);
    let counts = &counts[..length.min(counts.len())];
    let codes: Vec<u32> = counts.iter().map(|count| code_of(*count)).collect();
    let earlier = codes[..omit].iter().copied().max().unwrap_or(0);
    let written = codes[omit].max(earlier + 1).min(LOG_RANGE);
    writer.flag(false);
    writer.flag(false);
    let log = (shift + 1).ilog2();
    for _ in 0..log {
        writer.flag(true);
    }
    if log < 3 {
        writer.flag(false);
    }
    writer.write(log, u64::from(shift + 1 - (1 << log)));
    small(writer, (counts.len() - 3) as u32);
    let mut at = 0;
    let mut covered = vec![false; counts.len()];
    while at < counts.len() {
        let code = if at == omit { written } else { codes[at] };
        let (bits, value) = LOG_CODES[code as usize];
        writer.write(bits, value);
        if code == 0 && at + 1 != omit {
            let run = counts[at + 1..].iter().zip(at + 1..).take_while(|(count, place)| **count == 0 && *place != omit).count();
            if run >= 4 && at + 1 + run <= counts.len() {
                let (bits, value) = LOG_CODES[RUN_MARK as usize];
                writer.write(bits, value);
                small(writer, (run - 4) as u32);
                for flag in &mut covered[at + 1..at + 1 + run] {
                    *flag = true;
                }
                at += 1 + run;
                continue;
            }
        }
        at += 1;
    }
    for (place, count) in counts.iter().enumerate() {
        if place == omit || covered[place] || *count <= 1 {
            continue;
        }
        let code = codes[place];
        let kept = precision(code, shift);
        writer.write(kept, u64::from((count - (1 << (code - 1))) >> (code - 1 - kept)));
    }
}

pub(super) fn store(writer: &mut BitWriter, raw: &[u32], log_alpha: u32) -> Table {
    let used: Vec<usize> = (0..raw.len()).filter(|symbol| raw[*symbol] != 0).collect();
    let alphabet = 1usize << log_alpha;
    match used[..] {
        [] | [_] => {
            let symbol = used.first().copied().unwrap_or(0);
            writer.flag(true);
            writer.flag(false);
            small(writer, symbol as u32);
            let mut counts = vec![0u32; symbol + 1];
            counts[symbol] = RANGE;
            Table::new(&counts, log_alpha)
        }
        [first, second] => {
            let total = f64::from(raw[first]) + f64::from(raw[second]);
            let share = ((f64::from(raw[first]) * f64::from(RANGE) / total).round() as u32).clamp(1, RANGE - 1);
            writer.flag(true);
            writer.flag(true);
            small(writer, first as u32);
            small(writer, second as u32);
            writer.write(LOG_RANGE, u64::from(share));
            let mut counts = vec![0u32; second + 1];
            counts[first] = share;
            counts[second] = RANGE - share;
            Table::new(&counts, log_alpha)
        }
        _ => {
            let total: f64 = raw.iter().map(|count| f64::from(*count)).sum();
            let mut best: Option<(f64, u32, Vec<u32>, usize)> = None;
            for shift in (2..=LOG_RANGE).rev() {
                let Some((counts, omit)) = spread(raw, shift) else { continue };
                let mut trial = BitWriter::new();
                header(&mut trial, &counts, omit, shift);
                let data: f64 = raw.iter().zip(&counts).filter(|(count, _)| **count != 0).map(|(count, share)| f64::from(*count) * (f64::from(RANGE) / f64::from((*share).max(1))).log2()).sum();
                let bits = trial.bit_len() as f64 + data;
                if best.as_ref().is_none_or(|kept| bits < kept.0) {
                    best = Some((bits, shift, counts, omit));
                }
                if total < 64.0 {
                    break;
                }
            }
            let Some((_, shift, mut counts, omit)) = best else {
                writer.flag(true);
                writer.flag(false);
                small(writer, used[0] as u32);
                let mut counts = vec![0u32; used[0] + 1];
                counts[used[0]] = RANGE;
                return Table::new(&counts, log_alpha);
            };
            header(writer, &counts, omit, shift);
            counts.truncate(alphabet);
            Table::new(&counts, log_alpha)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{RANGE, SIGNATURE, Table, alias, spread};

    // Decodes one symbol the way the reference decoder's alias lookup does.
    fn lookup(table: &[(u32, u32, u32)], value: u32, log_alpha: u32) -> (usize, u32) {
        let shift = 12 - log_alpha;
        let (cutoff, right, offset) = table[(value >> shift) as usize];
        let position = value & ((1 << shift) - 1);
        if position >= cutoff { (right as usize, offset + position) } else { ((value >> shift) as usize, position) }
    }

    #[test]
    fn alias_slots_cover_every_symbol_once() {
        let counts = [1000, 3, 0, 2000, 1093];
        let table = alias(&counts, 5);
        let mut seen = vec![vec![false; 4096]; counts.len()];
        for value in 0..RANGE {
            let (symbol, offset) = lookup(&table, value, 5);
            assert!(offset < counts[symbol]);
            assert!(!seen[symbol][offset as usize]);
            seen[symbol][offset as usize] = true;
        }
    }

    #[test]
    fn states_round_trip_through_the_decoder_rule() {
        let counts = [1500, 7, 900, 1689];
        let table = Table::new(&counts, 5);
        let alias = alias(&counts, 5);
        let message = [0usize, 2, 3, 3, 1, 0, 0, 2, 3, 1, 1, 0, 3, 2, 0, 0, 0, 3];
        let mut state = SIGNATURE;
        let mut out = Vec::new();
        for symbol in message.iter().rev() {
            table.put(&mut state, *symbol, &mut out);
        }
        let mut words: Vec<u32> = out.iter().rev().map(|(_, bits)| *bits as u32).collect();
        words.reverse();
        for symbol in message {
            let (found, offset) = lookup(&alias, state & 4095, 5);
            assert_eq!(found, symbol);
            state = counts[found] * (state >> 12) + offset;
            if state < 1 << 16 {
                state = (state << 16) | words.pop().unwrap();
            }
        }
        assert_eq!(state, SIGNATURE);
    }

    #[test]
    fn spread_sums_to_the_range_at_every_precision() {
        let raw = [5000, 1, 0, 0, 300, 77, 0, 2];
        for shift in 2..=12 {
            let (counts, omit) = spread(&raw, shift).unwrap();
            assert_eq!(omit, 0);
            assert_eq!(counts.iter().sum::<u32>(), RANGE);
            assert!(raw.iter().zip(&counts).all(|(raw, count)| (*raw == 0) == (*count == 0)));
        }
    }
}
