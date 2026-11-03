#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::{
    __m128, _mm_add_ps, _mm_cmpeq_ps, _mm_cvtepi32_ps, _mm_cvtsi128_si32, _mm_cvtsi32_si128, _mm_cvtss_f32, _mm_cvttps_epi32, _mm_cvttss_si32,
    _mm_max_ps, _mm_min_ps, _mm_movemask_ps, _mm_mul_ps, _mm_packs_epi32, _mm_packus_epi16, _mm_set1_ps, _mm_set_ps, _mm_set_ss, _mm_setzero_ps,
    _mm_setzero_si128, _mm_shuffle_ps, _mm_sub_ps, _mm_unpacklo_epi8, _mm_unpacklo_epi16,
};

#[cfg(target_arch = "x86_64")]
use super::{FILTERED_SCALE, SUBTEXEL, TEXEL_SCALE};

#[cfg(target_arch = "x86_64")]
pub(super) type Texel = __m128;
#[cfg(not(target_arch = "x86_64"))]
pub(super) type Texel = [f32; 4];

#[cfg(target_arch = "x86_64")]
pub(super) type Corners = [__m128; 4];
#[cfg(not(target_arch = "x86_64"))]
pub(super) type Corners = [u32; 4];

#[cfg(target_arch = "x86_64")]
#[inline(always)]
fn widen(word: u32) -> __m128 {
    unsafe {
        let zero = _mm_setzero_si128();

        _mm_cvtepi32_ps(_mm_unpacklo_epi16(_mm_unpacklo_epi8(_mm_cvtsi32_si128(word as i32), zero), zero))
    }
}

#[cfg(target_arch = "x86_64")]
#[inline(always)]
pub(super) fn corners(words: [u32; 4]) -> Corners {
    [widen(words[0]), widen(words[1]), widen(words[2]), widen(words[3])]
}

#[cfg(not(target_arch = "x86_64"))]
#[inline(always)]
pub(super) fn corners(words: [u32; 4]) -> Corners {
    words
}

#[cfg(target_arch = "x86_64")]
#[inline(always)]
pub(super) fn filtered(corners: Corners, across: u32, down: u32) -> Texel {
    let [first, second, third, fourth] = corners;

    unsafe {
        let (left, right) = (_mm_set1_ps((SUBTEXEL - across) as f32), _mm_set1_ps(across as f32));
        let upper = _mm_add_ps(_mm_mul_ps(first, left), _mm_mul_ps(second, right));
        let lower = _mm_add_ps(_mm_mul_ps(third, left), _mm_mul_ps(fourth, right));
        let sum = _mm_add_ps(_mm_mul_ps(upper, _mm_set1_ps((SUBTEXEL - down) as f32)), _mm_mul_ps(lower, _mm_set1_ps(down as f32)));

        _mm_mul_ps(sum, _mm_set1_ps(FILTERED_SCALE))
    }
}

#[cfg(not(target_arch = "x86_64"))]
#[inline(always)]
pub(super) fn filtered(corners: Corners, across: u32, down: u32) -> Texel {
    super::filtered(corners, across, down)
}

#[cfg(target_arch = "x86_64")]
#[inline(always)]
pub(super) fn truncate(value: f32) -> i32 {
    unsafe { _mm_cvttss_si32(_mm_set_ss(value)) }
}

#[cfg(not(target_arch = "x86_64"))]
#[inline(always)]
pub(super) fn truncate(value: f32) -> i32 {
    value as i32
}

#[cfg(target_arch = "x86_64")]
#[inline(always)]
pub(super) fn splat(value: f32) -> Texel {
    unsafe { _mm_set1_ps(value) }
}

#[cfg(not(target_arch = "x86_64"))]
#[inline(always)]
pub(super) fn splat(value: f32) -> Texel {
    [value; 4]
}

#[cfg(target_arch = "x86_64")]
#[inline(always)]
pub(super) fn gather(values: [f32; 4]) -> Texel {
    unsafe { _mm_set_ps(values[3], values[2], values[1], values[0]) }
}

#[cfg(not(target_arch = "x86_64"))]
#[inline(always)]
pub(super) fn gather(values: [f32; 4]) -> Texel {
    values
}

#[cfg(target_arch = "x86_64")]
#[inline(always)]
pub(super) fn mul(left: Texel, right: Texel) -> Texel {
    unsafe { _mm_mul_ps(left, right) }
}

#[cfg(not(target_arch = "x86_64"))]
#[inline(always)]
pub(super) fn mul(left: Texel, right: Texel) -> Texel {
    std::array::from_fn(|channel| left[channel] * right[channel])
}

#[cfg(target_arch = "x86_64")]
#[inline(always)]
pub(super) fn empty(texel: Texel) -> bool {
    unsafe { _mm_movemask_ps(_mm_cmpeq_ps(texel, _mm_setzero_ps())) == 0xf }
}

#[cfg(not(target_arch = "x86_64"))]
#[inline(always)]
pub(super) fn empty(texel: Texel) -> bool {
    texel == [0.0; 4]
}

#[cfg(target_arch = "x86_64")]
#[inline(always)]
pub(super) fn blend<const MODE: u8>(pixel: &mut [u8; 4], source: Texel, carried: bool) {
    let packed = unsafe {
        let one = _mm_set1_ps(1.0);
        let zero = _mm_setzero_si128();
        let covering = _mm_shuffle_ps::<0xff>(source, source);
        let mixed = match MODE {
            0 if _mm_cvtss_f32(covering) == 1.0 => source,
            _ => {
                let target = _mm_mul_ps(widen(u32::from_le_bytes(*pixel)), _mm_set1_ps(TEXEL_SCALE));

                match MODE {
                    1 => _mm_add_ps(source, target),
                    2 => _mm_mul_ps(source, target),
                    3 => _mm_add_ps(source, _mm_mul_ps(target, _mm_sub_ps(one, source))),
                    _ => _mm_add_ps(source, _mm_mul_ps(target, _mm_sub_ps(one, covering))),
                }
            }
        };
        let clamped = _mm_min_ps(_mm_max_ps(mixed, _mm_setzero_ps()), one);
        let rounded = _mm_cvttps_epi32(_mm_add_ps(_mm_mul_ps(clamped, _mm_set1_ps(255.0)), _mm_set1_ps(0.5)));

        _mm_cvtsi128_si32(_mm_packus_epi16(_mm_packs_epi32(rounded, zero), zero)) as u32
    };
    let kept = if carried { (packed & 0x00ff_ffff) | (u32::from(pixel[3]) << 24) } else { packed };

    *pixel = kept.to_le_bytes();
}

#[cfg(not(target_arch = "x86_64"))]
#[inline(always)]
pub(super) fn blend<const MODE: u8>(pixel: &mut [u8; 4], source: Texel, carried: bool) {
    super::blend::<MODE>(pixel, source, carried)
}
