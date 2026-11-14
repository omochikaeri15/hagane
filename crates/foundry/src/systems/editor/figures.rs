use super::{Mark, Reader, Realm};
pub mod combat;
pub mod zombies;
pub mod upgrades;
pub mod unitlevel;
pub mod unitbuy;
pub mod tiprules;
pub mod talents;
pub mod sounds;
pub mod shrine;
pub mod shop;
pub mod shakes;
pub mod schema;
pub mod resolved;
pub mod recipes;
pub mod realms;
pub mod quest;
pub mod progress;
pub mod prizes;
pub mod popups;
pub mod orbtraits;
pub mod orbslots;
pub mod orbgrades;
pub mod missionsettings;
pub mod missions;
pub mod mapsheets;
pub mod mapoption;
pub mod mapdrops;
pub mod mapdata;
pub mod labyrinth;
pub mod itembuy;
pub mod guide;
pub mod gamatoto;
pub mod gacha;
pub mod expcurve;
pub mod exoption;
pub mod exlottery;
pub mod enigma;
pub mod difficulty;
pub mod development;
pub mod cpu;
pub mod costs;
pub mod conditions;
pub mod combos;
pub mod comboparams;
pub mod combocategories;
pub mod club;
pub mod chance;
pub mod castles;
pub mod castlegrowth;
pub mod cards;
pub mod cannonstatus;
pub mod backgrounds;
pub mod altar;
use codex::cat::unit::NyancomboData;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use codex::combat::Separator;
use codex::common;
use tracing::warn;
use crate::common::preview::{self, Stamp};
use crate::domains::{mods, settings::EditorMode};
use crate::Vfs;

use resolved::{Face, Rule};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Subject {
    Cat,
    Enemy,
    Buy,
    Curve,
    Talents,
    Costs,
    Combo,
    MapStage,
    MapDrops,
    DropChara,
    ItemBuy,
    MapOption,
    Background,
    Upgrade,
    ComboParam,
    OrbSlots,
    Difficulty,
    ExInvasion,
    CannonStatus,
    ExpCurve,
    OrbGrades,
    ExLottery,
    Shakes,
    Recipes,
    Shop,
    ZombieHeader,
    ZombieTier,
    Cpu,
    OrbTraits,
    Altar,
    GamatotoLimit,
    GamatotoUnlock,
    GamatotoDrop,
    Stamps,
    SlotUnlocks,
    Club,
    Missions,
    Conditions,
    MissionUnlocks,
    MissionGroups,
    GachaOptions,
    ChanceAnimations,
    Castles,
    TipRules,
    QuestStages,
    SlotLottery,
    RealmAltar,
    RealmDraws,
    LabyrinthFloors,
    LabyrinthLottery,
    MissionShortcuts,
    MissionDesignations,
    MonthlyMissions,
    SoundSettings,
    DojoChests,
    PopupTriggers,
    ComboCategories,
    CastleGrowth,
    DemonCastles,
    DevelopmentCosts,
    DevelopmentTiers,
    MaterialMixes,
    GuideEntries,
    UnitLimits,
    EnemyGuide,
    ExpeditionAreas,
    GamatotoLevels,
    PrizePatterns,
    ShrineOfferings,
    ShrineExtras,
    FortuneOdds,
    ZonedIn,
    ZonedInSteps,
    FilterMaps,
    LineupExclusions,
    EnigmaLotteries,
    EnigmaRates,
    EnigmaDrops,
    EnigmaGroups,
    DiscoveryMedals,
    MapMarkers,
    EnigmaTimes,
    RecommendedUpgrades,
    LineupCounters,
    OrbOptions,
    EnergyRefunds,
    StageShortcuts,
    FanfareTiers,
    GuideSpotlights,
    TipSettings,
    EnigmaRarities,
    EngineerCap,
    LineupRatings,
    LineupBands,
    LineupRankGroups,
    AbilityIcons,
    TraitIcons,
    MenuTextSettings,
    MapSheets,
    OrbFilterChoices,
    ReelSymbols,
    LevelUpOffers,
}

pub const COUNT: usize = 102;

pub const SUBJECTS: [Subject; COUNT] = [
    Subject::Cat,
    Subject::Enemy,
    Subject::Buy,
    Subject::Curve,
    Subject::Talents,
    Subject::Costs,
    Subject::Combo,
    Subject::MapStage,
    Subject::MapDrops,
    Subject::DropChara,
    Subject::ItemBuy,
    Subject::MapOption,
    Subject::Background,
    Subject::Upgrade,
    Subject::ComboParam,
    Subject::OrbSlots,
    Subject::Difficulty,
    Subject::ExInvasion,
    Subject::CannonStatus,
    Subject::ExpCurve,
    Subject::OrbGrades,
    Subject::ExLottery,
    Subject::Shakes,
    Subject::Recipes,
    Subject::Shop,
    Subject::ZombieHeader,
    Subject::ZombieTier,
    Subject::Cpu,
    Subject::OrbTraits,
    Subject::Altar,
    Subject::GamatotoLimit,
    Subject::GamatotoUnlock,
    Subject::GamatotoDrop,
    Subject::Stamps,
    Subject::SlotUnlocks,
    Subject::Club,
    Subject::Missions,
    Subject::Conditions,
    Subject::MissionUnlocks,
    Subject::MissionGroups,
    Subject::GachaOptions,
    Subject::ChanceAnimations,
    Subject::Castles,
    Subject::TipRules,
    Subject::QuestStages,
    Subject::SlotLottery,
    Subject::RealmAltar,
    Subject::RealmDraws,
    Subject::LabyrinthFloors,
    Subject::LabyrinthLottery,
    Subject::MissionShortcuts,
    Subject::MissionDesignations,
    Subject::MonthlyMissions,
    Subject::SoundSettings,
    Subject::DojoChests,
    Subject::PopupTriggers,
    Subject::ComboCategories,
    Subject::CastleGrowth,
    Subject::DemonCastles,
    Subject::DevelopmentCosts,
    Subject::DevelopmentTiers,
    Subject::MaterialMixes,
    Subject::GuideEntries,
    Subject::UnitLimits,
    Subject::EnemyGuide,
    Subject::ExpeditionAreas,
    Subject::GamatotoLevels,
    Subject::PrizePatterns,
    Subject::ShrineOfferings,
    Subject::ShrineExtras,
    Subject::FortuneOdds,
    Subject::ZonedIn,
    Subject::ZonedInSteps,
    Subject::FilterMaps,
    Subject::LineupExclusions,
    Subject::EnigmaLotteries,
    Subject::EnigmaRates,
    Subject::EnigmaDrops,
    Subject::EnigmaGroups,
    Subject::DiscoveryMedals,
    Subject::MapMarkers,
    Subject::EnigmaTimes,
    Subject::RecommendedUpgrades,
    Subject::LineupCounters,
    Subject::OrbOptions,
    Subject::EnergyRefunds,
    Subject::StageShortcuts,
    Subject::FanfareTiers,
    Subject::GuideSpotlights,
    Subject::TipSettings,
    Subject::EnigmaRarities,
    Subject::EngineerCap,
    Subject::LineupRatings,
    Subject::LineupBands,
    Subject::LineupRankGroups,
    Subject::AbilityIcons,
    Subject::TraitIcons,
    Subject::MenuTextSettings,
    Subject::MapSheets,
    Subject::OrbFilterChoices,
    Subject::ReelSymbols,
    Subject::LevelUpOffers,
];

pub(super) const FORMS: [&str; 4] = ["Normal", "Evolved", "True", "Ultra"];

impl Subject {
    pub fn slot(self) -> usize {
        match self {
            Subject::Cat => 0,
            Subject::Enemy => 1,
            Subject::Buy => 2,
            Subject::Curve => 3,
            Subject::Talents => 4,
            Subject::Costs => 5,
            Subject::Combo => 6,
            Subject::MapStage => 7,
            Subject::MapDrops => 8,
            Subject::DropChara => 9,
            Subject::ItemBuy => 10,
            Subject::MapOption => 11,
            Subject::Background => 12,
            Subject::Upgrade => 13,
            Subject::ComboParam => 14,
            Subject::OrbSlots => 15,
            Subject::Difficulty => 16,
            Subject::ExInvasion => 17,
            Subject::CannonStatus => 18,
            Subject::ExpCurve => 19,
            Subject::OrbGrades => 20,
            Subject::ExLottery => 21,
            Subject::Shakes => 22,
            Subject::Recipes => 23,
            Subject::Shop => 24,
            Subject::ZombieHeader => 25,
            Subject::ZombieTier => 26,
            Subject::Cpu => 27,
            Subject::OrbTraits => 28,
            Subject::Altar => 29,
            Subject::GamatotoLimit => 30,
            Subject::GamatotoUnlock => 31,
            Subject::GamatotoDrop => 32,
            Subject::Stamps => 33,
            Subject::SlotUnlocks => 34,
            Subject::Club => 35,
            Subject::Missions => 36,
            Subject::Conditions => 37,
            Subject::MissionUnlocks => 38,
            Subject::MissionGroups => 39,
            Subject::GachaOptions => 40,
            Subject::ChanceAnimations => 41,
            Subject::Castles => 42,
            Subject::TipRules => 43,
            Subject::QuestStages => 44,
            Subject::SlotLottery => 45,
            Subject::RealmAltar => 46,
            Subject::RealmDraws => 47,
            Subject::LabyrinthFloors => 48,
            Subject::LabyrinthLottery => 49,
            Subject::MissionShortcuts => 50,
            Subject::MissionDesignations => 51,
            Subject::MonthlyMissions => 52,
            Subject::SoundSettings => 53,
            Subject::DojoChests => 54,
            Subject::PopupTriggers => 55,
            Subject::ComboCategories => 56,
            Subject::CastleGrowth => 57,
            Subject::DemonCastles => 58,
            Subject::DevelopmentCosts => 59,
            Subject::DevelopmentTiers => 60,
            Subject::MaterialMixes => 61,
            Subject::GuideEntries => 62,
            Subject::UnitLimits => 63,
            Subject::EnemyGuide => 64,
            Subject::ExpeditionAreas => 65,
            Subject::GamatotoLevels => 66,
            Subject::PrizePatterns => 67,
            Subject::ShrineOfferings => 68,
            Subject::ShrineExtras => 69,
            Subject::FortuneOdds => 70,
            Subject::ZonedIn => 71,
            Subject::ZonedInSteps => 72,
            Subject::FilterMaps => 73,
            Subject::LineupExclusions => 74,
            Subject::EnigmaLotteries => 75,
            Subject::EnigmaRates => 76,
            Subject::EnigmaDrops => 77,
            Subject::EnigmaGroups => 78,
            Subject::DiscoveryMedals => 79,
            Subject::MapMarkers => 80,
            Subject::EnigmaTimes => 81,
            Subject::RecommendedUpgrades => 82,
            Subject::LineupCounters => 83,
            Subject::OrbOptions => 84,
            Subject::EnergyRefunds => 85,
            Subject::StageShortcuts => 86,
            Subject::FanfareTiers => 87,
            Subject::GuideSpotlights => 88,
            Subject::TipSettings => 89,
            Subject::EnigmaRarities => 90,
            Subject::EngineerCap => 91,
            Subject::LineupRatings => 92,
            Subject::LineupBands => 93,
            Subject::LineupRankGroups => 94,
            Subject::AbilityIcons => 95,
            Subject::TraitIcons => 96,
            Subject::MenuTextSettings => 97,
            Subject::MapSheets => 98,
            Subject::OrbFilterChoices => 99,
            Subject::ReelSymbols => 100,
            Subject::LevelUpOffers => 101,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Subject::Cat | Subject::Enemy => "Abilities",
            Subject::Buy => "Progression",
            Subject::Curve => "Level Curve",
            Subject::Talents => "Talents",
            Subject::Costs => "Talent Costs",
            Subject::Combo => "Cat Combos",
            Subject::MapStage => "Stage Settings",
            Subject::MapDrops => "Material Drops",
            Subject::DropChara => "Unit Drops",
            Subject::ItemBuy => "Item Catalog",
            Subject::MapOption => "Map Settings",
            Subject::Background => "Background",
            Subject::Upgrade => "Power Up",
            Subject::ComboParam => "Combo Strength",
            Subject::OrbSlots => "Orb Slots",
            Subject::Difficulty => "Difficulty",
            Subject::ExInvasion => "EX Invasion",
            Subject::CannonStatus => "Cannon Status",
            Subject::ExpCurve => "XP Curve",
            Subject::OrbGrades => "Orb Grade",
            Subject::ExLottery => "EX Lottery Group",
            Subject::Shakes => "Screen Shake",
            Subject::Recipes => "Recipe Unlock",
            Subject::Shop => "Shop Slot",
            Subject::ZombieHeader => "Lottery Settings",
            Subject::ZombieTier => "Lottery Tier",
            Subject::Cpu => "Super CPU",
            Subject::OrbTraits => "Orb Trait Targets",
            Subject::Altar => "Altar Reward",
            Subject::GamatotoLimit => "Expedition Limits",
            Subject::GamatotoUnlock => "Gamatoto Unlock",
            Subject::GamatotoDrop => "Special Drop",
            Subject::Stamps => "Login Stamp Day",
            Subject::SlotUnlocks => "Lineup Slot Unlock",
            Subject::Club => "Club Reward",
            Subject::Missions => "Mission Reward",
            Subject::Conditions => "Mission Condition",
            Subject::MissionUnlocks => "Mission Unlock",
            Subject::MissionGroups => "Mission Group",
            Subject::GachaOptions => "Set Options",
            Subject::ChanceAnimations => "Chance Animation",
            Subject::Castles => "Castle Placement",
            Subject::TipRules => "Defeat Tip Rule",
            Subject::QuestStages => "Quest Stage",
            Subject::SlotLottery => "Slot Lottery",
            Subject::RealmAltar => "Realm Lottery Altar",
            Subject::RealmDraws => "Realm Lottery Draw",
            Subject::LabyrinthFloors => "Labyrinth Floor",
            Subject::LabyrinthLottery => "Floor Lottery Rule",
            Subject::MissionShortcuts => "Mission Shortcut",
            Subject::MissionDesignations => "Mission Designation",
            Subject::MonthlyMissions => "Monthly Missions",
            Subject::SoundSettings => "Sound Setting",
            Subject::DojoChests => "Dojo Chest",
            Subject::PopupTriggers => "Popup Trigger",
            Subject::ComboCategories => "Combo Filter Category",
            Subject::CastleGrowth => "Castle Health Bracket",
            Subject::DemonCastles => "Demon Castle",
            Subject::LevelUpOffers => "Level-Up Offer",
            Subject::ReelSymbols => "Reel Symbol",
            Subject::OrbFilterChoices => "Orb Filter Choice",
            Subject::MapSheets => "Map Sheet Placement",
            Subject::MenuTextSettings => "Main Menu Message Setting",
            Subject::TraitIcons => "Trait Icon",
            Subject::AbilityIcons => "Ability Icon",
            Subject::LineupRankGroups => "Lineup Rank Group",
            Subject::LineupBands => "Lineup Cost Band",
            Subject::LineupRatings => "Lineup Rating",
            Subject::EngineerCap => "Engineer Cap",
            Subject::EnigmaRarities => "Enigma Rarity",
            Subject::TipSettings => "Title Tip Setting",
            Subject::GuideSpotlights => "Guide Spotlight",
            Subject::FanfareTiers => "Fanfare Override",
            Subject::StageShortcuts => "Stage Shortcut",
            Subject::EnergyRefunds => "Energy Refund",
            Subject::OrbOptions => "Orb Icon Option",
            Subject::LineupCounters => "Lineup Counter",
            Subject::RecommendedUpgrades => "Recommended Upgrade",
            Subject::EnigmaTimes => "Enigma Map Time",
            Subject::MapMarkers => "Map Marker",
            Subject::DiscoveryMedals => "Discovery Medal",
            Subject::EnigmaGroups => "Stage Discovery",
            Subject::EnigmaDrops => "Discovery Lotteries",
            Subject::EnigmaRates => "Discovery Chances",
            Subject::EnigmaLotteries => "Enigma Lottery",
            Subject::LineupExclusions => "Excluded Enemy",
            Subject::FilterMaps => "Filter Map",
            Subject::ZonedInSteps => "Zoned-In Triggers",
            Subject::ZonedIn => "Zoned-In",
            Subject::FortuneOdds => "Fortune Odds",
            Subject::ShrineExtras => "Offering Bonus",
            Subject::ShrineOfferings => "Shrine Offering",
            Subject::PrizePatterns => "Prize",
            Subject::GamatotoLevels => "Gamatoto Level",
            Subject::ExpeditionAreas => "Expedition Area",
            Subject::EnemyGuide => "Enemy Guide Entry",
            Subject::UnitLimits => "Level Cap Groups",
            Subject::GuideEntries => "Cat Guide Entry",
            Subject::MaterialMixes => "Material Mix",
            Subject::DevelopmentTiers => "Development Tier",
            Subject::DevelopmentCosts => "Development Cost",
        }
    }
}

pub const NEW_COMBO: &str = "New Combo\u{2026}";

pub fn titled(rows: &[NyancomboData], names: &[Option<String>], line: usize) -> String {
    if let Some(name) = spoken(names, line) {
        return name;
    }

    combo_key(rows, line)
}

pub(super) fn combo_key(rows: &[NyancomboData], line: usize) -> String {
    rows.get(line).map_or_else(|| keyed(RETIRED, RETIRED), |row| keyed(row.series, row.combo_id))
}

pub(super) const RETIRED: i32 = -1;

pub(super) fn spoken(names: &[Option<String>], line: usize) -> Option<String> {
    names.get(line).cloned().flatten().filter(|name| !name.trim().is_empty())
}

pub fn keyed(series: i32, combo_id: i32) -> String {
    format!("Combo {series}-{combo_id}")
}

pub(super) const COMMENT: &str = "//";

const BUFFER_MARK: char = '!';

static NEXT_TOKEN: AtomicU64 = AtomicU64::new(0);

fn next_token() -> u64 {
    NEXT_TOKEN.fetch_add(1, Ordering::Relaxed)
}

pub struct Row {
    pub cells: Vec<i32>,
    pub written: Vec<String>,
    pub stored: usize,
    pub comment: String,
}

pub fn split_row(line: &str, delimiter: char, schema: &schema::Schema) -> Row {
    split_span(line, delimiter, schema, 0, schema.known())
}

pub(super) fn split_span(line: &str, delimiter: char, schema: &schema::Schema, first: usize, len: usize) -> Row {
    let (numeric, comment) = match line.find(COMMENT) {
        Some(at) => {
            let (head, tail) = line.split_at(at);

            (head.trim_end(), tail[COMMENT.len()..].trim().to_owned())
        }
        None => (line, String::new()),
    };

    let mut fields: Vec<&str> = numeric.split(delimiter).collect();

    while fields.last().is_some_and(|field| field.trim().is_empty()) {
        fields.pop();
    }

    let stored = fields.len();
    let mut written: Vec<String> = fields.iter().map(|field| (*field).to_owned()).collect();
    let mut cells: Vec<i32> = fields
        .iter()
        .enumerate()
        .map(|(at, field)| parse_scaled(field, schema.decimals(first + at)).unwrap_or(0))
        .collect();

    while cells.len() < len {
        let fallback = schema.fallback(first + cells.len());
        written.push(fallback.to_string());
        cells.push(fallback);
    }

    Row { cells, written, stored, comment }
}

pub struct Slab {
    pub line: usize,
    pub len: usize,
    pub written: Vec<String>,
    pub stored: usize,
    pub touched: usize,
    pub comment: String,
}

fn chrome_rows(
    schema: &schema::Schema,
    lines: &[String],
    delimiter: char,
) -> (Vec<Slab>, Vec<i32>) {
    let mut slabs = Vec::new();
    let mut cells = Vec::new();

    for (line, len) in schema.chrome().iter().copied() {
        let raw = lines.get(line).map(String::as_str).unwrap_or_default();
        let row = split_span(raw, delimiter, schema, cells.len(), len);

        cells.extend(row.cells);
        slabs.push(Slab {
            line,
            len,
            written: row.written,
            stored: row.stored,
            touched: 0,
            comment: row.comment,
        });
    }

    (slabs, cells)
}

pub fn vacant(plan: &Plan, lines: &[String], delimiter: char) -> String {
    let mut fields: Vec<String> =
        (0..plan.schema.known()).map(|index| plan.schema.fallback(index).to_string()).collect();

    if let Some(id) = plan.address.key()
        && let Some(first) = fields.first_mut()
    {
        *first = id.to_string();
    }

    if plan.schema.subject() == Subject::Combo {
        combos::seed(&mut fields, plan.schema, plan.anchor);
        combos::seed_key(&mut fields, plan.schema, lines, delimiter);
    }

    if plan.schema.subject() == Subject::OrbSlots {
        fields.truncate(orbslots::SEEDED);
    }

    fields.join(&delimiter.to_string())
}

pub fn shown(schema: &schema::Schema, index: usize, raw: i32, values: EditorMode, rule: Rule) -> String {
    if raw == schema.fallback(index) {
        return String::new();
    }

    let held = rule.to_display(schema.to_display(index, raw, values), values);

    format_scaled(held, schema.decimals(index))
}

pub fn typable(value: &str, signed: bool, places: u32) -> bool {
    typable_digits(value.strip_prefix(BUFFER_MARK).unwrap_or(value), signed, places)
}

fn typable_digits(value: &str, signed: bool, places: u32) -> bool {
    let body = match value.strip_prefix('-') {
        Some(_) if !signed => return false,
        Some(rest) => rest,
        None => value,
    };

    let mut points = 0;

    for glyph in body.chars() {
        if glyph == '.' && places > 0 {
            points += 1;

            if points > 1 {
                return false;
            }

            continue;
        }

        if !glyph.is_ascii_digit() {
            return false;
        }
    }

    true
}

fn decimal_unit(places: u32) -> i32 {
    10_i32.saturating_pow(places)
}

pub fn parse_scaled(text: &str, places: u32) -> Option<i32> {
    let text = text.trim();

    if places == 0 {
        return text.parse::<i32>().ok();
    }

    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    let negative = whole.starts_with('-');

    let held: String = fraction.chars().take(places as usize).collect();
    let part: i32 = format!("{:0<width$}", held, width = places as usize).parse().ok()?;

    let whole: i32 = match whole {
        "" | "-" => 0,
        held => held.parse().ok()?,
    };

    let magnitude = whole.checked_abs()?.checked_mul(decimal_unit(places))?.checked_add(part)?;

    Some(if negative { -magnitude } else { magnitude })
}

pub fn format_scaled(value: i32, places: u32) -> String {
    if places == 0 {
        return value.to_string();
    }

    let unit = decimal_unit(places);
    let whole = value / unit;
    let part = (value % unit).abs();

    if part == 0 {
        return whole.to_string();
    }

    let digits = format!("{:0width$}", part, width = places as usize);
    let digits = digits.trim_end_matches('0');
    let sign = if value < 0 && whole == 0 { "-" } else { "" };

    format!("{sign}{whole}.{digits}")
}

pub fn wrapped_lines(label: &str, per_line: usize) -> usize {
    let mut lines = 1;
    let mut used = 0;

    for word in label.split_whitespace() {
        let length = word.chars().count();

        if used == 0 {
            used = length;
            continue;
        }

        if used + 1 + length <= per_line {
            used += 1 + length;
            continue;
        }

        lines += 1;
        used = length;
    }

    lines
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    Sync,
    Remove,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Address {
    Line(usize),
    Keyed(u32),
    Column { at: usize, id: u32 },
    Appended,
}

impl Address {
    pub fn locate(self, lines: &[String], delimiter: char) -> Option<usize> {
        match self {
            Address::Line(row) => Some(row),
            Address::Keyed(id) => lines.iter().position(|line| leading(line, delimiter) == Some(id)),
            Address::Column { at, id } => {
                lines.iter().position(|line| cell_of(line, delimiter, at) == Some(id))
            }
            Address::Appended => None,
        }
    }

    pub fn insertion(self, lines: &[String], delimiter: char) -> Option<usize> {
        let id = match self {
            Address::Appended => return Some(lines.len()),
            Address::Keyed(id) => id,
            Address::Line(_) | Address::Column { .. } => return None,
        };

        let at = lines
            .iter()
            .position(|line| leading(line, delimiter).is_some_and(|other| other > id))
            .unwrap_or(lines.len());

        Some(at)
    }

    pub fn nearest(self, lines: &[String], delimiter: char) -> Option<usize> {
        self.key()?;

        lines.iter().position(|line| leading(line, delimiter).is_some())
    }

    pub fn key(self) -> Option<u32> {
        match self {
            Address::Keyed(id) => Some(id),
            Address::Line(_) | Address::Column { .. } | Address::Appended => None,
        }
    }
}

pub fn leading(line: &str, delimiter: char) -> Option<u32> {
    cell_of(line, delimiter, 0)
}

fn cell_of(line: &str, delimiter: char, at: usize) -> Option<u32> {
    line.split(delimiter).nth(at)?.trim().parse().ok()
}

#[derive(Clone)]
pub struct Plan {
    pub address: Address,
    pub labels: Option<Arc<[String]>>,
    pub label: String,
    pub game: PathBuf,
    pub target_mod: Option<String>,
    pub schema: &'static schema::Schema,
    pub values: EditorMode,
    pub anchor: Option<(i32, i32)>,
}

impl Plan {
    pub fn subject(&self) -> Subject {
        self.schema.subject()
    }

    pub fn matches(&self, other: &Plan) -> bool {
        self.address == other.address
            && self.labels == other.labels
            && self.game == other.game
            && self.target_mod == other.target_mod
            && self.label == other.label
            && self.values == other.values
            && self.anchor == other.anchor
    }

    pub fn rowed(self, address: Address) -> Plan {
        Plan { address, ..self }
    }

    pub fn anchored(self, anchor: Option<(i32, i32)>) -> Plan {
        Plan { anchor, ..self }
    }

    pub fn named(self, labels: Vec<String>) -> Plan {
        Plan { labels: Some(labels.into()), ..self }
    }

    pub fn source(&self, vfs: &Vfs) -> PathBuf {
        self.target_mod
            .as_deref()
            .and_then(|name| mods::find(vfs, name, &self.game))
            .unwrap_or_else(|| self.game.clone())
    }
}

pub type Marks = [u8; schema::TALENT_SLOTS];

pub struct Draft {
    pub plan: Plan,
    pub chrome: Vec<Slab>,
    pub absent: bool,
    pub row: usize,
    pub read_from: PathBuf,
    pub stamp: Stamp,
    pub delimiter: char,
    pub keys: Vec<u32>,
    pub keyed: Option<u32>,
    pub lines: Vec<String>,
    pub cells: Vec<i32>,
    pub written: Vec<String>,
    pub stored: usize,
    pub touched: usize,
    pub rules: Vec<Rule>,
    pub comment: String,
    pub inputs: Vec<String>,
    pub failed: bool,
    pub buffer: Option<usize>,
    pub dirty: bool,
    pub writing: bool,
    pub token: u64,
}

pub fn write_now(path: &Path, body: &[u8], stamp: Stamp) -> Option<Stamp> {
    preview::save(path, body, stamp)
        .inspect_err(|err| warn!(path = %path.display(), "Stats editor could not write the file: {}", err))
        .ok()
}

pub(super) fn located(plan: &Plan, lines: &[String], delimiter: char) -> Option<usize> {
    plan.address
        .locate(lines, delimiter)
        .or_else(|| plan.schema.switches().then(|| plan.address.nearest(lines, delimiter)).flatten())
}

pub fn entry(plan: &Plan, body: &str) -> Option<String> {
    let delimiter = Separator::detect(body).unwrap_or(Separator::Comma).char();
    let lines: Vec<String> = body.lines().map(str::to_owned).collect();
    let row = located(plan, &lines, delimiter)?;

    let mut held: Vec<&str> =
        plan.schema.chrome().iter().filter_map(|(line, _)| lines.get(*line).map(String::as_str)).collect();

    held.push(lines.get(row)?);

    Some(held.join("\n"))
}

pub fn reset(plan: Plan, vfs: &Vfs) -> bool {
    let Some(mut draft) = Draft::load(plan, vfs) else {
        return false;
    };

    draft.sync();

    if draft.failed {
        return false;
    }

    draft.persist_now(vfs);

    !draft.failed
}

impl Draft {
    pub fn load(plan: Plan, vfs: &Vfs) -> Option<Draft> {
        let read_from = plan.source(vfs);

        let bytes = fs::read(&read_from)
            .inspect_err(|err| warn!(path = %read_from.display(), "Stats editor could not read the file: {}", err))
            .ok()?;

        let Some(stamp) = preview::stamp(&read_from) else {
            warn!(path = %read_from.display(), "Stats editor could not stamp the file");

            return None;
        };

        let body = common::scrub(&bytes);
        let delimiter = Separator::detect(&body).unwrap_or(Separator::Comma).char();
        let lines: Vec<String> = body.lines().map(str::to_owned).collect();

        let switches = plan.schema.switches();
        let keys: Vec<u32> = match switches {
            true => lines.iter().filter_map(|line| leading(line, delimiter)).collect(),
            false => Vec::new(),
        };

        let found = located(&plan, &lines, delimiter);
        let absent = found.is_none();

        let Some(row) = found.or_else(|| plan.address.insertion(&lines, delimiter)) else {
            warn!(path = %read_from.display(), rows = lines.len(), "Stats editor found no row for this subject");

            return None;
        };

        let blank = vacant(&plan, &lines, delimiter);
        let raw = match lines.get(row).filter(|_| !absent) {
            Some(line) => line.as_str(),
            None if plan.schema.creates() => blank.as_str(),
            None => {
                warn!(
                    path = %read_from.display(),
                    row,
                    rows = lines.len(),
                    "Stats editor found no row for this form"
                );

                return None;
            }
        };

        let keyed = switches.then(|| lines.get(row).and_then(|line| leading(line, delimiter))).flatten();

        let (chrome, lead) = chrome_rows(plan.schema, &lines, delimiter);
        let span = plan.schema.known() - lead.len();
        let Row { mut cells, written, stored, comment } =
            split_span(raw, delimiter, plan.schema, lead.len(), span);

        let mut held = lead;
        held.append(&mut cells);
        let cells = held;

        let rules: Vec<Rule> = (0..cells.len())
            .map(|index| resolved::rule(plan.subject(), index, plan.schema.field(index), &cells))
            .collect();
        let inputs = (0..cells.len())
            .map(|index| shown(plan.schema, index, cells[index], plan.values, rules[index]))
            .collect();

        Some(Draft {
            plan,
            chrome,
            absent,
            row,
            read_from,
            stamp,
            delimiter,
            keys,
            keyed,
            lines,
            cells,
            written,
            stored,
            touched: 0,
            rules,
            comment,
            inputs,
            failed: false,
            buffer: None,
            dirty: false,
            writing: false,
            token: next_token(),
        })
    }

    pub fn edit(&mut self, index: usize, value: &str) {
        if !self.write(index, value) {
            return;
        }

        if let Some(twin) = self.mirror(index) {
            self.write(twin, value);
        }

        self.stage();
    }

    pub fn resolve_buffer(&mut self) {
        let Some(index) = self.buffer.take() else {
            return;
        };

        let Some(typed) = self.inputs.get(index).and_then(|slot| slot.strip_prefix(BUFFER_MARK))
        else {
            return;
        };

        let typed = typed.to_owned();

        self.edit(index, &typed);
    }

    pub fn buffering(&self, index: usize) -> bool {
        self.buffer == Some(index)
    }

    pub fn mirror(&self, index: usize) -> Option<usize> {
        if self.plan.values != EditorMode::Resolved
            || self.plan.subject() != Subject::Talents
        {
            return None;
        }

        talents::mirror(index, &self.cells)
    }

    pub fn write(&mut self, index: usize, value: &str) -> bool {
        let rule = self.rule(index);
        let values = self.plan.values;

        let places = self.plan.schema.decimals(index);

        if !typable(value, rule.signed(values), places) {
            return false;
        }

        let buffering = value.starts_with(BUFFER_MARK);

        let Some(slot) = self.inputs.get_mut(index) else {
            return false;
        };

        if buffering {
            *slot = value.to_owned();
            self.buffer = Some(index);

            return false;
        }

        if self.buffer == Some(index) {
            self.buffer = None;
        }

        let raw = if value.is_empty() {
            *slot = String::new();
            self.plan.schema.fallback(index)
        } else {
            let Some(display) = parse_scaled(value, places) else {
                *slot = value.to_owned();

                return false;
            };

            let unshifted = rule.to_raw(rule.clamp(display, values), values);
            let raw = self.plan.schema.to_raw(index, unshifted, values);
            *slot = shown(self.plan.schema, index, raw, values, rule);

            raw
        };

        let Some(cell) = self.cells.get_mut(index) else {
            return false;
        };

        if *cell == raw {
            return false;
        }

        *cell = raw;
        self.record(index, raw);

        true
    }

    pub fn record(&mut self, index: usize, raw: i32) {
        let mut first = 0;

        for slab in &mut self.chrome {
            if index < first + slab.len {
                let at = index - first;

                if let Some(slot) = slab.written.get_mut(at) {
                    *slot = format_scaled(raw, self.plan.schema.decimals(index));
                }

                slab.touched = slab.touched.max(at + 1);

                return;
            }

            first += slab.len;
        }

        let at = index - first;

        if let Some(slot) = self.written.get_mut(at) {
            *slot = format_scaled(raw, self.plan.schema.decimals(index));
        }

        self.touched = self.touched.max(at + 1);
    }

    pub fn lead(&self) -> usize {
        self.chrome.iter().map(|slab| slab.len).sum()
    }

    fn restage_chrome(&mut self) {
        for slab in &self.chrome {
            let width = slab.stored.max(slab.touched);
            let mut line = slab.written[..width].join(&self.delimiter.to_string());

            if !slab.comment.is_empty() {
                line.push(self.delimiter);
                line.push(' ');
                line.push_str(COMMENT);
                line.push(' ');
                line.push_str(&slab.comment);
            }

            if let Some(slot) = self.lines.get_mut(slab.line) {
                *slot = line;
            }
        }
    }

    pub fn pick(&mut self, index: usize, raw: i32) {
        if self.cells.get(index) == Some(&raw) {
            return;
        }

        let Some(cell) = self.cells.get_mut(index) else {
            return;
        };

        *cell = raw;
        self.record(index, raw);

        let display = shown(self.plan.schema, index, raw, self.plan.values, self.rule(index));

        if let Some(slot) = self.inputs.get_mut(index) {
            *slot = display;
        }

        self.stage();
    }

    pub fn set_comment(&mut self, value: String) {
        if self.comment == value {
            return;
        }

        self.comment = value;
        self.stage();
    }

    pub fn sync(&mut self) {
        let Ok(bytes) = fs::read(&self.plan.game) else {
            warn!(path = %self.plan.game.display(), "Stats editor could not read the vanilla file");
            self.failed = true;

            return;
        };

        let body = common::scrub(&bytes);
        let delimiter = Separator::detect(&body).unwrap_or(Separator::Comma).char();
        let vanilla: Vec<String> = body.lines().map(str::to_owned).collect();

        let blank = vacant(&self.plan, &vanilla, delimiter);
        let located = self.address().locate(&vanilla, delimiter).and_then(|row| vanilla.get(row));

        let raw = match located {
            Some(line) => line.as_str(),
            None if self.plan.schema.creates() => blank.as_str(),
            None => {
                self.failed = true;

                return;
            }
        };

        let (chrome, lead) = chrome_rows(self.plan.schema, &vanilla, delimiter);
        let span = self.plan.schema.known() - lead.len();
        let Row { mut cells, written, stored, comment } =
            split_span(raw, delimiter, self.plan.schema, lead.len(), span);

        let mut held = lead;
        held.append(&mut cells);

        self.chrome = chrome;
        self.cells = held;
        self.written = written;
        self.stored = stored;
        self.touched = 0;
        self.comment = comment;

        self.inputs = (0..self.cells.len())
            .map(|index| shown(self.plan.schema, index, self.cells[index], self.plan.values, self.rule(index)))
            .collect();
        self.buffer = None;
        self.stage();
    }

    pub fn stage(&mut self) {
        self.restage_chrome();

        let width = self.stored.max(self.touched);
        let mut line = self.written[..width].join(&self.delimiter.to_string());

        if !self.comment.is_empty() {
            line.push(self.delimiter);
            line.push(' ');
            line.push_str(COMMENT);
            line.push(' ');
            line.push_str(&self.comment);
        }

        self.restate();

        let empty = self.plan.schema.vacant(&self.cells);

        if self.absent {
            if empty {
                return;
            }

            self.row = self.plan.address.insertion(&self.lines, self.delimiter).unwrap_or(self.row);
            self.lines.insert(self.row, line);
            self.absent = false;
            self.plan = self.plan.clone().rowed(Address::Line(self.row));
        } else if empty && self.plan.schema.creates() {
            self.lines.remove(self.row);
            self.absent = true;
        } else {
            let Some(slot) = self.lines.get_mut(self.row) else {
                self.failed = true;

                return;
            };

            *slot = line;
        }

        self.dirty = true;
    }

    pub fn prepare_write(&mut self, vfs: &Vfs) -> Option<(PathBuf, Vec<u8>, Stamp, u64)> {
        let Some((path, stamp)) = self.destination(vfs) else {
            self.failed = true;

            return None;
        };

        let mut body = self.lines.join("\n");
        body.push('\n');

        self.dirty = false;
        self.writing = true;

        Some((path, body.into_bytes(), stamp, self.token))
    }

    pub fn persist_now(&mut self, vfs: &Vfs) {
        if !self.dirty && !self.writing {
            return;
        }

        let Some((path, body, stamp, _)) = self.prepare_write(vfs) else {
            return;
        };

        match write_now(&path, &body, stamp) {
            Some(stamp) => {
                self.read_from = path;
                self.stamp = stamp;
                self.failed = false;
            }
            None => self.failed = true,
        }

        self.writing = false;
    }

    pub fn restate(&mut self) {
        if !self.plan.schema.creates() {
            return;
        }

        self.rules = (0..self.cells.len())
            .map(|index| {
                resolved::rule(self.plan.subject(), index, self.plan.schema.field(index), &self.cells)
            })
            .collect();
    }

    pub fn destination(&self, vfs: &Vfs) -> Option<(PathBuf, Stamp)> {
        let Some(name) = self.plan.target_mod.as_deref() else {
            return Some((self.plan.game.clone(), self.stamp));
        };

        if self.read_from != self.plan.game {
            return Some((self.read_from.clone(), self.stamp));
        }

        let path = mods::ensure(vfs, name, &self.plan.game)
            .inspect_err(|err| warn!(source = %self.plan.game.display(), "Stats editor could not stage the file: {}", err))
            .ok()?;

        let stamp = preview::stamp(&path)?;

        Some((path, stamp))
    }

    pub fn schema(&self) -> &'static schema::Schema {
        self.plan.schema
    }

    pub fn values(&self) -> EditorMode {
        self.plan.values
    }

    pub fn rule(&self, index: usize) -> Rule {
        self.rules.get(index).copied().unwrap_or(Rule::Opaque)
    }

    pub fn note(&self, index: usize) -> Option<&'static str> {
        resolved::note(self.plan.subject(), self.plan.schema.field(index))
    }

    pub fn face(&self, index: usize) -> Face {
        let rule = self.rule(index);
        let raw = self.cells.get(index).copied().unwrap_or_default();

        if let Some(gate) = rule.gate()
            && self.plan.values == EditorMode::Resolved
            && self.reads(gate.field) == Some(gate.blocked)
        {
            return Face::Disabled(gate.reason);
        }

        rule.face(raw, self.plan.values)
    }

    pub fn reads(&self, field: &str) -> Option<i32> {
        self.plan.schema.index_of(field).and_then(|index| self.cells.get(index).copied())
    }

    pub fn address(&self) -> Address {
        self.keyed.map_or(self.plan.address, Address::Keyed)
    }

    pub fn aimed(&self) -> Address {
        if self.absent {
            return self.plan.address;
        }

        self.keyed.map_or(Address::Line(self.row), Address::Keyed)
    }

    pub fn aim(&mut self, address: Address) -> bool {
        if let Some(row) = address.locate(&self.lines, self.delimiter) {
            return self.retarget(row);
        }

        if !self.plan.schema.creates() {
            return false;
        }

        let Some(row) = address.insertion(&self.lines, self.delimiter) else {
            return false;
        };

        let blank = vacant(&self.plan, &self.lines, self.delimiter);
        self.plan = self.plan.clone().rowed(address);
        self.adopt(&blank, row, true);

        true
    }

    pub fn keys(&self) -> &[u32] {
        &self.keys
    }

    pub fn keyed(&self) -> Option<u32> {
        self.keyed
    }

    pub fn pin(&self) -> Option<Address> {
        if let Some(id) = self.keyed {
            return Some(Address::Keyed(id));
        }

        (self.plan.schema.appends() && !self.absent).then_some(Address::Line(self.row))
    }

    pub fn row(&self) -> usize {
        self.row
    }

    pub fn at(&self, field: &str) -> Option<usize> {
        self.plan.schema.index_of(field)
    }

    pub fn slotted(&mut self, index: usize, unit: i32, form: i32) {
        let wanted: Vec<(usize, i32)> = [(index, unit), (index + 1, form)]
            .into_iter()
            .filter(|(slot, raw)| self.cells.get(*slot) != Some(raw))
            .collect();

        if wanted.is_empty() {
            return;
        }

        self.overwrite(&wanted);

        let packing = combos::packed(self.plan.schema, &self.cells);
        self.overwrite(&packing);

        self.stage();
    }

    fn overwrite(&mut self, cells: &[(usize, i32)]) {
        for (slot, raw) in cells.iter().copied() {
            let Some(cell) = self.cells.get_mut(slot) else {
                continue;
            };

            *cell = raw;
            self.record(slot, raw);

            let display = shown(self.plan.schema, slot, raw, self.plan.values, self.rule(slot));

            if let Some(field) = self.inputs.get_mut(slot) {
                *field = display;
            }
        }
    }

    pub fn anchor(&self) -> Option<(i32, i32)> {
        self.plan.anchor
    }

    pub fn absent(&self) -> bool {
        self.absent
    }

    pub fn retarget(&mut self, row: usize) -> bool {
        let Some(line) = self.lines.get(row).cloned() else {
            return false;
        };

        self.plan = self.plan.clone().rowed(Address::Line(row));
        self.adopt(&line, row, false);

        true
    }

    pub fn adopt(&mut self, line: &str, row: usize, absent: bool) {
        let Row { cells, written, stored, comment } = split_row(line, self.delimiter, self.plan.schema);

        self.row = row;
        self.absent = absent;
        self.cells = cells;
        self.written = written;
        self.stored = stored;
        self.touched = 0;
        self.comment = comment;
        self.keyed = self
            .plan
            .schema
            .switches()
            .then(|| self.lines.get(row).and_then(|line| leading(line, self.delimiter)))
            .flatten();

        self.rules = (0..self.cells.len())
            .map(|index| {
                resolved::rule(self.plan.subject(), index, self.plan.schema.field(index), &self.cells)
            })
            .collect();

        self.inputs = (0..self.cells.len())
            .map(|index| {
                shown(self.plan.schema, index, self.cells[index], self.plan.values, self.rules[index])
            })
            .collect();

        self.buffer = None;
    }

    pub fn remove(&mut self) -> bool {
        if self.absent || self.row >= self.lines.len() {
            return false;
        }

        if self.row + 1 == self.lines.len() {
            self.lines.remove(self.row);
            self.dirty = true;

            return true;
        }

        let Some(index) = self.plan.schema.index_of(combos::SERIES) else {
            return false;
        };

        self.overwrite(&[(index, RETIRED)]);
        self.stage();

        true
    }

    pub fn stored(&self) -> usize {
        self.stored
    }

    pub fn label(&self, index: usize) -> std::borrow::Cow<'static, str> {
        if let Some(named) =
            self.plan.labels.as_ref().and_then(|held| held.get(index)).filter(|held| !held.is_empty())
        {
            return std::borrow::Cow::Owned(named.clone());
        }

        if self.plan.subject() == Subject::MapStage
            && let Some(named) = mapdata::label(index, &self.cells)
        {
            return named;
        }

        self.plan.schema.label(index)
    }

    pub fn reads_at(&self, index: usize) -> Option<i32> {
        self.cells.get(index).copied()
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn input(&self, index: usize) -> &str {
        self.inputs.get(index).map_or("", String::as_str)
    }

    pub fn comment(&self) -> &str {
        &self.comment
    }
}

pub fn plan(
    subject: Subject,
    address: Address,
    label: String,
    game: &Path,
    target_mod: Option<String>,
    values: EditorMode,
) -> Plan {
    Plan {
        address,
        labels: None,
        label,
        game: game.to_path_buf(),
        target_mod,
        schema: schema::of(subject),
        values,
        anchor: None,
    }
}

impl Subject {
    pub fn realm(self) -> Realm {
        match self {
            Subject::Enemy | Subject::EnemyGuide => Realm::Enemies,
            Subject::Cat
            | Subject::LineupRatings
            | Subject::GuideSpotlights
            | Subject::UnitLimits
            | Subject::GuideEntries
            | Subject::Buy
            | Subject::Curve
            | Subject::Talents
            | Subject::Costs
            | Subject::Combo
            | Subject::ComboParam
            | Subject::OrbSlots
            | Subject::ExpCurve
            | Subject::OrbGrades
            | Subject::OrbTraits
            | Subject::ComboCategories => Realm::Cats,
            Subject::MapStage
            | Subject::MapDrops
            | Subject::DropChara
            | Subject::ItemBuy
            | Subject::MapOption
            | Subject::Background
            | Subject::Difficulty
            | Subject::ExInvasion
            | Subject::ExLottery
            | Subject::ZombieHeader
            | Subject::ZombieTier
            | Subject::Altar
            | Subject::Castles
            | Subject::QuestStages
            | Subject::SlotLottery
            | Subject::RealmAltar
            | Subject::RealmDraws
            | Subject::LabyrinthFloors
            | Subject::LabyrinthLottery
            | Subject::FilterMaps
            | Subject::EnigmaLotteries
            | Subject::EnigmaRates
            | Subject::EnigmaDrops
            | Subject::EnigmaGroups
            | Subject::DiscoveryMedals
            | Subject::MapMarkers
            | Subject::EnigmaTimes
            | Subject::EnergyRefunds
            | Subject::StageShortcuts
            | Subject::EnigmaRarities
            | Subject::MapSheets
            | Subject::DemonCastles => Realm::Stages,
            Subject::Upgrade
            | Subject::LevelUpOffers
            | Subject::ReelSymbols
            | Subject::MenuTextSettings
            | Subject::TraitIcons
            | Subject::AbilityIcons
            | Subject::LineupRankGroups
            | Subject::LineupBands
            | Subject::EngineerCap
            | Subject::TipSettings
            | Subject::FanfareTiers
            | Subject::OrbOptions
            | Subject::OrbFilterChoices
            | Subject::LineupCounters
            | Subject::RecommendedUpgrades
            | Subject::LineupExclusions
            | Subject::ZonedInSteps
            | Subject::ZonedIn
            | Subject::FortuneOdds
            | Subject::ShrineExtras
            | Subject::ShrineOfferings
            | Subject::PrizePatterns
            | Subject::GamatotoLevels
            | Subject::ExpeditionAreas
            | Subject::MaterialMixes
            | Subject::DevelopmentTiers
            | Subject::DevelopmentCosts
            | Subject::CannonStatus
            | Subject::Shakes
            | Subject::Recipes
            | Subject::Shop
            | Subject::Cpu
            | Subject::GamatotoLimit
            | Subject::GamatotoUnlock
            | Subject::GamatotoDrop
            | Subject::Stamps
            | Subject::SlotUnlocks
            | Subject::Club
            | Subject::Missions
            | Subject::Conditions
            | Subject::MissionUnlocks
            | Subject::MissionGroups
            | Subject::GachaOptions
            | Subject::ChanceAnimations
            | Subject::TipRules
            | Subject::MissionShortcuts
            | Subject::MissionDesignations
            | Subject::MonthlyMissions
            | Subject::SoundSettings
            | Subject::DojoChests
            | Subject::PopupTriggers
            | Subject::CastleGrowth => Realm::Home,
        }
    }
}

pub fn mark(plan: &Plan, vfs: &Vfs, reader: &mut Reader) -> Mark {
    let Some(held) = plan.target_mod.as_deref().and_then(|name| mods::find(vfs, name, &plan.game)) else {
        return Mark::Same;
    };

    let ours = reader.text(&held).and_then(|body| entry(plan, &body));
    let theirs = reader.text(&plan.game).and_then(|body| entry(plan, &body));

    Mark::of(ours, theirs)
}
