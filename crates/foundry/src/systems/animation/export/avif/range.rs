
const TOP: u32 = 32768;
const FLOOR: u32 = 4;

pub(super) struct Coder {
    low: u32,
    range: u32,
    count: i32,
    pending: Vec<u16>,
}

impl Coder {
    pub(super) fn new(capacity: usize) -> Self {
        Self { low: 0, range: TOP, count: -9, pending: Vec::with_capacity(capacity) }
    }

    pub(super) fn tell(&self) -> f32 {
        (self.pending.len() * 8) as f32 + (self.count + 10) as f32 - ((self.range as f32).log2() - 15.0)
    }

    pub(super) fn spot(&self) -> (u32, u32, i32, usize) {
        (self.low, self.range, self.count, self.pending.len())
    }

    pub(super) fn rewind(&mut self, spot: (u32, u32, i32, usize)) {
        (self.low, self.range, self.count) = (spot.0, spot.1, spot.2);
        self.pending.truncate(spot.3);
    }

    pub(super) fn tail(&self, from: usize) -> &[u16] {
        &self.pending[from..]
    }

    pub(super) fn resume(&mut self, spot: (u32, u32, i32, usize), tail: &[u16]) {
        self.pending.truncate(spot.3 - tail.len());
        self.pending.extend_from_slice(tail);
        (self.low, self.range, self.count) = (spot.0, spot.1, spot.2);
    }

    fn store(&mut self, below: u32, upto: u32, remaining: u32) {
        let range = self.range;
        let upper = if below >= TOP { range } else { (((range >> 8) * (below >> 6)) >> 1) + FLOOR * remaining };
        let lower = (((range >> 8) * (upto >> 6)) >> 1) + FLOOR * (remaining - 1);
        let mut low = self.low + (range - upper);
        let range = upper - lower;
        let shift = range.leading_zeros() as i32 - 16;
        let mut count = self.count;
        let mut total = count + shift;
        if total >= 0 {
            count += 16;
            let mut keep = (1u32 << count) - 1;
            if total >= 8 {
                self.pending.push((low >> count) as u16);
                low &= keep;
                count -= 8;
                keep >>= 8;
            }
            self.pending.push((low >> count) as u16);
            total = count + shift - 24;
            low &= keep;
        }
        self.low = low << shift;
        self.range = range << shift;
        self.count = total;
    }

    pub(super) fn symbol<const N: usize>(&mut self, value: usize, cdf: &mut [u16; N]) {
        let below = if value == 0 { TOP } else { TOP - u32::from(cdf[value - 1]) };
        let upto = TOP - u32::from(cdf[value]);
        self.store(below, upto, (N - 1 - value) as u32);
        adapt(cdf, value);
    }

    pub(super) fn chance(&mut self, value: bool, zero: u32) {
        if value {
            self.store(TOP - zero, 0, 1);
        } else {
            self.store(TOP, TOP - zero, 2);
        }
    }

    pub(super) fn bit(&mut self, value: bool) {
        self.chance(value, TOP / 2);
    }

    pub(super) fn finish(mut self) -> Vec<u8> {
        let mask = 0x3FFFu32;
        let mut end = ((self.low + mask) & !mask) | (mask + 1);
        let mut count = self.count;
        let mut left = 10 + count;
        if left > 0 {
            let mut keep = (1u32 << (count + 16)) - 1;
            while left > 0 {
                self.pending.push((end >> (count + 16)) as u16);
                end &= keep;
                left -= 8;
                count -= 8;
                keep >>= 8;
            }
        }
        let mut out = vec![0u8; self.pending.len()];
        let mut carry = 0u32;
        for (byte, &word) in out.iter_mut().zip(&self.pending).rev() {
            carry += u32::from(word);
            *byte = carry as u8;
            carry >>= 8;
        }
        out
    }
}

pub(super) fn adapt<const N: usize>(cdf: &mut [u16; N], value: usize) {
    let symbols = N - 1;
    let seen = cdf[symbols];
    let rate = 3 + u32::from(seen > 15) + u32::from(seen > 31) + (symbols.ilog2()).min(2);
    for (index, entry) in cdf[..symbols - 1].iter_mut().enumerate() {
        if index >= value {
            *entry += (32768 - *entry) >> rate;
        } else {
            *entry -= *entry >> rate;
        }
    }
    cdf[symbols] = seen + u16::from(seen < 32);
}

#[cfg(test)]
mod tests {
    use super::{Coder, adapt};

    struct Reader<'a> {
        bytes: &'a [u8],
        at: usize,
        value: u32,
        range: u32,
        max_bits: i64,
    }

    impl<'a> Reader<'a> {
        fn bits(&mut self, count: u32) -> u32 {
            let mut out = 0;
            for _ in 0..count {
                let byte = self.bytes.get(self.at / 8).copied().unwrap_or(0);
                out = (out << 1) | u32::from((byte >> (7 - self.at % 8)) & 1);
                self.at += 1;
            }
            out
        }

        fn new(bytes: &'a [u8]) -> Self {
            let mut reader = Self { bytes, at: 0, value: 0, range: 1 << 15, max_bits: 8 * bytes.len() as i64 - 15 };
            let count = (bytes.len() as u32 * 8).min(15);
            let first = reader.bits(count);
            reader.value = ((1 << 15) - 1) ^ (first << (15 - count));
            reader
        }

        // The symbol decoding process of the specification, kept literal on purpose.
        fn symbol<const N: usize>(&mut self, cdf: &mut [u16; N], update: bool) -> usize {
            let symbols = N as u32 - 1;
            let mut cur = self.range;
            let mut symbol = 0u32;
            let mut prev;
            loop {
                prev = cur;
                let f = (1u32 << 15) - u32::from(cdf[symbol as usize]);
                cur = (((self.range >> 8) * (f >> 6)) >> 1) + 4 * (symbols - symbol - 1);
                if self.value >= cur {
                    break;
                }
                symbol += 1;
            }
            self.range = prev - cur;
            self.value -= cur;
            let bits = 15 - self.range.ilog2();
            self.range <<= bits;
            let read = i64::from(bits).min(self.max_bits.max(0)) as u32;
            let fresh = self.bits(read);
            let padded = fresh << (bits - read);
            self.value = padded ^ (((self.value + 1) << bits) - 1);
            self.max_bits -= i64::from(bits);
            if update {
                adapt(cdf, symbol as usize);
            }
            symbol as usize
        }
    }

    fn noise(state: &mut u64) -> u32 {
        *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (*state >> 33) as u32
    }

    #[test]
    fn decoder_model_reads_back_every_symbol() {
        let fresh = || [[4000u16, 9000, 20000, 32768, 0], [30000, 31000, 32000, 32768, 0], [8192, 16384, 24576, 32768, 0]];
        let mut state = 7u64;
        let mut script = Vec::new();
        for _ in 0..20000 {
            let draw = noise(&mut state);
            let kind = draw % 5;
            // Skewed picks so the adaptive tables drift far from where they start.
            let value = if kind < 3 { ((draw >> 8) % 4) as usize * usize::from(!(draw >> 16).is_multiple_of(3)) } else { ((draw >> 8) & 1) as usize };
            script.push((kind, value));
        }
        let mut coder = Coder::new(0);
        let mut tables = fresh();
        for &(kind, value) in &script {
            match kind {
                0..=2 => coder.symbol(value, &mut tables[kind as usize]),
                3 => coder.bit(value == 1),
                _ => coder.chance(value == 1, 29000),
            }
        }
        let bytes = coder.finish();
        let mut reader = Reader::new(&bytes);
        let mut tables = fresh();
        for &(kind, value) in &script {
            let got = match kind {
                0..=2 => reader.symbol(&mut tables[kind as usize], true),
                3 => reader.symbol(&mut [16384, 32768, 0], false),
                _ => reader.symbol(&mut [29000, 32768, 0], false),
            };
            assert_eq!(got, value);
        }
        // The exit process wants a one right after the consumed bits and zeros to the end.
        let trailing = reader.at as i64 - 15i64.min(reader.max_bits + 15);
        let bit = |at: i64| (bytes[(at / 8) as usize] >> (7 - at % 8)) & 1;
        assert!(reader.max_bits >= -14);
        assert_eq!(bit(trailing), 1);
        assert!((trailing + 1..bytes.len() as i64 * 8).all(|at| bit(at) == 0));
    }

    #[test]
    fn empty_stream_still_terminates() {
        let bytes = Coder::new(0).finish();
        assert!(!bytes.is_empty());
    }
}
