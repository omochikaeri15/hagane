mod cdef;
mod coefs;
mod contest;
mod deblock;
mod exact;
mod inter;
mod lossy;
mod stack;
mod strength;

use super::{
    fan::fan,
    model::Model,
    motion::{self, Phases, Plane, Window},
    picture::Veil,
    predict::{self, DC, Edges, LEVEL, Reach, is_angled},
    range::Coder,
    survey::{CLEAR, Survey, VOID},
    tables::{self, AC_QUANT, DC_QUANT, MODE_CONTEXT, SIZE_GROUP},
    transform::{self, COSINE, FLIP, FLIP_ACROSS, FLIP_DOWN, FLIP_SINE, IDENTITY, SINE, SINE_ACROSS, SINE_DOWN, SINE_FLIP},
};

pub(super) use strength::{Canvas, Filters, Guide, settle};

const SPLIT: usize = 3;
const UNSEEN: u8 = 0;
const INTRA: u8 = 1;
const INTER: u8 = 2;
const FRESH: u8 = 3;

pub(super) struct Source<'a> {
    pub planes: &'a [Vec<u8>],
    pub stride: usize,
    pub columns4: usize,
    pub rows4: usize,
    pub width: usize,
    pub height: usize,
}

pub(super) struct Past<'a> {
    pub recon: &'a [Vec<u8>],
    pub phases: Phases,
}

#[derive(Clone, Copy)]
pub(super) struct Tuning {
    pub base_q: u8,
    pub bias: u32,
    pub calm: [u32; 4],
    pub plain: bool,
    pub trial: bool,
    pub modes: usize,
    pub turns: usize,
    pub tints: usize,
    pub shade: i8,
    pub smoothing: f32,
    pub ringing: f32,
    pub reach: i32,
    pub fractions: usize,
    pub eager: bool,
    pub sharp: bool,
    pub contest: bool,
    pub prune: [f32; 2],
    pub threads: usize,
    pub search: usize,
    pub bend: bool,
}

pub(super) struct Scene<'a> {
    pub frame: &'a Source<'a>,
    pub veil: Option<&'a Veil>,
    pub tuning: &'a Tuning,
    pub survey: &'a Survey,
    pub past: Option<&'a Past<'a>>,
}

pub(super) struct Area {
    pub left4: usize,
    pub right4: usize,
    pub top4: usize,
    pub bottom4: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Choice {
    mode: u8,
    turn: i8,
}

const PLAIN: Choice = Choice { mode: DC, turn: LEVEL };

#[derive(Clone, Copy)]
pub(super) struct Cell {
    mv: [i16; 2],
    rank: u8,
    kind: u8,
    skip: bool,
}

pub(super) fn grid(frame: &Source<'_>) -> Vec<Cell> {
    vec![Cell { mv: [0; 2], rank: 0, kind: UNSEEN, skip: false }; frame.columns4 * frame.rows4]
}

struct Draft {
    skip: bool,
    tone: Choice,
    shade: Choice,
    kinds: [u8; 3],
    levels: [[i32; 1024]; 3],
    motion: Option<[i32; 2]>,
}

struct Guesses {
    kept: [[[u8; 1024]; 2]; 3],
    moved: [[u8; 1024]; 3],
}

struct Scratch {
    trial: [u8; 4096],
    residual: [i32; 1024],
    coefs: [i32; 1024],
    other: [i32; 256],
    back: [i32; 256],
    plain: [i32; 256],
    shapes: [[i32; 256]; 3],
    sizes: [u8; 34 * 34],
}

struct Context {
    skip: Vec<u8>,
    mode: Vec<u8>,
    size: Vec<u8>,
    level: [Vec<u8>; 3],
    sign: [Vec<u8>; 3],
}

impl Context {
    fn new(length: usize) -> Self {
        let line = || vec![0u8; length];
        Self { skip: line(), mode: line(), size: line(), level: [line(), line(), line()], sign: [line(), line(), line()] }
    }

    fn clear(&mut self) {
        for plane in 0..3 {
            self.level[plane].fill(0);
            self.sign[plane].fill(0);
        }
    }
}

struct Step {
    size: i32,
    flip: f32,
}

impl Step {
    fn new(size: u16) -> Self {
        Self { size: i32::from(size), flip: (1.0 + 1.0 / 1_048_576.0) / f32::from(size) }
    }
}

struct Tile<'a> {
    frame: &'a Source<'a>,
    veil: Option<&'a Veil>,
    tuning: &'a Tuning,
    survey: &'a Survey,
    recon: &'a mut [Vec<u8>],
    past: Option<&'a Past<'a>>,
    area: Area,
    coder: Coder,
    model: Model,
    above: Context,
    left: Context,
    done: [[bool; 18]; 18],
    corner: (usize, usize),
    dc: [Step; 2],
    ac: [Step; 2],
    grid: &'a mut [Cell],
    hints: [[i32; 2]; 7],
    current: u8,
    aimed: u8,
    deltas: bool,
    draft: Box<Draft>,
    guesses: Box<Guesses>,
    scratch: Box<Scratch>,
    snaps: Box<[Model; 2]>,
    ends: Box<[contest::Ending; 2]>,
}

pub(super) fn encode(scene: &Scene<'_>, recon: &mut [Vec<u8>], grid: &mut [Cell], area: Area, model: Model) -> (Vec<u8>, Model) {
    let (frame, tuning, survey, past, veil) = (scene.frame, scene.tuning, scene.survey, scene.past, scene.veil);
    let (wide, tall) = (area.right4 - area.left4, area.bottom4 - area.top4);
    let tinted = (i32::from(tuning.base_q) + i32::from(tuning.shade)).clamp(0, 255) as u8;
    let mut tile = Tile {
        frame,
        veil,
        tuning,
        survey,
        recon,
        past,
        coder: Coder::new(wide * tall),
        snaps: Box::new([model.clone(), model.clone()]),
        ends: Box::new([contest::Ending::new(&model), contest::Ending::new(&model)]),
        model,
        above: Context::new(frame.columns4),
        left: Context::new(16),
        done: [[false; 18]; 18],
        corner: (0, 0),
        dc: [tuning.base_q, tinted].map(|index| Step::new(DC_QUANT[usize::from(index)])),
        ac: [tuning.base_q, tinted].map(|index| Step::new(AC_QUANT[usize::from(index)])),
        hints: [[0; 2]; 7],
        current: tuning.base_q,
        aimed: tuning.base_q,
        deltas: false,
        grid,
        draft: Box::new(Draft { skip: true, tone: PLAIN, shade: PLAIN, kinds: [COSINE; 3], levels: [[0; 1024]; 3], motion: None }),
        guesses: Box::new(Guesses { kept: [[[0; 1024]; 2]; 3], moved: [[0; 1024]; 3] }),
        scratch: Box::new(Scratch { trial: [0; 4096], residual: [0; 1024], coefs: [0; 1024], other: [0; 256], back: [0; 256], plain: [0; 256], shapes: [[0; 256]; 3], sizes: [0; 34 * 34] }),
        area,
    };
    for y4 in (tile.area.top4..tile.area.bottom4).step_by(16) {
        tile.left.clear();
        for x4 in (tile.area.left4..tile.area.right4).step_by(16) {
            tile.open(x4, y4);
            tile.partition(x4, y4, 6);
        }
    }
    (tile.coder.finish(), tile.model)
}

impl Tile<'_> {
    fn aim(&mut self, veil: &Veil, x4: usize, y4: usize) {
        let factor = veil.factors[(y4 / 16) * veil.across + x4 / 16];
        let wanted = (f32::from(AC_QUANT[usize::from(self.tuning.base_q)]) * factor) as u16;
        self.aimed = if factor == 0.0 { self.current } else { AC_QUANT.partition_point(|&step| step < wanted).clamp(1, 255) as u8 };
        let tinted = (i32::from(self.aimed) + i32::from(self.tuning.shade)).clamp(0, 255) as u8;
        self.dc = [self.aimed, tinted].map(|index| Step::new(DC_QUANT[usize::from(index)]));
        self.ac = [self.aimed, tinted].map(|index| Step::new(AC_QUANT[usize::from(index)]));
        self.deltas = true;
    }

    fn delta(&mut self) {
        let reduced = i32::from(self.aimed) - i32::from(self.current);
        let size = reduced.unsigned_abs();
        self.coder.symbol(size.min(3) as usize, &mut self.model.delta_q);
        if size >= 3 {
            let rest = size - 1;
            let bits = rest.ilog2();
            for shift in (0..3).rev() {
                self.coder.bit(((bits - 1) >> shift) & 1 != 0);
            }
            for shift in (0..bits).rev() {
                self.coder.bit(((rest - (1 << bits)) >> shift) & 1 != 0);
            }
        }
        if size > 0 {
            self.coder.bit(reduced < 0);
        }
        self.current = self.aimed;
    }

    fn open(&mut self, x4: usize, y4: usize) {
        if let Some(veil) = self.veil.filter(|_| self.tuning.base_q > 0) {
            self.aim(veil, x4, y4);
        }
        let (wide, tall) = (self.area.right4 - x4, self.area.bottom4 - y4);
        self.corner = (x4, y4);
        for (row, line) in self.done.iter_mut().enumerate() {
            for (column, cell) in line.iter_mut().enumerate() {
                *cell = (row == 0 && column <= wide) || (column == 0 && row <= tall);
            }
        }
        self.done[17][0] = false;
    }

    fn reach(&self, x4: usize, y4: usize, span: usize) -> Reach {
        let (column, row) = (x4 - self.corner.0, y4 - self.corner.1);
        Reach {
            above: y4 > self.area.top4,
            left: x4 > self.area.left4,
            above_right: self.done[row][column + span + 1],
            below_left: self.done[row + span + 1][column],
        }
    }

    fn mark(&mut self, x4: usize, y4: usize, span: usize) {
        let (column, row) = (x4 - self.corner.0 + 1, y4 - self.corner.1 + 1);
        for line in &mut self.done[row..row + span] {
            line[column..column + span].fill(true);
        }
    }

    fn partition(&mut self, x4: usize, y4: usize, level: u32) {
        let (columns4, rows4) = (self.frame.columns4, self.frame.rows4);
        if x4 >= columns4 || y4 >= rows4 {
            return;
        }
        let span = 1usize << (level - 2);
        let half = span >> 1;
        let has_rows = y4 + half < rows4;
        let has_columns = x4 + half < columns4;
        let fits = y4 + span <= rows4 && x4 + span <= columns4;
        let rank = (level - 2) as u8;
        let above = usize::from(y4 > self.area.top4 && self.above.size[x4] < rank);
        let left = usize::from(x4 > self.area.left4 && self.left.size[y4 & 15] < rank);
        let context = left * 2 + above;
        if fits && self.contested(level) && self.contest(x4, y4, level, context) {
            return;
        }
        let whole = fits && self.settle(x4, y4, level);
        self.announce(level, context, whole, has_rows, has_columns);
        if whole {
            self.leaf(x4, y4, level);
        } else if level == 3 {
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                if self.tuning.base_q == 0 {
                    self.exact(x4 + dx, y4 + dy, 2);
                } else {
                    self.work(x4 + dx, y4 + dy, 2);
                }
                self.leaf(x4 + dx, y4 + dy, 2);
            }
        } else {
            for (dx, dy) in [(0, 0), (half, 0), (0, half), (half, half)] {
                self.partition(x4 + dx, y4 + dy, level - 1);
            }
        }
    }

    fn announce(&mut self, level: u32, context: usize, whole: bool, has_rows: bool, has_columns: bool) {
        if level == 3 {
            self.coder.symbol(if whole { 0 } else { SPLIT }, &mut self.model.partition_8[context]);
        } else {
            let table = match level {
                4 => &mut self.model.partition_16[context],
                5 => &mut self.model.partition_32[context],
                _ => &mut self.model.partition_64[context],
            };
            if has_rows && has_columns {
                self.coder.symbol(if whole { 0 } else { SPLIT }, table);
            } else if has_rows || has_columns {
                let share = |index: usize| u32::from(table[index]) - u32::from(table[index - 1]);
                let sum = if has_columns {
                    share(2) + share(3) + share(4) + share(6) + share(7) + share(9)
                } else {
                    share(1) + share(3) + share(4) + share(5) + share(6) + share(8)
                };
                self.coder.chance(true, 32768 - sum);
            }
        }
    }

    fn settle(&mut self, x4: usize, y4: usize, level: u32) -> bool {
        self.draft.motion = None;
        if self.tuning.base_q == 0 {
            return self.exact(x4, y4, level);
        }
        if self.past.is_some() {
            return self.steer(x4, y4, level, false);
        }
        let void = self.survey.void(x4, y4, level);
        if void == VOID {
            self.flood(x4, y4, level, PLAIN, PLAIN);
            self.draft.skip = true;
            self.draft.tone = PLAIN;
            self.draft.shade = PLAIN;
            return true;
        }
        if void != CLEAR || self.survey.swing(x4, y4, level) > self.tuning.calm[level as usize - 3] {
            return false;
        }
        if level == 6 {
            return self.still(x4, y4);
        }
        self.work(x4, y4, level);
        true
    }

    fn leaf(&mut self, x4: usize, y4: usize, level: u32) {
        let (skip, tone, shade) = (self.draft.skip, self.draft.tone, self.draft.shade);
        self.describe(x4, y4, level, skip, tone, shade);
        if !skip && self.tuning.base_q == 0 {
            let span = 1usize << (level - 2);
            for plane in 0..self.frame.planes.len() {
                for (index, (dx, dy)) in (0..span).flat_map(|dy| (0..span).map(move |dx| (dx, dy))).enumerate() {
                    self.coefficients(plane, x4 + dx, y4 + dy, 4, level == 2, index * 16);
                }
            }
        } else if !skip {
            let size = 1usize << level;
            for plane in 0..self.frame.planes.len() {
                self.coefficients(plane, x4, y4, size, true, 0);
            }
        }

        self.mark(x4, y4, 1 << (level - 2));
    }

    fn describe(&mut self, x4: usize, y4: usize, level: u32, skip: bool, tone: Choice, shade: Choice) {
        let has_above = y4 > self.area.top4;
        let has_left = x4 > self.area.left4;
        let row = y4 & 15;
        let context = usize::from(has_above && self.above.skip[x4] != 0) + usize::from(has_left && self.left.skip[row] != 0);
        self.coder.symbol(usize::from(skip), &mut self.model.skip[context]);
        if self.deltas {
            if !(level == 6 && skip) {
                self.delta();
            }
            self.deltas = false;
        }
        if self.past.is_some() {
            self.intent(x4, y4, level, has_above, has_left);
        } else {
            let above = if has_above { MODE_CONTEXT[usize::from(self.above.mode[x4])] } else { 0 };
            let left = if has_left { MODE_CONTEXT[usize::from(self.left.mode[row])] } else { 0 };
            self.coder.symbol(usize::from(tone.mode), &mut self.model.y_mode[usize::from(above)][usize::from(left)]);
            self.angles(level, tone, shade);
            self.paint(x4, y4, level - 2, [0, 0], INTRA);
        }
        let span = 1usize << (level - 2);
        let rank = (level - 2) as u8;
        self.mute(x4, y4, span, skip);
        self.above.skip[x4..x4 + span].fill(u8::from(skip));
        self.above.mode[x4..x4 + span].fill(tone.mode);
        self.above.size[x4..x4 + span].fill(rank);
        self.left.skip[row..row + span].fill(u8::from(skip));
        self.left.mode[row..row + span].fill(tone.mode);
        self.left.size[row..row + span].fill(rank);
        if skip {
            for plane in 0..self.frame.planes.len() {
                self.above.level[plane][x4..x4 + span].fill(0);
                self.above.sign[plane][x4..x4 + span].fill(0);
                self.left.level[plane][row..row + span].fill(0);
                self.left.sign[plane][row..row + span].fill(0);
            }
        }
    }

    fn angles(&mut self, level: u32, tone: Choice, shade: Choice) {
        if level >= 3 && is_angled(tone.mode) {
            self.coder.symbol((tone.turn + 3) as usize, &mut self.model.angle_delta[usize::from(tone.mode) - 1]);
        }
        if self.frame.planes.len() > 1 {
            if if self.tuning.base_q == 0 { level == 2 } else { level <= 5 } {
                self.coder.symbol(usize::from(shade.mode), &mut self.model.uv_mode_luma[usize::from(tone.mode)]);
            } else {
                self.coder.symbol(usize::from(shade.mode), &mut self.model.uv_mode_plain[usize::from(tone.mode)]);
            }
            if level >= 3 && is_angled(shade.mode) {
                self.coder.symbol((shade.turn + 3) as usize, &mut self.model.angle_delta[usize::from(shade.mode) - 1]);
            }
        }
    }
}
