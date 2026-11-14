mod cards;
mod combat;
mod combos;
mod costs;
mod mapdata;
mod itembuy;
mod mapdrops;
mod mapoption;
mod backgrounds;
mod upgrades;
mod comboparams;
mod orbslots;
mod difficulty;
mod exoption;
mod cannonstatus;
mod expcurve;
mod orbgrades;
mod exlottery;
mod shakes;
mod recipes;
mod shop;
mod zombies;
mod cpu;
mod orbtraits;
mod altar;
mod gamatoto;
mod progress;
mod castlegrowth;
mod castles;
mod chance;
mod club;
mod combocategories;
mod labyrinth;
mod mapsheets;
mod enigma;
mod shrine;
mod prizes;
mod guide;
mod development;
mod missionsettings;
mod popups;
mod quest;
mod sounds;
mod realms;
mod tiprules;
mod gacha;
mod missions;
mod conditions;
pub(super) mod resolved;
mod schema;
mod talents;
mod unitbuy;
mod unitlevel;

pub(crate) use mapdata::MAP_HEADER_LINES;
pub(crate) use schema::{Subject, COUNT, SUBJECTS};

use std::path::PathBuf;
use std::sync::LazyLock;

use iced::widget::{operation, scrollable};
use iced::{Element, Size, Task};

use foundry::common::preview::{self, Stamp};
use foundry::systems::combat::NameBook;
use foundry::domains::cat::scanner::CatEntry;
use foundry::{Vault, Vfs};

pub(crate) use foundry::systems::editor::figures::*;

use crate::common::feedback::Slot;
use crate::widget::popup;


use cards::CARD_WIDTH;

const LABEL_SIZE: f32 = 12.0;

const CARD_PADDING: f32 = 6.0;

const LINE_RATIO: f32 = 1.3;

const GLYPH_RATIO: f32 = 0.55;

static LABEL_HEIGHTS: LazyLock<[f32; COUNT]> =
    LazyLock::new(|| SUBJECTS.map(|subject| label_height(schema::of(subject))));

fn label_height(schema: &schema::Schema) -> f32 {
    let inner = CARD_WIDTH - CARD_PADDING * 2.0;
    let per_line = ((inner / (LABEL_SIZE * GLYPH_RATIO)).floor() as usize).max(1);

    let lines = (0..schema.known())
        .map(|index| wrapped_lines(&schema.label(index), per_line))
        .max()
        .unwrap_or(1);

    lines as f32 * LABEL_SIZE * LINE_RATIO
}

const POPUP_SIZE: Size = Size::new(364.0, 376.0);

const CAT_POPUP: popup::Spec = popup::Spec::new(popup::Kind::CatAttributes, POPUP_SIZE);
const ENEMY_POPUP: popup::Spec = popup::Spec::new(popup::Kind::EnemyAttributes, POPUP_SIZE);
const BUY_POPUP: popup::Spec = popup::Spec::new(popup::Kind::UnitBuy, POPUP_SIZE);
const CURVE_POPUP: popup::Spec = popup::Spec::new(popup::Kind::LevelCurve, POPUP_SIZE);
const TALENT_SIZE: Size = Size::new(760.0, 540.0);
const TALENT_POPUP: popup::Spec = popup::Spec::new(popup::Kind::Talents, TALENT_SIZE);
const COSTS_SIZE: Size = Size::new(364.0, 520.0);
const COSTS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::TalentCosts, COSTS_SIZE);
pub(super) const COMBO_SIZE: Size = Size::new(420.0, 296.0);
const COMBO_POPUP: popup::Spec = popup::Spec::new(popup::Kind::Combos, COMBO_SIZE);
const MAP_STAGE_SIZE: Size = Size::new(364.0, 520.0);
const MAP_DROPS_SIZE: Size = Size::new(364.0, 520.0);
const MAP_DROPS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::MapDrops, MAP_DROPS_SIZE);
const DROP_CHARA_SIZE: Size = Size::new(364.0, 300.0);
const DROP_CHARA_POPUP: popup::Spec = popup::Spec::new(popup::Kind::DropChara, DROP_CHARA_SIZE);
const ITEM_BUY_SIZE: Size = Size::new(364.0, 460.0);
const ITEM_BUY_POPUP: popup::Spec = popup::Spec::new(popup::Kind::ItemBuy, ITEM_BUY_SIZE);
const MAP_STAGE_POPUP: popup::Spec = popup::Spec::new(popup::Kind::MapStage, MAP_STAGE_SIZE);
const MAP_OPTION_SIZE: Size = Size::new(364.0, 460.0);
const MAP_OPTION_POPUP: popup::Spec = popup::Spec::new(popup::Kind::MapOption, MAP_OPTION_SIZE);
const BACKGROUND_SIZE: Size = Size::new(364.0, 460.0);
const BACKGROUND_POPUP: popup::Spec = popup::Spec::new(popup::Kind::Background, BACKGROUND_SIZE);
const UPGRADE_SIZE: Size = Size::new(364.0, 360.0);
const UPGRADE_POPUP: popup::Spec = popup::Spec::new(popup::Kind::Upgrade, UPGRADE_SIZE);
const COMBO_PARAM_SIZE: Size = Size::new(364.0, 380.0);
const COMBO_PARAM_POPUP: popup::Spec = popup::Spec::new(popup::Kind::ComboParam, COMBO_PARAM_SIZE);
const ORB_SLOTS_SIZE: Size = Size::new(364.0, 320.0);
const ORB_SLOTS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::OrbSlots, ORB_SLOTS_SIZE);
const DIFFICULTY_SIZE: Size = Size::new(364.0, 460.0);
const DIFFICULTY_POPUP: popup::Spec = popup::Spec::new(popup::Kind::Difficulty, DIFFICULTY_SIZE);
const EX_INVASION_SIZE: Size = Size::new(364.0, 280.0);
const EX_INVASION_POPUP: popup::Spec = popup::Spec::new(popup::Kind::ExInvasion, EX_INVASION_SIZE);
const CANNON_STATUS_SIZE: Size = Size::new(364.0, 400.0);
const CANNON_STATUS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::CannonStatus, CANNON_STATUS_SIZE);
const EXP_CURVE_SIZE: Size = Size::new(364.0, 520.0);
const EXP_CURVE_POPUP: popup::Spec = popup::Spec::new(popup::Kind::ExpCurve, EXP_CURVE_SIZE);
const ORB_GRADES_SIZE: Size = Size::new(364.0, 300.0);
const ORB_GRADES_POPUP: popup::Spec = popup::Spec::new(popup::Kind::OrbGrades, ORB_GRADES_SIZE);
const EX_LOTTERY_SIZE: Size = Size::new(364.0, 260.0);
const EX_LOTTERY_POPUP: popup::Spec = popup::Spec::new(popup::Kind::ExLottery, EX_LOTTERY_SIZE);
const SHAKES_SIZE: Size = Size::new(364.0, 380.0);
const SHAKES_POPUP: popup::Spec = popup::Spec::new(popup::Kind::Shakes, SHAKES_SIZE);
const RECIPES_SIZE: Size = Size::new(364.0, 380.0);
const RECIPES_POPUP: popup::Spec = popup::Spec::new(popup::Kind::Recipes, RECIPES_SIZE);
const SHOP_SIZE: Size = Size::new(364.0, 380.0);
const SHOP_POPUP: popup::Spec = popup::Spec::new(popup::Kind::Shop, SHOP_SIZE);
const ZOMBIE_SIZE: Size = Size::new(364.0, 320.0);
const ZOMBIE_HEADER_POPUP: popup::Spec = popup::Spec::new(popup::Kind::ZombieHeader, ZOMBIE_SIZE);
const ZOMBIE_TIER_POPUP: popup::Spec = popup::Spec::new(popup::Kind::ZombieTier, ZOMBIE_SIZE);
const CPU_SIZE: Size = Size::new(364.0, 300.0);
const CPU_POPUP: popup::Spec = popup::Spec::new(popup::Kind::Cpu, CPU_SIZE);
const ORB_TRAITS_SIZE: Size = Size::new(364.0, 460.0);
const ORB_TRAITS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::OrbTraits, ORB_TRAITS_SIZE);
const ALTAR_SIZE: Size = Size::new(364.0, 300.0);
const ALTAR_POPUP: popup::Spec = popup::Spec::new(popup::Kind::Altar, ALTAR_SIZE);
const GAMATOTO_SIZE: Size = Size::new(364.0, 360.0);
const GAMATOTO_LIMIT_POPUP: popup::Spec = popup::Spec::new(popup::Kind::GamatotoLimit, GAMATOTO_SIZE);
const GAMATOTO_UNLOCK_POPUP: popup::Spec = popup::Spec::new(popup::Kind::GamatotoUnlock, GAMATOTO_SIZE);
const GAMATOTO_DROP_POPUP: popup::Spec = popup::Spec::new(popup::Kind::GamatotoDrop, GAMATOTO_SIZE);
const PROGRESS_SIZE: Size = Size::new(364.0, 280.0);
const STAMPS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::Stamps, PROGRESS_SIZE);
const SLOT_UNLOCKS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::SlotUnlocks, PROGRESS_SIZE);
const CLUB_SIZE: Size = Size::new(364.0, 460.0);
const CLUB_POPUP: popup::Spec = popup::Spec::new(popup::Kind::Club, CLUB_SIZE);
const MISSIONS_SIZE: Size = Size::new(364.0, 380.0);
const MISSIONS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::Missions, MISSIONS_SIZE);
const CONDITIONS_SIZE: Size = Size::new(364.0, 460.0);
const CONDITIONS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::Conditions, CONDITIONS_SIZE);
const MISSION_UNLOCKS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::MissionUnlocks, CONDITIONS_SIZE);
const MISSION_GROUPS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::MissionGroups, CONDITIONS_SIZE);
const GACHA_OPTIONS_SIZE: Size = Size::new(364.0, 460.0);
const GACHA_OPTIONS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::GachaOptions, GACHA_OPTIONS_SIZE);
const CHANCE_ANIMATIONS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::ChanceAnimations, GACHA_OPTIONS_SIZE);
const CASTLES_SIZE: Size = Size::new(364.0, 300.0);
const CASTLES_POPUP: popup::Spec = popup::Spec::new(popup::Kind::Castles, CASTLES_SIZE);
const TIP_RULES_SIZE: Size = Size::new(364.0, 520.0);
const TIP_RULES_POPUP: popup::Spec = popup::Spec::new(popup::Kind::TipRules, TIP_RULES_SIZE);
const QUEST_SIZE: Size = Size::new(364.0, 380.0);
const QUEST_STAGES_POPUP: popup::Spec = popup::Spec::new(popup::Kind::QuestStages, QUEST_SIZE);
const SLOT_LOTTERY_POPUP: popup::Spec = popup::Spec::new(popup::Kind::SlotLottery, QUEST_SIZE);
const REALM_SIZE: Size = Size::new(364.0, 300.0);
const REALM_ALTAR_POPUP: popup::Spec = popup::Spec::new(popup::Kind::RealmAltar, REALM_SIZE);
const REALM_DRAWS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::RealmDraws, REALM_SIZE);
const LABYRINTH_FLOORS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::LabyrinthFloors, REALM_SIZE);
const LABYRINTH_LOTTERY_POPUP: popup::Spec = popup::Spec::new(popup::Kind::LabyrinthLottery, QUEST_SIZE);
const MISSION_SHORTCUTS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::MissionShortcuts, REALM_SIZE);
const MISSION_DESIGNATIONS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::MissionDesignations, REALM_SIZE);
const MONTHLY_MISSIONS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::MonthlyMissions, QUEST_SIZE);
const SOUND_SETTINGS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::SoundSettings, REALM_SIZE);
const DOJO_CHESTS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::DojoChests, QUEST_SIZE);
const POPUP_TRIGGERS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::PopupTriggers, TIP_RULES_SIZE);
const COMBO_CATEGORIES_POPUP: popup::Spec = popup::Spec::new(popup::Kind::ComboCategories, QUEST_SIZE);
const CASTLE_GROWTH_POPUP: popup::Spec = popup::Spec::new(popup::Kind::CastleGrowth, REALM_SIZE);
const DEMON_CASTLES_POPUP: popup::Spec = popup::Spec::new(popup::Kind::DemonCastles, REALM_SIZE);
const LEVEL_UP_OFFERS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::LevelUpOffers, REALM_SIZE);
const REEL_SYMBOLS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::ReelSymbols, REALM_SIZE);
const ORB_FILTER_CHOICES_POPUP: popup::Spec = popup::Spec::new(popup::Kind::OrbFilterChoices, REALM_SIZE);
const MAP_SHEETS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::MapSheets, TIP_RULES_SIZE);
const MENU_TEXT_SETTINGS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::MenuTextSettings, TIP_RULES_SIZE);
const TRAIT_ICONS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::TraitIcons, TIP_RULES_SIZE);
const ABILITY_ICONS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::AbilityIcons, TIP_RULES_SIZE);
const LINEUP_RANK_GROUPS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::LineupRankGroups, TIP_RULES_SIZE);
const LINEUP_BANDS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::LineupBands, TIP_RULES_SIZE);
const LINEUP_RATINGS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::LineupRatings, TIP_RULES_SIZE);
const ENGINEER_CAP_POPUP: popup::Spec = popup::Spec::new(popup::Kind::EngineerCap, REALM_SIZE);
const ENIGMA_RARITIES_POPUP: popup::Spec = popup::Spec::new(popup::Kind::EnigmaRarities, REALM_SIZE);
const TIP_SETTINGS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::TipSettings, TIP_RULES_SIZE);
const GUIDE_SPOTLIGHTS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::GuideSpotlights, REALM_SIZE);
const FANFARE_TIERS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::FanfareTiers, QUEST_SIZE);
const STAGE_SHORTCUTS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::StageShortcuts, REALM_SIZE);
const ENERGY_REFUNDS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::EnergyRefunds, REALM_SIZE);
const ORB_OPTIONS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::OrbOptions, REALM_SIZE);
const LINEUP_COUNTERS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::LineupCounters, REALM_SIZE);
const RECOMMENDED_UPGRADES_POPUP: popup::Spec = popup::Spec::new(popup::Kind::RecommendedUpgrades, REALM_SIZE);
const ENIGMA_TIMES_POPUP: popup::Spec = popup::Spec::new(popup::Kind::EnigmaTimes, REALM_SIZE);
const MAP_MARKERS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::MapMarkers, REALM_SIZE);
const DISCOVERY_MEDALS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::DiscoveryMedals, TIP_RULES_SIZE);
const ENIGMA_GROUPS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::EnigmaGroups, TIP_RULES_SIZE);
const ENIGMA_DROPS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::EnigmaDrops, TIP_RULES_SIZE);
const ENIGMA_RATES_POPUP: popup::Spec = popup::Spec::new(popup::Kind::EnigmaRates, TIP_RULES_SIZE);
const ENIGMA_LOTTERIES_POPUP: popup::Spec = popup::Spec::new(popup::Kind::EnigmaLotteries, TIP_RULES_SIZE);
const LINEUP_EXCLUSIONS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::LineupExclusions, REALM_SIZE);
const FILTER_MAPS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::FilterMaps, REALM_SIZE);
const ZONED_IN_STEPS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::ZonedInSteps, TIP_RULES_SIZE);
const ZONED_IN_POPUP: popup::Spec = popup::Spec::new(popup::Kind::ZonedIn, REALM_SIZE);
const FORTUNE_ODDS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::FortuneOdds, QUEST_SIZE);
const SHRINE_EXTRAS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::ShrineExtras, REALM_SIZE);
const SHRINE_OFFERINGS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::ShrineOfferings, TIP_RULES_SIZE);
const PRIZE_PATTERNS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::PrizePatterns, QUEST_SIZE);
const GAMATOTO_LEVELS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::GamatotoLevels, REALM_SIZE);
const EXPEDITION_AREAS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::ExpeditionAreas, TIP_RULES_SIZE);
const ENEMY_GUIDE_POPUP: popup::Spec = popup::Spec::new(popup::Kind::EnemyGuide, REALM_SIZE);
const UNIT_LIMITS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::UnitLimits, TIP_RULES_SIZE);
const GUIDE_ENTRIES_POPUP: popup::Spec = popup::Spec::new(popup::Kind::GuideEntries, TIP_RULES_SIZE);
const MATERIAL_MIXES_POPUP: popup::Spec = popup::Spec::new(popup::Kind::MaterialMixes, QUEST_SIZE);
const DEVELOPMENT_TIERS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::DevelopmentTiers, QUEST_SIZE);
const DEVELOPMENT_COSTS_POPUP: popup::Spec = popup::Spec::new(popup::Kind::DevelopmentCosts, TIP_RULES_SIZE);

pub(super) fn kind(subject: Subject) -> popup::Kind {
    spec(subject).kind()
}

fn spec(subject: Subject) -> popup::Spec {
    match subject {
        Subject::Cat => CAT_POPUP,
        Subject::Enemy => ENEMY_POPUP,
        Subject::Buy => BUY_POPUP,
        Subject::Curve => CURVE_POPUP,
        Subject::Talents => TALENT_POPUP,
        Subject::Costs => COSTS_POPUP,
        Subject::Combo => COMBO_POPUP,
        Subject::MapStage => MAP_STAGE_POPUP,
        Subject::MapDrops => MAP_DROPS_POPUP,
        Subject::DropChara => DROP_CHARA_POPUP,
        Subject::ItemBuy => ITEM_BUY_POPUP,
        Subject::MapOption => MAP_OPTION_POPUP,
        Subject::Background => BACKGROUND_POPUP,
        Subject::Upgrade => UPGRADE_POPUP,
        Subject::ComboParam => COMBO_PARAM_POPUP,
        Subject::OrbSlots => ORB_SLOTS_POPUP,
        Subject::Difficulty => DIFFICULTY_POPUP,
        Subject::ExInvasion => EX_INVASION_POPUP,
        Subject::CannonStatus => CANNON_STATUS_POPUP,
        Subject::ExpCurve => EXP_CURVE_POPUP,
        Subject::OrbGrades => ORB_GRADES_POPUP,
        Subject::ExLottery => EX_LOTTERY_POPUP,
        Subject::Shakes => SHAKES_POPUP,
        Subject::Recipes => RECIPES_POPUP,
        Subject::Shop => SHOP_POPUP,
        Subject::ZombieHeader => ZOMBIE_HEADER_POPUP,
        Subject::ZombieTier => ZOMBIE_TIER_POPUP,
        Subject::Cpu => CPU_POPUP,
        Subject::OrbTraits => ORB_TRAITS_POPUP,
        Subject::Altar => ALTAR_POPUP,
        Subject::GamatotoLimit => GAMATOTO_LIMIT_POPUP,
        Subject::GamatotoUnlock => GAMATOTO_UNLOCK_POPUP,
        Subject::GamatotoDrop => GAMATOTO_DROP_POPUP,
        Subject::Stamps => STAMPS_POPUP,
        Subject::SlotUnlocks => SLOT_UNLOCKS_POPUP,
        Subject::Club => CLUB_POPUP,
        Subject::Missions => MISSIONS_POPUP,
        Subject::Conditions => CONDITIONS_POPUP,
        Subject::MissionUnlocks => MISSION_UNLOCKS_POPUP,
        Subject::MissionGroups => MISSION_GROUPS_POPUP,
        Subject::GachaOptions => GACHA_OPTIONS_POPUP,
        Subject::ChanceAnimations => CHANCE_ANIMATIONS_POPUP,
        Subject::Castles => CASTLES_POPUP,
        Subject::TipRules => TIP_RULES_POPUP,
        Subject::QuestStages => QUEST_STAGES_POPUP,
        Subject::SlotLottery => SLOT_LOTTERY_POPUP,
        Subject::RealmAltar => REALM_ALTAR_POPUP,
        Subject::RealmDraws => REALM_DRAWS_POPUP,
        Subject::LabyrinthFloors => LABYRINTH_FLOORS_POPUP,
        Subject::LabyrinthLottery => LABYRINTH_LOTTERY_POPUP,
        Subject::MissionShortcuts => MISSION_SHORTCUTS_POPUP,
        Subject::MissionDesignations => MISSION_DESIGNATIONS_POPUP,
        Subject::MonthlyMissions => MONTHLY_MISSIONS_POPUP,
        Subject::SoundSettings => SOUND_SETTINGS_POPUP,
        Subject::DojoChests => DOJO_CHESTS_POPUP,
        Subject::PopupTriggers => POPUP_TRIGGERS_POPUP,
        Subject::ComboCategories => COMBO_CATEGORIES_POPUP,
        Subject::CastleGrowth => CASTLE_GROWTH_POPUP,
        Subject::DemonCastles => DEMON_CASTLES_POPUP,
        Subject::LevelUpOffers => LEVEL_UP_OFFERS_POPUP,
        Subject::ReelSymbols => REEL_SYMBOLS_POPUP,
        Subject::OrbFilterChoices => ORB_FILTER_CHOICES_POPUP,
        Subject::MapSheets => MAP_SHEETS_POPUP,
        Subject::MenuTextSettings => MENU_TEXT_SETTINGS_POPUP,
        Subject::TraitIcons => TRAIT_ICONS_POPUP,
        Subject::AbilityIcons => ABILITY_ICONS_POPUP,
        Subject::LineupRankGroups => LINEUP_RANK_GROUPS_POPUP,
        Subject::LineupBands => LINEUP_BANDS_POPUP,
        Subject::LineupRatings => LINEUP_RATINGS_POPUP,
        Subject::EngineerCap => ENGINEER_CAP_POPUP,
        Subject::EnigmaRarities => ENIGMA_RARITIES_POPUP,
        Subject::TipSettings => TIP_SETTINGS_POPUP,
        Subject::GuideSpotlights => GUIDE_SPOTLIGHTS_POPUP,
        Subject::FanfareTiers => FANFARE_TIERS_POPUP,
        Subject::StageShortcuts => STAGE_SHORTCUTS_POPUP,
        Subject::EnergyRefunds => ENERGY_REFUNDS_POPUP,
        Subject::OrbOptions => ORB_OPTIONS_POPUP,
        Subject::LineupCounters => LINEUP_COUNTERS_POPUP,
        Subject::RecommendedUpgrades => RECOMMENDED_UPGRADES_POPUP,
        Subject::EnigmaTimes => ENIGMA_TIMES_POPUP,
        Subject::MapMarkers => MAP_MARKERS_POPUP,
        Subject::DiscoveryMedals => DISCOVERY_MEDALS_POPUP,
        Subject::EnigmaGroups => ENIGMA_GROUPS_POPUP,
        Subject::EnigmaDrops => ENIGMA_DROPS_POPUP,
        Subject::EnigmaRates => ENIGMA_RATES_POPUP,
        Subject::EnigmaLotteries => ENIGMA_LOTTERIES_POPUP,
        Subject::LineupExclusions => LINEUP_EXCLUSIONS_POPUP,
        Subject::FilterMaps => FILTER_MAPS_POPUP,
        Subject::ZonedInSteps => ZONED_IN_STEPS_POPUP,
        Subject::ZonedIn => ZONED_IN_POPUP,
        Subject::FortuneOdds => FORTUNE_ODDS_POPUP,
        Subject::ShrineExtras => SHRINE_EXTRAS_POPUP,
        Subject::ShrineOfferings => SHRINE_OFFERINGS_POPUP,
        Subject::PrizePatterns => PRIZE_PATTERNS_POPUP,
        Subject::GamatotoLevels => GAMATOTO_LEVELS_POPUP,
        Subject::ExpeditionAreas => EXPEDITION_AREAS_POPUP,
        Subject::EnemyGuide => ENEMY_GUIDE_POPUP,
        Subject::UnitLimits => UNIT_LIMITS_POPUP,
        Subject::GuideEntries => GUIDE_ENTRIES_POPUP,
        Subject::MaterialMixes => MATERIAL_MIXES_POPUP,
        Subject::DevelopmentTiers => DEVELOPMENT_TIERS_POPUP,
        Subject::DevelopmentCosts => DEVELOPMENT_COSTS_POPUP,
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Popup(popup::Message),
    Changed(usize, String),
    Picked(usize, i32),
    Scrolled(f32),
    Picker(Option<usize>),
    CommentChanged(String),
    SearchChanged(String),
    HuntChanged(String),
    Aimed(Address),
    Slotted(usize, i32, i32),
    Removed,
    Sync,
    ConfirmExpired,
    Persisted(u64, PathBuf, Option<Stamp>),
}

#[derive(Default)]
pub(super) struct State {
    draft: Option<Draft>,
    frame: popup::State,
    query: String,
    offset: f32,
    confirm: Slot<Intent>,
    pinned: Option<Address>,
    names: talents::Names,
    catalog: combos::Catalog,
    picker: Option<usize>,
    hunt: String,
}

impl State {
    pub(super) fn begin(&mut self, plan: Plan, nudge: usize, vfs: &Vfs) {
        self.frame = popup::cascaded(nudge);
        self.offset = 0.0;
        self.picker = None;
        self.reload(plan, vfs);
    }

    pub(super) fn restore_scroll<M: Send + 'static>(&self) -> Option<Task<M>> {
        let draft = self.draft.as_ref()?;
        let target = scrollable::AbsoluteOffset { x: 0.0, y: self.offset };

        Some(operation::scroll_to(cards::grid_id(draft.plan.subject()), target))
    }

    fn reload(&mut self, plan: Plan, vfs: &Vfs) {
        self.draft = Draft::load(plan, vfs);
        self.pinned = self.draft.as_ref().and_then(Draft::pin);
    }

    pub(super) fn relocalize(&self) {
        self.names.forget();
        self.catalog.forget();
    }

    pub(super) fn drafting(&self) -> bool {
        self.draft.is_some()
    }

    pub(super) fn raised(&self) -> u64 {
        self.frame.raised()
    }

    pub(super) fn drifted(&self) -> bool {
        self.draft.as_ref().is_some_and(|draft| preview::stamp(&draft.read_from) != Some(draft.stamp))
    }

    pub(super) fn flush(&mut self, vfs: &Vfs) -> Task<Message> {
        let Some(draft) = self.draft.as_mut() else {
            return Task::none();
        };

        draft.resolve_buffer();
        draft.persist_if_dirty(vfs)
    }

    pub(super) fn flush_now(&mut self, vfs: &Vfs) {
        let Some(draft) = self.draft.as_mut() else {
            return;
        };

        draft.resolve_buffer();
        draft.persist_now(vfs);
    }

    pub(super) fn sync(&mut self, plan: Option<Plan>, vfs: &Vfs) {
        let Some(current) = self.draft.as_ref() else {
            return;
        };

        if current.dirty || current.writing {
            return;
        }

        let Some(plan) = plan else {
            self.draft = None;

            return;
        };

        let plan = match self.pinned {
            Some(address) if plan.anchor == current.plan.anchor => plan.rowed(address),
            _ => plan,
        };

        if current.plan.target_mod.is_none() != plan.target_mod.is_none() {
            self.draft = None;

            return;
        }

        if !current.plan.matches(&plan) {
            self.reload(plan, vfs);

            return;
        }

        if preview::stamp(&current.read_from) != Some(current.stamp) {
            self.reload(plan, vfs);
        }
    }

    pub(super) fn update(&mut self, message: Message, vfs: &Vfs) -> Task<Message> {
        if let Some(draft) = self.draft.as_mut() {
            let still_typing = matches!(&message, Message::Changed(index, _) if draft.buffering(*index));

            if !still_typing {
                draft.resolve_buffer();
            }
        }

        match message {
            Message::Popup(msg) => {
                let Some(spec) = self.draft.as_ref().map(|draft| spec(draft.plan.subject())) else {
                    return Task::none();
                };

                if self.frame.update(msg, spec) {
                    let task = self.draft.as_mut().map_or(Task::none(), |draft| draft.persist_if_dirty(vfs));

                    self.draft = None;
                    self.offset = 0.0;
                    self.picker = None;
                    self.confirm.expire();

                    return task;
                }
            }
            Message::Changed(index, value) => {
                if let Some(draft) = self.draft.as_mut() {
                    draft.edit(index, &value);
                }
            }
            Message::Picked(index, raw) => {
                self.picker = None;
                self.hunt.clear();

                if let Some(draft) = self.draft.as_mut() {
                    draft.pick(index, raw);
                }
            }
            Message::Picker(index) => {
                self.picker = index;
                self.hunt.clear();
                self.offset = 0.0;
            }
            Message::Aimed(address) => {
                let Some(draft) = self.draft.as_mut().filter(|draft| draft.aimed() != address) else {
                    return Task::none();
                };

                draft.persist_now(vfs);

                if draft.aim(address) {
                    self.offset = 0.0;
                    self.picker = None;
                }
            }
            Message::Slotted(index, unit, form) => {
                self.picker = None;
                self.hunt.clear();

                if let Some(draft) = self.draft.as_mut() {
                    draft.slotted(index, unit, form);
                }
            }
            Message::Scrolled(offset) => self.offset = offset,
            Message::SearchChanged(query) => self.query = query,
            Message::HuntChanged(query) => {
                self.hunt = query;
                self.offset = 0.0;
            }
            Message::CommentChanged(value) => {
                if let Some(draft) = self.draft.as_mut() {
                    draft.set_comment(value);
                }
            }
            Message::ConfirmExpired => self.confirm.expire(),
            Message::Sync => {
                if !self.confirm.take(&Intent::Sync) {
                    return self.confirm.set(Intent::Sync, Message::ConfirmExpired);
                }

                if let Some(draft) = self.draft.as_mut() {
                    draft.sync();
                }
            }
            Message::Removed => {
                if !self.confirm.take(&Intent::Remove) {
                    return self.confirm.set(Intent::Remove, Message::ConfirmExpired);
                }

                let Some(draft) = self.draft.as_mut() else {
                    return Task::none();
                };

                if !draft.remove() {
                    return Task::none();
                }

                draft.persist_now(vfs);
                draft.aim(Address::Appended);

                self.offset = 0.0;
                self.picker = None;
            }
            Message::Persisted(token, path, stamp) => {
                if let Some(draft) = self.draft.as_mut()
                    && draft.token == token
                {
                    draft.writing = false;

                    match stamp {
                        Some(stamp) => {
                            draft.read_from = path;
                            draft.stamp = stamp;
                            draft.failed = false;
                        }
                        None => draft.failed = true,
                    }
                }
            }
        }

        self.pinned = self.draft.as_ref().and_then(Draft::pin);

        self.draft.as_mut().map_or(Task::none(), |draft| draft.persist_if_dirty(vfs))
    }

    pub(super) fn view<'a>(
        &'a self,
        window: Size,
        cap: Option<i32>,
        used: Marks,
        cats: &'a [CatEntry],
        vault: &'a Vault,
        glossary: &'a NameBook,
    ) -> Option<Element<'a, Message>> {
        let draft = self.draft.as_ref()?;
        let spec = spec(draft.plan.subject());
        let width = self.frame.body_width(spec, window);
        let height = self.frame.body_height(spec, window);
        let armed = self.confirm.armed_for(&Intent::Sync);
        let removing = self.confirm.armed_for(&Intent::Remove);
        let query = self.query.as_str();
        let frame = Frame {
            width,
            height,
            offset: self.offset,
            query,
            armed,
            removing,
            cap,
            used,
            cats,
            names: &self.names,
            catalog: &self.catalog,
            vault,
            glossary,
            picker: self.picker,
            hunt: &self.hunt,
        };

        Some(self.frame.view(
            &draft.plan.label,
            spec,
            window,
            Message::Popup,
            move || draft.body(frame),
            None,
        ))
    }
}

trait Shown {
    fn persist_if_dirty(&mut self, vfs: &Vfs) -> Task<Message>;
    fn persist(&mut self, vfs: &Vfs) -> Task<Message>;
    fn body<'a>(&'a self, frame: Frame<'a>) -> Element<'a, Message>;
}

impl Shown for Draft {
    fn persist_if_dirty(&mut self, vfs: &Vfs) -> Task<Message> {
        if !self.dirty || self.writing {
            return Task::none();
        }

        self.persist(vfs)
    }

    fn persist(&mut self, vfs: &Vfs) -> Task<Message> {
        let Some((path, body, stamp, token)) = self.prepare_write(vfs) else {
            return Task::none();
        };

        let reported = path.clone();

        Task::perform(
            smol::unblock(move || write_now(&path, &body, stamp)),
            move |stamp| Message::Persisted(token, reported.clone(), stamp),
        )
    }

    fn body<'a>(&'a self, frame: Frame<'a>) -> Element<'a, Message> {
        let Frame { width, query, armed, cap, .. } = frame;

        match self.plan.subject() {
            Subject::Cat | Subject::Enemy => combat::view(self, width, query, armed),
            Subject::Buy => unitbuy::view(self, width, query, armed),
            Subject::Curve => unitlevel::view(self, width, armed, cap),
            Subject::Talents => talents::view(self, frame),
            Subject::Costs => costs::view(self, frame),
            Subject::Combo => combos::view(self, frame),
            Subject::MapStage => mapdata::view(self, width, armed),
            Subject::MapDrops => mapdrops::view(self, width, query, armed),
            Subject::DropChara => mapdrops::chara_view(self, width, armed),
            Subject::ItemBuy => itembuy::view(self, width, query, armed),
            Subject::MapOption => mapoption::view(self, width, query, armed),
            Subject::Background => backgrounds::view(self, width, query, armed),
            Subject::Upgrade => upgrades::view(self, width, query, armed),
            Subject::ComboParam => comboparams::view(self, width, query, armed),
            Subject::OrbSlots => orbslots::view(self, width, query, armed),
            Subject::Difficulty => difficulty::view(self, width, query, armed),
            Subject::ExInvasion => exoption::view(self, width, query, armed),
            Subject::CannonStatus => cannonstatus::view(self, width, query, armed),
            Subject::ExpCurve => expcurve::view(self, width, query, armed),
            Subject::OrbGrades => orbgrades::view(self, width, query, armed),
            Subject::ExLottery => exlottery::view(self, width, query, armed),
            Subject::Shakes => shakes::view(self, width, query, armed),
            Subject::Recipes => recipes::view(self, width, query, armed),
            Subject::Shop => shop::view(self, width, query, armed),
            Subject::ZombieHeader => zombies::header_view(self, width, query, armed),
            Subject::ZombieTier => zombies::tier_view(self, width, query, armed),
            Subject::Cpu => cpu::view(self, width, query, armed),
            Subject::OrbTraits => orbtraits::view(self, width, query, armed),
            Subject::Altar => altar::view(self, width, query, armed),
            Subject::GamatotoLimit => gamatoto::limit_view(self, width, query, armed),
            Subject::GamatotoUnlock => gamatoto::unlock_view(self, width, query, armed),
            Subject::GamatotoDrop => gamatoto::drop_view(self, width, query, armed),
            Subject::Stamps => progress::stamp_view(self, width, query, armed),
            Subject::SlotUnlocks => progress::slot_view(self, width, query, armed),
            Subject::Club => club::view(self, width, query, armed),
            Subject::Missions => missions::view(self, width, query, armed),
            Subject::Conditions => conditions::view(self, width, query, armed),
            Subject::MissionUnlocks => missions::unlock_view(self, width, query, armed),
            Subject::MissionGroups => conditions::group_view(self, width, query, armed),
            Subject::GachaOptions => gacha::view(self, width, query, armed),
            Subject::ChanceAnimations => chance::view(self, width, query, armed),
            Subject::Castles => castles::view(self, width, query, armed),
            Subject::TipRules => tiprules::view(self, width, query, armed),
            Subject::QuestStages => quest::stage_view(self, width, query, armed),
            Subject::SlotLottery => quest::lottery_view(self, width, query, armed),
            Subject::RealmAltar => realms::altar_view(self, width, query, armed),
            Subject::RealmDraws => realms::draw_view(self, width, query, armed),
            Subject::LabyrinthFloors => labyrinth::view(self, width, query, armed),
            Subject::LabyrinthLottery => labyrinth::lottery_view(self, width, query, armed),
            Subject::MissionShortcuts => missionsettings::shortcut_view(self, width, query, armed),
            Subject::MissionDesignations => missionsettings::designation_view(self, width, query, armed),
            Subject::MonthlyMissions => missionsettings::monthly_view(self, width, query, armed),
            Subject::SoundSettings => sounds::sound_view(self, width, query, armed),
            Subject::DojoChests => sounds::chest_view(self, width, query, armed),
            Subject::PopupTriggers => popups::view(self, width, query, armed),
            Subject::ComboCategories => combocategories::view(self, width, query, armed),
            Subject::CastleGrowth => castlegrowth::view(self, width, query, armed),
            Subject::DemonCastles => realms::castle_view(self, width, query, armed),
            Subject::LevelUpOffers => guide::level_up_view(self, width, query, armed),
            Subject::ReelSymbols => prizes::reel_view(self, width, query, armed),
            Subject::OrbFilterChoices => guide::orb_choice_view(self, width, query, armed),
            Subject::MapSheets => mapsheets::view(self, width, query, armed),
            Subject::MenuTextSettings => guide::menu_text_view(self, width, query, armed),
            Subject::TraitIcons => guide::trait_icon_view(self, width, query, armed),
            Subject::AbilityIcons => guide::ability_icon_view(self, width, query, armed),
            Subject::LineupRankGroups => guide::rank_group_view(self, width, query, armed),
            Subject::LineupBands => guide::band_view(self, width, query, armed),
            Subject::LineupRatings => guide::rating_view(self, width, query, armed),
            Subject::EngineerCap => development::engineer_view(self, width, query, armed),
            Subject::EnigmaRarities => enigma::rarity_view(self, width, query, armed),
            Subject::TipSettings => guide::tip_setting_view(self, width, query, armed),
            Subject::GuideSpotlights => guide::spotlight_view(self, width, query, armed),
            Subject::FanfareTiers => guide::fanfare_view(self, width, query, armed),
            Subject::StageShortcuts => guide::shortcut_view(self, width, query, armed),
            Subject::EnergyRefunds => guide::refund_view(self, width, query, armed),
            Subject::OrbOptions => guide::orb_option_view(self, width, query, armed),
            Subject::LineupCounters => guide::counter_view(self, width, query, armed),
            Subject::RecommendedUpgrades => guide::recommend_view(self, width, query, armed),
            Subject::EnigmaTimes => enigma::time_view(self, width, query, armed),
            Subject::MapMarkers => guide::marker_view(self, width, query, armed),
            Subject::DiscoveryMedals => enigma::medal_view(self, width, query, armed),
            Subject::EnigmaGroups => enigma::group_view(self, width, query, armed),
            Subject::EnigmaDrops => enigma::drop_view(self, width, query, armed),
            Subject::EnigmaRates => enigma::rate_view(self, width, query, armed),
            Subject::EnigmaLotteries => enigma::lottery_view(self, width, query, armed),
            Subject::LineupExclusions => guide::exclusion_view(self, width, query, armed),
            Subject::FilterMaps => guide::filter_view(self, width, query, armed),
            Subject::ZonedInSteps => gamatoto::zone_step_view(self, width, query, armed),
            Subject::ZonedIn => gamatoto::zone_view(self, width, query, armed),
            Subject::FortuneOdds => shrine::fortune_view(self, width, query, armed),
            Subject::ShrineExtras => shrine::extra_view(self, width, query, armed),
            Subject::ShrineOfferings => shrine::offering_view(self, width, query, armed),
            Subject::PrizePatterns => prizes::pattern_view(self, width, query, armed),
            Subject::GamatotoLevels => gamatoto::level_view(self, width, query, armed),
            Subject::ExpeditionAreas => gamatoto::area_view(self, width, query, armed),
            Subject::EnemyGuide => guide::enemy_view(self, width, query, armed),
            Subject::UnitLimits => guide::limit_view(self, width, query, armed),
            Subject::GuideEntries => guide::guide_view(self, width, query, armed),
            Subject::MaterialMixes => development::mix_view(self, width, query, armed),
            Subject::DevelopmentTiers => development::tier_view(self, width, query, armed),
            Subject::DevelopmentCosts => development::cost_view(self, width, query, armed),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Frame<'a> {
    width: f32,
    height: f32,
    offset: f32,
    query: &'a str,
    armed: bool,
    removing: bool,
    cap: Option<i32>,
    used: Marks,
    cats: &'a [CatEntry],
    names: &'a talents::Names,
    catalog: &'a combos::Catalog,
    vault: &'a Vault,
    glossary: &'a NameBook,
    picker: Option<usize>,
    hunt: &'a str,
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use foundry::common::preview::{self, Stamp};
    use foundry::domains::settings::EditorMode;
    use foundry::Vfs;

    use super::resolved::{self, Rule};
    use super::schema::{self, Subject};
    use super::{plan, split_row, write_now, Address, Draft, Message, Row, State};

    const VANILLA: &str = "100,3,10,8,15,140,50,75,0,320,0,0,0,8,0,9,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0, // \u{30cd}\u{30b3}";

    fn rebuild(row: &Row, touched: usize) -> String {
        let width = row.stored.max(touched);
        let mut line = row.written[..width].join(",");

        if !row.comment.is_empty() {
            line.push_str(", // ");
            line.push_str(&row.comment);
        }

        line
    }

    fn rows(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|id| format!("{id},0,0")).collect()
    }

    #[test]
    fn a_new_keyed_row_lands_in_id_order() {
        let lines = rows(&["ID", "677", "678", "705", "709"]);

        let at = super::Address::Keyed(680).insertion(&lines, ',');

        assert_eq!(at, Some(3), "680 belongs between 678 and 705, not appended");
    }

    #[test]
    fn a_new_keyed_row_past_the_end_appends() {
        let lines = rows(&["ID", "677", "678"]);

        assert_eq!(super::Address::Keyed(900).insertion(&lines, ','), Some(3));
    }

    #[test]
    fn a_header_never_captures_the_insertion_point() {
        let lines = rows(&["ID", "9"]);

        assert_eq!(
            super::Address::Keyed(1).insertion(&lines, ','),
            Some(1),
            "the header parses to no id, so it must never be treated as a larger one",
        );
    }

    #[test]
    fn a_talent_row_is_vacant_only_when_every_slot_is_empty() {
        let talents = schema::of(Subject::Talents);
        let mut cells = vec![0; talents.known()];

        assert!(talents.vacant(&cells), "no ability ids means the row carries nothing");

        cells[super::schema::TALENT_HEAD] = 1;
        assert!(!talents.vacant(&cells), "slot A holds an ability, so the row must survive");

        assert!(
            !schema::of(Subject::Cat).vacant(&[0; 8]),
            "only talents may delete their own row",
        );
    }

    const VANILLA_CURVE: &str = "1,25,5,5,5,5,10,10,10,10,10,";
    const VANILLA_ULTRA: &str = "3,50";

    #[test]
    fn a_single_level_cost_curve_is_written_back_byte_for_byte() {
        let row = split_row(VANILLA_ULTRA, ',', schema::of(Subject::Costs));

        assert_eq!(rebuild(&row, 0), VANILLA_ULTRA, "an ultra curve stores one level and must stay that wide");
    }

    #[test]
    fn a_cost_curve_grows_only_as_far_as_the_level_edited() {
        let row = split_row(VANILLA_ULTRA, ',', schema::of(Subject::Costs));

        assert_eq!(rebuild(&row, 4), "3,50,0,0", "levels past the one typed into stay out of the file");
    }

    #[test]
    fn a_ten_level_curve_keeps_every_cost_and_drops_only_its_trailing_empty() {
        let row = split_row(VANILLA_CURVE, ',', schema::of(Subject::Costs));

        assert_eq!(row.stored, 11, "the id plus ten levels are data; the trailing comma is not");
        assert_eq!(rebuild(&row, 0), VANILLA_CURVE.trim_end_matches(','));
    }

    #[test]
    fn a_missing_cost_curve_falls_back_to_the_first_one_the_file_has() {
        let lines = rows(&["lvID", "1", "2"]);

        assert_eq!(
            Address::Keyed(7).nearest(&lines, ','),
            Some(1),
            "the header parses to no id, so the fallback must land on the first real curve",
        );

        assert_eq!(
            Address::Line(0).nearest(&lines, ','),
            None,
            "only a keyed subject may be re-pointed at another row",
        );
    }

    const VANILLA_COMBO: &str = "3066,6,-1,781,1,689,0,-1,-1,-1,-1,-1,-1,7,1,-1";

    #[test]
    fn a_combo_row_is_written_back_byte_for_byte() {
        let row = split_row(VANILLA_COMBO, ',', schema::of(Subject::Combo));

        assert_eq!(rebuild(&row, 0), VANILLA_COMBO, "a no-op commit must not rewrite the combo");
    }

    #[test]
    fn a_combo_slot_names_the_columns_nyanko_publishes() {
        let schema = schema::of(Subject::Combo);

        for slot in 0..super::schema::COMBO_SLOTS {
            for field in [super::schema::combo_unit(slot), super::schema::combo_form(slot)] {
                assert!(
                    schema.index_of(&field).is_some(),
                    "combo: the slot view addresses {field}, which codex no longer publishes",
                );
            }
        }
    }

    #[test]
    fn only_talents_may_delete_their_own_row() {
        let combo = schema::of(Subject::Combo);

        assert!(
            !combo.vacant(&vec![-1; combo.known()]),
            "an emptied combo must be left in the file, not removed under it",
        );
    }

    #[test]
    fn typing_accepts_only_digits_and_a_leading_minus() {
        for good in ["", "-", "0", "42", "-7"] {
            assert!(super::typable(good, true, 0), "{good:?} should be typable");
        }

        for bad in ["+5", " 5", "5 ", "5a", "1.5", "--1", "1-"] {
            assert!(!super::typable(bad, true, 0), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn an_unsigned_field_refuses_the_minus_key() {
        assert!(!super::typable("-", false, 0), "a non-negative column must not accept a lone minus");
        assert!(!super::typable("-1", false, 0), "a non-negative column must not accept a negative");
        assert!(super::typable("1", false, 0), "digits stay typable");
    }

    #[test]
    fn resolved_leaves_a_value_it_cannot_represent_alone() {
        use super::resolved::{self, Face, Rule};
        use foundry::domains::settings::EditorMode;

        let schema = schema::of(Subject::Cat);
        let Some(index) = schema.index_of("boss_wave_immune") else {
            panic!("codex no longer publishes boss_wave_immune");
        };

        let rule = resolved::rule(Subject::Cat, index, schema.field(index), &[]);
        assert_eq!(rule, Rule::Flag, "boss_wave_immune is a flag");

        assert_eq!(
            rule.face(-1, EditorMode::Resolved),
            Face::Danger,
            "-1 is not a flag state, so Resolved must fall back to the raw card",
        );

        assert_eq!(
            super::shown(schema, index, -1, EditorMode::Resolved, rule),
            "-1",
            "the unrepresentable value is shown literally, not blanked or coerced",
        );

        let mut cells = vec!["0"; schema.known()];
        cells[index] = "-1";
        let line = cells.join(",");

        let row = split_row(&line, ',', schema);
        assert_eq!(row.written[index], "-1", "the stored text is kept verbatim");
        assert_eq!(rebuild(&row, 0), line, "opening in Resolved must not rewrite the row");
    }

    // DropItem writes its crown multipliers with a decimal point and the file ships exactly
    // five of them, so the round trip has to reproduce each one character for character --
    // "1" and not "1.00", or an untouched row stops matching itself.
    #[test]
    fn a_decimal_cell_round_trips_every_multiplier_the_file_ships() {
        for (text, held) in [("0.5", 50), ("0.75", 75), ("1", 100), ("1.25", 125), ("1.5", 150)] {
            assert_eq!(super::parse_scaled(text, 2), Some(held), "{text} reads as hundredths");
            assert_eq!(super::format_scaled(held, 2), text, "{held} writes back as {text}");
        }
    }

    #[test]
    fn a_column_with_no_places_is_the_plain_integer_path() {
        assert_eq!(super::parse_scaled("-42", 0), Some(-42));
        assert_eq!(super::format_scaled(-42, 0), "-42");
        assert_eq!(super::parse_scaled("0.75", 0), None, "a point is not an integer");
    }

    #[test]
    fn a_negative_decimal_keeps_its_sign_either_side_of_the_point() {
        assert_eq!(super::parse_scaled("-0.75", 2), Some(-75));
        assert_eq!(super::format_scaled(-75, 2), "-0.75");
        assert_eq!(super::parse_scaled("-1.5", 2), Some(-150));
        assert_eq!(super::format_scaled(-150, 2), "-1.5");
    }

    #[test]
    fn a_decimal_column_accepts_one_point_and_no_more() {
        assert!(super::typable("0.7", false, 2));
        assert!(super::typable("0.", false, 2), "a half typed value has to survive the keystroke");
        assert!(!super::typable("0.7.5", false, 2));
        assert!(!super::typable("0.7", false, 0), "an integer column still refuses the point");
    }

    #[test]
    fn an_untouched_row_is_written_back_byte_for_byte() {
        let row = split_row(VANILLA, ',', schema::of(Subject::Cat));

        assert_eq!(rebuild(&row, 0), VANILLA, "a no-op commit must not rewrite the row");
    }

    #[test]
    fn a_short_row_is_not_padded_to_the_full_column_table() {
        let row = split_row(VANILLA, ',', schema::of(Subject::Cat));

        assert_eq!(row.stored, 52, "vanilla cat 001 stores 52 columns");
        assert_eq!(row.cells.len(), schema::of(Subject::Cat).known(), "cells pad for display");
        assert_eq!(rebuild(&row, 0).matches(',').count(), VANILLA.matches(',').count());
    }

    #[test]
    fn editing_one_column_leaves_every_other_column_verbatim() {
        let mut row = split_row(VANILLA, ',', schema::of(Subject::Cat));
        row.written[3] = "999".to_owned();

        let before: Vec<&str> = VANILLA.split(" // ").next().unwrap_or_default().split(',').collect();
        let after = rebuild(&row, 4);
        let after: Vec<&str> = after.split(" // ").next().unwrap_or_default().split(',').collect();

        assert_eq!(before.len(), after.len(), "editing must not change the column count");

        for (index, (was, now)) in before.iter().zip(&after).enumerate() {
            if index == 3 {
                assert_eq!(*now, "999");
                continue;
            }

            assert_eq!(was, now, "column {index} was rewritten by an unrelated edit");
        }
    }

    #[test]
    fn editing_past_the_stored_width_pads_only_that_far() {
        let mut row = split_row(VANILLA, ',', schema::of(Subject::Cat));
        row.written[90] = "1".to_owned();

        let line = rebuild(&row, 91);
        let head = line.strip_suffix(", // \u{30cd}\u{30b3}").unwrap_or(&line);

        assert_eq!(head.split(',').count(), 91, "padding must stop at the edited column");
    }

    #[test]
    fn typing_below_a_floor_without_buffering_snaps_immediately() {
        let schema = schema::of(Subject::Cat);
        let Some(index) = schema.index_of("cooldown") else {
            panic!("codex no longer publishes cooldown");
        };

        let mut draft = bare_draft(Subject::Cat, index, 100);

        assert!(draft.write(index, "1"), "the floor clamp still counts as a change");
        assert_eq!(
            draft.reads_at(index), Some(30),
            "1 frame clamps up to the 60-frame floor, then halves back into raw units",
        );
        assert_eq!(
            draft.input(index), "60",
            "without a buffer mark the field snaps to 60 on the very first digit",
        );
    }

    #[test]
    fn a_buffer_marked_cooldown_edit_defers_the_floor_and_the_scale() {
        let schema = schema::of(Subject::Cat);
        let Some(index) = schema.index_of("cooldown") else {
            panic!("codex no longer publishes cooldown");
        };

        let mut draft = bare_draft(Subject::Cat, index, 100);

        for partial in ["!1", "!15", "!150"] {
            assert!(!draft.write(index, partial), "a buffered keystroke never touches the cell");
            assert_eq!(draft.input(index), partial, "the field must echo exactly what was typed");
            assert_eq!(draft.reads_at(index), Some(100), "the cell stays untouched while buffering");
        }

        assert!(draft.write(index, "150"), "the debanged value commits a real change");
        assert_eq!(draft.reads_at(index), Some(75), "150 frames halves into 75 raw units");
        assert_eq!(draft.input(index), "150", "150 clears the floor, so the field shows it verbatim");
    }

    #[test]
    fn a_buffer_marked_money_value_stays_literal_until_flushed() {
        const COST_DOWN: i32 = 25;

        let schema = schema::of(Subject::Talents);
        let index = schema::TALENT_HEAD + 2;

        let mut cells = vec![0; schema.known()];
        cells[schema::TALENT_HEAD] = COST_DOWN;

        let rules: Vec<Rule> = (0..cells.len())
            .map(|position| resolved::rule(Subject::Talents, position, schema.field(position), &cells))
            .collect();

        assert_eq!(
            rules[index], Rule::Ratio(3, 2),
            "ability 25's first value pair should resolve as a 1.5x-money cost reduction",
        );

        let mut draft = draft_with(Subject::Talents, cells, rules);

        for partial in ["!1", "!15", "!150"] {
            assert!(!draft.write(index, partial), "a buffered keystroke never touches the cell");
            assert_eq!(draft.input(index), partial);
            assert_eq!(draft.reads_at(index), Some(0));
        }

        assert!(draft.write(index, "150"), "the debanged value commits a real change");
        assert_eq!(
            draft.reads_at(index), Some(100),
            "150 resolved money divides by the 1.5x ratio into 100 raw cost units",
        );
        assert_eq!(draft.input(index), "150");
    }

    // The map-wide cells and the stage's own row are one draft over three lines of the file,
    // so an edit above the scroll has to land on line 0 or 1 and leave the stage row alone.
    #[test]
    fn a_map_wide_edit_lands_on_its_own_line_and_not_the_stage_row() {
        let dir = scratch_dir("mapdata-chrome-lines");
        let path = dir.join("MapStageDataS_002.csv");
        let schema = schema::of(Subject::MapStage);

        let seeded = "19,194,-1,-1,-1,0\n3\n80,1520,3,99,33,100,2,1\n";
        std::fs::write(&path, seeded).expect("failed to seed the temp fixture file");

        let stage = 0;
        let seed = plan(
            Subject::MapStage,
            Address::Line(super::MAP_HEADER_LINES + stage),
            "test".to_owned(),
            &path,
            None,
            EditorMode::Raw,
        );

        let vfs = Vfs::with_priority(&[]);
        let mut state = State { draft: Draft::load(seed, &vfs), ..State::default() };
        assert!(state.draft.is_some(), "the draft should load from the temp fixture");

        let Some(map_number) = schema.index_of("map_number") else {
            panic!("codex no longer publishes map_number");
        };
        let Some(pattern) = schema.index_of(schema::MAP_PATTERN_FIELD) else {
            panic!("the joined table lost its map pattern column");
        };
        let Some(xp) = schema.index_of("xp") else {
            panic!("codex no longer publishes xp");
        };

        assert!(map_number < pattern && pattern < xp, "the map-wide cells must lead the stage row");

        let _ = state.update(Message::Changed(map_number, "42".to_owned()), &vfs);
        let _ = state.update(Message::Changed(pattern, "7".to_owned()), &vfs);
        let _ = state.update(Message::Changed(xp, "9999".to_owned()), &vfs);

        complete_write(&mut state, &vfs);

        let body = std::fs::read_to_string(&path).expect("the completed write should have landed");
        let lines: Vec<&str> = body.lines().collect();

        assert_eq!(lines[0], "42,194,-1,-1,-1,0", "the header edit belongs on line 0, padded no further");
        assert_eq!(lines[1], "7", "the pattern edit belongs on line 1");
        assert_eq!(lines[2], "80,9999,3,99,33,100,2,1", "the stage row keeps every column it had");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rapid_keystrokes_never_write_synchronously_and_coalesce_into_one_task() {
        let dir = scratch_dir("keystrokes-defer-disk");
        let path = dir.join("unitbuy.csv");
        let schema = schema::of(Subject::Buy);
        let Some(index) = schema.index_of("stage_unlock_requirement") else {
            panic!("codex no longer publishes stage_unlock_requirement");
        };

        let seed = plan(Subject::Buy, Address::Line(0), "test".to_owned(), &path, None, EditorMode::Resolved);
        std::fs::write(&path, format!("{}\n", super::vacant(&seed, &[], ',')))
            .expect("failed to seed the temp fixture file");
        let seeded = std::fs::read(&path).expect("seeded fixture should be readable");

        let vfs = Vfs::with_priority(&[]);
        let mut state = State { draft: Draft::load(seed, &vfs), ..State::default() };
        assert!(state.draft.is_some(), "the draft should load from the temp fixture");

        for typed in ["1", "12", "123"] {
            let _ = state.update(Message::Changed(index, typed.to_owned()), &vfs);
        }

        let untouched = std::fs::read(&path).expect("fixture should still be readable");
        assert_eq!(
            untouched, seeded,
            "no keystroke may block `update` on disk I/O. The write only happens once the async task the first keystroke started is driven to completion",
        );

        let draft = state.draft.as_ref().expect("the draft survives typing");
        assert!(draft.writing, "the first keystroke must have started the async write immediately, no delay");
        assert!(draft.dirty, "later keystrokes coalesce behind the in-flight write instead of starting a second one");
        assert_eq!(draft.input(index), "123");

        complete_write(&mut state, &vfs);

        let body = std::fs::read_to_string(&path).expect("the completed write should have landed on disk");
        assert!(body.contains("123"), "the completion must persist the latest coalesced value");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn state_flush_resolves_a_buffered_field_and_starts_the_write_without_any_message_at_all() {
        let dir = scratch_dir("flush-on-navigate");
        let path = dir.join("unitbuy.csv");
        let schema = schema::of(Subject::Buy);
        let Some(index) = schema.index_of("stage_unlock_requirement") else {
            panic!("codex no longer publishes stage_unlock_requirement");
        };

        let seed = plan(Subject::Buy, Address::Line(0), "test".to_owned(), &path, None, EditorMode::Resolved);
        std::fs::write(&path, format!("{}\n", super::vacant(&seed, &[], ',')))
            .expect("failed to seed the temp fixture file");

        let vfs = Vfs::with_priority(&[]);
        let mut state = State { draft: Draft::load(seed, &vfs), ..State::default() };
        assert!(state.draft.is_some(), "the draft should load from the temp fixture");

        let _ = state.update(Message::Changed(index, "!7".to_owned()), &vfs);
        assert_eq!(state.draft.as_ref().and_then(|draft| draft.buffer), Some(index));

        // No `Message` at all here. This is what page navigation calls directly, ahead of
        // switching `current_page`, since a page change never routes through `figures::Message`.
        let _ = state.flush(&vfs);

        let draft = state.draft.as_ref().expect("the draft survives a flush with no message");
        assert_eq!(draft.buffer, None, "flush() alone must resolve the buffered field");
        assert_eq!(draft.input(index), "7");
        assert!(draft.writing, "flush() must have started the write, not just staged it");
        assert!(!draft.failed);

        complete_write(&mut state, &vfs);

        let draft = state.draft.as_ref().expect("the draft survives the completed write");
        assert!(!draft.failed, "the flush should have committed cleanly to the temp file");
        assert!(!draft.writing && !draft.dirty, "nothing left pending once the write completes");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn state_update_resolves_a_buffered_field_once_a_different_message_arrives() {
        let dir = scratch_dir("flush-on-deselect");
        let path = dir.join("unitbuy.csv");
        let schema = schema::of(Subject::Buy);
        let Some(index) = schema.index_of("stage_unlock_requirement") else {
            panic!("codex no longer publishes stage_unlock_requirement");
        };

        let seed = plan(Subject::Buy, Address::Line(0), "test".to_owned(), &path, None, EditorMode::Resolved);
        std::fs::write(&path, format!("{}
", super::vacant(&seed, &[], ',')))
            .expect("failed to seed the temp fixture file");

        let vfs = Vfs::with_priority(&[]);
        let mut state = State { draft: Draft::load(seed, &vfs), ..State::default() };
        assert!(state.draft.is_some(), "the draft should load from the temp fixture");

        let _ = state.update(Message::Changed(index, "!12".to_owned()), &vfs);
        assert_eq!(state.draft.as_ref().and_then(|draft| draft.buffer), Some(index));

        let _ = state.update(Message::Changed(index, "!123".to_owned()), &vfs);
        assert_eq!(
            state.draft.as_ref().map(|draft| draft.input(index)), Some("!123"),
            "typing on into the same buffered field must not flush it",
        );

        let _ = state.update(Message::Scrolled(4.0), &vfs);

        let draft = state.draft.as_ref().expect("the draft survives an unrelated message");
        assert_eq!(draft.buffer, None, "any other message flushes the buffered field");
        assert_eq!(draft.input(index), "123", "the bang is gone and the value is written plain");
        assert!(draft.writing, "the resolved value's write starts immediately, no debounce");
        assert!(!draft.failed);

        std::fs::remove_dir_all(&dir).ok();
    }

    fn complete_write(state: &mut State, vfs: &Vfs) {
        let draft = state.draft.as_mut().expect("a write can only complete on a live draft");
        let (path, body, stamp, token) = draft.prepare_write(vfs).expect("destination must resolve in these fixtures");

        // Dropping the returned `Task` cancels the real write, but `blocking::unblock` may have
        // already run it on the pool. Either way the file ends up holding this body.
        let result = write_now(&path, &body, stamp)
            .or_else(|| preview::stamp(&path).and_then(|fresh| write_now(&path, &body, fresh)));

        let _ = state.update(Message::Persisted(token, path, result), vfs);
    }

    fn scratch_dir(label: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("bcc-figures-test-{label}-{}", std::process::id()));

        std::fs::create_dir_all(&dir).expect("failed to create the temp fixture dir");

        dir
    }

    fn bare_draft(subject: Subject, index: usize, seed: i32) -> Draft {
        let schema = schema::of(subject);
        let mut cells = vec![0; schema.known()];
        cells[index] = seed;

        let rules: Vec<Rule> = (0..cells.len())
            .map(|position| resolved::rule(subject, position, schema.field(position), &cells))
            .collect();

        draft_with(subject, cells, rules)
    }

    fn draft_with(subject: Subject, cells: Vec<i32>, rules: Vec<Rule>) -> Draft {
        let width = cells.len();
        let plan = plan(subject, Address::Line(0), "test".to_owned(), Path::new("test.csv"), None, EditorMode::Resolved);

        Draft {
            plan,
            chrome: Vec::new(),
            absent: false,
            row: 0,
            read_from: std::path::PathBuf::new(),
            stamp: Stamp::default(),
            delimiter: ',',
            keys: Vec::new(),
            keyed: None,
            lines: Vec::new(),
            cells,
            writing: false,
            token: 0,
            written: vec![String::new(); width],
            stored: width,
            touched: 0,
            rules,
            comment: String::new(),
            inputs: vec![String::new(); width],
            failed: false,
            buffer: None,
            dirty: false,
        }
    }
}

#[cfg(test)]
mod append_tests {
    use std::fs;

    use foundry::domains::settings::EditorMode;
    use foundry::Vfs;

    use super::schema::{self, Subject};
    use super::{plan, Address, Draft};

    // The empty entry is the only way to make a combo now, so editing it has to write the
    // row, give it a live key and re-address the popup onto it without a further click.
    #[test]
    fn editing_the_empty_entry_materializes_a_combo_and_selects_it() {
        let root = std::env::temp_dir().join(format!("bcc-combo-new-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("scratch");
        let file = root.join("NyancomboData.csv");

        fs::write(&file, "3066,6,-1,781,1,-1,-1,-1,-1,-1,-1,-1,-1,7,1,-1\n").expect("seed");

        let vfs = Vfs::with_priority(&[String::new()]);
        let made = plan(Subject::Combo, Address::Appended, "t".to_owned(), &file, None, EditorMode::Resolved)
            .anchored(Some((44, 0)));

        let mut state = super::State::default();
        state.begin(made, 0, &vfs);

        let schema = schema::of(Subject::Combo);
        let anchored = schema.index_of("slot_1_unit_id").expect("published slot column");
        let second = schema.index_of("slot_2_unit_id").expect("published slot column");

        let draft = state.draft.as_ref().expect("the blank combo opens");
        assert!(draft.absent(), "nothing is written until something is edited");
        assert_eq!(draft.reads_at(anchored), Some(44), "the blank opens holding its own unit");
        assert_eq!(fs::read_to_string(&file).expect("read").lines().count(), 1, "and wrote nothing");

        let _ = state.update(super::Message::Slotted(second, 90, 2), &vfs);

        assert_eq!(state.draft.as_ref().map(super::Draft::row), Some(1), "the popup moved onto it");
        assert_eq!(state.draft.as_ref().map(super::Draft::absent), Some(false), "and it is real now");
        assert_eq!(state.pinned, Some(Address::Line(1)), "so a refresh cannot rebase it");

        state.flush_now(&vfs);

        let written = fs::read_to_string(&file).expect("read back");
        let lines: Vec<&str> = written.lines().collect();

        assert_eq!(lines.len(), 2, "the edit materialized the row");
        assert!(lines[1].starts_with("3067,6,"), "with the next key in its series: {:?}", lines[1]);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn removing_a_combo_returns_the_popup_to_the_empty_entry() {
        let root = std::env::temp_dir().join(format!("bcc-combo-back-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("scratch");
        let file = root.join("NyancomboData.csv");

        fs::write(&file, "0,1,-1,44,0,-1,-1,-1,-1,-1,-1,-1,-1,5,0,-1\n1,1,-1,90,2,-1,-1,-1,-1,-1,-1,-1,-1,7,1,-1\n")
            .expect("seed");

        let vfs = Vfs::with_priority(&[String::new()]);
        let made = plan(Subject::Combo, Address::Line(0), "t".to_owned(), &file, None, EditorMode::Resolved)
            .anchored(Some((44, 0)));

        let mut state = super::State::default();
        state.begin(made, 0, &vfs);

        let _ = state.update(super::Message::Removed, &vfs);
        assert!(!state.draft.as_ref().is_some_and(super::Draft::absent), "the first press only arms it");

        let _ = state.update(super::Message::Removed, &vfs);

        let draft = state.draft.as_ref().expect("the popup stays open");
        assert!(draft.absent(), "and lands back on the empty entry rather than closing");
        assert_eq!(state.pinned, None, "an unwritten combo pins nothing");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn switching_unit_or_form_retargets_the_combo_popup_instead_of_holding_its_row() {
        let root = std::env::temp_dir().join(format!("bcc-combo-anchor-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("scratch");
        let file = root.join("NyancomboData.csv");

        fs::write(&file, "0,1,-1,44,0,-1,-1,-1,-1,-1,-1,-1,-1,5,0,-1\n1,1,-1,90,2,-1,-1,-1,-1,-1,-1,-1,-1,7,1,-1\n")
            .expect("seed");

        let vfs = Vfs::with_priority(&[String::new()]);
        let opened = plan(Subject::Combo, Address::Line(0), "t".to_owned(), &file, None, EditorMode::Resolved)
            .anchored(Some((44, 0)));

        let mut state = super::State::default();
        state.begin(opened, 0, &vfs);
        assert_eq!(state.pinned, Some(Address::Line(0)), "the popup pins the combo it opened on");

        let same = plan(Subject::Combo, Address::Line(1), "t".to_owned(), &file, None, EditorMode::Resolved)
            .anchored(Some((44, 0)));
        state.sync(Some(same), &vfs);
        assert_eq!(
            state.draft.as_ref().map(super::Draft::row),
            Some(0),
            "the same unit and form must not drag the popup off the combo being edited",
        );

        let moved = plan(Subject::Combo, Address::Line(1), "t".to_owned(), &file, None, EditorMode::Resolved)
            .anchored(Some((90, 2)));
        state.sync(Some(moved), &vfs);
        assert_eq!(
            state.draft.as_ref().map(super::Draft::row),
            Some(1),
            "a different unit or form re-targets the popup onto that unit's combo",
        );

        let _ = fs::remove_dir_all(&root);
    }

    // The shipped file keeps 126 superseded rows rather than deleting them, because the
    // localized name tables are addressed by line. Removing one must do the same.
    #[test]
    fn removing_a_combo_mid_file_retires_it_instead_of_shifting_every_name_below() {
        let root = std::env::temp_dir().join(format!("bcc-combo-remove-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("scratch");
        let file = root.join("NyancomboData.csv");

        let seeded = "0,1,-1,44,0,-1,-1,-1,-1,-1,-1,-1,-1,5,0,-1\n1,1,-1,90,2,-1,-1,-1,-1,-1,-1,-1,-1,7,1,-1\n";
        fs::write(&file, seeded).expect("seed");

        let vfs = Vfs::with_priority(&[String::new()]);
        let made = plan(Subject::Combo, Address::Line(0), "t".to_owned(), &file, None, EditorMode::Resolved);
        let mut draft = Draft::load(made, &vfs).expect("the seeded combo loads");

        assert!(draft.remove(), "a written combo can be removed");
        draft.persist_now(&vfs);

        let written = fs::read_to_string(&file).expect("read back");
        let lines: Vec<&str> = written.lines().collect();

        assert_eq!(lines.len(), 2, "the line stays so every name below keeps its address");
        assert!(lines[0].starts_with("0,-1,"), "and is retired by its series: {:?}", lines[0]);
        assert!(lines[1].starts_with("1,1,"), "the combo below is untouched: {:?}", lines[1]);

        let last = plan(Subject::Combo, Address::Line(1), "t".to_owned(), &file, None, EditorMode::Resolved);
        let mut tail = Draft::load(last, &vfs).expect("the last combo loads");

        assert!(tail.remove(), "the last row can go outright");
        tail.persist_now(&vfs);

        let after = fs::read_to_string(&file).expect("read back");
        assert_eq!(after.lines().count(), 1, "nothing below it, so nothing to keep aligned");

        let _ = fs::remove_dir_all(&root);
    }

}
