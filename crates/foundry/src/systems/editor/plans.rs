use std::path::{Path, PathBuf};
use std::slice;
use rustc_hash::FxHashMap;
use crate::domains::cat::files as cat_files;
use crate::domains::enemy::files as enemy_files;
use crate::domains::enemy::scanner::EnemyEntry;
use crate::common::architecture;
use crate::domains::mods;
use crate::domains::sandbox::{base, orb};
use crate::domains::stage::files as stage_files;
use crate::domains::stage::materials as stage_materials;
use crate::domains::stage::{names, GlobalMapId, GlobalStageId};
use crate::domains::cat::combo as cat_combo;
use crate::domains::cat::waiter as cat_waiter;
use codex::combat::{Faction, Identity};
use crate::domains::settings::EditorMode;

use super::tables::*;
use super::{altar_key, combo_effect, copies, drop_ids, figures, glossary_file, internal_name, item_label, prose, roster, visible_name, Focus, Pane, Pick, Realm, Sources, SEPARATOR};

#[derive(Default)]
pub struct Aim<'a> {
    pub key: Option<u32>,
    pub line: Option<usize>,
    pub file: Option<&'a str>,
}

pub fn current_plan(sources: &Sources<'_>, focus: &Focus, subject: figures::Subject, aim: Aim<'_>, values: EditorMode) -> Option<figures::Plan> {
    match subject {
        figures::Subject::Cat => {
            let cat = cat_subject(sources, focus)?;
            let active = cat.active_mod.clone();

            Some(cat_plan(&cat, active, values))
        }
        figures::Subject::Enemy => {
            let enemy = enemy_subject(sources, focus)?;
            let active = enemy.active_mod.clone();

            Some(enemy_plan(&enemy, active, values))
        }
        figures::Subject::Buy
        | figures::Subject::Curve
        | figures::Subject::Talents
        | figures::Subject::Costs
        | figures::Subject::Combo
        | figures::Subject::MapStage
        | figures::Subject::MapDrops
        | figures::Subject::DropChara
        | figures::Subject::ItemBuy
        | figures::Subject::MapOption
        | figures::Subject::Background
        | figures::Subject::Upgrade
        | figures::Subject::ComboParam
        | figures::Subject::OrbSlots
        | figures::Subject::Difficulty
        | figures::Subject::ExInvasion
        | figures::Subject::CannonStatus
        | figures::Subject::ExpCurve
        | figures::Subject::OrbGrades
        | figures::Subject::ExLottery
        | figures::Subject::Shakes
        | figures::Subject::Recipes
        | figures::Subject::Shop
        | figures::Subject::ZombieHeader
        | figures::Subject::ZombieTier
        | figures::Subject::Cpu
        | figures::Subject::OrbTraits
        | figures::Subject::Altar
        | figures::Subject::GamatotoLimit
        | figures::Subject::GamatotoUnlock
        | figures::Subject::GamatotoDrop
        | figures::Subject::Stamps
        | figures::Subject::SlotUnlocks
        | figures::Subject::Club
        | figures::Subject::Missions
        | figures::Subject::Conditions
        | figures::Subject::MissionUnlocks
        | figures::Subject::MissionGroups
        | figures::Subject::GachaOptions
        | figures::Subject::ChanceAnimations
        | figures::Subject::Castles
        | figures::Subject::TipRules
        | figures::Subject::QuestStages
        | figures::Subject::SlotLottery
        | figures::Subject::RealmAltar
        | figures::Subject::RealmDraws
        | figures::Subject::LabyrinthFloors
        | figures::Subject::LabyrinthLottery
        | figures::Subject::MissionShortcuts
        | figures::Subject::MissionDesignations
        | figures::Subject::MonthlyMissions
        | figures::Subject::SoundSettings
        | figures::Subject::DojoChests
        | figures::Subject::PopupTriggers
        | figures::Subject::ComboCategories
        | figures::Subject::CastleGrowth
        | figures::Subject::DevelopmentCosts
        | figures::Subject::DevelopmentTiers
        | figures::Subject::MaterialMixes
        | figures::Subject::GuideEntries
        | figures::Subject::UnitLimits
        | figures::Subject::EnemyGuide
        | figures::Subject::ExpeditionAreas
        | figures::Subject::GamatotoLevels
        | figures::Subject::PrizePatterns
        | figures::Subject::ShrineOfferings
        | figures::Subject::ShrineExtras
        | figures::Subject::FortuneOdds
        | figures::Subject::ZonedIn
        | figures::Subject::ZonedInSteps
        | figures::Subject::FilterMaps
        | figures::Subject::LineupExclusions
        | figures::Subject::EnigmaLotteries
        | figures::Subject::EnigmaRates
        | figures::Subject::EnigmaDrops
        | figures::Subject::EnigmaGroups
        | figures::Subject::DiscoveryMedals
        | figures::Subject::MapMarkers
        | figures::Subject::EnigmaTimes
        | figures::Subject::RecommendedUpgrades
        | figures::Subject::LineupCounters
        | figures::Subject::OrbOptions
        | figures::Subject::EnergyRefunds
        | figures::Subject::StageShortcuts
        | figures::Subject::FanfareTiers
        | figures::Subject::GuideSpotlights
        | figures::Subject::TipSettings
        | figures::Subject::EnigmaRarities
        | figures::Subject::EngineerCap
        | figures::Subject::LineupRatings
        | figures::Subject::LineupBands
        | figures::Subject::LineupRankGroups
        | figures::Subject::AbilityIcons
        | figures::Subject::TraitIcons
        | figures::Subject::MenuTextSettings
        | figures::Subject::MapSheets
        | figures::Subject::OrbFilterChoices
        | figures::Subject::ReelSymbols
        | figures::Subject::LevelUpOffers
        | figures::Subject::DemonCastles => {
            let sources = match subject {
                figures::Subject::Talents | figures::Subject::Costs => match focus.cat.is_some() {
                    true => roster_payloads(sources, focus, &TALENT_FILES),
                    false => Vec::new(),
                },
                figures::Subject::Combo => combo_target(sources, focus, aim.line).into_iter().collect(),
                figures::Subject::MapStage => stage_data_targets(sources, focus),
                figures::Subject::MapDrops => map_drop_targets(sources, focus),
                figures::Subject::DropChara => drop_chara_targets(sources, focus),
                figures::Subject::ItemBuy => item_buy_targets(sources, focus, aim.key),
                figures::Subject::MapOption => map_option_targets(sources, focus),
                figures::Subject::Background => background_targets(sources, focus, aim.key),
                figures::Subject::Upgrade => upgrade_targets(sources, focus, aim.line),
                figures::Subject::ComboParam => combo_param_targets(sources, focus, aim.line),
                figures::Subject::OrbSlots => orb_slot_targets(sources, focus),
                figures::Subject::Difficulty => keyed_map_targets(sources, focus, figures::Subject::Difficulty, DIFFICULTY_TABLE),
                figures::Subject::ExInvasion => keyed_map_targets(sources, focus, figures::Subject::ExInvasion, EX_OPTION_TABLE),
                figures::Subject::CannonStatus => cannon_status_targets(sources, focus, aim.key),
                figures::Subject::ExpCurve => roster_payloads(sources, focus, &EXP_FILES),
                figures::Subject::OrbGrades => orb_grade_targets(sources, focus, aim.line),
                figures::Subject::ExLottery => line_targets(sources, focus, aim.line, figures::Subject::ExLottery, EX_LOTTERY_TABLE, EX_GROUP),
                figures::Subject::Shakes => keyed_targets(sources, focus, aim.key, figures::Subject::Shakes, SHAKE_TABLE, SHAKE_NAME),
                figures::Subject::Recipes => keyed_targets(sources, focus, aim.key, figures::Subject::Recipes, RECIPE_TABLE, RECIPE_NAME),
                figures::Subject::Shop => keyed_targets(sources, focus, aim.key, figures::Subject::Shop, SHOP_TABLE, SHOP_NAME),
                figures::Subject::ZombieHeader | figures::Subject::ZombieTier => {
                    line_targets(sources, focus, aim.line, subject, ZOMBIE_TABLE, ZOMBIE_LINE)
                }
                figures::Subject::Cpu => line_targets(sources, focus, aim.line, subject, CPU_TABLE, CPU_LINE),
                figures::Subject::OrbTraits => line_targets(sources, focus, aim.line, subject, ORB_TRAIT_TABLE, ORB_TRAIT_LINE),
                figures::Subject::Altar => altar_targets(sources, focus),
                figures::Subject::GamatotoLimit => line_targets(sources, focus, aim.line, subject, GAMATOTO_LIMIT_TABLE, GAMATOTO_LIMIT_NAME),
                figures::Subject::GamatotoUnlock => keyed_targets(sources, focus, aim.key, subject, GAMATOTO_UNLOCK_TABLE, GAMATOTO_UNLOCK_NAME),
                figures::Subject::GamatotoDrop => keyed_targets(sources, focus, aim.key, subject, GAMATOTO_DROP_TABLE, GAMATOTO_DROP_NAME),
                figures::Subject::Stamps => line_targets(sources, focus, aim.line, subject, STAMP_TABLE, STAMP_LINE),
                figures::Subject::SlotUnlocks => line_targets(sources, focus, aim.line, subject, SLOT_TABLE, SLOT_LINE),
                figures::Subject::Club => keyed_targets(sources, focus, aim.key, subject, CLUB_TABLE, CLUB_NAME),
                figures::Subject::Missions => keyed_targets(sources, focus, aim.key, subject, MISSION_TABLE, MISSION_NAME),
                figures::Subject::Conditions => keyed_targets(sources, focus, aim.key, subject, CONDITION_TABLE, MISSION_NAME),
                figures::Subject::MissionUnlocks => keyed_targets(sources, focus, aim.key, subject, MISSION_UNLOCK_TABLE, MISSION_UNLOCK_NAME),
                figures::Subject::MissionGroups => keyed_targets(sources, focus, aim.key, subject, MISSION_GROUP_TABLE, MISSION_GROUP_NAME),
                figures::Subject::GachaOptions => aim.file.map_or_else(Vec::new, |file| keyed_targets(sources, focus, aim.key, subject, file, roster::SET_NAME)),
                figures::Subject::ChanceAnimations => line_targets(sources, focus, aim.line, subject, roster::CHANCE_TABLE, roster::CHANCE_NAME),
                figures::Subject::Castles => aim.file.map_or_else(Vec::new, |file| line_targets(sources, focus, aim.line, subject, file, CASTLE_NAME)),
                figures::Subject::TipRules => keyed_targets(sources, focus, aim.key, subject, TIP_RULE_TABLE, TIP_RULE_NAME),
                figures::Subject::QuestStages => aim.file.map_or_else(Vec::new, |file| line_targets(sources, focus, aim.line, subject, file, QUEST_STAGE_NAME)),
                figures::Subject::SlotLottery => aim.file.map_or_else(Vec::new, |file| line_targets(sources, focus, aim.line, subject, file, SLOT_LOTTERY_NAME)),
                figures::Subject::LabyrinthFloors => aim.file.map_or_else(Vec::new, |file| line_targets(sources, focus, aim.line, subject, file, LABYRINTH_FLOOR_NAME)),
                figures::Subject::LabyrinthLottery => keyed_targets(sources, focus, aim.key, subject, LABYRINTH_LOTTERY_TABLE, LABYRINTH_RULE_NAME),
                figures::Subject::MissionShortcuts => keyed_targets(sources, focus, aim.key, subject, MISSION_SHORTCUT_TABLE, MISSION_SHORTCUT_NAME),
                figures::Subject::MissionDesignations => keyed_targets(sources, focus, aim.key, subject, MISSION_DESIGNATION_TABLE, MISSION_NAME),
                figures::Subject::MonthlyMissions => keyed_targets(sources, focus, aim.key, subject, MONTHLY_MISSION_TABLE, MONTHLY_MISSION_NAME),
                figures::Subject::SoundSettings => line_targets(sources, focus, aim.line, subject, SOUND_TABLE, SOUND_NAME),
                figures::Subject::DojoChests => keyed_targets(sources, focus, aim.key, subject, CHEST_TABLE, CHEST_NAME),
                figures::Subject::PopupTriggers => keyed_targets(sources, focus, aim.key, subject, POPUP_TRIGGER_TABLE, POPUP_TRIGGER_NAME),
                figures::Subject::ComboCategories => keyed_targets(sources, focus, aim.key, subject, COMBO_CATEGORY_TABLE, COMBO_CATEGORY_NAME),
                figures::Subject::CastleGrowth => line_targets(sources, focus, aim.line, subject, CASTLE_GROWTH_TABLE, CASTLE_BRACKET_NAME),
                figures::Subject::DemonCastles => keyed_targets(sources, focus, aim.key, subject, DEMON_CASTLE_TABLE, DEMON_CASTLE_NAME),
                figures::Subject::LevelUpOffers => keyed_targets(sources, focus, aim.key, subject, LEVEL_UP_TABLE, LEVEL_UP_NAME),
                figures::Subject::ReelSymbols => aim.file.map_or_else(Vec::new, |file| line_targets(sources, focus, aim.line, subject, file, REEL_NAME)),
                figures::Subject::OrbFilterChoices => line_targets(sources, focus, aim.line, subject, ORB_CHOICE_TABLE, ORB_CHOICE_NAME),
                figures::Subject::MapSheets => keyed_targets(sources, focus, aim.key, subject, MAP_SHEET_TABLE, MAP_SHEET_NAME),
                figures::Subject::MenuTextSettings => keyed_targets(sources, focus, aim.key, subject, MENU_TEXT_TABLE, MENU_TEXT_NAME),
                figures::Subject::TraitIcons => line_targets(sources, focus, aim.line, subject, TRAIT_ICON_TABLE, TRAIT_ICON_NAME),
                figures::Subject::AbilityIcons => line_targets(sources, focus, aim.line, subject, ABILITY_ICON_TABLE, ABILITY_ICON_NAME),
                figures::Subject::LineupRankGroups => line_targets(sources, focus, aim.line, subject, RANK_GROUP_TABLE, RANK_GROUP_NAME),
                figures::Subject::LineupBands => line_targets(sources, focus, aim.line, subject, BAND_TABLE, BAND_NAME),
                figures::Subject::LineupRatings => line_targets(sources, focus, aim.line, subject, RATING_TABLE, RATING_NAME),
                figures::Subject::EngineerCap => line_targets(sources, focus, aim.line, subject, ENGINEER_CAP_TABLE, ENGINEER_CAP_NAME),
                figures::Subject::EnigmaRarities => line_targets(sources, focus, aim.line, subject, ENIGMA_RARITY_TABLE, ENIGMA_RARITY_NAME),
                figures::Subject::TipSettings => keyed_targets(sources, focus, aim.key, subject, TIP_SETTING_TABLE, TIP_SETTING_NAME),
                figures::Subject::GuideSpotlights => line_targets(sources, focus, aim.line, subject, SPOTLIGHT_TABLE, SPOTLIGHT_NAME),
                figures::Subject::FanfareTiers => line_targets(sources, focus, aim.line, subject, FANFARE_TABLE, FANFARE_NAME),
                figures::Subject::StageShortcuts => line_targets(sources, focus, aim.line, subject, STAGE_SHORTCUT_TABLE, STAGE_SHORTCUT_NAME),
                figures::Subject::EnergyRefunds => line_targets(sources, focus, aim.line, subject, REFUND_TABLE, REFUND_NAME),
                figures::Subject::OrbOptions => keyed_targets(sources, focus, aim.key, subject, ORB_OPTION_TABLE, ORB_OPTION_NAME),
                figures::Subject::LineupCounters => line_targets(sources, focus, aim.line, subject, COUNTER_TABLE, COUNTER_NAME),
                figures::Subject::RecommendedUpgrades => line_targets(sources, focus, aim.line, subject, RECOMMEND_TABLE, RECOMMEND_NAME),
                figures::Subject::EnigmaTimes => line_targets(sources, focus, aim.line, subject, ENIGMA_TIME_TABLE, ENIGMA_TIME_NAME),
                figures::Subject::MapMarkers => aim.file.map_or_else(Vec::new, |file| line_targets(sources, focus, aim.line, subject, file, MAP_MARKER_NAME)),
                figures::Subject::DiscoveryMedals => line_targets(sources, focus, aim.line, subject, ENIGMA_MEDAL_TABLE, ENIGMA_MEDAL_NAME),
                figures::Subject::EnigmaGroups => line_targets(sources, focus, aim.line, subject, ENIGMA_GROUP_TABLE, ENIGMA_GROUP_NAME),
                figures::Subject::EnigmaDrops => line_targets(sources, focus, aim.line, subject, ENIGMA_DROP_TABLE, ENIGMA_LEVEL_NAME),
                figures::Subject::EnigmaRates => line_targets(sources, focus, aim.line, subject, ENIGMA_RATE_TABLE, ENIGMA_LEVEL_NAME),
                figures::Subject::EnigmaLotteries => line_targets(sources, focus, aim.line, subject, ENIGMA_LOTTERY_TABLE, ENIGMA_LOTTERY_NAME),
                figures::Subject::LineupExclusions => line_targets(sources, focus, aim.line, subject, EXCLUSION_TABLE, EXCLUSION_NAME),
                figures::Subject::FilterMaps => line_targets(sources, focus, aim.line, subject, FILTER_MAP_TABLE, FILTER_MAP_NAME),
                figures::Subject::ZonedInSteps => line_targets(sources, focus, aim.line, subject, ZONED_IN_TABLE, ZONED_IN_STEPS_NAME),
                figures::Subject::ZonedIn => line_targets(sources, focus, aim.line, subject, ZONED_IN_TABLE, ZONED_IN_NAME),
                figures::Subject::FortuneOdds => keyed_targets(sources, focus, aim.key, subject, FORTUNE_TABLE, FORTUNE_NAME),
                figures::Subject::ShrineExtras => line_targets(sources, focus, aim.line, subject, SHRINE_EXTRA_TABLE, SHRINE_EXTRA_NAME),
                figures::Subject::ShrineOfferings => keyed_targets(sources, focus, aim.key, subject, SHRINE_OFFERING_TABLE, SHRINE_OFFERING_NAME),
                figures::Subject::PrizePatterns => aim.file.map_or_else(Vec::new, |file| line_targets(sources, focus, aim.line, subject, file, PRIZE_NAME)),
                figures::Subject::GamatotoLevels => line_targets(sources, focus, aim.line, subject, GAMATOTO_LEVEL_TABLE, GAMATOTO_LEVEL_NAME),
                figures::Subject::ExpeditionAreas => aim.file.map_or_else(Vec::new, |file| line_targets(sources, focus, aim.line, subject, file, EXPEDITION_AREA_NAME)),
                figures::Subject::EnemyGuide => keyed_targets(sources, focus, focus.enemy, subject, ENEMY_GUIDE_TABLE, ENEMY_GUIDE_NAME),
                figures::Subject::UnitLimits => focus.cat.map_or_else(Vec::new, |id| line_targets(sources, focus, Some(id as usize), subject, UNIT_LIMIT_TABLE, UNIT_LIMIT_NAME)),
                figures::Subject::GuideEntries => focus.cat.map_or_else(Vec::new, |id| line_targets(sources, focus, Some(id as usize), subject, GUIDE_TABLE, GUIDE_NAME)),
                figures::Subject::MaterialMixes => keyed_targets(sources, focus, aim.key, subject, MATERIAL_MIX_TABLE, MATERIAL_MIX_NAME),
                figures::Subject::DevelopmentTiers => line_targets(sources, focus, aim.line, subject, DEVELOPMENT_TIER_TABLE, DEVELOPMENT_TIER_NAME),
                figures::Subject::DevelopmentCosts => aim.file.map_or_else(Vec::new, |file| line_targets(sources, focus, aim.line, subject, file, DEVELOPMENT_COST_NAME)),
                figures::Subject::RealmAltar | figures::Subject::RealmDraws => {
                    aim.file.map_or_else(Vec::new, |file| line_targets(sources, focus, aim.line, subject, file, REALM_DRAW_NAME))
                }
                _ => level_payloads(sources, focus, true, false),
            };

            let target = sources.into_iter().find(|level| level.subject == subject)?;
            let mount = target.active_mod.clone();
            let scopes = target.asset.scopes(mount.is_some());

            let scope = match aim.file {
                Some(wanted) => scopes.iter().find(|scope| scope.name == wanted)?,
                None => scopes.first()?,
            };

            level_plan(&target, scope, mount, values)
        }
    }
}

pub fn cat_plan(cat: &CatTarget, target_mod: Option<String>, values: EditorMode) -> figures::Plan {
    figures::plan(figures::Subject::Cat, figures::Address::Line(cat.form), cat.title(), &cat.source, target_mod, values)
}

pub fn enemy_plan(enemy: &EnemyTarget, target_mod: Option<String>, values: EditorMode) -> figures::Plan {
    figures::plan(figures::Subject::Enemy, figures::Address::Line(enemy.row), enemy.title(), &enemy.source, target_mod, values)
}

pub fn level_plan(
    level: &LevelTarget,
    scope: &Scope<'_>,
    target_mod: Option<String>,
    values: EditorMode,
) -> Option<figures::Plan> {
    let source = scope.source.or(scope.present)?;

    let made = figures::plan(level.subject, level.address, level.label.clone(), source, target_mod, values);
    let made = made.anchored(level.anchor);

    Some(match level.columns.is_empty() {
        true => made,
        false => made.named(level.columns.clone()),
    })
}

pub struct LevelTarget {
    pub subject: figures::Subject,
    pub asset: Asset,
    pub label: String,
    pub columns: Vec<String>,
    pub address: figures::Address,
    pub anchor: Option<(i32, i32)>,
    pub unlocked: bool,
    pub active_mod: Option<String>,
}

const EXP_FILES: [(figures::Subject, &str); 1] = [(figures::Subject::ExpCurve, EXP_TABLE)];

pub const TALENT_FILES: [(figures::Subject, &str); 2] = [
    (figures::Subject::Talents, cat_files::SKILL_ACQUISITION),
    (figures::Subject::Costs, cat_files::SKILL_LEVEL),
];

pub struct Scope<'a> {
    pub name: &'a str,
    pub source: Option<&'a Path>,
    pub present: Option<&'a Path>,
    pub title: Option<&'a str>,
}

pub struct EnemyTarget {
    pub file: String,
    pub mod_copy: Option<PathBuf>,
    pub name: Option<String>,
    pub source: PathBuf,
    pub row: usize,
    pub unlocked: bool,
    pub active_mod: Option<String>,
}

impl EnemyTarget {
    pub fn title(&self) -> String {
        match self.name.as_deref() {
            Some(name) if !name.is_empty() => [name, self.file.as_str()].join(SEPARATOR),
            _ => self.file.clone(),
        }
    }
}

pub struct CatTarget {
    pub file: String,
    pub mod_copy: Option<PathBuf>,
    pub name: Option<String>,
    pub source: PathBuf,
    pub form: usize,
    pub unlocked: bool,
    pub active_mod: Option<String>,
}

impl CatTarget {
    pub fn title(&self) -> String {
        let form = figures::FORMS.get(self.form).copied().unwrap_or("Unknown");

        match self.name.as_deref() {
            Some(name) => [name, form, self.file.as_str()].join(SEPARATOR),
            None => [self.file.as_str(), form].join(SEPARATOR),
        }
    }
}

pub fn stage_data_targets(sources: &Sources<'_>, focus: &Focus) -> Vec<LevelTarget> {
    if focus.realm != Realm::Stages {
        return Vec::new();
    }

    let Some(chosen) = focus.stage.as_ref() else {
        return Vec::new();
    };

    let registry = &sources.registry;
    let map = GlobalMapId { category: chosen.category.clone(), map: chosen.map };

    let Some(name) = registry.addresses.get(&map).and_then(|held| held.data_file.as_deref()) else {
        return Vec::new();
    };

    let files = asset_files(sources, focus, name);

    if files.is_empty() {
        return Vec::new();
    }

    let chapter = registry
        .maps
        .get(&map)
        .map(|found| found.name.trim())
        .filter(|found| !found.is_empty())
        .map_or_else(|| format!("{:03}", map.map), str::to_owned);

    let stage = registry
        .stages
        .get(chosen)
        .map(|found| found.name.trim())
        .filter(|found| !found.is_empty())
        .map_or_else(|| format!("{:02}", chosen.stage), str::to_owned);

    let line = figures::mapdata::MAP_HEADER_LINES + chosen.stage as usize;

    vec![LevelTarget {
        subject: figures::Subject::MapStage,
        asset: Asset::Variants { key: name.to_owned(), files },
        label: [chapter.as_str(), stage.as_str(), name].join(SEPARATOR),
        columns: Vec::new(),
        address: figures::Address::Line(line),
        anchor: None,
        unlocked: focus.unlocked,
        active_mod: focus.active_mod.clone(),
    }]
}

// DropItem.csv holds one row per map, addressed by the routed global id in its leading cell.
// The eight material slots are named by the items `MAT_IDS` points them at, read from the
// game's own item table rather than translated here.
pub fn map_drop_targets(sources: &Sources<'_>, focus: &Focus) -> Vec<LevelTarget> {
    if focus.realm != Realm::Stages {
        return Vec::new();
    }

    let Some(map) = focus.map.as_ref() else {
        return Vec::new();
    };

    let registry = &sources.registry;

    let Some(global) = registry.addresses.get(map).and_then(|held| held.global) else {
        return Vec::new();
    };

    // The file covers a little over a hundred maps and is keyed by the routed global id, so
    // a story chapter's 3000-3008 reaches no row at all. The scanner already looked the row
    // up, so asking it costs nothing and stops the menu offering an Edit that opens nothing.
    if registry.maps.get(map).is_none_or(|found| found.drop_items.is_none()) {
        return Vec::new();
    }

    let files = asset_files(sources, focus, stage_files::DROP_ITEM);

    if files.is_empty() {
        return Vec::new();
    }

    let chapter = registry
        .maps
        .get(map)
        .map(|found| found.name.trim())
        .filter(|found| !found.is_empty())
        .map_or_else(|| format!("{:03}", map.map), str::to_owned);

    vec![LevelTarget {
        subject: figures::Subject::MapDrops,
        asset: Asset::Variants { key: stage_files::DROP_ITEM.to_owned(), files },
        label: [chapter.as_str(), stage_files::DROP_ITEM].join(SEPARATOR),
        columns: material_names(sources),
        address: figures::Address::Keyed(global),
        anchor: None,
        unlocked: focus.unlocked,
        active_mod: focus.active_mod.clone(),
    }]
}

fn map_option_targets(sources: &Sources<'_>, focus: &Focus) -> Vec<LevelTarget> {
    if focus.realm != Realm::Stages {
        return Vec::new();
    }

    let Some(map) = focus.map.as_ref() else {
        return Vec::new();
    };

    let registry = &sources.registry;

    let Some(global) = registry.addresses.get(map).and_then(|held| held.global) else {
        return Vec::new();
    };

    let files = asset_files(sources, focus, stage_files::MAP_OPTION);

    if files.is_empty() {
        return Vec::new();
    }

    let chapter = registry
        .maps
        .get(map)
        .map(|found| found.name.trim())
        .filter(|found| !found.is_empty())
        .map_or_else(|| format!("{:03}", map.map), str::to_owned);

    vec![LevelTarget {
        subject: figures::Subject::MapOption,
        asset: Asset::Variants { key: stage_files::MAP_OPTION.to_owned(), files },
        label: [chapter.as_str(), stage_files::MAP_OPTION].join(SEPARATOR),
        columns: Vec::new(),
        address: figures::Address::Keyed(global),
        anchor: None,
        unlocked: focus.unlocked,
        active_mod: focus.active_mod.clone(),
    }]
}

fn background_targets(sources: &Sources<'_>, focus: &Focus, id: Option<u32>) -> Vec<LevelTarget> {
    let Some(id) = id else {
        return Vec::new();
    };

    let files = asset_files(sources, focus, BACKGROUND_TABLE);

    if files.is_empty() {
        return Vec::new();
    }

    let label = format!("{BACKGROUND_LABEL} {id:03}");

    vec![LevelTarget {
        subject: figures::Subject::Background,
        asset: Asset::Variants { key: BACKGROUND_TABLE.to_owned(), files },
        label: [label.as_str(), BACKGROUND_TABLE].join(SEPARATOR),
        columns: Vec::new(),
        address: figures::Address::Keyed(id),
        anchor: None,
        unlocked: focus.unlocked,
        active_mod: focus.active_mod.clone(),
    }]
}

fn altar_targets(sources: &Sources<'_>, focus: &Focus) -> Vec<LevelTarget> {
    let Some(stage) = focus.stage.as_ref() else {
        return Vec::new();
    };

    let Some(key) = altar_key(sources, stage) else {
        return Vec::new();
    };

    let files = asset_files(sources, focus, ALTAR_TABLE);

    if files.is_empty() {
        return Vec::new();
    }

    let named = sources.registry.stages.get(stage).map(|found| found.name.trim().to_owned()).filter(|name| !name.is_empty());
    let label = named.unwrap_or_else(|| format!("{ALTAR_STAGE} {:02}", stage.stage));

    vec![LevelTarget {
        subject: figures::Subject::Altar,
        asset: Asset::Variants { key: ALTAR_TABLE.to_owned(), files },
        label: [label.as_str(), ALTAR_TABLE].join(SEPARATOR),
        columns: Vec::new(),
        address: figures::Address::Keyed(key),
        anchor: None,
        unlocked: focus.unlocked,
        active_mod: focus.active_mod.clone(),
    }]
}

fn keyed_targets(
    sources: &Sources<'_>,
    focus: &Focus,
    id: Option<u32>,
    subject: figures::Subject,
    table: &str,
    noun: &str,
) -> Vec<LevelTarget> {
    let Some(id) = id else {
        return Vec::new();
    };

    let files = asset_files(sources, focus, table);

    if files.is_empty() {
        return Vec::new();
    }

    let label = format!("{noun} {id}");

    vec![LevelTarget {
        subject,
        asset: Asset::Variants { key: table.to_owned(), files },
        label: [label.as_str(), table].join(SEPARATOR),
        columns: Vec::new(),
        address: figures::Address::Keyed(id),
        anchor: None,
        unlocked: focus.unlocked,
        active_mod: focus.active_mod.clone(),
    }]
}

fn line_targets(
    sources: &Sources<'_>,
    focus: &Focus,
    line: Option<usize>,
    subject: figures::Subject,
    table: &str,
    noun: &str,
) -> Vec<LevelTarget> {
    let Some(line) = line else {
        return Vec::new();
    };

    let files = asset_files(sources, focus, table);

    if files.is_empty() {
        return Vec::new();
    }

    let label = format!("{noun} {line}");

    vec![LevelTarget {
        subject,
        asset: Asset::Variants { key: table.to_owned(), files },
        label: [label.as_str(), table].join(SEPARATOR),
        columns: Vec::new(),
        address: figures::Address::Line(line),
        anchor: None,
        unlocked: focus.unlocked,
        active_mod: focus.active_mod.clone(),
    }]
}

fn orb_grade_targets(sources: &Sources<'_>, focus: &Focus, line: Option<usize>) -> Vec<LevelTarget> {
    let Some(line) = line else {
        return Vec::new();
    };

    let files = asset_files(sources, focus, ORB_GRADE_TABLE);

    if files.is_empty() {
        return Vec::new();
    }

    let grade = line.checked_sub(1).and_then(|at| orb::GRADES.get(at)).copied().unwrap_or_default();
    let label = format!("{ORB_GRADE_LABEL} {grade}");

    vec![LevelTarget {
        subject: figures::Subject::OrbGrades,
        asset: Asset::Variants { key: ORB_GRADE_TABLE.to_owned(), files },
        label: [label.as_str(), ORB_GRADE_TABLE].join(SEPARATOR),
        columns: Vec::new(),
        address: figures::Address::Line(line),
        anchor: None,
        unlocked: focus.unlocked,
        active_mod: focus.active_mod.clone(),
    }]
}

fn cannon_status_targets(sources: &Sources<'_>, focus: &Focus, part: Option<u32>) -> Vec<LevelTarget> {
    let Some(part) = part else {
        return Vec::new();
    };

    let files = asset_files(sources, focus, CANNON_STATUS_TABLE);

    if files.is_empty() {
        return Vec::new();
    }

    let held = i32::try_from(part).unwrap_or(i32::MAX);
    let label = sources.names.cannon(held).map_or_else(|| base::name(held), str::to_owned);

    vec![LevelTarget {
        subject: figures::Subject::CannonStatus,
        asset: Asset::Variants { key: CANNON_STATUS_TABLE.to_owned(), files },
        label: [label.as_str(), CANNON_STATUS_TABLE].join(SEPARATOR),
        columns: Vec::new(),
        address: figures::Address::Keyed(part),
        anchor: None,
        unlocked: focus.unlocked,
        active_mod: focus.active_mod.clone(),
    }]
}

fn keyed_map_targets(sources: &Sources<'_>, focus: &Focus, subject: figures::Subject, table: &str) -> Vec<LevelTarget> {
    let Some(map) = focus.map.as_ref() else {
        return Vec::new();
    };

    let registry = &sources.registry;

    let Some(global) = registry.addresses.get(map).and_then(|held| held.global) else {
        return Vec::new();
    };

    let files = asset_files(sources, focus, table);

    if files.is_empty() {
        return Vec::new();
    }

    let chapter = registry
        .maps
        .get(map)
        .map(|found| found.name.trim())
        .filter(|found| !found.is_empty())
        .map_or_else(|| format!("{:03}", map.map), str::to_owned);

    vec![LevelTarget {
        subject,
        asset: Asset::Variants { key: table.to_owned(), files },
        label: [chapter.as_str(), table].join(SEPARATOR),
        columns: Vec::new(),
        address: figures::Address::Keyed(global),
        anchor: None,
        unlocked: focus.unlocked,
        active_mod: focus.active_mod.clone(),
    }]
}

fn orb_slot_targets(sources: &Sources<'_>, focus: &Focus) -> Vec<LevelTarget> {
    let Some(id) = focus.cat else {
        return Vec::new();
    };

    let files = asset_files(sources, focus, ORB_SLOT_TABLE);

    if files.is_empty() {
        return Vec::new();
    }

    let label = sources
        .cats
        .iter()
        .find(|cat| cat.id == id)
        .map_or_else(|| format!("{id:03}"), |cat| cat.display_name(focus.form));

    vec![LevelTarget {
        subject: figures::Subject::OrbSlots,
        asset: Asset::Variants { key: ORB_SLOT_TABLE.to_owned(), files },
        label: [label.as_str(), ORB_SLOT_TABLE].join(SEPARATOR),
        columns: Vec::new(),
        address: figures::Address::Keyed(id),
        anchor: None,
        unlocked: focus.unlocked,
        active_mod: focus.active_mod.clone(),
    }]
}

fn combo_param_targets(sources: &Sources<'_>, focus: &Focus, line: Option<usize>) -> Vec<LevelTarget> {
    let Some(line) = line else {
        return Vec::new();
    };

    let files = asset_files(sources, focus, COMBO_PARAM_TABLE);

    if files.is_empty() {
        return Vec::new();
    }

    let label = combo_effect(sources, line);

    vec![LevelTarget {
        subject: figures::Subject::ComboParam,
        asset: Asset::Variants { key: COMBO_PARAM_TABLE.to_owned(), files },
        label: [label.as_str(), COMBO_PARAM_TABLE].join(SEPARATOR),
        columns: Vec::new(),
        address: figures::Address::Line(line),
        anchor: None,
        unlocked: focus.unlocked,
        active_mod: focus.active_mod.clone(),
    }]
}

fn upgrade_targets(sources: &Sources<'_>, focus: &Focus, line: Option<usize>) -> Vec<LevelTarget> {
    let Some(line) = line else {
        return Vec::new();
    };

    let files = asset_files(sources, focus, UPGRADE_TABLE);

    if files.is_empty() {
        return Vec::new();
    }

    let label = UPGRADE_NAMES.get(line).map_or_else(|| format!("{UPGRADE_LABEL} {line}"), |name| (*name).to_owned());

    vec![LevelTarget {
        subject: figures::Subject::Upgrade,
        asset: Asset::Variants { key: UPGRADE_TABLE.to_owned(), files },
        label: [label.as_str(), UPGRADE_TABLE].join(SEPARATOR),
        columns: Vec::new(),
        address: figures::Address::Line(line),
        anchor: None,
        unlocked: focus.unlocked,
        active_mod: focus.active_mod.clone(),
    }]
}

// Gatyaitembuy.csv names the item in its fourth column, so the row is found by that rather
// than by counting past a header the file need not carry.
pub fn item_buy_targets(sources: &Sources<'_>, focus: &Focus, item: Option<u32>) -> Vec<LevelTarget> {
    if focus.realm != Realm::Stages {
        return Vec::new();
    }

    let Some(item) = item.or_else(|| first_drop(sources, focus)) else {
        return Vec::new();
    };

    if sources.vault.vds.items.line(&sources.vault.vfs, item).is_none() {
        return Vec::new();
    }

    let files = asset_files(sources, focus, stage_files::GATYA_ITEM_BUY);

    if files.is_empty() {
        return Vec::new();
    }

    vec![LevelTarget {
        subject: figures::Subject::ItemBuy,
        asset: Asset::Variants { key: stage_files::GATYA_ITEM_BUY.to_owned(), files },
        label: [item_label(sources, item).as_str(), stage_files::GATYA_ITEM_BUY].join(SEPARATOR),
        columns: Vec::new(),
        address: figures::Address::Column { at: figures::itembuy::ITEM_ID_COLUMN, id: item },
        anchor: None,
        unlocked: focus.unlocked,
        active_mod: focus.active_mod.clone(),
    }]
}

// A stage unlocks a unit when one of its reward ids has a row in drop_chara.csv. Measured on
// the shipped corpus, 327 stages reference exactly one and none reference two, so the row is
// addressed outright; a stage that drops no unit offers nothing.
pub fn drop_chara_targets(sources: &Sources<'_>, focus: &Focus) -> Vec<LevelTarget> {
    if focus.realm != Realm::Stages {
        return Vec::new();
    }

    let Some(chosen) = focus.stage.as_ref() else {
        return Vec::new();
    };

    let Some(stage) = sources.registry.stages.get(chosen) else {
        return Vec::new();
    };

    let Some(drop) = drop_ids(stage).find(|id| sources.drops.contains_key(id)) else {
        return Vec::new();
    };

    let files = asset_files(sources, focus, stage_files::DROP_CHARA);

    if files.is_empty() {
        return Vec::new();
    }

    let unit = sources
        .drops
        .get(&drop)
        .and_then(|id| sources.cat_names.get(id))
        .and_then(|names| names.first())
        .filter(|name| !name.trim().is_empty())
        .map_or_else(|| format!("Drop {drop}"), |name| name.clone());

    vec![LevelTarget {
        subject: figures::Subject::DropChara,
        asset: Asset::Variants { key: stage_files::DROP_CHARA.to_owned(), files },
        label: [unit.as_str(), stage_files::DROP_CHARA].join(SEPARATOR),
        columns: Vec::new(),
        address: figures::Address::Keyed(drop),
        anchor: None,
        unlocked: focus.unlocked,
        active_mod: focus.active_mod.clone(),
    }]
}

pub fn level_payloads(sources: &Sources<'_>, focus: &Focus, levels: bool, forms: bool) -> Vec<LevelTarget> {
    let files: &[(figures::Subject, &str)] = match (levels, forms) {
        (true, _) => &LEVEL_FILES,
        (false, true) => &FORM_FILES,
        (false, false) => return Vec::new(),
    };

    roster_payloads(sources, focus, files)
}

pub fn combo_target(sources: &Sources<'_>, focus: &Focus, line: Option<usize>) -> Option<LevelTarget> {
    if focus.realm != Realm::Cats {
        return None;
    }

    let id = focus.cat?;
    let files = asset_files(sources, focus, cat_files::NYANCOMBO_DATA);

    if files.is_empty() {
        return None;
    }

    let address = line.map_or(figures::Address::Appended, figures::Address::Line);

    Some(LevelTarget {
        subject: figures::Subject::Combo,
        asset: Asset::Variants { key: cat_files::NYANCOMBO_DATA.to_owned(), files },
        label: [cat_label(sources, focus, id).as_str(), cat_files::NYANCOMBO_DATA].join(SEPARATOR),
        columns: Vec::new(),
        address,
        anchor: combo_anchor(focus, id),
        unlocked: focus.unlocked,
        active_mod: focus.active_mod.clone(),
    })
}

pub fn roster_payloads(sources: &Sources<'_>, focus: &Focus, files: &[(figures::Subject, &str)]) -> Vec<LevelTarget> {
    if focus.realm != Realm::Cats {
        return Vec::new();
    }

    let Some(id) = focus.cat else {
        return Vec::new();
    };

    let label = cat_label(sources, focus, id);

    files
        .iter()
        .copied()
        .filter_map(|(subject, name)| {
            let files = asset_files(sources, focus, name);

            if files.is_empty() {
                return None;
            }

            Some(LevelTarget {
                subject,
                asset: Asset::Variants { key: name.to_owned(), files },
                label: [label.as_str(), name].join(SEPARATOR),
                columns: Vec::new(),
                address: address(sources, subject, id),
                anchor: None,
                unlocked: focus.unlocked,
                active_mod: focus.active_mod.clone(),
            })
        })
        .collect()
}

pub fn cat_subject(sources: &Sources<'_>, focus: &Focus) -> Option<CatTarget> {
    if focus.realm != Realm::Cats {
        return None;
    }

    let id = focus.cat?;
    let file = cat_files::stats_file(id);
    let source = sources.vault.vfs.rooted(architecture::GAME, &file)?;
    let form = focus.form;

    let mod_copy = mod_copy(sources, focus, &file);

    let name = sources
        .cats
        .iter()
        .find(|cat| cat.id == id)
        .and_then(|cat| cat.names.get(form).cloned().flatten());

    Some(CatTarget {
        file,
        mod_copy,
        name,
        source,
        form,
        unlocked: focus.unlocked,
        active_mod: focus.active_mod.clone(),
    })
}

pub fn enemy_subject(sources: &Sources<'_>, focus: &Focus) -> Option<EnemyTarget> {
    if focus.realm != Realm::Enemies {
        return None;
    }

    let id = focus.enemy?;
    let file = enemy_files::STATS.to_owned();
    let source = sources.vault.vfs.rooted(architecture::GAME, &file)?;

    let mod_copy = mod_copy(sources, focus, &file);

    let name = sources
        .enemies
        .iter()
        .find(|enemy| enemy.id == id)
        .map(|enemy| enemy.name.clone());

    Some(EnemyTarget {
        file,
        mod_copy,
        name,
        source,
        row: ENEMY_HEADER_ROWS + id as usize,
        unlocked: focus.unlocked,
        active_mod: focus.active_mod.clone(),
    })
}

const LEVEL_FILES: [(figures::Subject, &str); 2] =
    [(figures::Subject::Buy, cat_files::UNIT_BUY), (figures::Subject::Curve, cat_files::UNIT_LEVEL)];

const FORM_FILES: [(figures::Subject, &str); 1] = [(figures::Subject::Buy, cat_files::UNIT_BUY)];

fn asset_files(sources: &Sources<'_>, focus: &Focus, base: &str) -> Vec<AssetFile> {
    variant_sets(sources, focus, slice::from_ref(&base)).pop().unwrap_or_default()
}

pub enum Asset {
    Variants { key: String, files: Vec<AssetFile> },
    Exception(Exception),
}

impl Asset {
    pub fn key(&self) -> &str {
        match self {
            Asset::Exception(exception) => &exception.internal,
            Asset::Variants { key, .. } => key,
        }
    }

    pub fn names(&self) -> Vec<String> {
        match self {
            Asset::Exception(exception) => {
                exception.variants.iter().map(|file| file.name.clone()).collect()
            }
            Asset::Variants { files, .. } => files.iter().map(|file| file.name.clone()).collect(),
        }
    }

    pub fn scopes(&self, in_mod: bool) -> Vec<Scope<'_>> {
        match self {
            Asset::Exception(exception) => exception.scopes(in_mod),
            Asset::Variants { files, .. } => files
                .iter()
                .map(|file| Scope {
                    name: &file.name,
                    source: file.game.as_deref(),
                    present: if in_mod { file.mod_copy.as_deref() } else { file.game.as_deref() },
                    title: None,
                })
                .collect(),
        }
    }
}

const ENEMY_HEADER_ROWS: usize = 2;

// What a broad right-click aims the item editors at when no treasure row was hit.
pub fn first_drop(sources: &Sources<'_>, focus: &Focus) -> Option<u32> {
    let chosen = focus.stage.as_ref()?;
    let stage = sources.registry.stages.get(chosen)?;

    drop_ids(stage).next()
}

fn material_names(sources: &Sources<'_>) -> Vec<String> {
    let items = &sources.vault.vds.items;
    let vfs = &sources.vault.vfs;

    (0..figures::mapdrops::WIDTH)
        .map(|index| {
            let slot = index.checked_sub(figures::mapdrops::MATERIAL_FIRST);

            slot.and_then(|slot| stage_materials::MAT_IDS.get(slot))
                .and_then(|id| items.name(vfs, *id))
                .filter(|name| !name.trim().is_empty())
                .map_or_else(String::new, |name| format!("{name} %"))
        })
        .collect()
}

fn combo_anchor(focus: &Focus, id: u32) -> Option<(i32, i32)> {
    let form = i32::try_from(focus.form).ok()?;

    Some((i32::try_from(id).ok()?, form))
}

pub fn address(sources: &Sources<'_>, subject: figures::Subject, id: u32) -> figures::Address {
    match subject {
        figures::Subject::Talents => figures::Address::Keyed(id),
        figures::Subject::Costs => {
            figures::Address::Keyed(talent_costs(sources, Some(id)).first().map_or(FIRST_COST, |cost| u32::from(*cost)))
        }
        _ => figures::Address::Line(id as usize),
    }
}

fn cat_label(sources: &Sources<'_>, focus: &Focus, id: u32) -> String {
    sources
        .cats
        .iter()
        .find(|cat| cat.id == id)
        .and_then(|cat| cat.names.get(focus.form).cloned().flatten())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| format!("{id:03}-C"))
}

pub fn mod_copy(sources: &Sources<'_>, focus: &Focus, file: &str) -> Option<PathBuf> {
    mods::find(&sources.vault.vfs, focus.mount()?, Path::new(file))
}

#[derive(Clone)]
pub struct AssetFile {
    pub name: String,
    pub game: Option<PathBuf>,
    pub mod_copy: Option<PathBuf>,
}

pub fn variant_sets(sources: &Sources<'_>, focus: &Focus, bases: &[&str]) -> Vec<Vec<AssetFile>> {
    let groups: Vec<Vec<String>> = bases.iter().map(|base| copies(&sources.vault.vfs, base)).collect();

    let located: FxHashMap<String, PathBuf> = {
        let names = groups.iter().flatten().map(String::as_str);

        mod_copies(sources, focus, names).into_iter().map(|(name, path)| (name.to_owned(), path)).collect()
    };

    groups
        .into_iter()
        .map(|group| {
            group
                .into_iter()
                .map(|name| AssetFile {
                    game: sources.vault.vfs.rooted(architecture::GAME, &name),
                    mod_copy: located.get(&name).cloned(),
                    name,
                })
                .collect()
        })
        .collect()
}

pub struct Exception {
    pub internal: String,
    pub vanilla: Option<PathBuf>,
    pub mod_copy: Option<PathBuf>,
    pub variants: Vec<AssetFile>,
}

impl Exception {
    pub fn scopes(&self, in_mod: bool) -> Vec<Scope<'_>> {
        if in_mod {
            return vec![Scope {
                name: &self.internal,
                source: self.vanilla.as_deref(),
                present: self.mod_copy.as_deref(),
                title: None,
            }];
        }

        self.variants
            .iter()
            .map(|file| Scope {
                name: &file.name,
                source: file.game.as_deref(),
                present: file.game.as_deref(),
                title: None,
            })
            .collect()
    }
}

pub fn talent_costs(sources: &Sources<'_>, cat: Option<u32>) -> figures::Marks {
    let mut used = figures::Marks::default();

    let Some(id) = cat else {
        return used;
    };

    let Some(talents) =
        sources.cats.iter().find(|cat| cat.id == id).and_then(|cat| cat.talent_data.as_ref())
    else {
        return used;
    };

    let mut costs: Vec<u8> = talents
        .groups
        .iter()
        .filter(|group| group.ability_id != 0)
        .map(|group| group.cost_id)
        .collect();

    costs.sort_unstable();
    costs.dedup();

    for (slot, cost) in used.iter_mut().zip(costs) {
        *slot = cost;
    }

    used
}

pub fn mod_copies<'a>(
    sources: &Sources<'_>,
    focus: &Focus,
    names: impl IntoIterator<Item = &'a str>,
) -> FxHashMap<&'a str, PathBuf> {
    focus.mount().map_or_else(FxHashMap::default, |active| mods::find_all(&sources.vault.vfs, active, names))
}

pub fn focus(pick: &Pick, target: Option<String>) -> Focus {
    Focus {
        realm: pick.pane.realm(),
        cat: pick.cat,
        form: pick.form,
        enemy: pick.enemy,
        category: pick.map.as_ref().map(|map| map.category.clone()),
        map: pick.map.clone(),
        stage: pick.stage.clone(),
        active_mod: target,
        unlocked: false,
    }
}

pub fn figures_plan(sources: &Sources<'_>, pick: &Pick, target: Option<String>, values: EditorMode) -> Option<figures::Plan> {
    let Pane::Figures(subject) = pick.pane else {
        return None;
    };

    let aim = Aim { key: pick.key, line: pick.line, file: Some(&pick.file) };

    current_plan(sources, &focus(pick, target), subject, aim, values)
}

pub fn prose_plan(sources: &Sources<'_>, pick: &Pick, target: Option<String>) -> Option<prose::Plan> {
    let Pane::Prose(subject) = pick.pane else {
        return None;
    };

    let focus = focus(pick, target);

    let held = match subject {
        prose::Subject::CatGlossary | prose::Subject::EnemyGlossary => {
            let (identity, faction) = pick.aim?;

            glossary_target(sources, &focus, identity, faction)
        }
        prose::Subject::ItemName => item_name_target(sources, &focus, pick.key),
        prose::Subject::ComboName => combo_name_target(sources, &focus, pick.line),
        _ => prose_target(sources, &focus, subject),
    }?;

    let mut plans = prose_plans(&held);
    let at = plans.iter().position(|plan| plan.file() == pick.file)?;

    Some(plans.swap_remove(at))
}


// The name file has no header, so an item's catalog line is its line here outright --
// codex's `icon_index` states that pairing.
pub fn item_name_target(sources: &Sources<'_>, focus: &Focus, item: Option<u32>) -> Option<ProseTarget> {
    if focus.realm != Realm::Stages {
        return None;
    }

    let item = item.or_else(|| first_drop(sources, focus))?;
    let row = sources.vault.vds.items.line(&sources.vault.vfs, item)?;

    Some(ProseTarget {
        subject: prose::Subject::ItemName,
        asset: Asset::Exception(exception(sources, focus, stage_files::GATYA_ITEM_NAME.to_owned())?),
        label: item_label(sources, item),
        row,
        rows: Vec::new(),
        keyed: None,
        cell: None,
        unlocked: focus.unlocked,
        active_mod: focus.active_mod.clone(),
        aim: None,
    })
}

pub fn glossary_target(sources: &Sources<'_>, focus: &Focus, identity: Identity, faction: Faction) -> Option<ProseTarget> {
    let (subject, page) = match faction {
        Faction::Cat => (prose::Subject::CatGlossary, Realm::Cats),
        Faction::Enemy => (prose::Subject::EnemyGlossary, Realm::Enemies),
    };

    if focus.realm != page {
        return None;
    }

    let row = usize::from(sources.names.glossary_line(identity, faction)?).checked_sub(1)?;

    Some(ProseTarget {
        subject,
        asset: Asset::Exception(exception(sources, focus, glossary_file(faction).to_owned())?),
        label: sources.names.ability(identity, faction).to_owned(),
        row,
        rows: Vec::new(),
        keyed: None,
        cell: None,
        unlocked: focus.unlocked,
        active_mod: focus.active_mod.clone(),
        aim: Some((identity, faction)),
    })
}

pub fn prose_target(sources: &Sources<'_>, focus: &Focus, subject: prose::Subject) -> Option<ProseTarget> {
    match subject {
        prose::Subject::Explanation => explanation_target(sources, focus, focus.cat?),
        prose::Subject::EnemyName => enemy_name_target(sources, focus),
        prose::Subject::EnemyDescription => enemy_description_target(sources, focus),
        prose::Subject::ComboName => combo_name_target(sources, focus, None),
        prose::Subject::TalentText => talent_text_target(sources, focus),
        prose::Subject::MapName => map_name_target(sources, focus, None),
        prose::Subject::StageName => stage_name_target(sources, focus, None),
        prose::Subject::ItemName => item_name_target(sources, focus, None),
        prose::Subject::CatGlossary | prose::Subject::EnemyGlossary => None,
    }
}

pub fn combo_name_target(sources: &Sources<'_>, focus: &Focus, line: Option<usize>) -> Option<ProseTarget> {
    if focus.realm != Realm::Cats {
        return None;
    }

    let id = focus.cat?;
    let row = line.or_else(|| combo_line(sources, id, focus.form))?;
    let files = asset_files(sources, focus, cat_files::NYANCOMBO_NAME);

    if files.is_empty() {
        return None;
    }

    Some(ProseTarget {
        subject: prose::Subject::ComboName,
        asset: Asset::Variants { key: cat_files::NYANCOMBO_NAME.to_owned(), files },
        label: [cat_label(sources, focus, id).as_str(), cat_files::NYANCOMBO_NAME].join(SEPARATOR),
        row,
        rows: Vec::new(),
        keyed: None,
        cell: None,
        unlocked: focus.unlocked,
        active_mod: focus.active_mod.clone(),
        aim: None,
    })
}

pub fn prose_plans(target: &ProseTarget) -> Vec<prose::Plan> {
    let mount = mount(target.active_mod.as_deref(), target.unlocked);

    plans(target, &target.asset.scopes(mount.target.is_some()), mount.target).into_iter().flatten().collect()
}

pub fn exception(sources: &Sources<'_>, focus: &Focus, wanted: String) -> Option<Exception> {
    exceptions(sources, focus, vec![wanted]).pop()
}

pub struct ProseTarget {
    pub subject: prose::Subject,
    pub asset: Asset,
    pub label: String,
    pub row: usize,
    pub rows: Vec<usize>,
    pub keyed: Option<prose::Keyed>,
    pub cell: Option<usize>,
    pub unlocked: bool,
    pub active_mod: Option<String>,
    pub aim: Option<(Identity, Faction)>,
}

impl ProseTarget {
    pub fn keyed(self, keyed: prose::Keyed) -> ProseTarget {
        ProseTarget { keyed: Some(keyed), ..self }
    }

    fn celled(self, cell: usize) -> ProseTarget {
        ProseTarget { cell: Some(cell), ..self }
    }

}

pub fn combo_line(sources: &Sources<'_>, id: u32, form: usize) -> Option<usize> {
    cat_combo::combo_lines(sources.vault, id, form).first().copied()
}

fn talent_text_target(sources: &Sources<'_>, focus: &Focus) -> Option<ProseTarget> {
    if focus.realm != Realm::Cats {
        return None;
    }

    let id = focus.cat?;
    let rows = talent_texts(sources, id);

    Some(ProseTarget {
        subject: prose::Subject::TalentText,
        asset: Asset::Exception(exception(sources, focus, cat_files::SKILL_DESCRIPTIONS.to_owned())?),
        label: [cat_label(sources, focus, id).as_str(), cat_files::SKILL_DESCRIPTIONS].join(SEPARATOR),
        row: rows.first().copied().unwrap_or(FIRST_TALENT_TEXT),
        rows,
        keyed: None,
        cell: None,
        unlocked: focus.unlocked,
        active_mod: focus.active_mod.clone(),
        aim: None,
    })
}

pub fn map_name_target(sources: &Sources<'_>, focus: &Focus, map: Option<u32>) -> Option<ProseTarget> {
    if focus.realm != Realm::Stages {
        return None;
    }

    let key = map_key(focus, map)?;
    let registry = &sources.registry;
    let global = registry.addresses.get(&key)?.global?;

    let label = registry
        .maps
        .get(&key)
        .map(|found| found.name.trim())
        .filter(|name| !name.is_empty())
        .map_or_else(|| format!("{:03}", key.map), str::to_owned);

    Some(ProseTarget {
        subject: prose::Subject::MapName,
        asset: Asset::Exception(exception(sources, focus, stage_files::MAP_NAME.to_owned())?),
        label,
        row: 0,
        rows: Vec::new(),
        keyed: None,
        cell: None,
        unlocked: focus.unlocked,
        active_mod: focus.active_mod.clone(),
        aim: None,
    })
    .map(|target| target.keyed(prose::Keyed::Map(global)))
}

// The story chapters put one stage per line and read the first cell; every other chapter
// puts a whole map on one line and reads the cell at the stage index.
pub fn stage_name_target(sources: &Sources<'_>, focus: &Focus, stage: Option<u32>) -> Option<ProseTarget> {
    if focus.realm != Realm::Stages {
        return None;
    }

    let key = stage_key(focus, stage)?;
    let registry = &sources.registry;
    let map = GlobalMapId { category: key.category.clone(), map: key.map };
    let file = registry.addresses.get(&map)?.name_file.as_deref()?;

    let (keyed, cell) = names::stage_name_address(&key.category.map_prefix(), key.map, key.stage);

    let label = registry
        .stages
        .get(&key)
        .map(|found| found.name.trim())
        .filter(|name| !name.is_empty())
        .map_or_else(|| format!("{:02}", key.stage), str::to_owned);

    let files = asset_files(sources, focus, file);

    if files.is_empty() {
        return None;
    }

    Some(ProseTarget {
        subject: prose::Subject::StageName,
        asset: Asset::Variants { key: file.to_owned(), files },
        label,
        row: 0,
        rows: Vec::new(),
        keyed: None,
        cell: None,
        unlocked: focus.unlocked,
        active_mod: focus.active_mod.clone(),
        aim: None,
    })
    .map(|target| target.keyed(prose::Keyed::Stage(keyed)).celled(cell))
}

fn enemy_name_target(sources: &Sources<'_>, focus: &Focus) -> Option<ProseTarget> {
    let id = focus.enemy?;

    Some(ProseTarget {
        subject: prose::Subject::EnemyName,
        asset: Asset::Exception(exception(sources, focus, enemy_files::NAMES.to_owned())?),
        label: enemy_label(sources, id),
        row: id as usize,
        rows: Vec::new(),
        keyed: None,
        cell: None,
        unlocked: focus.unlocked,
        active_mod: focus.active_mod.clone(),
        aim: None,
    })
}

fn enemy_description_target(sources: &Sources<'_>, focus: &Focus) -> Option<ProseTarget> {
    let id = focus.enemy?;
    let files = asset_files(sources, focus, enemy_files::PICTURE_BOOK);

    if files.is_empty() {
        return None;
    }

    Some(ProseTarget {
        subject: prose::Subject::EnemyDescription,
        asset: Asset::Variants { key: enemy_files::PICTURE_BOOK.to_owned(), files },
        label: enemy_label(sources, id),
        row: id as usize,
        rows: Vec::new(),
        keyed: None,
        cell: None,
        unlocked: focus.unlocked,
        active_mod: focus.active_mod.clone(),
        aim: None,
    })
}

fn explanation_target(sources: &Sources<'_>, focus: &Focus, id: u32) -> Option<ProseTarget> {
    let form = focus.form;

    cat_waiter::unitexplanation_source(&sources.vault.vfs, id, form)?;

    let key = cat_files::explanation_file(id);
    let files = asset_files(sources, focus, &key);

    if files.is_empty() {
        return None;
    }

    let name = sources
        .cats
        .iter()
        .find(|cat| cat.id == id)
        .and_then(|cat| cat.names.get(form).cloned().flatten());

    let form_label = figures::FORMS.get(form).copied().unwrap_or("Unknown");

    let label = match name.as_deref() {
        Some(name) if !name.is_empty() => [name, form_label].join(SEPARATOR),
        _ => form_label.to_owned(),
    };

    Some(ProseTarget {
        subject: prose::Subject::Explanation,
        asset: Asset::Variants { key, files },
        label,
        row: form,
        rows: Vec::new(),
        keyed: None,
        cell: None,
        unlocked: focus.unlocked,
        active_mod: focus.active_mod.clone(),
        aim: None,
    })
}

pub fn mount<'a>(active_mod: Option<&'a str>, unlocked: bool) -> Mount<'a> {
    match active_mod {
        Some(name) => Mount { name, target: Some(name), locked: false },
        None => Mount { name: architecture::GAME, target: None, locked: !unlocked },
    }
}

pub fn plans(target: &ProseTarget, scopes: &[Scope<'_>], target_mod: Option<&str>) -> Vec<Option<prose::Plan>> {
    scopes
        .iter()
        .map(|scope| {
            let source = scope.source.or(scope.present)?;

            let made = prose::plan(
                target.subject,
                target.row,
                [target.label.as_str(), scope.name].join(SEPARATOR),
                scope.name.to_owned(),
                source,
                target_mod.map(str::to_owned),
            );

            let made = match target.keyed {
                Some(keyed) => made.keyed(keyed),
                None => made,
            };

            let made = match target.cell {
                Some(cell) => made.at(cell),
                None => made,
            };

            let made = made.sourced().aiming(target.aim);

            prose::seated(&made).then(|| made.over(target.rows.clone()))
        })
        .collect()
}

pub fn exceptions(sources: &Sources<'_>, focus: &Focus, wanted: Vec<String>) -> Vec<Exception> {
    let visible: Vec<String> = wanted.iter().map(|name| visible_name(sources, name)).collect();

    let internal: Vec<String> = visible.iter().map(|name| internal_name(sources, name)).collect();

    let groups: Vec<Vec<String>> = internal.iter().map(|name| copies(&sources.vault.vfs, name)).collect();

    let located: FxHashMap<String, PathBuf> = {
        let names = internal.iter().map(String::as_str).chain(groups.iter().flatten().map(String::as_str));

        mod_copies(sources, focus, names).into_iter().map(|(name, path)| (name.to_owned(), path)).collect()
    };

    visible
        .into_iter()
        .zip(internal)
        .zip(groups)
        .map(|((seen, internal), group)| {
            let variants: Vec<AssetFile> = group
                .into_iter()
                .map(|name| AssetFile {
                    game: sources.vault.vfs.rooted(architecture::GAME, &name),
                    mod_copy: located.get(&name).cloned(),
                    name,
                })
                .collect();

            let vanilla = sources
                .vault
                .vfs
                .rooted(architecture::GAME, &seen)
                .or_else(|| variants.iter().find_map(|file| file.game.clone()));

            Exception { mod_copy: located.get(&internal).cloned(), variants, vanilla, internal }
        })
        .collect()
}

fn talent_texts(sources: &Sources<'_>, id: u32) -> Vec<usize> {
    let Some(talents) = sources
        .cats
        .iter()
        .find(|cat| cat.id == id)
        .and_then(|cat| cat.talent_data.as_ref())
    else {
        return Vec::new();
    };

    let mut held: Vec<usize> = talents
        .groups
        .iter()
        .filter(|group| group.ability_id != 0)
        .map(|group| usize::from(group.text_id))
        .collect();

    held.sort_unstable();
    held.dedup();

    held
}

fn map_key(focus: &Focus, map: Option<u32>) -> Option<GlobalMapId> {
    let Some(map) = map else {
        return focus.map.clone();
    };

    Some(GlobalMapId { category: focus.category.clone()?, map })
}

fn stage_key(focus: &Focus, stage: Option<u32>) -> Option<GlobalStageId> {
    let Some(stage) = stage else {
        return focus.stage.clone();
    };

    let map = focus.map.as_ref()?;

    Some(GlobalStageId { category: map.category.clone(), map: map.map, stage })
}

#[derive(Clone, Copy)]
pub struct Mount<'a> {
    pub name: &'a str,
    pub target: Option<&'a str>,
    pub locked: bool,
}

fn enemy_label(sources: &Sources<'_>, id: u32) -> String {
    sources
        .enemies
        .iter()
        .find(|enemy| enemy.id == id)
        .map_or_else(|| format!("{id:03}-E"), EnemyEntry::display_name)
}
