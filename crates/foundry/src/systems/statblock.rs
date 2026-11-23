#[cfg(feature = "statblock")]
mod draw;
#[cfg(feature = "statblock")]
mod render;
#[cfg(feature = "statblock")]
mod stage;

use crate::Source;
use crate::systems::combat::AbilityItem;

#[cfg(feature = "statblock")]
pub use render::{Fonts, build_statblock_image};
#[cfg(feature = "statblock")]
pub use stage::{StageFonts, build_stage_image, build_stage_pages};
#[cfg(all(feature = "statblock", feature = "disk"))]
pub use stage::{save_stage_to_disk, stage_file_name};
#[cfg(feature = "disk")]
pub use render::save_to_disk;

#[derive(Clone, Debug)]
pub struct SpiritData {
    pub dmg_text: String,
    pub traits: Vec<AbilityItem>,
    pub h1: Vec<AbilityItem>,
    pub h2: Vec<AbilityItem>,
    pub b1: Vec<AbilityItem>,
    pub b2: Vec<AbilityItem>,
    pub footer: Vec<AbilityItem>,
}

#[derive(Clone, Debug)]
pub enum StatCell {
    Text(String),
    Frames(i32),
}

#[derive(Clone, Debug)]
pub struct StatblockData {
    pub is_cat: bool,
    pub id_str: String,
    pub name: String,
    pub icon: Option<Source>,
    pub top_label: String,
    pub top_value: String,
    pub headers_1: Vec<String>,
    pub data_1: Vec<StatCell>,
    pub headers_2: Vec<String>,
    pub data_2: Vec<StatCell>,
    pub traits: Vec<AbilityItem>,
    pub h1: Vec<AbilityItem>,
    pub h2: Vec<AbilityItem>,
    pub b1: Vec<AbilityItem>,
    pub b2: Vec<AbilityItem>,
    pub footer: Vec<AbilityItem>,
    pub spirit_data: Option<SpiritData>,
}

#[derive(Clone, Debug)]
pub struct StagePlate {
    pub image: Option<Source>,
    pub name: String,
}

#[derive(Clone, Copy, Debug)]
pub struct StageCrown {
    pub selected: bool,
    pub enabled: bool,
}

#[derive(Clone, Debug)]
pub struct StageMaterial {
    pub name: String,
    pub icon: Option<Source>,
    pub odds: String,
    pub empty: bool,
}

#[derive(Clone, Debug)]
pub struct StageMaterials {
    pub heading: String,
    pub chunks: Vec<Vec<StageMaterial>>,
}

#[derive(Clone, Debug)]
pub struct StageDrop {
    pub left: String,
    pub icon: Option<Source>,
    pub name: String,
    pub amount: String,
}

#[derive(Clone, Debug)]
pub struct StageDropTable {
    pub title: String,
    pub first_header: String,
    pub rows: Vec<StageDrop>,
}

#[derive(Clone, Debug)]
pub struct StageNote {
    pub title: String,
    pub lines: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct StageSpawn {
    pub icon: Option<Source>,
    pub fallback: String,
    pub cells: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct StageGround {
    pub headers: Vec<(String, u16)>,
    pub spawns: Vec<StageSpawn>,
}

#[derive(Clone, Debug)]
pub struct StageSheet {
    pub id: String,
    pub crown: u8,
    pub map: StagePlate,
    pub stage: StagePlate,
    pub crowns: Vec<StageCrown>,
    pub headers_1: Vec<String>,
    pub values_1: Vec<String>,
    pub headers_2: Vec<String>,
    pub values_2: Vec<String>,
    pub materials: Option<StageMaterials>,
    pub tables: Vec<StageDropTable>,
    pub notes: Vec<StageNote>,
    pub ground: StageGround,
}
