pub(super) const CACHED: u32 = u32::MAX;
pub(super) const PLANE_LEAST: usize = 6;
pub(super) const COLOR_LEAST: usize = 3;
const REACH: usize = 4096;
const TABLE_BITS: u32 = 15;
const ABSENT: u32 = u32::MAX;
const PLENTY: usize = 48;

pub(super) trait Sample: Copy + Eq {
    const SPAN: usize;
    fn mix(window: &[Self]) -> u32;
    fn raw(self) -> u32;
}

impl Sample for u8 {
    const SPAN: usize = 4;

    #[inline]
    fn mix(window: &[Self]) -> u32 {
        let word = u32::from(window[0]) | u32::from(window[1]) << 8 | u32::from(window[2]) << 16 | u32::from(window[3]) << 24;
        word.wrapping_mul(0x9e37_79b1) >> (32 - TABLE_BITS)
    }

    #[inline]
    fn raw(self) -> u32 {
        u32::from(self)
    }
}

impl Sample for u32 {
    const SPAN: usize = 3;

    #[inline]
    fn mix(window: &[Self]) -> u32 {
        let word = window[0].wrapping_mul(0x9e37_79b1) ^ window[1].wrapping_mul(0x85eb_ca6b).rotate_left(13) ^ window[2].wrapping_mul(0xc2b2_ae35).rotate_left(7);
        word.wrapping_mul(0x27d4_eb2f) >> (32 - TABLE_BITS)
    }

    #[inline]
    fn raw(self) -> u32 {
        self
    }
}

#[derive(Clone, Copy)]
pub(super) struct Token {
    pub(super) length: u32,
    pub(super) value: u32,
}

#[inline]
fn same<T: Sample>(data: &[T], at: usize, from: usize, room: usize) -> usize {
    data[at..at + room].iter().zip(&data[from..]).take_while(|(a, b)| a == b).count()
}

pub(super) fn parse<T: Sample>(data: &[T], width: usize, depth: usize, least: usize) -> Vec<Token> {
    let mut tokens = Vec::with_capacity(data.len() / 3 + 16);
    let mut heads = vec![ABSENT; if depth > 0 { 1 << TABLE_BITS } else { 0 }];
    let mut links = vec![ABSENT; if depth > 0 { data.len() } else { 0 }];
    let mut at = 0;
    while at < data.len() {
        let room = (data.len() - at).min(REACH);
        let mut length = 0;
        let mut distance = 0;
        if at >= 1 && data[at] == data[at - 1] {
            length = same(data, at, at - 1, room);
            distance = 1;
        }
        if at >= width && length < room && data[at + length] == data[at + length - width] {
            let above = same(data, at, at - width, room);
            if above > length {
                length = above;
                distance = width;
            }
        }
        let hashed = depth > 0 && room >= T::SPAN;
        let slot = if hashed { T::mix(&data[at..at + T::SPAN]) as usize } else { 0 };
        if hashed && length < PLENTY {
            let mut candidate = heads[slot];
            let mut tries = depth;
            while candidate != ABSENT && tries > 0 {
                let from = candidate as usize;
                if length < room && data[from + length] == data[at + length] {
                    let found = same(data, at, from, room);
                    if found > length {
                        length = found;
                        distance = at - from;
                    }
                }
                candidate = links[from];
                tries -= 1;
            }
        }
        if hashed {
            links[at] = heads[slot];
            heads[slot] = at as u32;
        }
        if length >= least {
            tokens.push(Token { length: length as u32, value: distance as u32 });
            if depth > 0 && length < PLENTY {
                for inside in at + 1..(at + length).min(data.len().saturating_sub(T::SPAN)) {
                    let slot = T::mix(&data[inside..inside + T::SPAN]) as usize;
                    links[inside] = heads[slot];
                    heads[slot] = inside as u32;
                }
            }
            at += length;
        } else {
            tokens.push(Token { length: 0, value: data[at].raw() });
            at += 1;
        }
    }
    tokens
}

pub(super) fn cached(tokens: &mut [Token], data: &[u32], bits: u32) {
    let mut cache = vec![0u32; 1 << bits];
    let slot = |pixel: u32| (pixel.wrapping_mul(0x1e35_a7bd) >> (32 - bits)) as usize;
    let mut at = 0;
    for token in tokens {
        if token.length == 0 {
            let place = slot(token.value);
            if cache[place] == token.value {
                *token = Token { length: CACHED, value: place as u32 };
            } else {
                cache[place] = token.value;
            }
            at += 1;
        } else {
            let length = token.length as usize;
            for &pixel in &data[at..at + length] {
                cache[slot(pixel)] = pixel;
            }
            at += length;
        }
    }
}
