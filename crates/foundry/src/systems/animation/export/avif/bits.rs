pub(super) struct BitWriter {
    bytes: Vec<u8>,
    free: u32,
}

impl BitWriter {
    pub(super) fn new() -> Self {
        Self { bytes: Vec::new(), free: 0 }
    }

    pub(super) fn put(&mut self, value: u32, count: u32) {
        for shift in (0..count).rev() {
            if self.free == 0 {
                self.bytes.push(0);
                self.free = 8;
            }
            self.free -= 1;
            if let Some(last) = self.bytes.last_mut() {
                *last |= (((value >> shift) & 1) as u8) << self.free;
            }
        }
    }

    pub(super) fn flag(&mut self, value: bool) {
        self.put(u32::from(value), 1);
    }

    pub(super) fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

pub(super) fn leb128(out: &mut Vec<u8>, mut value: usize) {
    loop {
        let low = (value & 0x7F) as u8;
        value >>= 7;
        if value == 0 {
            out.push(low);
            return;
        }
        out.push(low | 0x80);
    }
}

#[cfg(test)]
mod tests {
    use super::{BitWriter, leb128};

    #[test]
    fn packs_most_significant_first() {
        let mut writer = BitWriter::new();
        writer.put(0b101, 3);
        writer.flag(true);
        writer.put(0x1FF, 9);
        assert_eq!(writer.finish(), vec![0b1011_1111, 0b1111_1000]);
    }

    #[test]
    fn leb128_splits_at_seven_bits() {
        let mut out = Vec::new();
        leb128(&mut out, 127);
        leb128(&mut out, 128);
        leb128(&mut out, 300);
        assert_eq!(out, vec![0x7F, 0x80, 0x01, 0xAC, 0x02]);
    }
}
