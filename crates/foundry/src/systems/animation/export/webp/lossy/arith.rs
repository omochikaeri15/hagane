pub(super) struct BoolWriter {
    bytes: Vec<u8>,
    low: u64,
    range: u32,
    pending: u32,
}

impl BoolWriter {
    pub(super) fn new(capacity: usize) -> Self {
        Self { bytes: Vec::with_capacity(capacity), low: 0, range: 255, pending: 0 }
    }

    #[inline]
    pub(super) fn bit(&mut self, value: bool, prob: u8) {
        let split = 1 + (((self.range - 1) * u32::from(prob)) >> 8);
        let mask = u32::from(value).wrapping_neg();
        self.low += u64::from(split & mask);
        self.range = split ^ ((split ^ (self.range - split)) & mask);
        let shift = self.range.leading_zeros() - 24;
        self.range <<= shift;
        self.low <<= shift;
        self.pending += shift;
        if self.pending >= 32 {
            self.drain();
        }
    }

    pub(super) fn literal(&mut self, value: u32, count: u32) {
        for place in (0..count).rev() {
            self.bit(value >> place & 1 == 1, 128);
        }
    }

    pub(super) fn flag(&mut self, value: bool) {
        self.bit(value, 128);
    }

    fn carry(&mut self) {
        if self.low >> (8 + self.pending) != 0 {
            self.low &= (1u64 << (8 + self.pending)) - 1;
            for byte in self.bytes.iter_mut().rev() {
                *byte = byte.wrapping_add(1);
                if *byte != 0 {
                    break;
                }
            }
        }
    }

    fn drain(&mut self) {
        self.carry();
        while self.pending >= 8 {
            self.pending -= 8;
            self.bytes.push((self.low >> (8 + self.pending)) as u8);
            self.low &= (1u64 << (8 + self.pending)) - 1;
        }
    }

    pub(super) fn finish(mut self) -> Vec<u8> {
        self.drain();
        self.low <<= 8 - self.pending;
        self.bytes.push((self.low >> 8) as u8);
        self.bytes.push(self.low as u8);
        self.bytes.extend_from_slice(&[0, 0]);
        self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::BoolWriter;

    struct BoolReader<'a> {
        bytes: &'a [u8],
        at: usize,
        value: u32,
        range: u32,
        count: u32,
    }

    impl<'a> BoolReader<'a> {
        fn new(bytes: &'a [u8]) -> Self {
            Self { bytes, at: 2, value: u32::from(bytes[0]) << 8 | u32::from(bytes[1]), range: 255, count: 0 }
        }

        fn bit(&mut self, prob: u8) -> bool {
            let split = 1 + (((self.range - 1) * u32::from(prob)) >> 8);
            let big = split << 8;
            let out = self.value >= big;
            if out {
                self.range -= split;
                self.value -= big;
            } else {
                self.range = split;
            }
            while self.range < 128 {
                self.value <<= 1;
                self.range <<= 1;
                self.count += 1;
                if self.count == 8 {
                    self.count = 0;
                    self.value |= u32::from(self.bytes.get(self.at).copied().unwrap_or(0));
                    self.at += 1;
                }
            }
            out
        }
    }

    #[test]
    fn round_trip_with_carries() {
        // A long run of near-certain ones followed by surprises forces carries through 0xff bytes.
        let mut state = 0x1234_5678u32;
        let mut plan = Vec::new();
        for index in 0..20000 {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let prob = match index % 7 { 0 => 1, 1 => 255, 2 => 128, _ => (state >> 24) as u8 | 1 };
            let value = if index % 5 < 3 { prob < 128 } else { state >> 13 & 1 == 1 };
            plan.push((value, prob));
        }
        let mut writer = BoolWriter::new(0);
        for &(value, prob) in &plan {
            writer.bit(value, prob);
        }
        let bytes = writer.finish();
        let mut reader = BoolReader::new(&bytes);
        for (index, &(value, prob)) in plan.iter().enumerate() {
            assert_eq!(reader.bit(prob), value, "bit {index}");
        }
        assert!(reader.at <= bytes.len());
    }
}
