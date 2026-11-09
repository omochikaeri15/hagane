use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

const NOTCH_SHARE: f32 = 0.04;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Seeds {
    pub rng: u32,
    pub entropy: u64,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Insets {
    pub side: i32,
    pub top: i32,
    pub bottom: i32,
    pub window_ratio: f32,
    pub tablet: bool,
}

impl Default for Insets {
    fn default() -> Self {
        Self { side: 0, top: 0, bottom: 0, window_ratio: 1.0, tablet: false }
    }
}

impl Insets {
    pub fn in_force(known: Option<Self>, phone: bool, width: f32) -> Self {
        match (known, phone) {
            (Some(known), true) => known,
            (Some(known), false) => Self { window_ratio: known.window_ratio, tablet: true, ..Self::default() },
            (None, true) => Self { side: (width * NOTCH_SHARE).round() as i32, ..Self::default() },
            (None, false) => Self { tablet: true, ..Self::default() },
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
pub struct Screen {
    pub width: f32,
    pub height: f32,
    pub phone: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insets: Option<Insets>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub music: i32,
    pub effects: i32,
    pub two_rows: bool,
    pub vibrate: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(default)]
pub struct Stage {
    pub map_id: i32,
    pub stage: i32,
    pub layout: Option<i32>,
    pub crown: i32,
    pub map_name: String,
    pub stage_name: String,
    pub dungeon_stage: Option<i32>,
    pub variant: Variant,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Variant {
    #[default]
    Plain,
    Outbreak,
    Invasion,
    ZombieInvasion,
    ExInvasion,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(default)]
pub struct Tutorial {
    pub battle_cleared: bool,
    pub deck_seen: bool,
    pub two_rows_seen: bool,
    pub cat_god_seen: bool,
    pub shop_seen: bool,
}

impl Default for Tutorial {
    fn default() -> Self {
        Self { battle_cleared: true, deck_seen: true, two_rows_seen: true, cat_god_seen: true, shop_seen: true }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct Unit {
    pub unit: i32,
    pub form: i32,
    pub level: u32,
    pub plus: u32,
    pub talents: Vec<(i32, i32)>,
    pub orbs: Vec<(i32, i32)>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Level {
    pub level: u32,
    pub plus: u32,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Parts {
    pub cannon: i32,
    pub foundation: i32,
    pub style: i32,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum God {
    Absent,
    #[default]
    Present,
    Discounted,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(default)]
pub struct Jailbreak {
    pub roster: Vec<(bool, i32)>,
    pub bonuses: Vec<(i32, i32)>,
    pub cannon: i32,
    pub registered: Vec<i32>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct Setup {
    pub stage: Stage,
    pub lineup: Vec<Unit>,
    pub tech: Vec<Level>,
    pub treasures: Vec<Vec<i32>>,
    pub cannon: i32,
    pub style: i32,
    pub foundation: i32,
    pub parts: BTreeMap<i32, Parts>,
    pub items: Vec<bool>,
    pub speed_engaged: bool,
    pub altar: Option<i32>,
    pub cat_god: God,
    #[serde(default)]
    pub tutorial: Tutorial,
    #[serde(default)]
    pub war_funds: i32,
    #[serde(default)]
    pub expenses: i32,
    #[serde(default)]
    pub jailbreak: Jailbreak,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Save {
    pub version: String,
    pub app: String,
    pub seeds: Seeds,
    pub screen: Screen,
    pub options: Options,
    pub setup: Setup,
    pub icons: Vec<String>,
    pub costs: Vec<i32>,
    pub altar_cap: Option<i32>,
    pub terminated: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    // A replay from before real insets carries none, and has to keep playing with the share it was recorded under.
    #[test]
    fn a_tape_without_insets_keeps_the_old_notch() {
        let old: Screen = serde_json::from_str(r#"{"width":780.0,"height":360.0,"phone":true}"#).unwrap();

        assert_eq!(old.insets, None);
        assert_eq!(Insets::in_force(old.insets, old.phone, old.width).side, 31);
        assert!(!Insets::in_force(old.insets, old.phone, old.width).tablet);
        assert_eq!(Insets::in_force(None, false, old.width), Insets { tablet: true, ..Insets::default() });
    }

    #[test]
    fn known_insets_win_and_only_a_phone_gets_them() {
        let known = Insets { side: 100, top: 4, bottom: 8, window_ratio: 0.5, tablet: false };

        assert_eq!(Insets::in_force(Some(known), true, 2340.0), known);
        assert_eq!(Insets::in_force(Some(known), false, 2340.0), Insets { window_ratio: 0.5, tablet: true, ..Insets::default() });
    }
}
