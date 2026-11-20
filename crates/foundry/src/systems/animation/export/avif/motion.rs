use super::{fan::fan, tables::SUBPEL_FILTERS};

pub(super) struct Plane<'a> {
    pub data: &'a [u8],
    pub stride: usize,
    pub width: usize,
    pub height: usize,
}

pub(super) fn fetch(plane: &Plane<'_>, x: isize, y: isize, size: usize, out: &mut [u8]) {
    let (width, height) = (plane.width as isize, plane.height as isize);
    if x >= 0 && y >= 0 && x + size as isize <= width && y + size as isize <= height {
        for (row, line) in out.chunks_exact_mut(size).take(size).enumerate() {
            let at = (y as usize + row) * plane.stride + x as usize;
            line.copy_from_slice(&plane.data[at..at + size]);
        }
        return;
    }
    for (row, line) in out.chunks_exact_mut(size).take(size).enumerate() {
        let at = (y + row as isize).clamp(0, height - 1) as usize * plane.stride;
        for (column, value) in line.iter_mut().enumerate() {
            *value = plane.data[at + (x + column as isize).clamp(0, width - 1) as usize];
        }
    }
}

pub(super) struct Window<'a> {
    pub source: &'a [u8],
    pub at: usize,
    pub stride: usize,
}

pub(super) fn distance(plane: &Plane<'_>, x: isize, y: isize, size: usize, window: &Window<'_>) -> u32 {
    let (source, at, stride) = (window.source, window.at, window.stride);
    let (width, height) = (plane.width as isize, plane.height as isize);
    if x >= 0 && y >= 0 && x + size as isize <= width && y + size as isize <= height {
        let mut total = 0u32;
        for row in 0..size {
            let from = (y as usize + row) * plane.stride + x as usize;
            let line = &plane.data[from..from + size];
            let real = &source[at + row * stride..at + row * stride + size];
            total += line.iter().zip(real).map(|(&a, &b)| u32::from(a.abs_diff(b))).sum::<u32>();
        }
        return total;
    }
    let mut total = 0u32;
    for row in 0..size {
        let from = (y + row as isize).clamp(0, height - 1) as usize * plane.stride;
        let real = &source[at + row * stride..at + row * stride + size];
        for (column, &value) in real.iter().enumerate() {
            total += u32::from(plane.data[from + (x + column as isize).clamp(0, width - 1) as usize].abs_diff(value));
        }
    }
    total
}

fn gather(plane: &Plane<'_>, x: isize, y: isize, count: usize, line: &mut [u8]) {
    let (width, height) = (plane.width as isize, plane.height as isize);
    let at = y.clamp(0, height - 1) as usize * plane.stride;
    if x >= 0 && x + count as isize <= width {
        line[..count].copy_from_slice(&plane.data[at + x as usize..at + x as usize + count]);
    } else {
        for (index, value) in line[..count].iter_mut().enumerate() {
            *value = plane.data[at + (x + index as isize).clamp(0, width - 1) as usize];
        }
    }
}

pub(super) fn predict(plane: &Plane<'_>, x: isize, y: isize, size: usize, mv: [i32; 2], out: &mut [u8]) {
    let (left, top) = (x + (mv[1] >> 3) as isize, y + (mv[0] >> 3) as isize);
    let (across, down) = (((mv[1] & 7) * 2) as usize, ((mv[0] & 7) * 2) as usize);
    if across == 0 && down == 0 {
        fetch(plane, left, top, size, out);
        return;
    }
    let set = usize::from(size <= 4);
    let (horizontal, vertical) = (&SUBPEL_FILTERS[set][across], &SUBPEL_FILTERS[set][down]);
    let mut middle = [0i16; 39 * 32];
    let mut line = [0u8; 40];
    let mut wide = [0i16; 40];
    let mut sums = [0i32; 32];
    for (row, slot) in middle.chunks_exact_mut(size).take(size + 7).enumerate() {
        gather(plane, left - 3, top + row as isize - 3, size + 7, &mut line);
        if across == 0 {
            for (value, &pixel) in slot.iter_mut().zip(&line[3..]) {
                *value = i16::from(pixel) << 4;
            }
            continue;
        }
        for (value, &pixel) in wide.iter_mut().zip(&line) {
            *value = i16::from(pixel);
        }
        let sums = &mut sums[..size];
        sums.fill(4);
        for (tap, &weight) in horizontal.iter().enumerate().filter(|&(_, &weight)| weight != 0) {
            for (sum, &pixel) in sums.iter_mut().zip(&wide[tap..tap + size]) {
                *sum += i32::from(weight) * i32::from(pixel);
            }
        }
        for (value, &sum) in slot.iter_mut().zip(sums.iter()) {
            *value = (sum >> 3) as i16;
        }
    }
    for (row, slot) in out.chunks_exact_mut(size).take(size).enumerate() {
        let sums = &mut sums[..size];
        if down == 0 {
            for (value, &middle) in slot.iter_mut().zip(&middle[(row + 3) * size..(row + 4) * size]) {
                *value = ((i32::from(middle) * 128 + 1024) >> 11).clamp(0, 255) as u8;
            }
            continue;
        }
        sums.fill(1024);
        for (tap, &weight) in vertical.iter().enumerate().filter(|&(_, &weight)| weight != 0) {
            for (sum, &value) in sums.iter_mut().zip(&middle[(row + tap) * size..(row + tap + 1) * size]) {
                *sum += i32::from(weight) * i32::from(value);
            }
        }
        for (value, &sum) in slot.iter_mut().zip(sums.iter()) {
            *value = (sum >> 11).clamp(0, 255) as u8;
        }
    }
}

pub(super) struct Phases {
    planes: Vec<Vec<u8>>,
    left: isize,
    top: isize,
    wide: usize,
    tall: usize,
}

const BORDER: isize = 24;

type Band<'a> = (usize, Vec<(usize, &'a mut [i16])>);

impl Phases {
    pub(super) fn new(plane: &Plane<'_>, region: [usize; 4], quarter: bool, threads: usize) -> Self {
        let used = |phase: usize| quarter || phase.is_multiple_of(2);
        let (left, top) = (region[0] as isize - BORDER, region[1] as isize - BORDER);
        let wide = (region[2] - region[0]) + 2 * BORDER as usize;
        let tall = (region[3] - region[1]) + 2 * BORDER as usize;
        let rows = tall + 7;
        let mut middles = vec![vec![0i16; wide * rows]; 4];
        let band = rows.div_ceil(threads.clamp(1, rows));
        let mut bands: Vec<Band<'_>> = (0..rows.div_ceil(band)).map(|index| (index * band, Vec::new())).collect();
        for (phase, middle) in middles.iter_mut().enumerate().filter(|&(phase, _)| used(phase)) {
            for ((_, list), chunk) in bands.iter_mut().zip(middle.chunks_mut(band * wide)) {
                list.push((phase, chunk));
            }
        }
        fan(bands, threads, |(first, mut list)| {
            let mut line = vec![0u8; wide + 7];
            let mut wider = vec![0i16; wide + 7];
            let mut halves = vec![0i16; wide];
            let count = list.first().map_or(0, |(_, chunk)| chunk.len() / wide);
            for local in 0..count {
                gather(plane, left - 3, top + (first + local) as isize - 3, wide + 7, &mut line);
                for (value, &pixel) in wider.iter_mut().zip(&line) {
                    *value = i16::from(pixel);
                }
                for (phase, chunk) in &mut list {
                    let slot = &mut chunk[local * wide..(local + 1) * wide];
                    if *phase == 0 {
                        for (value, &pixel) in slot.iter_mut().zip(&wider[3..]) {
                            *value = pixel << 4;
                        }
                        continue;
                    }
                    halves.fill(2);
                    for (tap, &weight) in SUBPEL_FILTERS[0][*phase * 4].iter().enumerate().filter(|&(_, &weight)| weight != 0) {
                        let half = weight / 2;
                        for (sum, &pixel) in halves.iter_mut().zip(&wider[tap..tap + wide]) {
                            *sum += half * pixel;
                        }
                    }
                    for (value, &sum) in slot.iter_mut().zip(&halves) {
                        *value = sum >> 2;
                    }
                }
            }
        });
        let mut planes = vec![Vec::new(); 16];
        fan(planes.iter_mut().enumerate().filter(|&(index, _)| used(index / 4) && used(index % 4)).collect(), threads, |(index, out)| {
            let (down, across) = (index / 4, index % 4);
            let middle = &middles[across];
            let mut plane = vec![0u8; wide * tall];
            let mut sums = vec![0i32; wide];
            for (row, slot) in plane.chunks_exact_mut(wide).enumerate() {
                if down == 0 {
                    for (value, &middle) in slot.iter_mut().zip(&middle[(row + 3) * wide..(row + 4) * wide]) {
                        *value = ((i32::from(middle) * 128 + 1024) >> 11).clamp(0, 255) as u8;
                    }
                    continue;
                }
                sums.fill(1024);
                for (tap, &weight) in SUBPEL_FILTERS[0][down * 4].iter().enumerate().filter(|&(_, &weight)| weight != 0) {
                    for (sum, &value) in sums.iter_mut().zip(&middle[(row + tap) * wide..(row + tap + 1) * wide]) {
                        *sum += i32::from(weight) * i32::from(value);
                    }
                }
                for (value, &sum) in slot.iter_mut().zip(&sums) {
                    *value = (sum >> 11).clamp(0, 255) as u8;
                }
            }
            *out = plane;
        });
        Self { planes, left, top, wide, tall }
    }

    pub(super) fn distance(&self, x: isize, y: isize, size: usize, mv: [i32; 2], window: &Window<'_>) -> Option<u32> {
        let (source, at, stride) = (window.source, window.at, window.stride);
        let (down, across) = (((mv[0] & 7) / 2) as usize, ((mv[1] & 7) / 2) as usize);
        let (left, top) = (x + (mv[1] >> 3) as isize - self.left, y + (mv[0] >> 3) as isize - self.top);
        if (down == 0 && across == 0) || size <= 4 || mv[0] & 1 != 0 || mv[1] & 1 != 0 || left < 0 || top < 0 || left as usize + size + 1 > self.wide || top as usize + size + 1 > self.tall {
            return None;
        }
        let (left, top) = (left as usize, top as usize);
        let exact = &self.planes[down * 4 + across];
        if !exact.is_empty() {
            return Some(
                (0..size)
                    .map(|row| {
                        let from = (top + row) * self.wide + left;
                        exact[from..from + size].iter().zip(&source[at + row * stride..at + row * stride + size]).map(|(&a, &b)| u32::from(a.abs_diff(b))).sum::<u32>()
                    })
                    .sum(),
            );
        }
        let ends = |phase: usize| if phase.is_multiple_of(2) { [phase, phase] } else { [phase - 1, phase + 1] };
        let mut taps = [(0usize, 0usize); 4];
        for (slot, (vertical, horizontal)) in taps.iter_mut().zip(ends(down).into_iter().flat_map(|vertical| ends(across).into_iter().map(move |horizontal| (vertical, horizontal)))) {
            let index = (vertical % 4) * 4 + horizontal % 4;
            *slot = (index, (top + vertical / 4) * self.wide + left + horizontal / 4);
        }
        let diagonal = down % 2 == 1 && across % 2 == 1;
        let planes = taps.map(|(index, _)| &self.planes[index]);
        Some(
            (0..size)
                .map(|row| {
                    let real = &source[at + row * stride..at + row * stride + size];
                    let lines = [0, 1, 2, 3].map(|tap| &planes[tap][taps[tap].1 + row * self.wide..taps[tap].1 + row * self.wide + size]);
                    if diagonal {
                        (0..size).map(|column| ((lines.iter().map(|line| u32::from(line[column])).sum::<u32>() + 2) >> 2).abs_diff(u32::from(real[column]))).sum::<u32>()
                    } else {
                        lines[0].iter().zip(lines[3]).zip(real).map(|((&a, &c), &b)| ((u32::from(a) + u32::from(c) + 1) >> 1).abs_diff(u32::from(b))).sum::<u32>()
                    }
                })
                .sum(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{Phases, Plane, Window, predict};

    #[test]
    fn phase_planes_match_the_eight_tap_prediction() {
        let (width, height, stride) = (37, 29, 40);
        let mut state = 11u32;
        let data: Vec<u8> = (0..stride * height)
            .map(|_| {
                state = state.wrapping_mul(1_103_515_245).wrapping_add(12345);
                (state >> 16) as u8
            })
            .collect();
        let plane = Plane { data: &data, stride, width, height };
        let phases = Phases::new(&plane, [8, 8, 24, 24], true, 3);
        let source: Vec<u8> = data.iter().map(|value| value.wrapping_mul(7)).collect();
        // Every quarter-pel phase, with blocks that reach past the frame edge.
        for (x, y) in [(8isize, 8isize), (0, 0), (24, 16)] {
            for row in 0..4 {
                for column in 0..4 {
                    let mv = [row * 2 - 8, column * 2 + 6];
                    let mut block = [0u8; 64];
                    predict(&plane, x, y, 8, mv, &mut block);
                    let at = y as usize * stride + x as usize;
                    let expected: u32 = (0..8).map(|line| (0..8).map(|index| u32::from(block[line * 8 + index].abs_diff(source[at + line * stride + index]))).sum::<u32>()).sum();
                    if let Some(found) = phases.distance(x, y, 8, mv, &Window { source: &source, at, stride }) {
                        assert_eq!(found, expected);
                    }
                }
            }
        }
        assert!(phases.distance(8, 8, 8, [2, 2], &Window { source: &source, at: 8 * stride + 8, stride }).is_some());
    }
}
