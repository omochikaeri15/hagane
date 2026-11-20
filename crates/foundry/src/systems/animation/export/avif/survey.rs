pub(super) const CLEAR: u8 = 0;
pub(super) const MIXED: u8 = 1;
pub(super) const VOID: u8 = 2;

#[derive(Clone, Copy)]
struct Cell {
    low: [u8; 3],
    high: [u8; 3],
    void: u8,
    still: bool,
}

const BLANK: Cell = Cell { low: [255; 3], high: [0; 3], void: VOID, still: true };

pub(super) struct Survey {
    wide: [usize; 4],
    cells: [Vec<Cell>; 4],
}

fn unchanged(planes: &[Vec<u8>], before: &[Vec<u8>], stride: usize, at: usize) -> bool {
    planes.iter().zip(before).all(|(plane, old)| (0..8).all(|row| plane[at + row * stride..at + row * stride + 8] == old[at + row * stride..at + row * stride + 8]))
}

impl Survey {
    pub(super) fn new(planes: &[Vec<u8>], stride: usize, empty: &[bool], before: Option<&[Vec<u8>]>) -> Self {
        let wide = stride / 8;
        let mut base = vec![BLANK; empty.len()];
        for (index, cell) in base.iter_mut().enumerate().filter(|&(index, _)| !empty[index]) {
            let at = (index / wide) * 8 * stride + (index % wide) * 8;
            cell.void = CLEAR;
            cell.still = before.is_some_and(|old| unchanged(planes, old, stride, at));
            for (plane, (low, high)) in planes.iter().zip(cell.low.iter_mut().zip(&mut cell.high)) {
                for row in 0..8 {
                    for &value in &plane[at + row * stride..at + row * stride + 8] {
                        *low = (*low).min(value);
                        *high = (*high).max(value);
                    }
                }
            }
        }
        let mut survey = Self { wide: [wide, 0, 0, 0], cells: [base, Vec::new(), Vec::new(), Vec::new()] };
        for tier in 1..4 {
            let below = survey.wide[tier - 1];
            let rows = survey.cells[tier - 1].len() / below;
            let (across, down) = (below.div_ceil(2), rows.div_ceil(2));
            let mut cells = vec![BLANK; across * down];
            for (index, cell) in cells.iter_mut().enumerate() {
                let (column, row) = ((index % across) * 2, (index / across) * 2);
                let mut seen = [false; 3];
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    if column + dx < below && row + dy < rows {
                        let child = &survey.cells[tier - 1][(row + dy) * below + column + dx];
                        seen[usize::from(child.void)] = true;
                        cell.still &= child.still;
                        for plane in 0..3 {
                            cell.low[plane] = cell.low[plane].min(child.low[plane]);
                            cell.high[plane] = cell.high[plane].max(child.high[plane]);
                        }
                    }
                }
                cell.void = match seen {
                    [_, false, false] => CLEAR,
                    [false, false, true] => VOID,
                    _ => MIXED,
                };
            }
            survey.wide[tier] = across;
            survey.cells[tier] = cells;
        }
        survey
    }

    fn cell(&self, x4: usize, y4: usize, level: u32) -> &Cell {
        let tier = level.saturating_sub(3) as usize;
        &self.cells[tier][(y4 >> (tier + 1)) * self.wide[tier] + (x4 >> (tier + 1))]
    }

    pub(super) fn swing(&self, x4: usize, y4: usize, level: u32) -> u32 {
        let cell = self.cell(x4, y4, level);
        cell.low.iter().zip(&cell.high).map(|(&low, &high)| u32::from(high.saturating_sub(low))).max().unwrap_or(0)
    }

    pub(super) fn void(&self, x4: usize, y4: usize, level: u32) -> u8 {
        self.cell(x4, y4, level).void
    }

    pub(super) fn active(&self) -> [usize; 4] {
        let wide = self.wide[0];
        let mut region = [usize::MAX, usize::MAX, 0, 0];
        for (index, _) in self.cells[0].iter().enumerate().filter(|(_, cell)| cell.void != VOID && !cell.still) {
            let (x, y) = ((index % wide) * 8, (index / wide) * 8);
            region = [region[0].min(x), region[1].min(y), region[2].max(x + 8), region[3].max(y + 8)];
        }
        if region[0] < region[2] { region } else { [0, 0, 8, 8] }
    }

    pub(super) fn still(&self, x4: usize, y4: usize, level: u32) -> bool {
        self.cell(x4, y4, level).still
    }
}
