use std::borrow::Cow;
use std::sync::LazyLock;
use codex::cat::unit::{NyancomboData, UnitBuy};
use codex::cat::AbilityDataRow;
use codex::cat::unitid;
use codex::chapter::map::MapOptionEntry;
use codex::chapter::stage::{BackgroundEntry, MapStageDataEntry, MapStageDataHeader};
use codex::combat::Scale;
use codex::common::{Column, FromColumn};
use codex::enemy::t_unit;
use codex::files::GatyaItemBuy;
use crate::domains::settings::EditorMode;
use crate::systems::editor::figures::Subject;

use super::{
    mapsheets,
    enigma,
    shrine,
    prizes,
    guide,
    altar, backgrounds, development, castlegrowth, castles, chance, club, combocategories, gacha, labyrinth, missionsettings, popups, quest, realms, sounds, tiprules, conditions, gamatoto, missions, progress, cannonstatus, cpu, orbtraits, comboparams, difficulty, exlottery, exoption, expcurve, mapdata, mapdrops, mapoption, orbgrades,
    orbslots, recipes, shakes, shop, upgrades, zombies,
};

pub struct Schema {
    pub subject: Subject,
    pub comments: bool,
}

static CAT: Schema = Schema {
    subject: Subject::Cat,
    comments: true,
};

static ENEMY: Schema = Schema {
    subject: Subject::Enemy,
    comments: false,
};

static BUY: Schema = Schema {
    subject: Subject::Buy,
    comments: false,
};

static CURVE: Schema = Schema {
    subject: Subject::Curve,
    comments: false,
};

static TALENTS: Schema = Schema {
    subject: Subject::Talents,
    comments: false,
};

static COSTS: Schema = Schema {
    subject: Subject::Costs,
    comments: false,
};

static COMBO: Schema = Schema {
    subject: Subject::Combo,
    comments: false,
};

static MAP_STAGE: Schema = Schema {
    subject: Subject::MapStage,
    comments: true,
};

static MAP_DROPS: Schema = Schema {
    subject: Subject::MapDrops,
    comments: true,
};

pub(super) static DROP_CHARA: Schema = Schema {
    subject: Subject::DropChara,
    comments: true,
};

static ITEM_BUY: Schema = Schema {
    subject: Subject::ItemBuy,
    comments: false,
};

pub(super) static MAP_OPTION: Schema = Schema {
    subject: Subject::MapOption,
    comments: false,
};

static BACKGROUND: Schema = Schema {
    subject: Subject::Background,
    comments: false,
};

static UPGRADE: Schema = Schema {
    subject: Subject::Upgrade,
    comments: true,
};

static COMBO_PARAM: Schema = Schema {
    subject: Subject::ComboParam,
    comments: false,
};

static ORB_SLOTS: Schema = Schema {
    subject: Subject::OrbSlots,
    comments: false,
};

static DIFFICULTY: Schema = Schema {
    subject: Subject::Difficulty,
    comments: false,
};

static EX_INVASION: Schema = Schema {
    subject: Subject::ExInvasion,
    comments: false,
};

static CANNON_STATUS: Schema = Schema {
    subject: Subject::CannonStatus,
    comments: false,
};

static EXP_CURVE: Schema = Schema {
    subject: Subject::ExpCurve,
    comments: false,
};

pub(super) static ORB_GRADES: Schema = Schema {
    subject: Subject::OrbGrades,
    comments: false,
};

pub(super) static EX_LOTTERY: Schema = Schema {
    subject: Subject::ExLottery,
    comments: false,
};

pub(super) static SHAKES: Schema = Schema {
    subject: Subject::Shakes,
    comments: false,
};

pub(super) static RECIPES: Schema = Schema {
    subject: Subject::Recipes,
    comments: false,
};

static SHOP: Schema = Schema {
    subject: Subject::Shop,
    comments: false,
};

static ZOMBIE_HEADER: Schema = Schema {
    subject: Subject::ZombieHeader,
    comments: true,
};

pub(super) static ZOMBIE_TIER: Schema = Schema {
    subject: Subject::ZombieTier,
    comments: true,
};

pub(super) static CPU: Schema = Schema {
    subject: Subject::Cpu,
    comments: false,
};

static ORB_TRAITS: Schema = Schema {
    subject: Subject::OrbTraits,
    comments: false,
};

static ALTAR: Schema = Schema {
    subject: Subject::Altar,
    comments: false,
};

static GAMATOTO_LIMIT: Schema = Schema {
    subject: Subject::GamatotoLimit,
    comments: true,
};

static GAMATOTO_UNLOCK: Schema = Schema {
    subject: Subject::GamatotoUnlock,
    comments: false,
};

static GAMATOTO_DROP: Schema = Schema {
    subject: Subject::GamatotoDrop,
    comments: false,
};

static STAMPS: Schema = Schema {
    subject: Subject::Stamps,
    comments: true,
};

static SLOT_UNLOCKS: Schema = Schema {
    subject: Subject::SlotUnlocks,
    comments: false,
};

static CLUB: Schema = Schema {
    subject: Subject::Club,
    comments: false,
};

pub(super) static MISSIONS: Schema = Schema {
    subject: Subject::Missions,
    comments: false,
};

static CONDITIONS: Schema = Schema {
    subject: Subject::Conditions,
    comments: false,
};

pub(super) static MISSION_UNLOCKS: Schema = Schema {
    subject: Subject::MissionUnlocks,
    comments: false,
};

static DEMON_CASTLES: Schema = Schema {
    subject: Subject::DemonCastles,
    comments: true,
};

static CASTLE_GROWTH: Schema = Schema {
    subject: Subject::CastleGrowth,
    comments: true,
};

static COMBO_CATEGORIES: Schema = Schema {
    subject: Subject::ComboCategories,
    comments: false,
};

static POPUP_TRIGGERS: Schema = Schema {
    subject: Subject::PopupTriggers,
    comments: false,
};

static SOUND_SETTINGS: Schema = Schema {
    subject: Subject::SoundSettings,
    comments: false,
};

static DOJO_CHESTS: Schema = Schema {
    subject: Subject::DojoChests,
    comments: false,
};

pub(super) static MISSION_SHORTCUTS: Schema = Schema {
    subject: Subject::MissionShortcuts,
    comments: false,
};

pub(super) static MISSION_DESIGNATIONS: Schema = Schema {
    subject: Subject::MissionDesignations,
    comments: false,
};

pub(super) static MONTHLY_MISSIONS: Schema = Schema {
    subject: Subject::MonthlyMissions,
    comments: false,
};

static DEVELOPMENT_COSTS: Schema = Schema {
    subject: Subject::DevelopmentCosts,
    comments: false,
};

static DEVELOPMENT_TIERS: Schema = Schema {
    subject: Subject::DevelopmentTiers,
    comments: false,
};

static MATERIAL_MIXES: Schema = Schema {
    subject: Subject::MaterialMixes,
    comments: false,
};

static GUIDE_ENTRIES: Schema = Schema {
    subject: Subject::GuideEntries,
    comments: false,
};

static UNIT_LIMITS: Schema = Schema {
    subject: Subject::UnitLimits,
    comments: false,
};

static ENEMY_GUIDE: Schema = Schema {
    subject: Subject::EnemyGuide,
    comments: false,
};

static EXPEDITION_AREAS: Schema = Schema {
    subject: Subject::ExpeditionAreas,
    comments: true,
};

static GAMATOTO_LEVELS: Schema = Schema {
    subject: Subject::GamatotoLevels,
    comments: true,
};

static PRIZE_PATTERNS: Schema = Schema {
    subject: Subject::PrizePatterns,
    comments: false,
};

static SHRINE_OFFERINGS: Schema = Schema {
    subject: Subject::ShrineOfferings,
    comments: false,
};

static SHRINE_EXTRAS: Schema = Schema {
    subject: Subject::ShrineExtras,
    comments: false,
};

static FORTUNE_ODDS: Schema = Schema {
    subject: Subject::FortuneOdds,
    comments: false,
};

static ZONED_IN: Schema = Schema {
    subject: Subject::ZonedIn,
    comments: false,
};

static ZONED_IN_STEPS: Schema = Schema {
    subject: Subject::ZonedInSteps,
    comments: false,
};

static FILTER_MAPS: Schema = Schema {
    subject: Subject::FilterMaps,
    comments: false,
};

static LINEUP_EXCLUSIONS: Schema = Schema {
    subject: Subject::LineupExclusions,
    comments: true,
};

static ENIGMA_LOTTERIES: Schema = Schema {
    subject: Subject::EnigmaLotteries,
    comments: false,
};

static ENIGMA_RATES: Schema = Schema {
    subject: Subject::EnigmaRates,
    comments: false,
};

static ENIGMA_DROPS: Schema = Schema {
    subject: Subject::EnigmaDrops,
    comments: false,
};

static ENIGMA_GROUPS: Schema = Schema {
    subject: Subject::EnigmaGroups,
    comments: false,
};

static DISCOVERY_MEDALS: Schema = Schema {
    subject: Subject::DiscoveryMedals,
    comments: false,
};

static MAP_MARKERS: Schema = Schema {
    subject: Subject::MapMarkers,
    comments: true,
};

static ENIGMA_TIMES: Schema = Schema {
    subject: Subject::EnigmaTimes,
    comments: false,
};

static RECOMMENDED_UPGRADES: Schema = Schema {
    subject: Subject::RecommendedUpgrades,
    comments: false,
};

static LINEUP_COUNTERS: Schema = Schema {
    subject: Subject::LineupCounters,
    comments: false,
};

static ORB_OPTIONS: Schema = Schema {
    subject: Subject::OrbOptions,
    comments: false,
};

static ENERGY_REFUNDS: Schema = Schema {
    subject: Subject::EnergyRefunds,
    comments: false,
};

static STAGE_SHORTCUTS: Schema = Schema {
    subject: Subject::StageShortcuts,
    comments: false,
};

static FANFARE_TIERS: Schema = Schema {
    subject: Subject::FanfareTiers,
    comments: false,
};

static GUIDE_SPOTLIGHTS: Schema = Schema {
    subject: Subject::GuideSpotlights,
    comments: false,
};

static TIP_SETTINGS: Schema = Schema {
    subject: Subject::TipSettings,
    comments: false,
};

static ENIGMA_RARITIES: Schema = Schema {
    subject: Subject::EnigmaRarities,
    comments: true,
};

static ENGINEER_CAP: Schema = Schema {
    subject: Subject::EngineerCap,
    comments: false,
};

static LINEUP_RATINGS: Schema = Schema {
    subject: Subject::LineupRatings,
    comments: false,
};

static LINEUP_BANDS: Schema = Schema {
    subject: Subject::LineupBands,
    comments: false,
};

static LINEUP_RANK_GROUPS: Schema = Schema {
    subject: Subject::LineupRankGroups,
    comments: false,
};

static ABILITY_ICONS: Schema = Schema {
    subject: Subject::AbilityIcons,
    comments: false,
};

static TRAIT_ICONS: Schema = Schema {
    subject: Subject::TraitIcons,
    comments: false,
};

static MENU_TEXT_SETTINGS: Schema = Schema {
    subject: Subject::MenuTextSettings,
    comments: false,
};

static MAP_SHEETS: Schema = Schema {
    subject: Subject::MapSheets,
    comments: false,
};

static ORB_FILTER_CHOICES: Schema = Schema {
    subject: Subject::OrbFilterChoices,
    comments: false,
};

static REEL_SYMBOLS: Schema = Schema {
    subject: Subject::ReelSymbols,
    comments: false,
};

static LEVEL_UP_OFFERS: Schema = Schema {
    subject: Subject::LevelUpOffers,
    comments: false,
};

static LABYRINTH_LOTTERY: Schema = Schema {
    subject: Subject::LabyrinthLottery,
    comments: true,
};

static LABYRINTH_FLOORS: Schema = Schema {
    subject: Subject::LabyrinthFloors,
    comments: true,
};

static REALM_ALTAR: Schema = Schema {
    subject: Subject::RealmAltar,
    comments: true,
};

static REALM_DRAWS: Schema = Schema {
    subject: Subject::RealmDraws,
    comments: true,
};

static QUEST_STAGES: Schema = Schema {
    subject: Subject::QuestStages,
    comments: false,
};

static SLOT_LOTTERY: Schema = Schema {
    subject: Subject::SlotLottery,
    comments: true,
};

static TIP_RULES: Schema = Schema {
    subject: Subject::TipRules,
    comments: false,
};

static CASTLES: Schema = Schema {
    subject: Subject::Castles,
    comments: true,
};

static CHANCE_ANIMATIONS: Schema = Schema {
    subject: Subject::ChanceAnimations,
    comments: false,
};

static GACHA_OPTIONS: Schema = Schema {
    subject: Subject::GachaOptions,
    comments: false,
};

pub(super) static MISSION_GROUPS: Schema = Schema {
    subject: Subject::MissionGroups,
    comments: false,
};

pub fn of(subject: Subject) -> &'static Schema {
    match subject {
        Subject::Cat => &CAT,
        Subject::Enemy => &ENEMY,
        Subject::Buy => &BUY,
        Subject::Curve => &CURVE,
        Subject::Talents => &TALENTS,
        Subject::Costs => &COSTS,
        Subject::Combo => &COMBO,
        Subject::MapStage => &MAP_STAGE,
        Subject::MapDrops => &MAP_DROPS,
        Subject::DropChara => &DROP_CHARA,
        Subject::ItemBuy => &ITEM_BUY,
        Subject::MapOption => &MAP_OPTION,
        Subject::Background => &BACKGROUND,
        Subject::Upgrade => &UPGRADE,
        Subject::ComboParam => &COMBO_PARAM,
        Subject::OrbSlots => &ORB_SLOTS,
        Subject::Difficulty => &DIFFICULTY,
        Subject::ExInvasion => &EX_INVASION,
        Subject::CannonStatus => &CANNON_STATUS,
        Subject::ExpCurve => &EXP_CURVE,
        Subject::OrbGrades => &ORB_GRADES,
        Subject::ExLottery => &EX_LOTTERY,
        Subject::Shakes => &SHAKES,
        Subject::Recipes => &RECIPES,
        Subject::Shop => &SHOP,
        Subject::ZombieHeader => &ZOMBIE_HEADER,
        Subject::ZombieTier => &ZOMBIE_TIER,
        Subject::Cpu => &CPU,
        Subject::OrbTraits => &ORB_TRAITS,
        Subject::Altar => &ALTAR,
        Subject::GamatotoLimit => &GAMATOTO_LIMIT,
        Subject::GamatotoUnlock => &GAMATOTO_UNLOCK,
        Subject::GamatotoDrop => &GAMATOTO_DROP,
        Subject::Stamps => &STAMPS,
        Subject::SlotUnlocks => &SLOT_UNLOCKS,
        Subject::Club => &CLUB,
        Subject::Missions => &MISSIONS,
        Subject::Conditions => &CONDITIONS,
        Subject::MissionUnlocks => &MISSION_UNLOCKS,
        Subject::MissionGroups => &MISSION_GROUPS,
        Subject::GachaOptions => &GACHA_OPTIONS,
        Subject::ChanceAnimations => &CHANCE_ANIMATIONS,
        Subject::Castles => &CASTLES,
        Subject::TipRules => &TIP_RULES,
        Subject::QuestStages => &QUEST_STAGES,
        Subject::SlotLottery => &SLOT_LOTTERY,
        Subject::RealmAltar => &REALM_ALTAR,
        Subject::RealmDraws => &REALM_DRAWS,
        Subject::LabyrinthFloors => &LABYRINTH_FLOORS,
        Subject::LabyrinthLottery => &LABYRINTH_LOTTERY,
        Subject::MissionShortcuts => &MISSION_SHORTCUTS,
        Subject::MissionDesignations => &MISSION_DESIGNATIONS,
        Subject::MonthlyMissions => &MONTHLY_MISSIONS,
        Subject::SoundSettings => &SOUND_SETTINGS,
        Subject::DojoChests => &DOJO_CHESTS,
        Subject::PopupTriggers => &POPUP_TRIGGERS,
        Subject::ComboCategories => &COMBO_CATEGORIES,
        Subject::CastleGrowth => &CASTLE_GROWTH,
        Subject::DemonCastles => &DEMON_CASTLES,
        Subject::LevelUpOffers => &LEVEL_UP_OFFERS,
        Subject::ReelSymbols => &REEL_SYMBOLS,
        Subject::OrbFilterChoices => &ORB_FILTER_CHOICES,
        Subject::MapSheets => &MAP_SHEETS,
        Subject::MenuTextSettings => &MENU_TEXT_SETTINGS,
        Subject::TraitIcons => &TRAIT_ICONS,
        Subject::AbilityIcons => &ABILITY_ICONS,
        Subject::LineupRankGroups => &LINEUP_RANK_GROUPS,
        Subject::LineupBands => &LINEUP_BANDS,
        Subject::LineupRatings => &LINEUP_RATINGS,
        Subject::EngineerCap => &ENGINEER_CAP,
        Subject::EnigmaRarities => &ENIGMA_RARITIES,
        Subject::TipSettings => &TIP_SETTINGS,
        Subject::GuideSpotlights => &GUIDE_SPOTLIGHTS,
        Subject::FanfareTiers => &FANFARE_TIERS,
        Subject::StageShortcuts => &STAGE_SHORTCUTS,
        Subject::EnergyRefunds => &ENERGY_REFUNDS,
        Subject::OrbOptions => &ORB_OPTIONS,
        Subject::LineupCounters => &LINEUP_COUNTERS,
        Subject::RecommendedUpgrades => &RECOMMENDED_UPGRADES,
        Subject::EnigmaTimes => &ENIGMA_TIMES,
        Subject::MapMarkers => &MAP_MARKERS,
        Subject::DiscoveryMedals => &DISCOVERY_MEDALS,
        Subject::EnigmaGroups => &ENIGMA_GROUPS,
        Subject::EnigmaDrops => &ENIGMA_DROPS,
        Subject::EnigmaRates => &ENIGMA_RATES,
        Subject::EnigmaLotteries => &ENIGMA_LOTTERIES,
        Subject::LineupExclusions => &LINEUP_EXCLUSIONS,
        Subject::FilterMaps => &FILTER_MAPS,
        Subject::ZonedInSteps => &ZONED_IN_STEPS,
        Subject::ZonedIn => &ZONED_IN,
        Subject::FortuneOdds => &FORTUNE_ODDS,
        Subject::ShrineExtras => &SHRINE_EXTRAS,
        Subject::ShrineOfferings => &SHRINE_OFFERINGS,
        Subject::PrizePatterns => &PRIZE_PATTERNS,
        Subject::GamatotoLevels => &GAMATOTO_LEVELS,
        Subject::ExpeditionAreas => &EXPEDITION_AREAS,
        Subject::EnemyGuide => &ENEMY_GUIDE,
        Subject::UnitLimits => &UNIT_LIMITS,
        Subject::GuideEntries => &GUIDE_ENTRIES,
        Subject::MaterialMixes => &MATERIAL_MIXES,
        Subject::DevelopmentTiers => &DEVELOPMENT_TIERS,
        Subject::DevelopmentCosts => &DEVELOPMENT_COSTS,
    }
}

pub const BRACKET: usize = 10;

pub const TALENT_HEAD: usize = 2;

pub const TALENT_STRIDE: usize = 14;

pub const TALENT_SLOTS: usize = 11;

const TALENT_WIDTH: usize = TALENT_HEAD + TALENT_STRIDE * TALENT_SLOTS;

pub const COST_HEAD: usize = 1;

pub const COST_LEVELS: usize = 10;

const COST_WIDTH: usize = COST_HEAD + COST_LEVELS;

const COST_HEADING: &str = "Cost ID";

const TALENT_HEADINGS: [&str; TALENT_HEAD] = ["Unit ID", "Type ID"];

const TALENT_FIELDS: [&str; TALENT_STRIDE] = [
    "Ability ID",
    "Max Level",
    "Min 1",
    "Max 1",
    "Min 2",
    "Max 2",
    "Min 3",
    "Max 3",
    "Min 4",
    "Max 4",
    "Text ID",
    "Cost ID",
    "Name ID",
    "Limit",
];

pub(super) const NAME_ID: usize = 12;

pub(super) fn slot_of(index: usize) -> Option<usize> {
    index
        .checked_sub(TALENT_HEAD)
        .map(|offset| offset / TALENT_STRIDE)
}

fn talent_label(index: usize) -> String {
    let Some(offset) = index.checked_sub(TALENT_HEAD) else {
        return TALENT_HEADINGS[index].to_owned();
    };

    let slot = offset / TALENT_STRIDE;
    let field = TALENT_FIELDS[offset % TALENT_STRIDE];
    let letter = char::from(b'A' + slot as u8);

    format!("{field} ({letter})")
}

pub(super) const BRACKETS: usize = 20;

const FIRST_GROWTH_LEVEL: usize = 2;

pub struct Entry {
    pub field: &'static str,
    pub index: usize,
    pub scale: Scale,
    pub default: &'static str,
}

const CAT_NAMES: &[(&str, &str)] = &[
    ("hitpoints", "Base Hitpoints"),
    ("attack_1_damage", "Attack 1 Base Damage"),
    ("eoc1_cost", "EoC1 Cost"),
    ("trait_red", "Target Red"),
    ("trait_floating", "Target Floating"),
    ("trait_dark", "Target Dark"),
    ("trait_metal", "Target Metal"),
    ("trait_traitless", "Target Traitless"),
    ("trait_angel", "Target Angel"),
    ("trait_alien", "Target Alien"),
    ("trait_zombie", "Target Zombie"),
    ("is_metal", "Metal"),
    ("trait_witch", "Target Witch"),
    ("attack_2_damage", "Attack 2 Base Damage"),
    ("attack_3_damage", "Attack 3 Base Damage"),
    ("trait_eva", "Target Eva"),
    ("trait_relic", "Target Relic"),
    ("trait_aku", "Target Aku"),
];

const ENEMY_NAMES: &[(&str, &str)] = &[
    ("hitpoints", "Base Hitpoints"),
    ("attack_1_damage", "Attack 1 Base Damage"),
    ("attack_2_damage", "Attack 2 Base Damage"),
    ("attack_3_damage", "Attack 3 Base Damage"),
];

const BUY_NAMES: &[(&str, &str)] = &[
    ("stage_unlock_requirement", "Stage Unlock"),
    ("chapter_unlock_requirement", "Chapter Unlock"),
    ("sell_xp_yield", "Sell XP"),
    ("sell_np_yield", "Sell NP"),
    ("level_cap_ch2", "Level Cap Ch2"),
    ("true_form_obtain_level", "True Form Obtain Level"),
    ("egg_id_normal", "Egg ID Normal"),
    ("egg_id_evolved", "Egg ID Evolved"),
];

const ITEM_BUY_NAMES: &[(&str, &str)] = &[
    ("reflect_or_storage", "Goes To Storage"),
    ("stage_drop_item_id", "Drop ID"),
    ("sever_id", "Server ID"),
    ("src_item_id", "Source Item"),
    ("main_menu_type", "Menu Type"),
    ("gatya_ticket_id", "Ticket ID"),
    ("img_id", "Sprite ID"),
];

const MAP_STAGE_NAMES: &[(&str, &str)] = &[
    ("item_reward_setting", "Item Reward Set"),
    ("score_reward_setting", "Score Reward Set"),
    ("user_rank_threshold", "User Rank Needed"),
    ("map_pattern", "Map Pattern"),
    ("cost", "Entry Cost"),
    ("xp", "XP Reward"),
    ("init_track", "Music Track"),
    ("bgm_change_percent", "Boss Music At"),
    ("boss_track", "Boss Music Track"),
];

pub const MAP_PATTERN_FIELD: &str = "map_pattern";

const MAP_PATTERN_DEFAULT: &str = "0";

// (line, width) for every cell that comes off a line other than the addressed one.
const MAP_STAGE_CHROME: [(usize, usize); 2] = [(0, 7), (1, 1)];

// Where the stage's own row starts in the joined table.
pub(super) const MAP_STAGE_LEAD: usize = 8;

const COMBO_NAMES: &[(&str, &str)] = &[
    ("combo_id", "Combo ID"),
    ("charagroup_id", "Restriction Group"),
    ("slot_1_unit_id", "Slot 1 Unit"),
    ("slot_2_unit_id", "Slot 2 Unit"),
    ("slot_3_unit_id", "Slot 3 Unit"),
    ("slot_4_unit_id", "Slot 4 Unit"),
    ("slot_5_unit_id", "Slot 5 Unit"),
    ("effect_type", "Effect"),
    ("effect_level", "Power"),
];

pub const COMBO_SLOTS: usize = 5;

pub fn combo_unit(slot: usize) -> String {
    format!("slot_{}_unit_id", slot + 1)
}

pub fn combo_form(slot: usize) -> String {
    format!("slot_{}_form", slot + 1)
}

static CAT_LABELS: LazyLock<Vec<String>> = LazyLock::new(|| labels(&CAT_ORDER, CAT_NAMES));

pub(super) static ENEMY_LABELS: LazyLock<Vec<String>> = LazyLock::new(|| labels(&ENEMY_ORDER, ENEMY_NAMES));

static BUY_LABELS: LazyLock<Vec<String>> = LazyLock::new(|| labels(&BUY_ORDER, BUY_NAMES));

static CAT_ORDER: LazyLock<Vec<Entry>> = LazyLock::new(|| order(unitid::COLUMNS));

static ENEMY_ORDER: LazyLock<Vec<Entry>> = LazyLock::new(|| order(t_unit::COLUMNS));

static BUY_ORDER: LazyLock<Vec<Entry>> = LazyLock::new(|| order(UnitBuy::COLUMNS));

static COMBO_ORDER: LazyLock<Vec<Entry>> = LazyLock::new(|| order(NyancomboData::COLUMNS));

static ITEM_BUY_ORDER: LazyLock<Vec<Entry>> = LazyLock::new(|| order(GatyaItemBuy::COLUMNS));

static ITEM_BUY_LABELS: LazyLock<Vec<String>> =
    LazyLock::new(|| labels(&ITEM_BUY_ORDER, ITEM_BUY_NAMES));

static MAP_OPTION_ORDER: LazyLock<Vec<Entry>> = LazyLock::new(|| order(MapOptionEntry::COLUMNS));

static MAP_OPTION_LABELS: LazyLock<Vec<String>> =
    LazyLock::new(|| labels(&MAP_OPTION_ORDER, mapoption::NAMES));

static BACKGROUND_ORDER: LazyLock<Vec<Entry>> = LazyLock::new(|| order(BackgroundEntry::COLUMNS));

static BACKGROUND_LABELS: LazyLock<Vec<String>> =
    LazyLock::new(|| labels(&BACKGROUND_ORDER, backgrounds::NAMES));

static UPGRADE_ORDER: LazyLock<Vec<Entry>> = LazyLock::new(|| order(AbilityDataRow::COLUMNS));

static UPGRADE_LABELS: LazyLock<Vec<String>> = LazyLock::new(|| labels(&UPGRADE_ORDER, upgrades::NAMES));

// The popup is one draft over three lines of the file, so the three column tables are one
// table: the map-wide header, then the map pattern, then the stage's own row. `CHROME`
// says how many leading cells come off a line other than the addressed one.
static MAP_STAGE_ORDER: LazyLock<Vec<Entry>> = LazyLock::new(|| {
    let mut joined = order(MapStageDataHeader::COLUMNS);

    joined.push(Entry {
        field: MAP_PATTERN_FIELD,
        index: 0,
        scale: Scale::Raw,
        default: MAP_PATTERN_DEFAULT,
    });

    joined.extend(order(MapStageDataEntry::COLUMNS));

    joined
});

static MAP_STAGE_LABELS: LazyLock<Vec<String>> =
    LazyLock::new(|| labels(&MAP_STAGE_ORDER, MAP_STAGE_NAMES));

static COMBO_LABELS: LazyLock<Vec<String>> = LazyLock::new(|| labels(&COMBO_ORDER, COMBO_NAMES));

pub(super) fn order<T>(columns: &'static [Column<T>]) -> Vec<Entry> {
    let mut sorted: Vec<Entry> = columns
        .iter()
        .map(|column| Entry {
            field: column.field,
            index: column.index,
            scale: column.scale,
            default: column.default,
        })
        .collect();

    sorted.sort_by_key(|entry| entry.index);

    sorted
}

pub(super) fn labels(order: &[Entry], names: &[(&str, &str)]) -> Vec<String> {
    order
        .iter()
        .map(|entry| {
            names
                .iter()
                .find(|(field, _)| *field == entry.field)
                .map_or_else(|| prettify(entry.field), |(_, label)| (*label).to_owned())
        })
        .collect()
}

pub(super) fn prettify(field: &str) -> String {
    field
        .split('_')
        .map(|word| {
            let mut chars = word.chars();

            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(chars).collect()
            })
        })
        .collect::<Vec<String>>()
        .join(" ")
}

impl Schema {
    pub fn order(&self) -> &'static [Entry] {
        match self.subject {
            Subject::Cat => &CAT_ORDER,
            Subject::Enemy => &ENEMY_ORDER,
            Subject::Buy => &BUY_ORDER,
            Subject::Combo => &COMBO_ORDER,
            Subject::MapStage => &MAP_STAGE_ORDER,
            Subject::ItemBuy => &ITEM_BUY_ORDER,
            Subject::MapOption => &MAP_OPTION_ORDER,
            Subject::Background => &BACKGROUND_ORDER,
            Subject::Upgrade => &UPGRADE_ORDER,
            Subject::Curve
            | Subject::Talents
            | Subject::Costs
            | Subject::MapDrops
            | Subject::DropChara
            | Subject::ComboParam
            | Subject::OrbSlots
            | Subject::Difficulty
            | Subject::ExInvasion
            | Subject::CannonStatus
            | Subject::ExpCurve
            | Subject::OrbGrades
            | Subject::ExLottery
            | Subject::Shakes
            | Subject::Recipes
            | Subject::Shop
            | Subject::ZombieHeader
            | Subject::ZombieTier
            | Subject::Cpu
            | Subject::OrbTraits
            | Subject::Altar
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
            | Subject::Castles
            | Subject::TipRules
            | Subject::QuestStages
            | Subject::SlotLottery
            | Subject::RealmAltar
            | Subject::RealmDraws
            | Subject::LabyrinthFloors
            | Subject::LabyrinthLottery
            | Subject::MissionShortcuts
            | Subject::MissionDesignations
            | Subject::MonthlyMissions
            | Subject::SoundSettings
            | Subject::DojoChests
            | Subject::PopupTriggers
            | Subject::ComboCategories
            | Subject::CastleGrowth
            | Subject::DevelopmentCosts
            | Subject::DevelopmentTiers
            | Subject::MaterialMixes
            | Subject::GuideEntries
            | Subject::UnitLimits
            | Subject::EnemyGuide
            | Subject::ExpeditionAreas
            | Subject::GamatotoLevels
            | Subject::PrizePatterns
            | Subject::ShrineOfferings
            | Subject::ShrineExtras
            | Subject::FortuneOdds
            | Subject::ZonedIn
            | Subject::ZonedInSteps
            | Subject::FilterMaps
            | Subject::LineupExclusions
            | Subject::EnigmaLotteries
            | Subject::EnigmaRates
            | Subject::EnigmaDrops
            | Subject::EnigmaGroups
            | Subject::DiscoveryMedals
            | Subject::MapMarkers
            | Subject::EnigmaTimes
            | Subject::RecommendedUpgrades
            | Subject::LineupCounters
            | Subject::OrbOptions
            | Subject::EnergyRefunds
            | Subject::StageShortcuts
            | Subject::FanfareTiers
            | Subject::GuideSpotlights
            | Subject::TipSettings
            | Subject::EnigmaRarities
            | Subject::EngineerCap
            | Subject::LineupRatings
            | Subject::LineupBands
            | Subject::LineupRankGroups
            | Subject::AbilityIcons
            | Subject::TraitIcons
            | Subject::MenuTextSettings
            | Subject::MapSheets
            | Subject::OrbFilterChoices
            | Subject::ReelSymbols
            | Subject::LevelUpOffers
            | Subject::DemonCastles => &[],
        }
    }

    pub fn column(&self, index: usize) -> Option<&'static Entry> {
        self.order().get(index)
    }

    pub fn subject(&self) -> Subject {
        self.subject
    }

    pub fn comments(&self) -> bool {
        self.comments
    }

    pub fn known(&self) -> usize {
        match self.subject {
            Subject::Curve => BRACKETS,
            Subject::Talents => TALENT_WIDTH,
            Subject::Costs => COST_WIDTH,
            Subject::MapDrops => mapdrops::WIDTH,
            Subject::DropChara => mapdrops::CHARA_WIDTH,
            Subject::ComboParam => comboparams::BANDS.len(),
            Subject::OrbSlots => orbslots::LABELS.len(),
            Subject::Difficulty => difficulty::WIDTH,
            Subject::ExInvasion => exoption::LABELS.len(),
            Subject::CannonStatus => cannonstatus::LABELS.len(),
            Subject::ExpCurve => expcurve::BRACKETS,
            Subject::OrbGrades => orbgrades::LABELS.len(),
            Subject::ExLottery => exlottery::LABELS.len(),
            Subject::Shakes => shakes::LABELS.len(),
            Subject::Recipes => recipes::LABELS.len(),
            Subject::Shop => shop::LABELS.len(),
            Subject::ZombieHeader => zombies::HEADER_LABELS.len(),
            Subject::ZombieTier => zombies::TIER_LABELS.len(),
            Subject::Cpu => cpu::LABELS.len(),
            Subject::OrbTraits => orbtraits::LABELS.len(),
            Subject::Altar => altar::LABELS.len(),
            Subject::GamatotoLimit => gamatoto::LIMIT_LABELS.len(),
            Subject::GamatotoUnlock => gamatoto::UNLOCK_LABELS.len(),
            Subject::GamatotoDrop => gamatoto::DROP_LABELS.len(),
            Subject::Stamps => progress::STAMP_LABELS.len(),
            Subject::SlotUnlocks => progress::SLOT_LABELS.len(),
            Subject::Club => club::LABELS.len(),
            Subject::Missions => missions::LABELS.len(),
            Subject::Conditions => conditions::WIDTH,
            Subject::MissionUnlocks => missions::UNLOCK_LABELS.len(),
            Subject::MissionGroups => conditions::GROUP_WIDTH,
            Subject::GachaOptions => gacha::LABELS.len(),
            Subject::ChanceAnimations => chance::LABELS.len(),
            Subject::Castles => castles::LABELS.len(),
            Subject::TipRules => tiprules::LABELS.len(),
            Subject::QuestStages => quest::STAGE_LABELS.len(),
            Subject::SlotLottery => quest::LOTTERY_LABELS.len(),
            Subject::RealmAltar => realms::ALTAR_LABELS.len(),
            Subject::RealmDraws => realms::DRAW_LABELS.len(),
            Subject::LabyrinthFloors => labyrinth::LABELS.len(),
            Subject::LabyrinthLottery => labyrinth::LOTTERY_LABELS.len(),
            Subject::MissionShortcuts => missionsettings::SHORTCUT_LABELS.len(),
            Subject::MissionDesignations => missionsettings::DESIGNATION_LABELS.len(),
            Subject::MonthlyMissions => missionsettings::MONTHLY_LABELS.len(),
            Subject::SoundSettings => sounds::SOUND_LABELS.len(),
            Subject::DojoChests => sounds::CHEST_LABELS.len(),
            Subject::PopupTriggers => popups::LABELS.len(),
            Subject::ComboCategories => combocategories::LABELS.len(),
            Subject::CastleGrowth => castlegrowth::LABELS.len(),
            Subject::DemonCastles => realms::CASTLE_LABELS.len(),
            Subject::LevelUpOffers => guide::LEVEL_UP_LABELS.len(),
            Subject::ReelSymbols => prizes::REEL_LABELS.len(),
            Subject::OrbFilterChoices => guide::ORB_CHOICE_LABELS.len(),
            Subject::MapSheets => mapsheets::LABELS.len(),
            Subject::MenuTextSettings => guide::MENU_TEXT_LABELS.len(),
            Subject::TraitIcons => guide::TRAIT_ICON_LABELS.len(),
            Subject::AbilityIcons => guide::ABILITY_ICON_LABELS.len(),
            Subject::LineupRankGroups => guide::RANK_GROUP_LABELS.len(),
            Subject::LineupBands => guide::BAND_LABELS.len(),
            Subject::LineupRatings => guide::RATING_LABELS.len(),
            Subject::EngineerCap => development::ENGINEER_LABELS.len(),
            Subject::EnigmaRarities => enigma::RARITY_LABELS.len(),
            Subject::TipSettings => guide::TIP_SETTING_LABELS.len(),
            Subject::GuideSpotlights => guide::SPOTLIGHT_LABELS.len(),
            Subject::FanfareTiers => guide::FANFARE_LABELS.len(),
            Subject::StageShortcuts => guide::SHORTCUT_LABELS.len(),
            Subject::EnergyRefunds => guide::REFUND_LABELS.len(),
            Subject::OrbOptions => guide::ORB_OPTION_LABELS.len(),
            Subject::LineupCounters => guide::COUNTER_LABELS.len(),
            Subject::RecommendedUpgrades => guide::RECOMMEND_LABELS.len(),
            Subject::EnigmaTimes => enigma::TIME_LABELS.len(),
            Subject::MapMarkers => guide::MARKER_LABELS.len(),
            Subject::DiscoveryMedals => enigma::MEDAL_LABELS.len(),
            Subject::EnigmaGroups => enigma::GROUP_LABELS.len(),
            Subject::EnigmaDrops => enigma::DROP_LABELS.len(),
            Subject::EnigmaRates => enigma::RATE_LABELS.len(),
            Subject::EnigmaLotteries => enigma::LOTTERY_LABELS.len(),
            Subject::LineupExclusions => guide::EXCLUSION_LABELS.len(),
            Subject::FilterMaps => guide::FILTER_LABELS.len(),
            Subject::ZonedInSteps => gamatoto::ZONE_STEP_LABELS.len(),
            Subject::ZonedIn => gamatoto::ZONE_LABELS.len(),
            Subject::FortuneOdds => shrine::FORTUNE_LABELS.len(),
            Subject::ShrineExtras => shrine::EXTRA_LABELS.len(),
            Subject::ShrineOfferings => shrine::OFFERING_LABELS.len(),
            Subject::PrizePatterns => prizes::PATTERN_LABELS.len(),
            Subject::GamatotoLevels => gamatoto::LEVEL_LABELS.len(),
            Subject::ExpeditionAreas => gamatoto::AREA_LABELS.len(),
            Subject::EnemyGuide => guide::ENEMY_LABELS.len(),
            Subject::UnitLimits => guide::LIMIT_LABELS.len(),
            Subject::GuideEntries => guide::GUIDE_LABELS.len(),
            Subject::MaterialMixes => development::MIX_LABELS.len(),
            Subject::DevelopmentTiers => development::TIER_LABELS.len(),
            Subject::DevelopmentCosts => development::COST_LABELS.len(),
            _ => self.order().len(),
        }
    }

    pub fn label(&self, index: usize) -> Cow<'static, str> {
        if self.subject == Subject::Talents {
            return Cow::Owned(talent_label(index));
        }

        if self.subject == Subject::Costs {
            return match index.checked_sub(COST_HEAD) {
                Some(level) => Cow::Owned(format!("Level {}", level + 1)),
                None => Cow::Borrowed(COST_HEADING),
            };
        }

        if self.subject == Subject::MapDrops {
            return Cow::Borrowed(mapdrops::label(index));
        }

        if self.subject == Subject::DropChara {
            return Cow::Borrowed(mapdrops::chara_label(index));
        }

        if self.subject == Subject::Conditions {
            return Cow::Owned(conditions::label(index));
        }

        if self.subject == Subject::MissionGroups {
            return Cow::Owned(conditions::group_label(index));
        }

        if self.subject == Subject::Difficulty {
            return Cow::Owned(difficulty::label(index));
        }

        let fixed: Option<&'static [&'static str]> = match self.subject {
            Subject::GamatotoLimit => Some(&gamatoto::LIMIT_LABELS),
            Subject::GamatotoUnlock => Some(&gamatoto::UNLOCK_LABELS),
            Subject::GamatotoDrop => Some(&gamatoto::DROP_LABELS),
            Subject::Stamps => Some(&progress::STAMP_LABELS),
            Subject::SlotUnlocks => Some(&progress::SLOT_LABELS),
            Subject::Club => Some(&club::LABELS),
            Subject::Missions => Some(&missions::LABELS),
            Subject::MissionUnlocks => Some(&missions::UNLOCK_LABELS),
            Subject::GachaOptions => Some(&gacha::LABELS),
            Subject::ChanceAnimations => Some(&chance::LABELS),
            Subject::Castles => Some(&castles::LABELS),
            Subject::TipRules => Some(&tiprules::LABELS),
            Subject::QuestStages => Some(&quest::STAGE_LABELS),
            Subject::SlotLottery => Some(&quest::LOTTERY_LABELS),
            Subject::RealmAltar => Some(&realms::ALTAR_LABELS),
            Subject::RealmDraws => Some(&realms::DRAW_LABELS),
            Subject::LabyrinthFloors => Some(&labyrinth::LABELS),
            Subject::LabyrinthLottery => Some(&labyrinth::LOTTERY_LABELS),
            Subject::MissionShortcuts => Some(&missionsettings::SHORTCUT_LABELS),
            Subject::MissionDesignations => Some(&missionsettings::DESIGNATION_LABELS),
            Subject::MonthlyMissions => Some(&missionsettings::MONTHLY_LABELS),
            Subject::SoundSettings => Some(&sounds::SOUND_LABELS),
            Subject::DojoChests => Some(&sounds::CHEST_LABELS),
            Subject::PopupTriggers => Some(&popups::LABELS),
            Subject::ComboCategories => Some(&combocategories::LABELS),
            Subject::CastleGrowth => Some(&castlegrowth::LABELS),
            Subject::DemonCastles => Some(&realms::CASTLE_LABELS),
            Subject::LevelUpOffers => Some(&guide::LEVEL_UP_LABELS),
            Subject::ReelSymbols => Some(&prizes::REEL_LABELS),
            Subject::OrbFilterChoices => Some(&guide::ORB_CHOICE_LABELS),
            Subject::MapSheets => Some(&mapsheets::LABELS),
            Subject::MenuTextSettings => Some(&guide::MENU_TEXT_LABELS),
            Subject::TraitIcons => Some(&guide::TRAIT_ICON_LABELS),
            Subject::AbilityIcons => Some(&guide::ABILITY_ICON_LABELS),
            Subject::LineupRankGroups => Some(&guide::RANK_GROUP_LABELS),
            Subject::LineupBands => Some(&guide::BAND_LABELS),
            Subject::LineupRatings => Some(&guide::RATING_LABELS),
            Subject::EngineerCap => Some(&development::ENGINEER_LABELS),
            Subject::EnigmaRarities => Some(&enigma::RARITY_LABELS),
            Subject::TipSettings => Some(&guide::TIP_SETTING_LABELS),
            Subject::GuideSpotlights => Some(&guide::SPOTLIGHT_LABELS),
            Subject::FanfareTiers => Some(&guide::FANFARE_LABELS),
            Subject::StageShortcuts => Some(&guide::SHORTCUT_LABELS),
            Subject::EnergyRefunds => Some(&guide::REFUND_LABELS),
            Subject::OrbOptions => Some(&guide::ORB_OPTION_LABELS),
            Subject::LineupCounters => Some(&guide::COUNTER_LABELS),
            Subject::RecommendedUpgrades => Some(&guide::RECOMMEND_LABELS),
            Subject::EnigmaTimes => Some(&enigma::TIME_LABELS),
            Subject::MapMarkers => Some(&guide::MARKER_LABELS),
            Subject::DiscoveryMedals => Some(&enigma::MEDAL_LABELS),
            Subject::EnigmaGroups => Some(&enigma::GROUP_LABELS),
            Subject::EnigmaDrops => Some(&enigma::DROP_LABELS),
            Subject::EnigmaRates => Some(&enigma::RATE_LABELS),
            Subject::EnigmaLotteries => Some(&enigma::LOTTERY_LABELS),
            Subject::LineupExclusions => Some(&guide::EXCLUSION_LABELS),
            Subject::FilterMaps => Some(&guide::FILTER_LABELS),
            Subject::ZonedInSteps => Some(&gamatoto::ZONE_STEP_LABELS),
            Subject::ZonedIn => Some(&gamatoto::ZONE_LABELS),
            Subject::FortuneOdds => Some(&shrine::FORTUNE_LABELS),
            Subject::ShrineExtras => Some(&shrine::EXTRA_LABELS),
            Subject::ShrineOfferings => Some(&shrine::OFFERING_LABELS),
            Subject::PrizePatterns => Some(&prizes::PATTERN_LABELS),
            Subject::GamatotoLevels => Some(&gamatoto::LEVEL_LABELS),
            Subject::ExpeditionAreas => Some(&gamatoto::AREA_LABELS),
            Subject::EnemyGuide => Some(&guide::ENEMY_LABELS),
            Subject::UnitLimits => Some(&guide::LIMIT_LABELS),
            Subject::GuideEntries => Some(&guide::GUIDE_LABELS),
            Subject::MaterialMixes => Some(&development::MIX_LABELS),
            Subject::DevelopmentTiers => Some(&development::TIER_LABELS),
            Subject::DevelopmentCosts => Some(&development::COST_LABELS),
            _ => None,
        };

        if let Some(labels) = fixed {
            return labels.get(index).map_or_else(|| Cow::Owned(format!("Column {}", index + 1)), |label| Cow::Borrowed(*label));
        }

        if self.subject == Subject::Altar {
            return altar::LABELS.get(index).map_or_else(|| Cow::Owned(format!("Column {}", index + 1)), |label| Cow::Borrowed(*label));
        }

        if self.subject == Subject::OrbTraits {
            return orbtraits::LABELS.get(index).map_or_else(|| Cow::Owned(format!("Column {}", index + 1)), |label| Cow::Borrowed(*label));
        }

        if self.subject == Subject::Cpu {
            return cpu::LABELS.get(index).map_or_else(|| Cow::Owned(format!("Column {}", index + 1)), |label| Cow::Borrowed(*label));
        }

        if self.subject == Subject::ZombieHeader {
            return zombies::HEADER_LABELS.get(index).map_or_else(|| Cow::Owned(format!("Column {}", index + 1)), |label| Cow::Borrowed(*label));
        }

        if self.subject == Subject::ZombieTier {
            return zombies::TIER_LABELS.get(index).map_or_else(|| Cow::Owned(format!("Column {}", index + 1)), |label| Cow::Borrowed(*label));
        }

        if self.subject == Subject::Shop {
            return shop::LABELS.get(index).map_or_else(|| Cow::Owned(format!("Column {}", index + 1)), |label| Cow::Borrowed(*label));
        }

        if self.subject == Subject::Recipes {
            return recipes::LABELS.get(index).map_or_else(|| Cow::Owned(format!("Column {}", index + 1)), |label| Cow::Borrowed(*label));
        }

        if self.subject == Subject::Shakes {
            return shakes::LABELS.get(index).map_or_else(|| Cow::Owned(format!("Column {}", index + 1)), |label| Cow::Borrowed(*label));
        }

        if self.subject == Subject::ExLottery {
            return exlottery::LABELS.get(index).map_or_else(|| Cow::Owned(format!("Column {}", index + 1)), |label| Cow::Borrowed(*label));
        }

        if self.subject == Subject::OrbGrades {
            return orbgrades::LABELS.get(index).map_or_else(|| Cow::Owned(format!("Column {}", index + 1)), |label| Cow::Borrowed(*label));
        }

        if self.subject == Subject::ExpCurve {
            return Cow::Owned(expcurve::label(index));
        }

        if self.subject == Subject::CannonStatus {
            return cannonstatus::LABELS.get(index).map_or_else(|| Cow::Owned(format!("Column {}", index + 1)), |label| Cow::Borrowed(*label));
        }

        if self.subject == Subject::ExInvasion {
            return exoption::LABELS.get(index).map_or_else(|| Cow::Owned(format!("Column {}", index + 1)), |label| Cow::Borrowed(*label));
        }

        if self.subject == Subject::OrbSlots {
            return orbslots::LABELS.get(index).map_or_else(|| Cow::Owned(format!("Column {}", index + 1)), |label| Cow::Borrowed(*label));
        }

        if self.subject == Subject::ComboParam {
            return comboparams::BANDS.get(index).map_or_else(|| Cow::Owned(format!("Column {}", index + 1)), |band| Cow::Borrowed(*band));
        }

        if self.subject == Subject::Curve {
            let first = (index * BRACKET + 1).max(FIRST_GROWTH_LEVEL);

            return Cow::Owned(format!("Levels {first}-{}", (index + 1) * BRACKET));
        }

        let table = match self.subject {
            Subject::Cat => &CAT_LABELS,
            Subject::Enemy => &ENEMY_LABELS,
            Subject::Combo => &COMBO_LABELS,
            Subject::MapStage => &MAP_STAGE_LABELS,
            Subject::ItemBuy => &ITEM_BUY_LABELS,
            Subject::MapOption => &MAP_OPTION_LABELS,
            Subject::Background => &BACKGROUND_LABELS,
            Subject::Upgrade => &UPGRADE_LABELS,
            _ => &BUY_LABELS,
        };

        table.get(index).map_or_else(
            || Cow::Owned(format!("Column {}", index + 1)),
            |label| Cow::Borrowed(label.as_str()),
        )
    }

    // The cells that live on a line other than the addressed one, always leading.
    pub fn chrome(&self) -> &'static [(usize, usize)] {
        match self.subject {
            Subject::MapStage => &MAP_STAGE_CHROME,
            _ => &[],
        }
    }

    // How many decimal places a column's cells are written with. Everything but DropItem's
    // crown multipliers is a plain integer.
    pub(super) fn decimals(&self, index: usize) -> u32 {
        match self.subject {
            Subject::MapDrops => mapdrops::decimals(index),
            Subject::Difficulty => difficulty::decimals(index),
            _ => 0,
        }
    }

    pub(super) fn creates(&self) -> bool {
        matches!(self.subject, Subject::Talents | Subject::Combo | Subject::OrbSlots)
    }

    pub(super) fn switches(&self) -> bool {
        self.subject == Subject::Costs
    }

    pub(super) fn appends(&self) -> bool {
        self.subject == Subject::Combo
    }

    pub fn vacant(&self, cells: &[i32]) -> bool {
        if self.subject == Subject::OrbSlots {
            return cells.get(orbslots::SLOTS).copied().unwrap_or_default() == 0;
        }

        if self.subject != Subject::Talents {
            return false;
        }

        (0..TALENT_SLOTS).all(|slot| {
            cells
                .get(TALENT_HEAD + slot * TALENT_STRIDE)
                .copied()
                .unwrap_or_default()
                == 0
        })
    }

    pub fn index_of(&self, field: &str) -> Option<usize> {
        self.order().iter().position(|entry| entry.field == field)
    }

    pub fn field(&self, index: usize) -> Option<&'static str> {
        self.column(index).map(|column| column.field)
    }

    pub fn to_display(&self, index: usize, raw: i32, values: EditorMode) -> i32 {
        if values == EditorMode::Raw {
            return raw;
        }

        self.column(index)
            .map_or(raw, |column| column.scale.apply(raw))
    }

    pub fn to_raw(&self, index: usize, display: i32, values: EditorMode) -> i32 {
        if values == EditorMode::Raw {
            return display;
        }

        match self.column(index).map(|column| column.scale) {
            Some(Scale::Double) => display / 2,
            Some(Scale::Quarter) => display * 4,
            _ => display,
        }
    }

    pub fn fallback(&self, index: usize) -> i32 {
        if self.subject == Subject::Talents {
            return match slot_of(index) {
                Some(_) if (index - TALENT_HEAD) % TALENT_STRIDE == NAME_ID => -1,
                _ => 0,
            };
        }

        if self.subject == Subject::MapDrops {
            return mapdrops::fallback(index);
        }

        if self.subject == Subject::GachaOptions {
            return gacha::fallback(index);
        }

        if self.subject == Subject::ChanceAnimations {
            return chance::fallback(index);
        }

        if self.subject == Subject::TipRules {
            return tiprules::fallback(index);
        }

        if self.subject == Subject::MonthlyMissions {
            return missionsettings::monthly_fallback(index);
        }

        if self.subject == Subject::DojoChests {
            return sounds::chest_fallback(index);
        }

        if let Some(held) = self.field(index).and_then(mapdata::fallback) {
            return held;
        }

        self.column(index)
            .and_then(|column| i32::from_column(column.default))
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::{CAT, CAT_NAMES, ENEMY, ENEMY_NAMES, FromColumn, Schema};

    fn check(schema: &Schema, names: &[(&str, &str)], subject: &str) {
        for (field, _) in names {
            assert!(
                schema.order().iter().any(|column| column.field == *field),
                "{subject}: override names {field}, which codex no longer publishes"
            );
        }
    }

    fn defaults_parse(schema: &Schema, subject: &str) {
        for column in schema.order() {
            assert!(
                i32::from_column(column.default).is_some(),
                "{subject}: {} declares default {:?}, which is not an integer and would silently fall back to 0",
                column.field,
                column.default
            );
        }
    }

    #[test]
    fn cat_defaults_are_integers() {
        defaults_parse(&CAT, "cat");
    }

    #[test]
    fn enemy_defaults_are_integers() {
        defaults_parse(&ENEMY, "enemy");
    }

    #[test]
    fn cat_overrides_match_nyanko_fields() {
        check(&CAT, CAT_NAMES, "cat");
    }

    #[test]
    fn enemy_overrides_match_nyanko_fields() {
        check(&ENEMY, ENEMY_NAMES, "enemy");
    }
}
