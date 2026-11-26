use std::fmt;

use serde::{Deserialize, Serialize};

use crate::common::file::{self, Separator};

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EquipmentAttributeError {
    EmptyFile,
}

impl fmt::Display for EquipmentAttributeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyFile => write!(f, "The provided file bytes contained no Talent Orb trait rows."),
        }
    }
}

impl std::error::Error for EquipmentAttributeError {}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquipmentAttribute {
    pub cells: Vec<Option<i32>>,
}

impl EquipmentAttribute {
    pub fn parse<B: AsRef<[u8]>>(bytes: B, separator: Option<Separator>) -> Result<Vec<Self>, EquipmentAttributeError> {
        parse_inner(bytes.as_ref(), separator)
    }

    pub fn mask(&self) -> u32 {
        self.cells
            .iter()
            .map_while(|cell| *cell)
            .enumerate()
            .filter(|(_, cell)| *cell == 1)
            .fold(0, |mask, (column, _)| mask | 1u32.wrapping_shl(column as u32))
    }
}

fn parse_inner(bytes: &[u8], separator: Option<Separator>) -> Result<Vec<EquipmentAttribute>, EquipmentAttributeError> {
    let file_content = file::scrub(bytes);
    let delimiter = file::resolve(separator, &file_content);

    let rows: Vec<EquipmentAttribute> = file_content
        .lines()
        .skip(1)
        .map(|line| EquipmentAttribute { cells: line.split(delimiter).map(|cell| cell.trim().parse::<i32>().ok()).collect() })
        .collect();

    if rows.is_empty() {
        return Err(EquipmentAttributeError::EmptyFile);
    }

    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_row_turns_its_ones_into_bits() {
        let rows = EquipmentAttribute::parse("//header\n1,0,0\n0,1,1\n0,0,0\n", None).unwrap();

        assert_eq!(rows.iter().map(EquipmentAttribute::mask).collect::<Vec<_>>(), [0b1, 0b110, 0]);
    }

    // The game stops reading a row at its first cell that is not a number.
    #[test]
    fn a_broken_cell_ends_the_row() {
        let rows = EquipmentAttribute::parse("//header\n0,x,1\n", None).unwrap();

        assert_eq!(rows[0].mask(), 0);
        assert_eq!(rows[0].cells, [Some(0), None, Some(1)]);
    }
}
