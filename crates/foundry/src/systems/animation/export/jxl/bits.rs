pub(super) struct BitWriter {
    bytes: Vec<u8>,
    hold: u64,
    fill: u32,
}

impl BitWriter {
    pub(super) fn new() -> Self {
        Self { bytes: Vec::new(), hold: 0, fill: 0 }
    }

    pub(super) fn with_capacity(bytes: usize) -> Self {
        Self { bytes: Vec::with_capacity(bytes), hold: 0, fill: 0 }
    }

    #[inline]
    pub(super) fn write(&mut self, count: u32, bits: u64) {
        self.hold |= bits << self.fill;
        self.fill += count;
        if self.fill >= 32 {
            self.bytes.extend_from_slice(&(self.hold as u32).to_le_bytes());
            self.hold >>= 32;
            self.fill -= 32;
        }
    }

    pub(super) fn flag(&mut self, value: bool) {
        self.write(1, u64::from(value));
    }

    pub(super) fn select(&mut self, value: u32, choices: [(u32, u32); 4]) {
        let fits = |choice: &(u32, u32)| value >= choice.1 && u64::from(value - choice.1) < (1u64 << choice.0);
        let index = choices.iter().position(fits).unwrap_or(3);
        let (count, offset) = choices[index];
        self.write(2, index as u64);
        self.write(count, u64::from(value.saturating_sub(offset)));
    }

    pub(super) fn length(&mut self, value: u32) {
        if value == 0 {
            self.write(1, 0);
        } else {
            let count = value.ilog2();
            self.write(1, 1);
            self.write(4, u64::from(count));
            self.write(count, u64::from(value - (1 << count)));
        }
    }

    pub(super) fn pad(&mut self) {
        let spare = (8 - self.fill % 8) % 8;
        self.write(spare, 0);
    }

    pub(super) fn bit_len(&self) -> usize {
        self.bytes.len() * 8 + self.fill as usize
    }

    pub(super) fn append(&mut self, other: &Self) {
        for byte in &other.bytes {
            self.write(8, u64::from(*byte));
        }
        self.write(other.fill, other.hold);
    }

    pub(super) fn extend(&mut self, bytes: &[u8]) {
        self.pad();
        self.flush();
        self.bytes.extend_from_slice(bytes);
    }

    pub(super) fn finish(mut self) -> Vec<u8> {
        self.pad();
        self.flush();
        self.bytes
    }

    fn flush(&mut self) {
        while self.fill >= 8 {
            self.bytes.push(self.hold as u8);
            self.hold >>= 8;
            self.fill -= 8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BitWriter;

    #[test]
    fn packs_low_bits_first() {
        let mut writer = BitWriter::new();
        writer.write(3, 0b101);
        writer.write(7, 0b1111111);
        writer.write(32, 0xdead_beef);
        writer.write(1, 1);
        assert_eq!(writer.bit_len(), 43);
        let bytes = writer.finish();
        let mut value = 0u64;
        for (index, byte) in bytes.iter().enumerate() {
            value |= u64::from(*byte) << (8 * index);
        }
        assert_eq!(value & 0b111, 0b101);
        assert_eq!((value >> 3) & 0x7f, 0x7f);
        assert_eq!((value >> 10) & 0xffff_ffff, 0xdead_beef);
        assert_eq!(value >> 42, 1);
        assert_eq!(bytes.len(), 6);
    }

    #[test]
    fn select_picks_the_first_range_that_fits() {
        let mut writer = BitWriter::new();
        writer.select(300, [(8, 0), (11, 256), (14, 2304), (30, 18688)]);
        let bytes = writer.finish();
        let value = u32::from(bytes[0]) | u32::from(bytes[1]) << 8;
        assert_eq!(value & 3, 1);
        assert_eq!(value >> 2, 300 - 256);
    }
}
