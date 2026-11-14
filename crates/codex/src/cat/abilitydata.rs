use std::fmt;

use serde::{Deserialize, Serialize};

use crate::common::columns::{self, Column};
use crate::common::file::{self, Separator};

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AbilityDataError {
    EmptyFile,
}

impl fmt::Display for AbilityDataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyFile => write!(f, "The provided file bytes contained no upgrade rows."),
        }
    }
}

impl std::error::Error for AbilityDataError {}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbilityDataRow {
    pub xp_price: i32,
    pub rarity: i32,
    pub level_cap: i32,
    pub plus_cap: i32,
    pub story_level_cap: i32,
    pub rest: Vec<String>,
}

impl AbilityDataRow {
    pub const COLUMNS: &'static [Column<Self>] = columns::columns! {
        xp_price        : 0;
        rarity          : 1;
        level_cap       : 2;
        plus_cap        : 3;
        story_level_cap : 4;
    };

    const WIDTH: usize = 5;
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbilityData {
    pub rows: Vec<AbilityDataRow>,
}

impl AbilityData {
    pub fn parse<B: AsRef<[u8]>>(bytes: B, separator: Option<Separator>) -> Result<Self, AbilityDataError> {
        parse_inner(bytes.as_ref(), separator)
    }

    pub fn caps(&self, kind: usize) -> Option<(i32, i32)> {
        self.rows.get(kind).map(|row| (row.level_cap, row.plus_cap))
    }
}

fn parse_inner(bytes: &[u8], separator: Option<Separator>) -> Result<AbilityData, AbilityDataError> {
    let text = file::scrub(bytes);
    let delimiter = file::resolve(separator, &text);
    let mut rows = Vec::new();

    for line in text.lines() {
        let data = file::uncommented(line).trim();

        if data.is_empty() {
            continue;
        }

        let cells: Vec<&str> = data.split(delimiter).map(str::trim).collect();
        let mut row = AbilityDataRow::default();

        columns::apply(&cells, AbilityDataRow::COLUMNS, &mut row);

        row.rest = cells.iter().skip(AbilityDataRow::WIDTH).map(|cell| (*cell).to_owned()).collect();

        while row.rest.last().is_some_and(String::is_empty) {
            row.rest.pop();
        }

        rows.push(row);
    }

    if rows.is_empty() {
        return Err(AbilityDataError::EmptyFile);
    }

    Ok(AbilityData { rows })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_column_reaches_a_field_of_its_own() {
        columns::assert_one_field_per_column(AbilityDataRow::COLUMNS);
    }

    #[test]
    fn rows_skip_comments_and_keep_their_caps() {
        let data = AbilityData::parse("5000,0,20,10,10,\t\t//attack\n5000,0,10,0,10,\t//range\n\n//price\n", None).unwrap();

        assert_eq!(data.rows.len(), 2);
        assert_eq!(data.caps(0), Some((20, 10)));
        assert_eq!(data.caps(1), Some((10, 0)));
        assert!(data.rows[0].rest.is_empty(), "the trailing comma is not a column");
    }
}
