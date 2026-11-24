use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::common::file::{self, Separator};

const FIRST_DRAW: usize = 3;
const KEY_SPAN: i32 = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExGroupError {
    EmptyFile,
}

impl fmt::Display for ExGroupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyFile => write!(formatter, "The provided byte slice contained no valid EX group data."),
        }
    }
}

impl std::error::Error for ExGroupError {}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExDraw {
    pub lottery_row: i32,
    pub threshold: i32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExGroupEntry {
    pub group: i32,
    pub map_id: i32,
    pub stage_id: i32,
    pub draws: Vec<ExDraw>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExGroup {
    pub entries: HashMap<i32, ExGroupEntry>,
}

impl ExGroup {
    pub fn parse<B: AsRef<[u8]>>(bytes: B, separator: Option<Separator>) -> Result<Self, ExGroupError> {
        parse_inner(bytes.as_ref(), separator)
    }

    pub fn key(map_id: i32, stage_id: i32) -> i32 {
        map_id.wrapping_mul(KEY_SPAN).wrapping_add(stage_id)
    }

    pub fn entry(&self, map_id: i32, stage_id: i32) -> Option<&ExGroupEntry> {
        self.entries.get(&Self::key(map_id, stage_id))
    }
}

fn whole(cell: Option<&&str>) -> Option<i32> {
    cell.and_then(|cell| cell.trim().parse().ok())
}

fn parse_inner(bytes: &[u8], separator: Option<Separator>) -> Result<ExGroup, ExGroupError> {
    let file_content = file::scrub(bytes);
    let separator_char = file::resolve(separator, &file_content);
    let mut entries = HashMap::new();

    for line in file_content.lines() {
        let cells: Vec<&str> = file::uncommented(line).split(separator_char).collect();
        let Some(group) = whole(cells.first()) else { break };
        let map_id = whole(cells.get(1)).unwrap_or_default();
        let stage_id = whole(cells.get(2)).unwrap_or_default();
        let mut draws = Vec::new();
        let mut column = FIRST_DRAW;

        while column + 1 < cells.len() {
            let Some(lottery_row) = whole(cells.get(column)) else { break };

            draws.push(ExDraw { lottery_row, threshold: whole(cells.get(column + 1)).unwrap_or_default() });
            column += 2;
        }

        entries.insert(ExGroup::key(map_id, stage_id), ExGroupEntry { group, map_id, stage_id, draws });
    }

    if entries.is_empty() {
        return Err(ExGroupError::EmptyFile);
    }

    Ok(ExGroup { entries })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: &[u8] = b"100,24020,9,0,30\n100,24020,10,1,1,0,34\n100,24020,10,2,5\n//group,map,stage,row,odds\n100,1,1,0,50\n";

    #[test]
    fn draws_are_read_in_pairs_until_the_first_row_that_is_not_a_number() {
        let held = ExGroup::parse(TABLE, None).expect("a table");

        assert_eq!(held.entries.len(), 2, "rows after the comment line are never reached");
        assert_eq!(held.entry(24020, 9).map(|entry| entry.draws.clone()), Some(vec![ExDraw { lottery_row: 0, threshold: 30 }]));
        assert_eq!(held.entry(24020, 10).map(|entry| entry.draws.clone()), Some(vec![ExDraw { lottery_row: 2, threshold: 5 }]), "a later row for the same stage replaces the earlier one");
        assert_eq!(ExGroup::parse(b"100,7,3,4,10,\n", None).expect("a table").entry(7, 3).map(|entry| entry.draws.len()), Some(1));
        assert_eq!(ExGroup::parse(b"100,7,3,4\n", None).expect("a table").entry(7, 3).map(|entry| entry.draws.len()), Some(0), "a row index with no odds after it is not a draw");
        assert_eq!(ExGroup::parse(b"//nothing\n", None), Err(ExGroupError::EmptyFile));
    }
}
