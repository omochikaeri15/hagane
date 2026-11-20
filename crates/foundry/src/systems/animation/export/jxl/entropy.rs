use super::{
    ans::{self, Table},
    bits::BitWriter,
    prefix::Prefix,
};

pub(super) const RUN_SYMBOL: usize = 224;
pub(super) const RUN_FLOOR: u32 = 8;
pub(super) const SYMBOLS: usize = RUN_SYMBOL + 40;
const RUN_HYBRID: Hybrid = Hybrid::new(4, 0, 0);
const MAP_FLOOR: u32 = 3;
const FAST: usize = 1024;

#[derive(Clone, Copy)]
pub(super) struct Hybrid {
    split: u32,
    msb: u32,
    lsb: u32,
}

impl Hybrid {
    pub(super) const fn new(split: u32, msb: u32, lsb: u32) -> Self {
        Self { split, msb, lsb }
    }

    #[inline]
    pub(super) fn token(self, value: u32) -> (u32, u32, u32) {
        if value < (1 << self.split) {
            return (value, 0, 0);
        }
        let high = value.ilog2();
        let rest = value - (1 << high);
        let kept = self.msb + self.lsb;
        let token = (1 << self.split) + ((high - self.split) << kept) + ((rest >> (high - self.msb)) << self.lsb) + (rest & ((1 << self.lsb) - 1));
        let count = high - kept;
        (token, count, (value >> self.lsb) & ((1 << count) - 1))
    }

    pub(super) fn extra(self, token: u32) -> u32 {
        let split = 1 << self.split;
        if token < split {
            return 0;
        }
        let kept = self.msb + self.lsb;
        self.split - kept + ((token - split) >> kept)
    }

    fn store(self, writer: &mut BitWriter, log_alphabet: u32) {
        writer.write((log_alphabet + 1).next_power_of_two().ilog2(), u64::from(self.split));
        if self.split == log_alphabet {
            return;
        }
        writer.write(ceil_log(self.split + 1), u64::from(self.msb));
        writer.write(ceil_log(self.split - self.msb + 1), u64::from(self.lsb));
    }
}

fn ceil_log(value: u32) -> u32 {
    value.next_power_of_two().ilog2()
}

pub(super) struct Coder {
    hybrid: Hybrid,
    codes: Vec<Prefix>,
    fast: Vec<u16>,
    runs: bool,
    ans: Option<Ans>,
}

struct Ans {
    log_alpha: u32,
    header: BitWriter,
    tables: Vec<Table>,
}

impl Coder {
    pub(super) fn new(hybrid: Hybrid, histograms: &[Vec<u32>], runs: bool, ans: bool) -> Self {
        let fast = (0..FAST as u32).map(|value| {
            let (token, count, _) = hybrid.token(value);
            (token | count << 10) as u16
        }).collect();
        let codes: Vec<Prefix> = histograms.iter().map(|counts| Prefix::build(counts)).collect();
        if ans {
            let alphabet = histograms.iter().map(|counts| counts.iter().rposition(|count| *count != 0).map_or(1, |last| last + 1)).max().unwrap_or(1);
            let log_alpha = (alphabet as u32).next_power_of_two().ilog2().clamp(5, 8);
            let mut header = BitWriter::new();
            let mut tables: Vec<Table> = histograms.iter().map(|counts| ans::store(&mut header, &counts[..counts.len().min(1 << log_alpha)], log_alpha)).collect();
            let ans_bits = header.bit_len() as f64 + histograms.iter().zip(&tables).map(|(counts, table)| table.cost(counts)).sum::<f64>();
            let mut scratch = BitWriter::new();
            for code in &codes {
                scratch.length(code.size() as u32 - 1);
                code.store(&mut scratch);
            }
            let prefix_bits = scratch.bit_len() as f64 + histograms.iter().zip(&codes).map(|(counts, code)| counts.iter().zip(&code.depths).map(|(count, depth)| f64::from(*count) * f64::from(*depth)).sum::<f64>()).sum::<f64>();
            if ans_bits < prefix_bits {
                if runs {
                    tables.push(ans::store(&mut header, &[0, 1], log_alpha));
                }
                return Self { hybrid, codes: Vec::new(), fast, runs, ans: Some(Ans { log_alpha, header, tables }) };
            }
        }
        Self { hybrid, codes, fast, runs, ans: None }
    }

    pub(super) fn idle(&self, writer: &mut BitWriter) {
        if self.ans.is_some() {
            writer.write(32, u64::from(ans::SIGNATURE));
        }
    }

    fn tables(&self) -> usize {
        self.ans.as_ref().map_or(self.codes.len(), |ans| ans.tables.len() - usize::from(self.runs))
    }

    pub(super) fn emit(&self, writer: &mut BitWriter, tokens: impl Iterator<Item = (usize, bool, u32)>) {
        let Some(ans) = &self.ans else {
            for (table, run, value) in tokens {
                if run {
                    self.put_run(writer, table, value);
                } else {
                    self.put(writer, table, value);
                }
            }
            return;
        };
        let distance = (ans.tables.len() - 1) as u16;
        let mut items: Vec<(u16, u16, u32, u32)> = Vec::with_capacity(tokens.size_hint().0 + 16);
        for (table, run, value) in tokens {
            if run {
                let (token, count, bits) = RUN_HYBRID.token(value);
                items.push((table as u16, (RUN_SYMBOL as u32 + token) as u16, count, bits));
                items.push((distance, 1, 0, 0));
            } else {
                let (token, count, bits) = self.hybrid.token(value);
                items.push((table as u16, token as u16, count, bits));
            }
        }
        ans::encode(writer, &ans.tables, &items);
    }

    pub(super) fn store(&self, writer: &mut BitWriter, map: &[u8]) {
        writer.flag(self.runs);
        if self.runs {
            writer.write(2, 0);
            writer.select(RUN_FLOOR, [(0, 3), (0, 4), (2, 5), (8, 9)]);
            RUN_HYBRID.store(writer, 8);
        }
        let tables = self.tables() + usize::from(self.runs);
        if self.runs {
            let mut full = map.to_vec();
            full.push(self.tables() as u8);
            store_map(writer, &full, tables);
        } else {
            store_map(writer, map, tables);
        }
        if let Some(ans) = &self.ans {
            writer.flag(false);
            writer.write(2, u64::from(ans.log_alpha - 5));
            for _ in 0..self.tables() {
                self.hybrid.store(writer, ans.log_alpha);
            }
            if self.runs {
                Hybrid::new(0, 0, 0).store(writer, ans.log_alpha);
            }
            writer.append(&ans.header);
            return;
        }
        writer.flag(true);
        for _ in &self.codes {
            self.hybrid.store(writer, 15);
        }
        if self.runs {
            Hybrid::new(0, 0, 0).store(writer, 15);
        }
        for code in &self.codes {
            writer.length(code.size() as u32 - 1);
        }
        if self.runs {
            writer.length(1);
        }
        for code in &self.codes {
            code.store(writer);
        }
        if self.runs {
            writer.write(4, 1);
            writer.write(1, 1);
        }
    }

    #[inline]
    pub(super) fn put(&self, writer: &mut BitWriter, table: usize, value: u32) {
        let code = &self.codes[table];
        if (value as usize) < FAST {
            let entry = u32::from(self.fast[value as usize]);
            let (token, count) = ((entry & 1023) as usize, entry >> 10);
            let depth = u32::from(code.depths[token]);
            let low = u64::from(value & ((1 << count) - 1));
            writer.write(depth + count, u64::from(code.codes[token]) | low << depth);
        } else {
            let (token, count, bits) = self.hybrid.token(value);
            let token = token as usize;
            writer.write(u32::from(code.depths[token]), u64::from(code.codes[token]));
            writer.write(count, u64::from(bits));
        }
    }

    #[inline]
    pub(super) fn put_run(&self, writer: &mut BitWriter, table: usize, extra: u32) {
        let code = &self.codes[table];
        let (token, count, bits) = RUN_HYBRID.token(extra);
        let token = RUN_SYMBOL + token as usize;
        writer.write(u32::from(code.depths[token]), u64::from(code.codes[token]));
        writer.write(count, u64::from(bits));
    }
}

#[inline]
pub(super) fn run_token(extra: u32) -> usize {
    RUN_SYMBOL + RUN_HYBRID.token(extra).0 as usize
}

pub(super) fn store_map(writer: &mut BitWriter, map: &[u8], tables: usize) {
    if map.len() <= 1 {
        return;
    }
    if tables == 1 {
        writer.write(3, 1);
        return;
    }
    let width = ceil_log(tables as u32);
    let mut best = BitWriter::new();
    if width <= 3 {
        best.write(1, 1);
        best.write(2, u64::from(width));
        for entry in map {
            best.write(width, u64::from(*entry));
        }
    } else {
        store_ranked(&mut best, map, false);
    }
    if map.len() > 2 {
        let mut ranked = BitWriter::new();
        store_ranked(&mut ranked, map, true);
        if ranked.bit_len() < best.bit_len() {
            best = ranked;
        }
    }
    writer.append(&best);
}

fn store_ranked(writer: &mut BitWriter, map: &[u8], runs: bool) {
    let mut order: Vec<u8> = (0..=255).collect();
    let mut ranks = Vec::with_capacity(map.len());
    for entry in map {
        let rank = order.iter().position(|held| held == entry).unwrap_or(0);
        order[..=rank].rotate_right(1);
        ranks.push(rank as u32);
    }
    let mut items: Vec<(usize, u32, u32)> = Vec::with_capacity(ranks.len());
    let mut at = 0;
    while at < ranks.len() {
        let same = if runs && at > 0 { ranks[at..].iter().take_while(|rank| **rank == ranks[at - 1]).count() as u32 } else { 0 };
        if same >= MAP_FLOOR {
            let (token, count, bits) = RUN_HYBRID.token(same - MAP_FLOOR);
            items.push((RUN_SYMBOL + token as usize, count, bits));
            at += same as usize;
        } else {
            items.push((ranks[at] as usize, 0, 0));
            at += 1;
        }
    }
    let mut counts = vec![0u32; SYMBOLS];
    for item in &items {
        counts[item.0] += 1;
    }
    let code = Prefix::build(&counts);
    writer.write(2, 2);
    writer.flag(runs);
    if runs {
        writer.write(2, 0);
        writer.select(MAP_FLOOR, [(0, 3), (0, 4), (2, 5), (8, 9)]);
        RUN_HYBRID.store(writer, 8);
        writer.write(5, 0b10011);
    }
    writer.flag(true);
    Hybrid::new(15, 0, 0).store(writer, 15);
    if runs {
        Hybrid::new(0, 0, 0).store(writer, 15);
    }
    writer.length(code.size() as u32 - 1);
    if runs {
        writer.length(0);
    }
    code.store(writer);
    for (symbol, count, bits) in items {
        writer.write(u32::from(code.depths[symbol]), u64::from(code.codes[symbol]));
        writer.write(count, u64::from(bits));
    }
}

pub(super) fn store_tokens(writer: &mut BitWriter, contexts: usize, tokens: &[(u8, u32)]) {
    let hybrid = Hybrid::new(4, 2, 0);
    let mut histograms = vec![vec![0u32; 96]; contexts];
    for (context, value) in tokens {
        histograms[usize::from(*context)][hybrid.token(*value).0 as usize] += 1;
    }
    let coder = Coder::new(hybrid, &histograms, false, false);
    let map: Vec<u8> = (0..contexts as u8).collect();
    coder.store(writer, &map);
    for (context, value) in tokens {
        coder.put(writer, usize::from(*context), *value);
    }
}

#[cfg(test)]
mod tests {
    use super::Hybrid;

    // Mirrors the decoder's reading of a hybrid integer.
    fn read(hybrid: Hybrid, token: u32, bits: u32) -> u32 {
        let split = 1 << hybrid.split;
        if token < split {
            return token;
        }
        let kept = hybrid.msb + hybrid.lsb;
        let count = hybrid.split - kept + ((token - split) >> kept);
        let low = token & ((1 << hybrid.lsb) - 1);
        let top = (token >> hybrid.lsb) & ((1 << hybrid.msb) - 1);
        ((((1 << hybrid.msb) | top) << count | bits) << hybrid.lsb) | low
    }

    #[test]
    fn hybrid_integers_round_trip() {
        for hybrid in [Hybrid::new(0, 0, 0), Hybrid::new(4, 0, 0), Hybrid::new(4, 1, 0), Hybrid::new(4, 2, 0), Hybrid::new(4, 1, 1), Hybrid::new(15, 0, 0)] {
            for value in (0..70_000u32).chain([1 << 20, (1 << 24) - 1]) {
                let (token, count, bits) = hybrid.token(value);
                assert!(bits < (1 << count));
                assert_eq!(read(hybrid, token, bits), value);
            }
        }
    }
}
