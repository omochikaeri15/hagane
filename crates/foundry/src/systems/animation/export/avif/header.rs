use super::bits::{BitWriter, leb128};

const SEQUENCE: u8 = 1;
const FRAME: u8 = 6;
const LIMITS: [(u8, u64, u32, u32); 6] = [
    (0, 147_456, 2048, 1152),
    (1, 278_784, 2816, 1584),
    (4, 665_856, 4352, 2448),
    (5, 1_065_024, 5504, 3096),
    (8, 2_359_296, 6144, 3456),
    (12, 8_912_896, 8192, 4352),
];
const TILE_WIDTH: usize = 4096 >> 6;
const TILE_AREA: usize = (4096 * 2304) >> 12;

#[derive(Clone, Copy)]
pub(super) struct Stream {
    pub width: u32,
    pub height: u32,
    pub color: bool,
    pub identity: bool,
}

pub(super) struct Controls {
    pub base_q: u8,
    pub filter: [u8; 4],
    pub sharpness: u8,
    pub ring: Option<[u8; 5]>,
    pub shade: i8,
    pub inter: bool,
    pub reduced: bool,
    pub adaptive: bool,
}

pub(super) struct Tiling {
    pub columns: Vec<usize>,
    pub rows: Vec<usize>,
    column_bits: u32,
    row_bits: u32,
    column_room: bool,
    row_room: bool,
}

pub(super) fn level(width: u32, height: u32) -> u8 {
    let area = u64::from(width) * u64::from(height);
    LIMITS.iter().find(|&&(_, most, wide, tall)| area <= most && width <= wide && height <= tall).map_or(16, |&(index, ..)| index)
}

fn log2_at_least(unit: usize, target: usize) -> u32 {
    (0..).find(|&shift| (unit << shift) >= target).unwrap_or(0)
}

fn starts(count: usize, bits: u32, end: usize) -> Vec<usize> {
    let step = (count + (1 << bits) - 1) >> bits;
    (0..count).step_by(step.max(1)).map(|block| block << 4).chain([end]).collect()
}

pub(super) fn tiling(stream: &Stream) -> Tiling {
    let columns4 = 2 * (stream.width as usize).div_ceil(8);
    let rows4 = 2 * (stream.height as usize).div_ceil(8);
    let (wide, tall) = (columns4.div_ceil(16), rows4.div_ceil(16));
    let column_bits = log2_at_least(TILE_WIDTH, wide);
    let fewest = column_bits.max(log2_at_least(TILE_AREA, wide * tall));
    let row_bits = fewest.saturating_sub(column_bits);
    Tiling {
        columns: starts(wide, column_bits, columns4),
        rows: starts(tall, row_bits, rows4),
        column_bits,
        row_bits,
        column_room: column_bits < log2_at_least(1, wide.min(64)),
        row_room: row_bits < log2_at_least(1, tall.min(64)),
    }
}

fn unit(kind: u8, body: &[u8]) -> Vec<u8> {
    let mut out = vec![(kind << 3) | 2];
    leb128(&mut out, body.len());
    out.extend_from_slice(body);
    out
}

pub(super) fn sequence(stream: &Stream, ring: bool) -> Vec<u8> {
    let mut bits = BitWriter::new();
    let level = level(stream.width, stream.height);
    bits.put(u32::from(stream.color), 3);
    bits.put(0, 2);
    bits.put(0, 2);
    bits.put(0, 5);
    bits.put(0, 12);
    bits.put(u32::from(level), 5);
    if level > 7 {
        bits.put(0, 1);
    }
    let width_bits = (32 - (stream.width - 1).leading_zeros()).max(1);
    let height_bits = (32 - (stream.height - 1).leading_zeros()).max(1);
    bits.put(width_bits - 1, 4);
    bits.put(height_bits - 1, 4);
    bits.put(stream.width - 1, width_bits);
    bits.put(stream.height - 1, height_bits);
    bits.put(0, 1);
    bits.put(0, 3);
    bits.put(0, 5);
    bits.put(0, 2);
    bits.put(0, 1);
    bits.flag(ring);
    bits.put(0, 1);
    bits.put(0, 1);
    if stream.color {
        bits.put(1, 1);
        bits.put(1, 8);
        bits.put(13, 8);
        bits.put(if stream.identity { 0 } else { 6 }, 8);
        if !stream.identity {
            bits.put(1, 1);
        }
        bits.put(0, 1);
    } else {
        bits.put(1, 1);
        bits.put(0, 1);
        bits.put(1, 1);
    }
    bits.put(0, 1);
    bits.put(1, 1);
    unit(SEQUENCE, &bits.finish())
}

pub(super) fn frame(stream: &Stream, controls: &Controls, layout: &Tiling, tiles: &[Vec<u8>]) -> Vec<u8> {
    let mut bits = BitWriter::new();
    bits.put(0, 1);
    bits.put(u32::from(controls.inter), 2);
    bits.put(1, 1);
    if controls.inter {
        bits.put(0, 1);
    }
    bits.put(0, 1);
    bits.put(0, 1);
    if controls.inter {
        bits.put(0, 3);
        bits.put(1, 8);
        bits.put(0, 21);
    }
    bits.put(0, 1);
    if controls.inter {
        bits.put(0, 5);
    }
    bits.put(0, 1);
    bits.put(1, 1);
    if layout.column_room {
        bits.put(0, 1);
    }
    if layout.row_room {
        bits.put(0, 1);
    }
    let many = layout.column_bits + layout.row_bits > 0;
    if many {
        bits.put(0, layout.column_bits + layout.row_bits);
        bits.put(3, 2);
    }
    bits.put(u32::from(controls.base_q), 8);
    bits.put(0, 1);
    if stream.color {
        for _ in 0..2 {
            bits.flag(controls.shade != 0);
            if controls.shade != 0 {
                bits.put(u32::from(controls.shade as u8 & 0x7F), 7);
            }
        }
    }
    bits.put(0, 1);
    bits.put(0, 1);
    if controls.base_q > 0 {
        bits.flag(controls.adaptive);
        if controls.adaptive {
            bits.put(0, 2);
            bits.put(0, 1);
        }
        bits.put(u32::from(controls.filter[0]), 6);
        bits.put(u32::from(controls.filter[1]), 6);
        if stream.color && (controls.filter[0] > 0 || controls.filter[1] > 0) {
            bits.put(u32::from(controls.filter[2]), 6);
            bits.put(u32::from(controls.filter[3]), 6);
        }
        bits.put(u32::from(controls.sharpness), 3);
        bits.put(0, 1);
        if let Some([damping, tone_main, tone_side, shade_main, shade_side]) = controls.ring {
            bits.put(u32::from(damping) - 3, 2);
            bits.put(0, 2);
            bits.put(u32::from(tone_main), 4);
            bits.put(u32::from(tone_side), 2);
            if stream.color {
                bits.put(u32::from(shade_main), 4);
                bits.put(u32::from(shade_side), 2);
            }
        }
        bits.put(0, 1);
    }
    if controls.inter {
        bits.put(0, 1);
    }
    bits.flag(controls.reduced);
    if controls.inter {
        bits.put(0, 7);
    }
    let mut body = bits.finish();
    if many {
        body.push(0);
    }
    for (index, tile) in tiles.iter().enumerate() {
        if index + 1 < tiles.len() {
            body.extend_from_slice(&(tile.len() as u32 - 1).to_le_bytes());
        }
        body.extend_from_slice(tile);
    }
    unit(FRAME, &body)
}

#[cfg(test)]
mod tests {
    use super::{Stream, level, tiling};

    #[test]
    fn level_follows_the_picture_size() {
        assert_eq!(level(82, 100), 0);
        assert_eq!(level(594, 470), 4);
        assert_eq!(level(1270, 970), 8);
        assert_eq!(level(9000, 100), 16);
    }

    #[test]
    fn wide_pictures_split_into_tiles() {
        let small = tiling(&Stream { width: 1270, height: 970, color: true, identity: false });
        assert_eq!((small.columns.len(), small.rows.len()), (2, 2));
        let wide = tiling(&Stream { width: 5000, height: 64, color: true, identity: false });
        assert_eq!(wide.columns, vec![0, 640, 1250]);
        assert_eq!(wide.rows, vec![0, 16]);
    }
}
