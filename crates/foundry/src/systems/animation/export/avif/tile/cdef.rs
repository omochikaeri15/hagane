use std::array;

use super::{Cell, Source};

const DIVISORS: [i64; 9] = [0, 840, 420, 280, 210, 168, 140, 120, 105];
const DIRECTIONS: [[[isize; 2]; 2]; 8] = [
    [[-1, 1], [-2, 2]],
    [[0, 1], [-1, 2]],
    [[0, 1], [0, 2]],
    [[0, 1], [1, 2]],
    [[1, 1], [2, 2]],
    [[1, 0], [2, 1]],
    [[1, 0], [2, 0]],
    [[1, 0], [2, -1]],
];
const PRIMARY: [[i32; 2]; 2] = [[4, 2], [3, 3]];
const SECONDARY: [i32; 2] = [2, 1];

pub(super) struct Ring {
    pub damping: i32,
    pub luma: [i32; 2],
    pub chroma: [i32; 2],
}

struct Strength {
    primary: i32,
    secondary: i32,
    damping: i32,
    direction: usize,
}

fn direction(plane: &[u8], stride: usize, x0: usize, y0: usize) -> (usize, i32) {
    let mut partial = [[0i64; 15]; 8];
    for i in 0..8 {
        for j in 0..8 {
            let x = i64::from(plane[(y0 + i) * stride + x0 + j]) - 128;
            partial[0][i + j] += x;
            partial[1][i + j / 2] += x;
            partial[2][i] += x;
            partial[3][3 + i - j / 2] += x;
            partial[4][7 + i - j] += x;
            partial[5][3 - i / 2 + j] += x;
            partial[6][j] += x;
            partial[7][i / 2 + j] += x;
        }
    }
    let mut cost = [0i64; 8];
    cost[2] = partial[2][..8].iter().map(|value| value * value).sum();
    cost[6] = partial[6][..8].iter().map(|value| value * value).sum();
    cost[2] *= DIVISORS[8];
    cost[6] *= DIVISORS[8];
    for i in 0..7 {
        cost[0] += (partial[0][i] * partial[0][i] + partial[0][14 - i] * partial[0][14 - i]) * DIVISORS[i + 1];
        cost[4] += (partial[4][i] * partial[4][i] + partial[4][14 - i] * partial[4][14 - i]) * DIVISORS[i + 1];
    }
    cost[0] += partial[0][7] * partial[0][7] * DIVISORS[8];
    cost[4] += partial[4][7] * partial[4][7] * DIVISORS[8];
    for i in (1..8).step_by(2) {
        for j in 0..5 {
            cost[i] += partial[i][3 + j] * partial[i][3 + j];
        }
        cost[i] *= DIVISORS[8];
        for j in 0..3 {
            cost[i] += (partial[i][j] * partial[i][j] + partial[i][10 - j] * partial[i][10 - j]) * DIVISORS[2 * j + 2];
        }
    }
    let mut best = (0i64, 0usize);
    for (index, &value) in cost.iter().enumerate() {
        if value > best.0 {
            best = (value, index);
        }
    }
    (best.1, ((best.0 - cost[(best.1 + 4) & 7]) >> 10) as i32)
}

fn constrain(difference: i32, threshold: i32, shift: i32) -> i32 {
    let size = (threshold - (difference.abs() >> shift)).clamp(0, difference.abs());
    if difference < 0 { -size } else { size }
}

fn shift(threshold: i32, damping: i32) -> i32 {
    if threshold == 0 { 0 } else { (damping - threshold.ilog2() as i32).max(0) }
}

fn block(source: &[u8], target: &mut [u8], frame: &Source<'_>, x0: usize, y0: usize, strength: &Strength) {
    let stride = frame.stride;
    let (wide, tall) = (frame.columns4 * 4, frame.rows4 * 4);
    let inside = x0 >= 2 && y0 >= 2 && x0 + 10 <= wide && y0 + 10 <= tall;
    let taps = PRIMARY[(strength.primary & 1) as usize];
    let (primary_shift, secondary_shift) = (shift(strength.primary, strength.damping), shift(strength.secondary, strength.damping));
    if strength.primary == 0 && strength.secondary == 0 {
        return;
    }
    let lines: [[[isize; 2]; 2]; 3] = [
        DIRECTIONS[strength.direction],
        DIRECTIONS[(strength.direction + 6) & 7],
        DIRECTIONS[(strength.direction + 2) & 7],
    ];
    let used = if strength.primary == 0 {
        1..3
    } else if strength.secondary == 0 {
        0..1
    } else {
        0..3
    };
    if inside {
        let weights: [[i16; 2]; 3] = [[taps[0] as i16, taps[1] as i16], [SECONDARY[0] as i16, SECONDARY[1] as i16], [SECONDARY[0] as i16, SECONDARY[1] as i16]];
        let limits = [strength.primary as i16, strength.secondary as i16, strength.secondary as i16];
        let shifts = [primary_shift, secondary_shift, secondary_shift];
        for i in 0..8 {
            let at = (y0 + i) * stride + x0;
            let center: [i16; 8] = array::from_fn(|lane| i16::from(source[at + lane]));
            let (mut sum, mut most, mut least) = ([0i16; 8], center, center);
            for line in used.clone() {
                for k in 0..2 {
                    for sign in [-1isize, 1] {
                        let [down, across] = lines[line][k];
                        let from = (at as isize + sign * (down * stride as isize + across)) as usize;
                        let row = &source[from..from + 8];
                        for lane in 0..8 {
                            let value = i16::from(row[lane]);
                            let difference = value - center[lane];
                            let magnitude = difference.abs();
                            let size = (limits[line] - (magnitude >> shifts[line])).max(0).min(magnitude);
                            let mask = difference >> 15;
                            sum[lane] += weights[line][k] * ((size ^ mask) - mask);
                            most[lane] = most[lane].max(value);
                            least[lane] = least[lane].min(value);
                        }
                    }
                }
            }
            for lane in 0..8 {
                let adjusted = center[lane] + ((8 + sum[lane] - i16::from(sum[lane] < 0)) >> 4);
                target[at + lane] = adjusted.clamp(least[lane], most[lane]) as u8;
            }
        }
        return;
    }
    for i in 0..8 {
        for j in 0..8 {
            let (y, x) = ((y0 + i) as isize, (x0 + j) as isize);
            let center = i32::from(source[(y0 + i) * stride + x0 + j]);
            let (mut sum, mut most, mut least) = (0i32, center, center);
            for k in 0..2 {
                for sign in [-1isize, 1] {
                    for (line, offsets) in lines.iter().enumerate().take(used.end).skip(used.start) {
                        let (down, across) = (y + sign * offsets[k][0], x + sign * offsets[k][1]);
                        if !inside && (down < 0 || across < 0 || down >= tall as isize || across >= wide as isize) {
                            continue;
                        }
                        let value = i32::from(source[down as usize * stride + across as usize]);
                        let difference = value - center;
                        sum += if line == 0 {
                            taps[k] * constrain(difference, strength.primary, primary_shift)
                        } else {
                            SECONDARY[k] * constrain(difference, strength.secondary, secondary_shift)
                        };
                        most = most.max(value);
                        least = least.min(value);
                    }
                }
            }
            let adjusted = center + ((8 + sum - i32::from(sum < 0)) >> 4);
            target[(y0 + i) * stride + x0 + j] = adjusted.clamp(least, most) as u8;
        }
    }
}

pub(super) struct Spot {
    x0: usize,
    y0: usize,
    found: usize,
    spread: Option<i32>,
}

impl Spot {
    pub(super) fn at(&self, stride: usize) -> usize {
        self.y0 * stride + self.x0
    }
}

pub(super) fn spots(luma: &[u8], frame: &Source<'_>, grid: &[Cell]) -> Vec<Spot> {
    let (columns4, rows4) = (frame.columns4, frame.rows4);
    let mut spots = Vec::new();
    for top in (0..rows4).step_by(16) {
        for left in (0..columns4).step_by(16) {
            let active = (top..(top + 16).min(rows4)).any(|row| grid[row * columns4 + left..row * columns4 + (left + 16).min(columns4)].iter().any(|cell| !cell.skip));
            if !active {
                continue;
            }
            for row in (top..(top + 16).min(rows4)).step_by(2) {
                for column in (left..(left + 16).min(columns4)).step_by(2) {
                    let still = [(0, 0), (0, 1), (1, 0), (1, 1)].iter().all(|&(dy, dx)| grid[(row + dy) * columns4 + column + dx].skip);
                    if still {
                        continue;
                    }
                    let (x0, y0) = (column * 4, row * 4);
                    let (found, variance) = direction(luma, frame.stride, x0, y0);
                    let spread = (variance != 0).then(|| if variance >> 6 != 0 { ((variance >> 6).ilog2() as i32).min(12) } else { 0 });
                    spots.push(Spot { x0, y0, found, spread });
                }
            }
        }
    }
    spots
}

pub(super) fn apply(source: &[u8], plane: &mut [u8], frame: &Source<'_>, spots: &[Spot], ring: &Ring, chroma: bool) {
    for spot in spots {
        let strength = if chroma {
            Strength { primary: ring.chroma[0], secondary: ring.chroma[1], damping: ring.damping - 1, direction: if ring.chroma[0] == 0 { 0 } else { spot.found } }
        } else {
            let primary = spot.spread.map_or(0, |spread| (ring.luma[0] * (4 + spread) + 8) >> 4);
            Strength { primary, secondary: ring.luma[1], damping: ring.damping, direction: if ring.luma[0] == 0 { 0 } else { spot.found } }
        };
        block(source, plane, frame, spot.x0, spot.y0, &strength);
    }
}
