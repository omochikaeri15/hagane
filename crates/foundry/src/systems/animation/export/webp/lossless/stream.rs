use super::bits::BitWriter;
use super::huffman::Code;
use super::refs::{CACHED, Token};

const OFFSETS: [(i8, i8); 120] = [
    (0, 1), (1, 0), (1, 1), (-1, 1), (0, 2), (2, 0), (1, 2), (-1, 2), (2, 1), (-2, 1), (2, 2), (-2, 2), (0, 3), (3, 0), (1, 3), (-1, 3), (3, 1), (-3, 1), (2, 3), (-2, 3),
    (3, 2), (-3, 2), (0, 4), (4, 0), (1, 4), (-1, 4), (4, 1), (-4, 1), (3, 3), (-3, 3), (2, 4), (-2, 4), (4, 2), (-4, 2), (0, 5), (3, 4), (-3, 4), (4, 3), (-4, 3), (5, 0),
    (1, 5), (-1, 5), (5, 1), (-5, 1), (2, 5), (-2, 5), (5, 2), (-5, 2), (4, 4), (-4, 4), (3, 5), (-3, 5), (5, 3), (-5, 3), (0, 6), (6, 0), (1, 6), (-1, 6), (6, 1), (-6, 1),
    (2, 6), (-2, 6), (6, 2), (-6, 2), (4, 5), (-4, 5), (5, 4), (-5, 4), (3, 6), (-3, 6), (6, 3), (-6, 3), (0, 7), (7, 0), (1, 7), (-1, 7), (5, 5), (-5, 5), (7, 1), (-7, 1),
    (4, 6), (-4, 6), (6, 4), (-6, 4), (2, 7), (-2, 7), (7, 2), (-7, 2), (3, 7), (-3, 7), (7, 3), (-7, 3), (5, 6), (-5, 6), (6, 5), (-6, 5), (8, 0), (4, 7), (-4, 7), (7, 4),
    (-7, 4), (8, 1), (8, 2), (6, 6), (-6, 6), (8, 3), (5, 7), (-5, 7), (7, 5), (-7, 5), (8, 4), (6, 7), (-6, 7), (7, 6), (-7, 6), (8, 5), (7, 7), (-7, 7), (8, 6), (8, 7),
];
const NEAR: [u8; 128] = near();
const LITERALS: usize = 256;
const LENGTHS: usize = 24;
const DISTANCES: usize = 40;

const fn near() -> [u8; 128] {
    let mut table = [0u8; 128];
    let mut index = 0;
    while index < OFFSETS.len() {
        let (across, down) = OFFSETS[index];
        table[down as usize * 16 + (8 - across) as usize] = index as u8 + 1;
        index += 1;
    }
    table
}

fn plane_code(width: usize, distance: usize) -> u32 {
    let down = distance / width;
    let across = distance - down * width;
    let mut code = 0;
    if across <= 8 && down < 8 {
        code = NEAR[down * 16 + 8 - across];
    }
    if code == 0 && across + 8 > width && down < 7 {
        code = NEAR[(down + 1) * 16 + 8 + width - across];
    }
    if code == 0 { distance as u32 + 120 } else { u32::from(code) }
}

#[inline]
fn prefix(value: u32) -> (usize, u32, u32) {
    let value = value - 1;
    if value < 2 {
        return (value as usize, 0, 0);
    }
    let high = 31 - value.leading_zeros();
    let second = (value >> (high - 1)) & 1;
    let extra = high - 1;
    ((2 * high + second) as usize, extra, value & ((1 << extra) - 1))
}

pub(super) struct Zones<'a> {
    pub(super) classes: &'a [u8],
    pub(super) count: usize,
    pub(super) bits: u32,
}

struct Tally {
    green: Vec<u32>,
    red: [u32; LITERALS],
    blue: [u32; LITERALS],
    alpha: [u32; LITERALS],
    far: [u32; DISTANCES],
}

struct Group {
    green: Code,
    red: Code,
    blue: Code,
    alpha: Code,
    far: Code,
}

struct Walk<'a> {
    zones: Option<&'a Zones<'a>>,
    width: usize,
    columns: usize,
    x: usize,
    y: usize,
}

impl<'a> Walk<'a> {
    fn new(zones: Option<&'a Zones<'a>>, width: usize) -> Self {
        Self { zones, width, columns: zones.map_or(1, |zones| width.div_ceil(1 << zones.bits)), x: 0, y: 0 }
    }

    #[inline]
    fn group(&self) -> usize {
        self.zones.map_or(0, |zones| usize::from(zones.classes[(self.y >> zones.bits) * self.columns + (self.x >> zones.bits)]))
    }

    #[inline]
    fn advance(&mut self, count: usize) {
        self.x += count;
        if self.x >= self.width {
            self.y += self.x / self.width;
            self.x %= self.width;
        }
    }
}

#[inline]
fn span(token: &Token) -> usize {
    if token.length == 0 || token.length == CACHED { 1 } else { token.length as usize }
}

pub(super) fn write(writer: &mut BitWriter, tokens: &[Token], width: usize, colored: bool, cache_bits: u32, zones: Option<&Zones<'_>>) {
    let cache = if cache_bits > 0 { 1usize << cache_bits } else { 0 };
    let count = zones.map_or(1, |zones| zones.count);
    let mut tallies: Vec<Tally> = (0..count).map(|_| Tally { green: vec![0; LITERALS + LENGTHS + cache], red: [0; LITERALS], blue: [0; LITERALS], alpha: [0; LITERALS], far: [0; DISTANCES] }).collect();
    let mut walk = Walk::new(zones, width);
    for token in tokens {
        let tally = &mut tallies[walk.group()];
        match token.length {
            0 if colored => {
                tally.green[(token.value >> 8 & 255) as usize] += 1;
                tally.red[(token.value >> 16 & 255) as usize] += 1;
                tally.blue[(token.value & 255) as usize] += 1;
                tally.alpha[(token.value >> 24) as usize] += 1;
            }
            0 => tally.green[token.value as usize] += 1,
            CACHED => tally.green[LITERALS + LENGTHS + token.value as usize] += 1,
            length => {
                tally.green[LITERALS + prefix(length).0] += 1;
                tally.far[prefix(plane_code(width, token.value as usize)).0] += 1;
            }
        }
        walk.advance(span(token));
    }
    let groups: Vec<Group> = tallies.iter().map(|tally| Group { green: Code::build(&tally.green), red: Code::build(&tally.red), blue: Code::build(&tally.blue), alpha: Code::build(&tally.alpha), far: Code::build(&tally.far) }).collect();
    for group in &groups {
        for code in [&group.green, &group.red, &group.blue, &group.alpha, &group.far] {
            code.describe(writer);
        }
    }
    let mut walk = Walk::new(zones, width);
    for token in tokens {
        let group = &groups[walk.group()];
        match token.length {
            0 if colored => {
                group.green.put(writer, (token.value >> 8 & 255) as usize);
                group.red.put(writer, (token.value >> 16 & 255) as usize);
                group.blue.put(writer, (token.value & 255) as usize);
                group.alpha.put(writer, (token.value >> 24) as usize);
            }
            0 => group.green.put(writer, token.value as usize),
            CACHED => group.green.put(writer, LITERALS + LENGTHS + token.value as usize),
            length => {
                let (symbol, bits, extra) = prefix(length);
                group.green.put(writer, LITERALS + symbol);
                writer.put(extra, bits);
                let (symbol, bits, extra) = prefix(plane_code(width, token.value as usize));
                group.far.put(writer, symbol);
                writer.put(extra, bits);
            }
        }
        walk.advance(span(token));
    }
}

#[cfg(test)]
mod tests {
    use super::{OFFSETS, plane_code, prefix};

    fn value_of(symbol: usize, extra: u32) -> u32 {
        // The decoder's side of the length and distance prefix coding, as the format defines it.
        if symbol < 4 {
            return symbol as u32 + 1;
        }
        let bits = (symbol as u32 - 2) >> 1;
        ((2 + (symbol as u32 & 1)) << bits) + extra + 1
    }

    #[test]
    fn prefix_codes_decode_back() {
        for value in (1..=4096).chain([5000, 65_536, 300_000, 1_048_456]) {
            let (symbol, bits, extra) = prefix(value);
            assert!(symbol < 40 && extra < (1 << bits).max(1));
            assert_eq!(value_of(symbol, extra), value);
        }
    }

    #[test]
    fn plane_codes_decode_back() {
        for width in [1usize, 2, 3, 7, 8, 9, 15, 16, 17, 100, 594] {
            for distance in 1..(width * 9 + 20) {
                let code = plane_code(width, distance) as usize;
                let decoded = if code > 120 {
                    code as i64 - 120
                } else {
                    let (across, down) = OFFSETS[code - 1];
                    (i64::from(across) + i64::from(down) * width as i64).max(1)
                };
                assert_eq!(decoded, distance as i64, "width {width} distance {distance}");
            }
        }
    }
}
