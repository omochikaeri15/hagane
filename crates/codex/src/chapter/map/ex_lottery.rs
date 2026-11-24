use std::fmt;

use serde::{Deserialize, Serialize};

use crate::common::file::{self, Separator};

const STORY_FIRST: i32 = 3000;
const STORY_CHAPTERS: i32 = 9;
const STORY_SAGA: i32 = 3;
const CATEGORY_SPAN: i32 = 1000;

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExLotteryError {
    EmptyFile,
}

impl fmt::Display for ExLotteryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyFile => write!(formatter, "The provided byte slice contained no valid EX lottery data."),
        }
    }
}

impl std::error::Error for ExLotteryError {}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExLotteryEntry {
    pub map_id: i32,
    pub stage_id: i32,
}

impl ExLotteryEntry {
    pub fn map_index(&self) -> i32 {
        let story = self.map_id.wrapping_sub(STORY_FIRST);

        if (0..STORY_CHAPTERS).contains(&story) {
            return story % STORY_SAGA;
        }

        self.map_id % CATEGORY_SPAN
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExLottery {
    pub entries: Vec<ExLotteryEntry>,
}

impl ExLottery {
    pub fn parse<B: AsRef<[u8]>>(bytes: B, separator: Option<Separator>) -> Result<Self, ExLotteryError> {
        parse_inner(bytes.as_ref(), separator)
    }
}

fn parse_inner(bytes: &[u8], separator: Option<Separator>) -> Result<ExLottery, ExLotteryError> {
    let file_content = file::scrub(bytes);
    let separator_char = file::resolve(separator, &file_content);
    let mut entries = Vec::new();

    for line in file_content.lines() {
        let mut cells = file::uncommented(line).split(separator_char).map(|cell| cell.trim().parse::<i32>().ok());
        let Some(Some(map_id)) = cells.next() else { break };

        entries.push(ExLotteryEntry { map_id, stage_id: cells.next().flatten().unwrap_or_default() });
    }

    if entries.is_empty() {
        return Err(ExLotteryError::EmptyFile);
    }

    Ok(ExLottery { entries })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_keep_their_place_and_stop_at_the_first_that_is_not_a_number() {
        let held = ExLottery::parse(b"4043,0\n4043,2\n4059,1\n//map,stage\n4064,0\n", None).expect("a table");

        assert_eq!(held.entries, [ExLotteryEntry { map_id: 4043, stage_id: 0 }, ExLotteryEntry { map_id: 4043, stage_id: 2 }, ExLotteryEntry { map_id: 4059, stage_id: 1 }]);
        assert_eq!(ExLottery::parse(b"//map,stage\n", None), Err(ExLotteryError::EmptyFile));
    }

    // A row names its map by the game-wide number, and the game keeps only the map's place inside its own category.
    #[test]
    fn a_map_is_reduced_to_its_place_in_its_category() {
        let index = |map_id: i32| ExLotteryEntry { map_id, stage_id: 0 }.map_index();

        assert_eq!((index(4043), index(4000), index(24020)), (43, 0, 20));
        assert_eq!((index(3000), index(3004), index(3008), index(3009)), (0, 1, 2, 9));
    }
}
