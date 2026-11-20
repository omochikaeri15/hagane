use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::common::file::{self, Separator};

const MAX_ROLLS: f64 = 2_147_483_648.0;

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DropItemError {
    EmptyFile,
}

impl fmt::Display for DropItemError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyFile => write!(
                formatter,
                "The provided byte slice contained no valid drop item data."
            ),
        }
    }
}

impl std::error::Error for DropItemError {}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DropItemEntry {
    pub map_id: u32,
    pub crown_multipliers: [f32; 4],
    pub stage_drops: [f32; 8],
    pub dud_chance: u32,
    pub material_drops: [u32; 16],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DropRoll {
    pub rate: f32,
    pub multiplier: f32,
    pub rolls: u32,
}

impl DropItemEntry {
    pub fn has_materials(&self) -> bool {
        self.material_drops.iter().any(|&weight| weight > 0)
    }

    pub fn roll(&self, stage: usize, crown: usize) -> Option<DropRoll> {
        let rate = *self.stage_drops.get(stage)?;
        let multiplier = *self.crown_multipliers.get(crown)?;
        let rounded = f64::from(multiplier * rate) + 0.5;
        let rolls = if (1.0..MAX_ROLLS).contains(&rounded) { rounded as u32 } else { 0 };

        Some(DropRoll { rate, multiplier, rolls })
    }

    pub fn odds(&self, index: usize) -> f64 {
        let weight = self.material_drops.get(index).copied().unwrap_or(0);
        let total = self.weight_total();

        if weight == 0 || total == 0 {
            return 0.0;
        }

        let landed = f64::from(100 - self.dud_chance.min(100));

        landed * f64::from(weight) / total as f64
    }

    pub fn weight_total(&self) -> u64 {
        self.material_drops.iter().map(|&each| u64::from(each)).sum()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct DropItem {
    pub map_drops: HashMap<u32, DropItemEntry>,
}

impl DropItem {
    pub fn parse<B: AsRef<[u8]>>(bytes: B, separator: Option<Separator>) -> Result<Self, DropItemError> {
        parse_inner(bytes.as_ref(), separator)
    }
}

fn extract_f32_array<const SIZE: usize>(parts: &[&str], start_index: usize) -> Option<[f32; SIZE]> {
    let mut result = [0.0; SIZE];
    for (offset, slot) in result.iter_mut().enumerate() {
        let string_part = parts.get(start_index + offset)?;
        *slot = string_part.trim().parse().ok()?;
    }
    Some(result)
}

fn extract_u32_array<const SIZE: usize>(parts: &[&str], start_index: usize) -> Option<[u32; SIZE]> {
    let mut result = [0; SIZE];
    for (offset, slot) in result.iter_mut().enumerate() {
        let string_part = parts.get(start_index + offset)?;
        *slot = string_part.trim().parse().ok()?;
    }
    Some(result)
}

fn extract_u32_array_optional<const SIZE: usize>(parts: &[&str], start_index: usize) -> [u32; SIZE] {
    let mut result = [0; SIZE];
    for (offset, slot) in result.iter_mut().enumerate() {
        if let Some(string_part) = parts.get(start_index + offset)
            && let Ok(parsed_value) = string_part.trim().parse::<u32>() {
                *slot = parsed_value;
            }
    }
    result
}

fn parse_inner(bytes: &[u8], separator: Option<Separator>) -> Result<DropItem, DropItemError> {
    let file_content = file::scrub(bytes);
    let separator_char = file::resolve(separator, &file_content);

    let mut map_drops = HashMap::new();
    let mut has_content = false;

    for file_line in file_content.lines().skip(1) {
        let mut clean_line = file_line;

        if let Some((before_comment, _)) = file_line.split_once("//") {
            clean_line = before_comment;
        }

        let trimmed_line = clean_line.trim();
        if trimmed_line.is_empty() {
            continue;
        }

        has_content = true;

        let parts: Vec<&str> = trimmed_line.split(separator_char).collect();
        if parts.len() < 22 {
            continue;
        }

        let Some(map_id_string) = parts.first() else { continue; };
        let Ok(map_id) = map_id_string.trim().parse::<u32>() else { continue; };

        let Some(crown_multipliers) = extract_f32_array::<4>(&parts, 1) else { continue; };
        let Some(stage_drops) = extract_f32_array::<8>(&parts, 5) else { continue; };

        let Some(dud_chance_string) = parts.get(13) else { continue; };
        let Ok(dud_chance) = dud_chance_string.trim().parse::<u32>() else { continue; };

        let Some(base_materials) = extract_u32_array::<8>(&parts, 14) else { continue; };
        let z_materials = extract_u32_array_optional::<8>(&parts, 22);

        let mut material_drops = [0; 16];
        material_drops[..8].copy_from_slice(&base_materials);
        material_drops[8..].copy_from_slice(&z_materials);

        map_drops.insert(
            map_id,
            DropItemEntry {
                map_id,
                crown_multipliers,
                stage_drops,
                dud_chance,
                material_drops,
            },
        );
    }

    if !has_content {
        return Err(DropItemError::EmptyFile);
    }

    Ok(DropItem { map_drops })
}
#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: &str = "mapid,star1,star2,star3,star4,stage1,stage2,stage3,stage4,stage5,stage6,stage7,stage8,miss,m1,m2,m3,m4,m5,m6,m7,m8\n";

    fn entry(row: &str) -> DropItemEntry {
        let parsed = DropItem::parse(format!("{HEADER}{row}\n"), None).unwrap();
        parsed.map_drops.into_values().next().unwrap()
    }

    // The engine multiplies in f32, widens to f64, adds a half and truncates.
    #[test]
    fn the_roll_count_rounds_half_up_like_the_engine() {
        let drops = entry("0,0.75,1,1.25,1.5,3,3,3,3,3,3,3,1,60,13,0,13,13,0,0,0,0");

        assert_eq!(drops.roll(0, 0).map(|roll| roll.rolls), Some(2));
        assert_eq!(drops.roll(0, 1).map(|roll| roll.rolls), Some(3));
        assert_eq!(drops.roll(0, 2).map(|roll| roll.rolls), Some(4));
        assert_eq!(drops.roll(0, 3).map(|roll| roll.rolls), Some(5));
        assert_eq!(drops.roll(7, 0).map(|roll| roll.rolls), Some(1));
        assert_eq!(drops.roll(8, 0), None);
        assert_eq!(drops.roll(0, 4), None);
    }

    // The game reads stage rates as floats, so a modded half rate still rolls instead of vanishing.
    #[test]
    fn a_fractional_stage_rate_keeps_its_row() {
        let drops = entry("0,1,1,1,1,1.5,3,3,3,3,3,3,3,60,13,0,13,13,0,0,0,0");

        assert_eq!(drops.roll(0, 0).map(|roll| roll.rolls), Some(2));
        assert!(drops.has_materials());
    }

    // A roll first has to beat the dud chance, then picks a material by its share of the weights.
    #[test]
    fn the_odds_follow_the_engine_not_the_raw_weight() {
        let drops = entry("0,1,1,1,1,3,3,3,3,3,3,3,3,60,13,0,13,13,0,0,0,0");

        assert!((drops.odds(0) - 40.0 * 13.0 / 39.0).abs() < 1e-9);
        assert_eq!(drops.odds(1), 0.0);
        assert_eq!(drops.odds(15), 0.0);
        assert_eq!(drops.odds(99), 0.0);
        assert_eq!(drops.weight_total(), 39);

        let never = entry("0,1,1,1,1,3,3,3,3,3,3,3,3,100,13,0,13,13,0,0,0,0");
        assert_eq!(never.odds(0), 0.0);
    }

    #[test]
    fn a_negative_or_empty_rate_never_rolls() {
        let drops = entry("0,-1,0,1,1,3,0,3,3,3,3,3,3,60,0,0,0,0,0,0,0,0");

        assert_eq!(drops.roll(0, 0).map(|roll| roll.rolls), Some(0));
        assert_eq!(drops.roll(1, 2).map(|roll| roll.rolls), Some(0));
        assert!(!drops.has_materials());
    }
}
