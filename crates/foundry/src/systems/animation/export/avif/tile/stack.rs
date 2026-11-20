use std::mem;

use super::{Cell, FRESH, INTER, Tile, UNSEEN};

const BONUS: u32 = 640;
const BORDER: i32 = 128;
const DEPTH: usize = 8;

pub(super) struct Stack {
    pub mvs: [[i32; 2]; DEPTH],
    weights: [u32; DEPTH],
    pub found: usize,
    pub fresh_context: usize,
    pub near_context: usize,
    pub drl: [usize; DEPTH],
}

struct Search {
    stack: Stack,
    fresh: usize,
    matched: bool,
}

fn lower(mv: [i16; 2]) -> [i32; 2] {
    mv.map(|value| {
        let value = i32::from(value);
        value - value.signum() * (value & 1)
    })
}

fn sort(stack: &mut Stack, start: usize, end: usize) {
    let mut end = end;
    while end > start {
        let mut last = start;
        for index in start + 1..end {
            if stack.weights[index - 1] < stack.weights[index] {
                stack.weights.swap(index - 1, index);
                stack.mvs.swap(index - 1, index);
                last = index;
            }
        }
        end = last;
    }
}

impl Tile<'_> {
    pub(super) fn cell(&self, x4: usize, y4: usize) -> Cell {
        self.grid[y4 * self.frame.columns4 + x4]
    }

    pub(super) fn paint(&mut self, x4: usize, y4: usize, rank: u32, mv: [i32; 2], kind: u8) {
        let span = 1usize << rank;
        let wide = self.frame.columns4;
        let cell = Cell { mv: mv.map(|value| value as i16), rank: rank as u8, kind, skip: false };
        for row in y4..y4 + span {
            self.grid[row * wide + x4..row * wide + x4 + span].fill(cell);
        }
    }

    pub(super) fn mute(&mut self, x4: usize, y4: usize, span: usize, skip: bool) {
        let wide = self.frame.columns4;
        for row in y4..y4 + span {
            for cell in &mut self.grid[row * wide + x4..row * wide + x4 + span] {
                cell.skip = skip;
            }
        }
    }

    fn inside(&self, x4: isize, y4: isize) -> bool {
        x4 >= self.area.left4 as isize && x4 < self.area.right4 as isize && y4 >= self.area.top4 as isize && y4 < self.area.bottom4 as isize
    }

    fn consider(&self, search: &mut Search, cell: Cell, weight: u32) {
        if cell.kind < INTER {
            return;
        }
        let mv = lower(cell.mv);
        search.fresh += usize::from(cell.kind == FRESH);
        search.matched = true;
        let stack = &mut search.stack;
        if let Some(index) = stack.mvs[..stack.found].iter().position(|&known| known == mv) {
            stack.weights[index] += weight;
        } else if stack.found < DEPTH {
            stack.mvs[stack.found] = mv;
            stack.weights[stack.found] = weight;
            stack.found += 1;
        }
    }

    fn scan_row(&self, search: &mut Search, x4: usize, y4: usize, span: usize, delta: isize) {
        let end = span.min(self.frame.columns4 - x4).min(16);
        let far = delta.abs() > 1;
        let (down, across) = if far { (delta + (y4 & 1) as isize, 1 - (x4 & 1) as isize) } else { (delta, 0) };
        let mut index = 0;
        while index < end {
            let (column, row) = (x4 as isize + across + index as isize, y4 as isize + down);
            if !self.inside(column, row) {
                break;
            }
            let cell = self.cell(column as usize, row as usize);
            let mut length = span.min(1 << cell.rank);
            if far {
                length = length.max(2);
            }
            if span >= 16 {
                length = length.max(4);
            }
            self.consider(search, cell, 2 * length as u32);
            index += length;
        }
    }

    fn scan_column(&self, search: &mut Search, x4: usize, y4: usize, span: usize, delta: isize) {
        let end = span.min(self.frame.rows4 - y4).min(16);
        let far = delta.abs() > 1;
        let (down, across) = if far { (1 - (y4 & 1) as isize, delta + (x4 & 1) as isize) } else { (0, delta) };
        let mut index = 0;
        while index < end {
            let (column, row) = (x4 as isize + across, y4 as isize + down + index as isize);
            if !self.inside(column, row) {
                break;
            }
            let cell = self.cell(column as usize, row as usize);
            let mut length = span.min(1 << cell.rank);
            if far {
                length = length.max(2);
            }
            if span >= 16 {
                length = length.max(4);
            }
            self.consider(search, cell, 2 * length as u32);
            index += length;
        }
    }

    fn scan_point(&self, search: &mut Search, x4: usize, y4: usize, down: isize, across: isize) {
        let (column, row) = (x4 as isize + across, y4 as isize + down);
        if self.inside(column, row) {
            let cell = self.cell(column as usize, row as usize);
            if cell.kind != UNSEEN {
                self.consider(search, cell, 4);
            }
        }
    }

    fn extra(&self, stack: &mut Stack, x4: usize, y4: usize, span: usize) {
        let wide = span.min(16).min(self.frame.columns4 - x4);
        let tall = span.min(16).min(self.frame.rows4 - y4);
        let count = wide.min(tall);
        for pass in 0..2 {
            let mut index = 0;
            while index < count && stack.found < 2 {
                let (column, row) = if pass == 0 { ((x4 + index) as isize, y4 as isize - 1) } else { (x4 as isize - 1, (y4 + index) as isize) };
                if !self.inside(column, row) {
                    break;
                }
                let cell = self.cell(column as usize, row as usize);
                let mv = cell.mv.map(i32::from);
                if cell.kind >= INTER && !stack.mvs[..stack.found].contains(&mv) {
                    stack.mvs[stack.found] = mv;
                    stack.weights[stack.found] = 2;
                    stack.found += 1;
                }
                index += 1 << cell.rank;
            }
        }
        for mv in &mut stack.mvs[stack.found..2] {
            *mv = [0, 0];
        }
    }

    pub(super) fn stack(&self, x4: usize, y4: usize, rank: u32) -> Stack {
        let span = 1usize << rank;
        let empty = Stack { mvs: [[0; 2]; DEPTH], weights: [0; DEPTH], found: 0, fresh_context: 0, near_context: 0, drl: [0; DEPTH] };
        let mut search = Search { stack: empty, fresh: 0, matched: false };
        self.scan_row(&mut search, x4, y4, span, -1);
        let mut above = mem::take(&mut search.matched);
        self.scan_column(&mut search, x4, y4, span, -1);
        let mut left = mem::take(&mut search.matched);
        if span <= 16 {
            self.scan_point(&mut search, x4, y4, -1, span as isize);
        }
        above |= search.matched;
        let close = usize::from(above) + usize::from(left);
        let nearest = search.stack.found;
        let fresh = search.fresh;
        for weight in &mut search.stack.weights[..nearest] {
            *weight += BONUS;
        }
        self.scan_point(&mut search, x4, y4, -1, -1);
        above |= mem::take(&mut search.matched);
        self.scan_row(&mut search, x4, y4, span, -3);
        above |= mem::take(&mut search.matched);
        self.scan_column(&mut search, x4, y4, span, -3);
        left |= mem::take(&mut search.matched);
        if span > 1 {
            self.scan_row(&mut search, x4, y4, span, -5);
            above |= mem::take(&mut search.matched);
            self.scan_column(&mut search, x4, y4, span, -5);
            left |= mem::take(&mut search.matched);
        }
        let total = usize::from(above) + usize::from(left);
        let mut stack = search.stack;
        let found = stack.found;
        sort(&mut stack, 0, nearest);
        sort(&mut stack, nearest, found);
        if found < 2 {
            self.extra(&mut stack, x4, y4, span);
        }
        let found = stack.found;
        for index in 0..found {
            stack.drl[index] = if index + 1 < found {
                if stack.weights[index] >= BONUS { usize::from(stack.weights[index + 1] < BONUS) } else { 2 }
            } else {
                0
            };
        }
        let size = (span * 4) as i32;
        let top = -((y4 * 32) as i32) - BORDER - size * 8;
        let bottom = (self.frame.rows4 as i32 - span as i32 - y4 as i32) * 32 + BORDER + size * 8;
        let left_edge = -((x4 * 32) as i32) - BORDER - size * 8;
        let right_edge = (self.frame.columns4 as i32 - span as i32 - x4 as i32) * 32 + BORDER + size * 8;
        for mv in &mut stack.mvs[..found] {
            mv[0] = mv[0].clamp(top, bottom);
            mv[1] = mv[1].clamp(left_edge, right_edge);
        }
        (stack.fresh_context, stack.near_context) = match close {
            0 => (total.min(1), total),
            1 => (3 - fresh.min(1), 2 + total),
            _ => (5 - fresh.min(1), 5),
        };
        stack
    }
}
