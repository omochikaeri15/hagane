#[derive(Clone)]
pub(super) struct BitWriter {
    bytes: Vec<u8>,
    hold: u64,
    used: u32,
}

impl BitWriter {
    pub(super) fn new(capacity: usize) -> Self {
        Self { bytes: Vec::with_capacity(capacity), hold: 0, used: 0 }
    }

    #[inline]
    pub(super) fn put(&mut self, value: u32, count: u32) {
        self.hold |= u64::from(value) << self.used;
        self.used += count;
        if self.used >= 32 {
            self.bytes.extend_from_slice(&(self.hold as u32).to_le_bytes());
            self.hold >>= 32;
            self.used -= 32;
        }
    }

    pub(super) fn position(&self) -> u64 {
        self.bytes.len() as u64 * 8 + u64::from(self.used)
    }

    pub(super) fn finish(mut self) -> Vec<u8> {
        let tail = self.hold.to_le_bytes();
        self.bytes.extend_from_slice(&tail[..self.used.div_ceil(8) as usize]);
        self.bytes
    }
}
