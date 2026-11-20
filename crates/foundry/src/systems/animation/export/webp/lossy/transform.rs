const WIDE: i32 = 5352;
const NARROW: i32 = 2217;
const UPPER: i32 = 20091;
const LOWER: i32 = 35468;

#[inline]
pub(super) fn forward(residual: &[i16; 16]) -> [i16; 16] {
    let mut rows = [0i32; 16];
    for (out, line) in rows.as_chunks_mut::<4>().0.iter_mut().zip(residual.as_chunks::<4>().0.iter()) {
        let (a, b, c, d) = (i32::from(line[0]), i32::from(line[1]), i32::from(line[2]), i32::from(line[3]));
        let (outer, inner, narrow, wide) = (a + d, b + c, b - c, a - d);
        out[0] = (outer + inner) * 8;
        out[1] = (wide * WIDE + narrow * NARROW + 256) >> 9;
        out[2] = (outer - inner) * 8;
        out[3] = (wide * NARROW - narrow * WIDE + 256) >> 9;
    }
    let mut out = [0i16; 16];
    for column in 0..4 {
        let (a, b, c, d) = (rows[column], rows[4 + column], rows[8 + column], rows[12 + column]);
        let (outer, inner, narrow, wide) = (a + d, b + c, b - c, a - d);
        out[column] = ((outer + inner + 8) >> 4) as i16;
        out[4 + column] = ((wide * WIDE + narrow * NARROW + 32768) >> 16) as i16;
        out[8 + column] = ((outer - inner + 8) >> 4) as i16;
        out[12 + column] = ((wide * NARROW - narrow * WIDE + 32768) >> 16) as i16;
    }
    out
}

#[inline]
pub(super) fn inverse(coefficients: &[i16; 16]) -> [i16; 16] {
    let mut columns = [0i32; 16];
    for column in 0..4 {
        let (a, b, c, d) = (i32::from(coefficients[column]), i32::from(coefficients[4 + column]), i32::from(coefficients[8 + column]), i32::from(coefficients[12 + column]));
        let (sum, difference) = (a + c, a - c);
        let low = ((b * LOWER) >> 16) - (d + ((d * UPPER) >> 16));
        let high = (b + ((b * UPPER) >> 16)) + ((d * LOWER) >> 16);
        columns[column] = sum + high;
        columns[4 + column] = difference + low;
        columns[8 + column] = difference - low;
        columns[12 + column] = sum - high;
    }
    let mut out = [0i16; 16];
    for (out, line) in out.as_chunks_mut::<4>().0.iter_mut().zip(columns.as_chunks::<4>().0.iter()) {
        let (a, b, c, d) = (line[0], line[1], line[2], line[3]);
        let (sum, difference) = (a + c + 4, a - c + 4);
        let low = ((b * LOWER) >> 16) - (d + ((d * UPPER) >> 16));
        let high = (b + ((b * UPPER) >> 16)) + ((d * LOWER) >> 16);
        out[0] = ((sum + high) >> 3) as i16;
        out[1] = ((difference + low) >> 3) as i16;
        out[2] = ((difference - low) >> 3) as i16;
        out[3] = ((sum - high) >> 3) as i16;
    }
    out
}

#[inline]
pub(super) fn forward_walsh(input: &[i16; 16]) -> [i16; 16] {
    let mut rows = [0i32; 16];
    for (out, line) in rows.as_chunks_mut::<4>().0.iter_mut().zip(input.as_chunks::<4>().0.iter()) {
        let (a, b, c, d) = (i32::from(line[0]), i32::from(line[1]), i32::from(line[2]), i32::from(line[3]));
        out[0] = a + b + c + d;
        out[1] = a + b - c - d;
        out[2] = a - b - c + d;
        out[3] = a - b + c - d;
    }
    let mut out = [0i16; 16];
    for column in 0..4 {
        let (a, b, c, d) = (rows[column], rows[4 + column], rows[8 + column], rows[12 + column]);
        out[column] = ((a + b + c + d + 1) >> 1) as i16;
        out[4 + column] = ((a + b - c - d + 1) >> 1) as i16;
        out[8 + column] = ((a - b - c + d + 1) >> 1) as i16;
        out[12 + column] = ((a - b + c - d + 1) >> 1) as i16;
    }
    out
}

#[inline]
pub(super) fn inverse_walsh(input: &[i16; 16]) -> [i16; 16] {
    let mut columns = [0i32; 16];
    for column in 0..4 {
        let (a, b, c, d) = (i32::from(input[column]), i32::from(input[4 + column]), i32::from(input[8 + column]), i32::from(input[12 + column]));
        let (outer, inner, narrow, wide) = (a + d, b + c, b - c, a - d);
        columns[column] = outer + inner;
        columns[4 + column] = narrow + wide;
        columns[8 + column] = outer - inner;
        columns[12 + column] = wide - narrow;
    }
    let mut out = [0i16; 16];
    for (out, line) in out.as_chunks_mut::<4>().0.iter_mut().zip(columns.as_chunks::<4>().0.iter()) {
        let (a, b, c, d) = (line[0], line[1], line[2], line[3]);
        let (outer, inner, narrow, wide) = (a + d, b + c, b - c, a - d);
        out[0] = ((outer + inner + 3) >> 3) as i16;
        out[1] = ((narrow + wide + 3) >> 3) as i16;
        out[2] = ((outer - inner + 3) >> 3) as i16;
        out[3] = ((wide - narrow + 3) >> 3) as i16;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{forward, forward_walsh, inverse, inverse_walsh};

    #[test]
    fn transforms_invert() {
        // Unquantized, the decoder's inverse must give back what the forward transform took in, within rounding.
        let mut state = 7u32;
        for _ in 0..2000 {
            let block: [i16; 16] = std::array::from_fn(|_| {
                state = state.wrapping_mul(1_103_515_245).wrapping_add(12345);
                ((state >> 16) % 511) as i16 - 255
            });
            let back = inverse(&forward(&block));
            assert!(block.iter().zip(&back).all(|(a, b)| (a - b).abs() <= 1), "{block:?} {back:?}");
            let wide: [i16; 16] = std::array::from_fn(|index| block[index] * 8);
            let back = inverse_walsh(&forward_walsh(&wide));
            assert!(wide.iter().zip(&back).all(|(a, b)| (a - b).abs() <= 1), "{wide:?} {back:?}");
        }
    }
}
