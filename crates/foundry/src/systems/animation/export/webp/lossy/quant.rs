use super::tables::{AC_STEPS, DC_STEPS, ZIGZAG};

const CEILING: i16 = 2047;
const TOLL: u64 = 285;
const TEXTURE: u64 = 50;

pub(super) struct Steps {
    step: [i16; 16],
    inverse: [u16; 16],
    bias: [u16; 16],
}

pub(super) struct Quantizer {
    pub(super) luma: Steps,
    pub(super) second: Steps,
    pub(super) chroma: Steps,
    pub(super) lambda: u64,
    pub(super) toll: u64,
    pub(super) large: u64,
    pub(super) mode: u64,
    pub(super) texture: u64,
    pub(super) tint: u64,
    pub(super) trellis_small: u64,
    pub(super) trellis_large: u64,
    pub(super) trellis_tint: u64,
}

impl Steps {
    fn new(dc: u32, ac: u32) -> Self {
        let each = |place: usize| if place == 0 { dc } else { ac };
        Self { step: std::array::from_fn(|place| each(place) as i16), inverse: std::array::from_fn(|place| (1u32 << 16).div_ceil(each(place)) as u16), bias: std::array::from_fn(|place| (each(place) / 2) as u16) }
    }

    #[inline]
    pub(super) fn quantize(&self, coefficients: &[i16; 16], first: usize) -> ([i16; 16], bool) {
        let mut sized = [0i16; 16];
        for (((out, value), inverse), bias) in sized.iter_mut().zip(coefficients).zip(&self.inverse).zip(&self.bias) {
            let size = ((u32::from(value.unsigned_abs() + bias) * u32::from(*inverse)) >> 16) as i16;
            *out = if *value < 0 { -size } else { size };
        }
        let mut levels = [0i16; 16];
        let mut any = 0i16;
        for (place, level) in levels.iter_mut().enumerate().skip(first) {
            *level = sized[ZIGZAG[place]].clamp(-CEILING, CEILING);
            any |= *level;
        }
        (levels, any != 0)
    }

    #[inline]
    pub(super) fn size(&self, place: usize) -> i16 {
        self.step[place]
    }

    fn average(&self) -> u64 {
        (u64::from(self.step[0].unsigned_abs()) + 15 * u64::from(self.step[1].unsigned_abs()) + 8) >> 4
    }

    #[inline]
    pub(super) fn restore(&self, levels: &[i16; 16]) -> [i16; 16] {
        let mut out = [0i16; 16];
        for (place, level) in levels.iter().enumerate() {
            out[ZIGZAG[place]] = level * self.step[ZIGZAG[place]];
        }
        out
    }
}

impl Quantizer {
    pub(super) fn new(index: usize, shift: i32) -> Self {
        let moved = (index as i32 + shift).clamp(0, 127) as usize;
        let dc = u32::from(DC_STEPS[index]);
        let ac = u32::from(AC_STEPS[index]);
        let toll = u64::from(ac * ac) * TOLL;
        let luma = Steps::new(dc, ac);
        let second = Steps::new(dc * 2, (ac * 155 / 100).max(8));
        let chroma = Steps::new(u32::from(DC_STEPS[moved]).min(132), u32::from(AC_STEPS[moved]));
        let small = luma.average();
        let double = second.average();
        let shade = chroma.average();
        let large = 3 * double * double;
        let tint = (3 * shade * shade) >> 6;
        let texture = (TEXTURE * small) >> 5;
        Self { luma, second, chroma, lambda: u64::from((3 * ac * ac) >> 7).max(1), toll, large: large.max(1), mode: ((small * small) >> 7).max(1), texture, tint: tint.max(1), trellis_small: ((7 * small * small) >> 4).max(1), trellis_large: ((double * double) >> 3).max(1), trellis_tint: (shade * shade).max(1) }
    }
}
