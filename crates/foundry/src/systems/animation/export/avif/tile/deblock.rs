use super::{Cell, INTRA, Source, fan};

struct Strength {
    limit: i32,
    blimit: i32,
    thresh: i32,
}

fn narrow(plane: &mut [u8], at: usize, step: usize, hev: bool) {
    let clamp = |value: i32| value.clamp(-128, 127);
    let read = |plane: &[u8], offset: isize| i32::from(plane[(at as isize + offset * step as isize) as usize]) - 128;
    let (ps1, ps0, qs0, qs1) = (read(plane, -2), read(plane, -1), read(plane, 0), read(plane, 1));
    let base = if hev { clamp(ps1 - qs1) } else { 0 };
    let base = clamp(base + 3 * (qs0 - ps0));
    let first = clamp(base + 4) >> 3;
    let second = clamp(base + 3) >> 3;
    plane[at] = (clamp(qs0 - first) + 128) as u8;
    plane[at - step] = (clamp(ps0 + second) + 128) as u8;
    if !hev {
        let half = (first + 1) >> 1;
        plane[at + step] = (clamp(qs1 - half) + 128) as u8;
        plane[at - 2 * step] = (clamp(ps1 + half) + 128) as u8;
    }
}

fn wide(plane: &mut [u8], at: usize, step: usize, log2: u32, chroma: bool) {
    let taps: isize = if log2 == 4 {
        6
    } else if chroma {
        2
    } else {
        3
    };
    let doubled: isize = if log2 == 3 && !chroma { 0 } else { 1 };
    let mut values = [0i32; 14];
    for offset in -(taps + 1)..=taps {
        values[(offset + 7) as usize] = i32::from(plane[(at as isize + offset * step as isize) as usize]);
    }
    let mut filtered = [0i32; 12];
    for index in -taps..taps {
        let mut total = 0;
        for tap in -taps..=taps {
            let position = (index + tap).clamp(-(taps + 1), taps);
            total += values[(position + 7) as usize] * if tap.abs() <= doubled { 2 } else { 1 };
        }
        filtered[(index + 6) as usize] = (total + (1 << (log2 - 1))) >> log2;
    }
    for index in -taps..taps {
        plane[(at as isize + index * step as isize) as usize] = filtered[(index + 6) as usize] as u8;
    }
}

fn sample(plane: &mut [u8], at: usize, step: usize, size: usize, chroma: bool, strength: &Strength) {
    let length = if size == 4 {
        4
    } else if chroma {
        6
    } else {
        size
    };
    let reach = if size >= 16 { 7 } else { (length / 2).max(2) };
    let mut values = [0i32; 14];
    for (index, value) in values[7 - reach..7 + reach].iter_mut().enumerate() {
        *value = i32::from(plane[at + index * step - reach * step]);
    }
    let first = values[7 - reach];
    if values[7 - reach..7 + reach].iter().all(|&value| value == first) {
        return;
    }
    let (p3, p2, p1, p0, q0, q1, q2, q3) = (values[3], values[4], values[5], values[6], values[7], values[8], values[9], values[10]);
    let near = |a: i32, b: i32| (a - b).abs() <= strength.limit;
    let mut pass = near(p1, p0) && near(q1, q0) && (p0 - q0).abs() * 2 + (p1 - q1).abs() / 2 <= strength.blimit;
    if length >= 6 {
        pass &= near(p2, p1) && near(q2, q1);
    }
    if length >= 8 {
        pass &= near(p3, p2) && near(q3, q2);
    }
    if !pass {
        return;
    }
    let hev = (p1 - p0).abs() > strength.thresh || (q1 - q0).abs() > strength.thresh;
    let level = |a: i32, b: i32| (a - b).abs() <= 1;
    let flat = size >= 8 && level(p1, p0) && level(q1, q0) && level(p2, p0) && level(q2, q0) && (length < 8 || (level(p3, p0) && level(q3, q0)));
    if size == 4 || !flat {
        narrow(plane, at, step, hev);
        return;
    }
    let flatter = size >= 16 && (4..7).all(|far| level(values[6 - far], p0) && level(values[7 + far], q0));
    if size == 8 || !flatter {
        wide(plane, at, step, 3, chroma);
    } else {
        wide(plane, at, step, 4, chroma);
    }
}

pub(super) fn filter(planes: &mut [Vec<u8>], frame: &Source<'_>, grid: &[Cell], levels: [u8; 4], threads: usize) {
    let color = planes.len() > 1;
    fan(planes.iter_mut().enumerate().collect(), threads, |(index, plane)| {
        let chroma = color && index > 0;
        edges(plane, frame, grid, chroma, if chroma { [levels[index + 1]; 2] } else { [levels[0], levels[1]] });
    });
}

pub(super) fn edges(plane: &mut [u8], frame: &Source<'_>, grid: &[Cell], chroma: bool, levels: [u8; 2]) {
    let stride = frame.stride;
    let (columns4, rows4) = (frame.columns4, frame.rows4);
    let rank = |cell: Cell| if chroma { cell.rank.min(3) } else { cell.rank };
    for (pass, &level) in levels.iter().enumerate().filter(|&(_, &level)| level > 0) {
        let level = i32::from(level);
        let strength = Strength { limit: level, blimit: 2 * (level + 2) + level, thresh: level >> 4 };
        for row in 0..rows4 {
            for column in 0..columns4 {
                let (x, y) = (column * 4, row * 4);
                if x >= frame.width || y >= frame.height || (pass == 0 && x == 0) || (pass == 1 && y == 0) {
                    continue;
                }
                let here = grid[row * columns4 + column];
                let before = if pass == 0 { grid[row * columns4 + column - 1] } else { grid[(row - 1) * columns4 + column] };
                let position = if pass == 0 { column } else { row };
                let block_edge = position & ((1 << here.rank) - 1) == 0;
                let transform_edge = position & ((1 << rank(here)) - 1) == 0;
                if !transform_edge || !(block_edge || !here.skip || here.kind == INTRA) {
                    continue;
                }
                let size = (4usize << rank(here).min(rank(before))).min(if chroma { 8 } else { 16 });
                let step = if pass == 0 { 1 } else { stride };
                for offset in 0..4 {
                    let at = if pass == 0 { (y + offset) * stride + x } else { y * stride + x + offset };
                    sample(plane, at, step, size, chroma, &strength);
                }
            }
        }
    }
}
