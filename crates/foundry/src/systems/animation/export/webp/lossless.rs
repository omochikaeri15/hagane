mod argb;
mod bits;
mod huffman;
mod plane;
mod refs;
mod stream;

use bits::BitWriter;
use stream::Zones;

const SIGNATURE: u32 = 0x2f;
const PREDICTOR: u32 = 0;
const SUBTRACT_GREEN: u32 = 2;
const PLANE_BOUNDS: [u32; 3] = [2, 24, 80];
const COLOR_BOUNDS: [u32; 3] = [8, 60, 200];
const MODES: [u8; 14] = [12, 1, 2, 11, 7, 13, 6, 8, 0, 3, 4, 5, 9, 10];
const QUICK_MODES: usize = 3;
const COLOR_MODES: usize = 5;
const QUICK_EFFORT: u8 = 10;
const SHORT_COPIES: [usize; 3] = [4, 6, 9];
const COARSE_BITS: u32 = 4;
const FINE_BITS: u32 = 3;
const CACHE_EFFORT: u8 = 20;
const CACHE_AREA: usize = 1 << 15;
const CACHE_BITS: u32 = 10;

fn block_bits(effort: u8) -> u32 {
    if effort < QUICK_EFFORT { COARSE_BITS } else { FINE_BITS }
}

fn candidates(effort: u8) -> &'static [u8] {
    if effort < QUICK_EFFORT { &MODES[..QUICK_MODES] } else { &MODES }
}

fn depth(effort: u8) -> usize {
    usize::from(effort) / 8
}

fn predictor(writer: &mut BitWriter, modes: &[u8], width: usize, bits: u32) {
    writer.put(1, 1);
    writer.put(PREDICTOR, 2);
    writer.put(bits - 2, 3);
    writer.put(0, 1);
    let columns = width.div_ceil(1 << bits);
    stream::write(writer, &refs::parse(modes, columns, 0, refs::PLANE_LEAST), columns, false, 0, None);
}

fn zoned(writer: &mut BitWriter, energy: &[u8], width: usize, height: usize, bits: u32) -> (Vec<u8>, usize) {
    let (classes, count) = plane::classify(energy, width, height, bits, &PLANE_BOUNDS);
    let columns = width.div_ceil(1 << bits);
    writer.put(0, 2);
    writer.put(u32::from(count > 1), 1);
    if count > 1 {
        writer.put(bits - 2, 3);
        writer.put(0, 1);
        stream::write(writer, &refs::parse(&classes, columns, 0, refs::PLANE_LEAST), columns, false, 0, None);
    }
    (classes, count)
}

pub(super) fn encode_plane(plane: &[u8], width: usize, height: usize, effort: u8) -> Vec<u8> {
    let block = block_bits(effort);
    let mut best = encode_alpha(plane, width, height, effort, block, refs::PLANE_LEAST);
    if effort < QUICK_EFFORT || plane.len() >= CACHE_AREA {
        return best;
    }
    for (setting, bits) in [(effort, block), (effort, COARSE_BITS), (0, COARSE_BITS)] {
        for least in SHORT_COPIES {
            if (setting, bits, least) == (effort, block, refs::PLANE_LEAST) {
                continue;
            }
            let other = encode_alpha(plane, width, height, setting, bits, least);
            if other.len() < best.len() {
                best = other;
            }
        }
    }
    best
}

fn encode_alpha(plane: &[u8], width: usize, height: usize, effort: u8, block: u32, least: usize) -> Vec<u8> {
    let mut writer = BitWriter::new(plane.len() / 4 + 64);
    let modes = plane::choose(plane, width, height, block, candidates(effort), effort >= QUICK_EFFORT);
    let residual = plane::residuals(plane, width, height, block, &modes);
    predictor(&mut writer, &modes, width, block);
    let tokens = refs::parse(&residual, width, 0, least);
    let mut single = writer.clone();
    single.put(0, 3);
    stream::write(&mut single, &tokens, width, false, 0, None);
    let single = single.finish();
    let (classes, count) = zoned(&mut writer, &residual, width, height, block);
    if count < 2 {
        return single;
    }
    let zones = Zones { classes: &classes, count, bits: block };
    stream::write(&mut writer, &tokens, width, false, 0, Some(&zones));
    let zoned = writer.finish();
    if zoned.len() < single.len() { zoned } else { single }
}

pub(super) fn encode_argb(rgba: &[u8], width: usize, height: usize, effort: u8) -> Vec<u8> {
    let mut pixels: Vec<u32> = rgba.as_chunks::<4>().0.iter().map(|pixel| {
        let green = pixel[1];
        let folded = [pixel[2].wrapping_sub(green), green, pixel[0].wrapping_sub(green), pixel[3]];
        if pixel[3] == 0 { 0 } else { u32::from_le_bytes(folded) }
    }).collect();
    pixels.truncate(width * height);
    let count = if effort < QUICK_EFFORT { QUICK_MODES } else { COLOR_MODES };
    encode_colors(&pixels, width, height, effort, &MODES[..count], block_bits(effort))
}

fn encode_colors(pixels: &[u32], width: usize, height: usize, effort: u8, candidates: &[u8], block: u32) -> Vec<u8> {
    let modes = argb::choose(pixels, width, height, block, candidates);
    let residual = argb::residuals(pixels, width, height, block, &modes);
    let mut writer = BitWriter::new(pixels.len() + 64);
    writer.put(SIGNATURE, 8);
    writer.put(width as u32 - 1, 14);
    writer.put(height as u32 - 1, 14);
    writer.put(1, 4);
    writer.put(1, 1);
    writer.put(SUBTRACT_GREEN, 2);
    predictor(&mut writer, &modes, width, block);
    let bits = if effort >= CACHE_EFFORT && pixels.len() >= CACHE_AREA { CACHE_BITS } else { 0 };
    let mut tokens = refs::parse(&residual, width, depth(effort), refs::COLOR_LEAST);
    writer.put(0, 1);
    writer.put(u32::from(bits > 0), 1);
    if bits > 0 {
        writer.put(bits, 4);
        refs::cached(&mut tokens, &residual, bits);
    }
    let mut single = writer.clone();
    single.put(0, 1);
    stream::write(&mut single, &tokens, width, true, bits, None);
    let single = single.finish();
    let energy: Vec<u8> = residual.iter().map(|&pixel| pixel.to_le_bytes().iter().map(|&byte| u32::from((byte as i8).unsigned_abs())).sum::<u32>().min(255) as u8).collect();
    let (classes, count) = plane::classify(&energy, width, height, block, &COLOR_BOUNDS);
    if count < 2 {
        return single;
    }
    writer.put(1, 1);
    writer.put(block - 2, 3);
    writer.put(0, 1);
    let columns = width.div_ceil(1 << block);
    stream::write(&mut writer, &refs::parse(&classes, columns, 0, refs::PLANE_LEAST), columns, false, 0, None);
    let zones = Zones { classes: &classes, count, bits: block };
    stream::write(&mut writer, &tokens, width, true, bits, Some(&zones));
    let zoned = writer.finish();
    if zoned.len() < single.len() { zoned } else { single }
}
