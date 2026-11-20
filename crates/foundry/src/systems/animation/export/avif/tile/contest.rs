use super::{Cell, Model, Tile, UNSEEN, VOID};

const WEIGHT: f32 = 0.1;

pub(super) struct Mark {
    spot: (u32, u32, i32, usize),
    hints: [[i32; 2]; 7],
    current: u8,
    deltas: bool,
    above: [[u8; 16]; 9],
    left: [[u8; 16]; 9],
}

pub(super) struct Ending {
    mark: Mark,
    model: Model,
    tail: Vec<u16>,
    recon: Vec<u8>,
    cells: Vec<Cell>,
}

impl Ending {
    pub(super) fn new(model: &Model) -> Self {
        let mark = Mark { spot: (0, 0, 0, 0), hints: [[0; 2]; 7], current: 0, deltas: false, above: [[0; 16]; 9], left: [[0; 16]; 9] };
        Self { mark, model: model.clone(), tail: Vec::new(), recon: Vec::with_capacity(3 * 1024), cells: Vec::with_capacity(64) }
    }
}

fn save(lines: [&[u8]; 9], from: usize, span: usize, out: &mut [[u8; 16]; 9]) {
    for (saved, line) in out.iter_mut().zip(lines) {
        saved[..span].copy_from_slice(&line[from..from + span]);
    }
}

impl Tile<'_> {
    pub(super) fn contested(&self, level: u32) -> bool {
        self.tuning.contest && self.past.is_some() && self.tuning.base_q > 0 && (4..=5).contains(&level)
    }

    fn snapshot(&self, x4: usize, y4: usize, level: u32) -> Mark {
        let span = 1usize << (level - 2);
        let row = y4 & 15;
        let mut mark = Mark { spot: self.coder.spot(), hints: self.hints, current: self.current, deltas: self.deltas, above: [[0; 16]; 9], left: [[0; 16]; 9] };
        let (above, left) = (&self.above, &self.left);
        save([&above.skip, &above.mode, &above.size, &above.level[0], &above.level[1], &above.level[2], &above.sign[0], &above.sign[1], &above.sign[2]], x4, span, &mut mark.above);
        save([&left.skip, &left.mode, &left.size, &left.level[0], &left.level[1], &left.level[2], &left.sign[0], &left.sign[1], &left.sign[2]], row, span, &mut mark.left);
        mark
    }

    fn restore(&mut self, mark: &Mark, x4: usize, y4: usize, level: u32) {
        let span = 1usize << (level - 2);
        let row = y4 & 15;
        (self.hints, self.current, self.deltas) = (mark.hints, mark.current, mark.deltas);
        let above = &mut self.above;
        for (saved, line) in mark.above.iter().zip([&mut above.skip, &mut above.mode, &mut above.size]) {
            line[x4..x4 + span].copy_from_slice(&saved[..span]);
        }
        for plane in 0..3 {
            above.level[plane][x4..x4 + span].copy_from_slice(&mark.above[3 + plane][..span]);
            above.sign[plane][x4..x4 + span].copy_from_slice(&mark.above[6 + plane][..span]);
        }
        let left = &mut self.left;
        for (saved, line) in mark.left.iter().zip([&mut left.skip, &mut left.mode, &mut left.size]) {
            line[row..row + span].copy_from_slice(&saved[..span]);
        }
        for plane in 0..3 {
            left.level[plane][row..row + span].copy_from_slice(&mark.left[3 + plane][..span]);
            left.sign[plane][row..row + span].copy_from_slice(&mark.left[6 + plane][..span]);
        }
    }

    fn rewind(&mut self, mark: &Mark, x4: usize, y4: usize, level: u32) {
        let span = 1usize << (level - 2);
        self.coder.rewind(mark.spot);
        self.model.clone_from(&self.snaps[level as usize - 4]);
        self.restore(mark, x4, y4, level);
        let (column, top) = (x4 - self.corner.0 + 1, y4 - self.corner.1 + 1);
        for line in &mut self.done[top..top + span] {
            line[column..column + span].fill(false);
        }
        let wide = self.frame.columns4;
        for line in y4..y4 + span {
            self.grid[line * wide + x4..line * wide + x4 + span].fill(Cell { mv: [0; 2], rank: 0, kind: UNSEEN, skip: false });
        }
    }

    fn hold(&mut self, start: usize, x4: usize, y4: usize, level: u32) {
        let (span, size) = (1usize << (level - 2), 1usize << level);
        let mark = self.snapshot(x4, y4, level);
        let stride = self.frame.stride;
        let wide = self.frame.columns4;
        let ending = &mut self.ends[level as usize - 4];
        ending.mark = mark;
        ending.model.clone_from(&self.model);
        ending.tail.clear();
        ending.tail.extend_from_slice(self.coder.tail(start));
        ending.recon.clear();
        for plane in self.recon.iter() {
            for row in 0..size {
                let at = (y4 * 4 + row) * stride + x4 * 4;
                ending.recon.extend_from_slice(&plane[at..at + size]);
            }
        }
        ending.cells.clear();
        for line in y4..y4 + span {
            ending.cells.extend_from_slice(&self.grid[line * wide + x4..line * wide + x4 + span]);
        }
    }

    fn resume(&mut self, x4: usize, y4: usize, level: u32) {
        let (span, size) = (1usize << (level - 2), 1usize << level);
        let stride = self.frame.stride;
        let wide = self.frame.columns4;
        let ending = &self.ends[level as usize - 4];
        self.coder.resume(ending.mark.spot, &ending.tail);
        self.model.clone_from(&ending.model);
        for (plane, pixels) in self.recon.iter_mut().zip(ending.recon.chunks_exact(size * size)) {
            for (row, line) in pixels.chunks_exact(size).enumerate() {
                let at = (y4 * 4 + row) * stride + x4 * 4;
                plane[at..at + size].copy_from_slice(line);
            }
        }
        for (line, cells) in (y4..y4 + span).zip(ending.cells.chunks_exact(span)) {
            self.grid[line * wide + x4..line * wide + x4 + span].copy_from_slice(cells);
        }
        let (column, top) = (x4 - self.corner.0 + 1, y4 - self.corner.1 + 1);
        for line in &mut self.done[top..top + span] {
            line[column..column + span].fill(true);
        }
        let mark = Mark { ..ending.mark };
        self.restore(&mark, x4, y4, level);
    }

    fn error(&self, x4: usize, y4: usize, level: u32) -> f32 {
        let size = 1usize << level;
        let stride = self.frame.stride;
        let mut error = 0u64;
        for cell in 0..(size / 8) * (size / 8) {
            let (left, top) = ((cell % (size / 8)) * 8, (cell / (size / 8)) * 8);
            if self.survey.void(x4 + left / 4, y4 + top / 4, 3) == VOID {
                continue;
            }
            for (plane, recon) in self.frame.planes.iter().zip(self.recon.iter()) {
                for row in top..top + 8 {
                    let at = (y4 * 4 + row) * stride + x4 * 4 + left;
                    error += plane[at..at + 8].iter().zip(&recon[at..at + 8]).map(|(&a, &b)| u64::from(a.abs_diff(b)).pow(2)).sum::<u64>();
                }
            }
        }
        error as f32
    }

    fn rate(&self, start: f32) -> f32 {
        let step = self.ac[0].size as f32 / 8.0;
        WEIGHT * step * step * (self.coder.tell() - start)
    }

    pub(super) fn contest(&mut self, x4: usize, y4: usize, level: u32, context: usize) -> bool {
        if self.survey.void(x4, y4, level) == VOID || self.survey.still(x4, y4, level) {
            return false;
        }
        let mark = self.snapshot(x4, y4, level);
        self.snaps[level as usize - 4].clone_from(&self.model);
        let start = self.coder.tell();
        self.draft.motion = None;
        let whole = if self.steer(x4, y4, level, true) {
            self.announce(level, context, true, true, true);
            self.leaf(x4, y4, level);
            let cost = self.error(x4, y4, level) + self.rate(start);
            self.hold(mark.spot.3, x4, y4, level);
            cost
        } else {
            f32::MAX
        };
        self.rewind(&mark, x4, y4, level);
        self.announce(level, context, false, true, true);
        let half = 1usize << (level - 3);
        let mut error = 0.0;
        for (dx, dy) in [(0, 0), (half, 0), (0, half), (half, half)] {
            self.partition(x4 + dx, y4 + dy, level - 1);
            error += self.error(x4 + dx, y4 + dy, level - 1);
            if error + self.rate(start) >= whole {
                self.resume(x4, y4, level);
                return true;
            }
        }
        true
    }
}
