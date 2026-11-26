#[cfg(feature = "statblock")]
mod board;
#[cfg(feature = "statblock")]
mod details;
#[cfg(feature = "statblock")]
mod draw;
#[cfg(feature = "statblock")]
mod render;
#[cfg(feature = "statblock")]
mod stage;
#[cfg(feature = "statblock")]
mod talents;

use crate::Source;
use crate::systems::combat::{AbilityItem, Mark};

#[cfg(feature = "statblock")]
pub use details::build_details_image;
#[cfg(feature = "statblock")]
pub use render::{Fonts, PageFonts, build_statblock_image};
#[cfg(feature = "statblock")]
pub use talents::build_talents_image;
#[cfg(feature = "statblock")]
pub use stage::{StageFonts, build_stage_image, build_stage_pages};
#[cfg(all(feature = "statblock", feature = "disk"))]
pub use stage::{save_stage_to_disk, stage_file_name};
#[cfg(feature = "disk")]
pub use render::{save_page_to_disk, save_to_disk};

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
    pub marks_1: Vec<Mark>,
    pub marks_2: Vec<Mark>,
    pub traits: Vec<AbilityItem>,
    pub h1: Vec<AbilityItem>,
    pub h2: Vec<AbilityItem>,
    pub b1: Vec<AbilityItem>,
    pub b2: Vec<AbilityItem>,
    pub footer: Vec<AbilityItem>,
    pub spirit_data: Option<SpiritData>,
}

#[derive(Clone, Debug)]
pub struct SheetHead {
    pub id_str: String,
    pub name: String,
    pub icon: Option<Source>,
    pub top: Option<(String, String)>,
}

#[derive(Clone, Debug)]
pub struct EvolveItem {
    pub icon: Option<Source>,
    pub fallback: String,
    pub amount: i32,
}

#[derive(Clone, Debug)]
pub struct EvolveCost {
    pub icon: Option<Source>,
    pub amount: i32,
}

#[derive(Clone, Debug)]
pub struct EvolveData {
    pub lines: Vec<String>,
    pub items: Vec<EvolveItem>,
    pub cost: Option<EvolveCost>,
}

#[derive(Clone, Debug)]
pub struct ComboSeat {
    pub icon: Option<Source>,
    pub label: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ComboCard {
    pub name: String,
    pub effect: String,
    pub note: Option<String>,
    pub seats: Vec<ComboSeat>,
}

#[derive(Clone, Debug)]
pub struct DetailsData {
    pub head: SheetHead,
    pub description: Vec<String>,
    pub evolve: Option<EvolveData>,
    pub combos: Vec<ComboCard>,
}

#[derive(Clone, Debug)]
pub struct TalentCard {
    pub item: AbilityItem,
    pub plate: Option<Source>,
    pub description: String,
    pub ultra: bool,
    pub level: u8,
    pub max_level: u8,
    pub cost: i32,
    pub lines: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct TalentsData {
    pub head: SheetHead,
    pub cost: i32,
    pub cards: Vec<TalentCard>,
}

impl DetailsData {
    pub fn file_name(&self) -> String {
        format!("{}.details.png", self.head.id_str)
    }
}

impl TalentsData {
    pub fn file_name(&self) -> String {
        let level: String = self.head.top.iter().flat_map(|(_, value)| value.chars()).filter(|glyph| glyph.is_alphanumeric() || *glyph == '+').collect();

        format!("{}.Lv{}.talents.png", self.head.id_str, level)
    }
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
pub struct StageContinuation {
    pub chance: String,
    pub plate: StagePlate,
}

#[derive(Clone, Debug)]
pub struct StageNote {
    pub title: String,
    pub lines: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct StageSpawn {
    pub icon: Option<Source>,
    pub name: Option<String>,
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
    pub continuations: Vec<StageContinuation>,
    pub notes: Vec<StageNote>,
    pub ground: StageGround,
}
