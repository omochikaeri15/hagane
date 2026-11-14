use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};

use iced::alignment::Vertical;
use iced::widget::{button, column, container, pick_list, row, scrollable, text, text_input, Column};
use iced::{Element, Length, Padding, Size, Task};
use tracing::warn;

use codex::chapter::map::{BonusType, RuleType};
use foundry::common::formats::json::{self, Field, Record, Value};
use foundry::common::preview::{self, Stamp};
use foundry::domains::mods;
use foundry::domains::settings::EditorMode;
use foundry::Vfs;

use crate::app::theme;
use crate::common::feedback::{Slot, CONFIRM_LABEL, FAILURE_LABEL};
use crate::widget::{popup, smooth_scroll};

use super::{Mark, Reader};

const SIZE: Size = Size::new(420.0, 460.0);
const POPUP: popup::Spec = popup::Spec::new(popup::Kind::Records, SIZE);

const BODY_PADDING: f32 = 12.0;
const GAP: f32 = 8.0;
const ROW_GAP: f32 = 6.0;
const LABEL_SIZE: f32 = 12.0;
const INPUT_SIZE: f32 = 13.0;
const INPUT_PADDING: f32 = 4.0;
const INPUT_WIDTH: f32 = 170.0;
const BUTTON_SIZE: f32 = 13.0;
const BUTTON_PADDING: f32 = 8.0;
const SCROLL_GAP: f32 = 12.0;
const FRAME_GAP: f32 = 3.0;
const FRAME_INSET: f32 = theme::CARD_BORDER_WIDTH + FRAME_GAP;

const SYNC_LABEL: &str = "Sync With \"game\"";
const MISSING_LABEL: &str = "This record is no longer in the file";
const LIST_JOINT: &str = ", ";
const PATH_JOINT: &str = "/";
const QUOTE: char = '"';
const HEADING_SIZE: f32 = 13.0;
const SECTION_GAP: f32 = 6.0;

type Choices = &'static [(i32, &'static str)];

const RARITIES: Choices = &[
    (-1, "Any Rarity"),
    (0, "Normal"),
    (1, "Special"),
    (2, "Rare"),
    (3, "Super Rare"),
    (4, "Uber Rare"),
    (5, "Legend Rare"),
];

const BUFF_TYPES: Choices = &[(0, "Level Up"), (1, "Buff Skill"), (2, "Combo Effect"), (3, "Cannon")];

const TARGETS: Choices = &[(0, "Bonus Cats"), (1, "Bonus Buffs")];

const FUNDS: Choices = &[(0, "Neither"), (1, "War Funds"), (2, "Expenses")];

const SCORING: Choices = &[(0, "Points per Kill")];

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum Book {
    Buffs,
    BuffSkills,
    Charas,
    EffectAbilities,
    LotteryGroups,
    StageLotteries,
    Permanent,
    MapSettings,
    SpecialRules,
    ScoreBonus,
    RuleBans,
    PointRules,
    Orbs,
    Medals,
    Gauge,
    MapConditions,
    ClearRewards,
    PointMaps,
    PointUnlocks,
    PointRewards,
    EventDisplay,
    MissionLimits,
    GachaMissions,
    GachaBonus,
    LoginStamps,
    Catseyes,
    UnlockPopups,
    RemoteConfig,
}

pub(crate) const BOOKS: [Book; 8] = [
    Book::Buffs,
    Book::BuffSkills,
    Book::Charas,
    Book::LotteryGroups,
    Book::StageLotteries,
    Book::Permanent,
    Book::MapSettings,
    Book::EffectAbilities,
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Rule {
    Plain,
    Floor(i32),
    Choice(Choices),
}

impl Book {
    pub(super) fn base(self) -> &'static str {
        match self {
            Book::Buffs => "RoguelikeBonusBuff.json",
            Book::BuffSkills => "RoguelikeBonusBuffSkill.json",
            Book::Charas => "RoguelikeBonusChara.json",
            Book::EffectAbilities => "RoguelikeBonusEffectAbility.json",
            Book::LotteryGroups => "RoguelikeBonusLotteryGroup.json",
            Book::StageLotteries => "RoguelikeBonusMapStageLottery.json",
            Book::Permanent => "RoguelikeBonusPermanent.json",
            Book::MapSettings => "RoguelikeMapSettings.json",
            Book::SpecialRules => "SpecialRulesMap.json",
            Book::ScoreBonus => "ScoreBonusMap.json",
            Book::RuleBans => "SpecialRulesMapOption.json",
            Book::PointRules => "StagePointRulesMap.json",
            Book::Orbs => "equipmentlist.json",
            Book::Medals => "medallist.json",
            Book::Gauge => "Treasure_gauge.json",
            Book::MapConditions => "MapConditions.json",
            Book::ClearRewards => "MapStageDataClearCountReward.json",
            Book::PointMaps => "PointStageSettings.json",
            Book::PointUnlocks => "PointRelease.json",
            Book::PointRewards => "PointEventReward.json",
            Book::EventDisplay => "eventDisplayData.json",
            Book::MissionLimits => "Mission_LimitOption.json",
            Book::GachaMissions => "Mission_NewGatyaSetting.json",
            Book::GachaBonus => "EventGatyaMedamaItem.json",
            Book::LoginStamps => "DailyLoginEventGrade.json",
            Book::Catseyes => "unititempowerup.json",
            Book::UnlockPopups => "unlockparam.json",
            Book::RemoteConfig => "remote_config_defaults.json",
        }
    }

    pub(super) fn title(self) -> &'static str {
        match self {
            Book::Buffs => "Bonus Buffs",
            Book::BuffSkills => "Buff Skills",
            Book::Charas => "Bonus Cats",
            Book::EffectAbilities => "Ability Order",
            Book::LotteryGroups => "Lottery Groups",
            Book::StageLotteries => "Stage Lotteries",
            Book::Permanent => "Permanent Bonuses",
            Book::MapSettings => "Map Settings",
            Book::SpecialRules => "Special Rules",
            Book::ScoreBonus => "Score Bonuses",
            Book::RuleBans => "Rule Combo Bans",
            Book::PointRules => "Point Rules",
            Book::Orbs => "Talent Orbs",
            Book::Medals => "Meow Medals",
            Book::Gauge => "Treasure Gauge",
            Book::MapConditions => "Map Unlock Conditions",
            Book::ClearRewards => "Clear Count Rewards",
            Book::PointMaps => "Point Maps",
            Book::PointUnlocks => "Point Unlocks",
            Book::PointRewards => "Point Rewards",
            Book::EventDisplay => "Event Display",
            Book::MissionLimits => "Mission Limits",
            Book::GachaMissions => "Gacha Missions",
            Book::GachaBonus => "Gacha Bonus Items",
            Book::LoginStamps => "Login Stamps",
            Book::Catseyes => "Catseye Rules",
            Book::UnlockPopups => "Unlock Popups",
            Book::RemoteConfig => "Remote Config Defaults",
        }
    }

    pub(super) fn noun(self) -> &'static str {
        match self {
            Book::Buffs => "Buff",
            Book::BuffSkills => "Skill",
            Book::Charas => "Bonus Cat",
            Book::EffectAbilities => "Ability",
            Book::LotteryGroups => "Group",
            Book::StageLotteries => "Lottery",
            Book::Permanent => "Bonus",
            Book::MapSettings => "Setting",
            Book::SpecialRules => "Rules",
            Book::ScoreBonus => "Bonuses",
            Book::RuleBans => "Rule",
            Book::PointRules => "Stage",
            Book::Orbs => "Orb",
            Book::Medals => "Medal",
            Book::Gauge => "Gauge",
            Book::MapConditions => "Map",
            Book::ClearRewards => "Reward",
            Book::PointMaps | Book::PointUnlocks | Book::PointRewards => "Event",
            Book::EventDisplay => "Map",
            Book::MissionLimits => "Limit",
            Book::GachaMissions => "Mission",
            Book::GachaBonus => "Gacha",
            Book::LoginStamps => "Stamp",
            Book::Catseyes => "Rule",
            Book::UnlockPopups => "Popup",
            Book::RemoteConfig => "Defaults",
        }
    }

    fn label(self, key: &str) -> String {
        let known = match (self, key) {
            (Book::Buffs | Book::Charas, "Rarity") => Some("Rarity Tier"),
            (Book::Buffs, "Limit") => Some("Max Stacks"),
            (Book::Buffs, "TargetRarity") => Some("Target Rarity"),
            (Book::Buffs, "TargetCharaID") => Some("Target Cat ID"),
            (Book::Buffs, "RoguelikeBonusBuffName") => Some("Name Text Key"),
            (Book::Buffs, "RoguelikeBonusBuffExplanation") => Some("Explanation Text Key"),
            (Book::Buffs, "BuffType") => Some("Buff Type"),
            (Book::Buffs, "LevelUp") => Some("Levels Added"),
            (Book::Buffs, "PlusUp") => Some("Plus Levels Added"),
            (Book::Buffs, "BuffSkillID") => Some("Buff Skill ID"),
            (Book::Buffs, "EffectID") => Some("Combo Effect ID"),
            (Book::Buffs, "VolumeID") => Some("Combo Strength"),
            (Book::Buffs, "ID") => Some("Cannon ID"),
            (Book::Buffs, "Level") => Some("Cannon Level"),
            (Book::BuffSkills, "AbilityID") => Some("Ability ID"),
            (Book::BuffSkills, "MaxLevel") => Some("Max Level"),
            (Book::BuffSkills, "Parameters") => Some("Values"),
            (Book::Charas, "CharaID") => Some("Cat ID"),
            (Book::Charas, "Level") => Some("Level"),
            (Book::EffectAbilities, "Order") => Some("Display Order"),
            (Book::LotteryGroups, "BonusTarget") => Some("Draws From"),
            (Book::LotteryGroups, "BonusTargetID") => Some("Entry IDs"),
            (Book::StageLotteries, "DrawCount") => Some("Choices Offered"),
            (Book::StageLotteries | Book::Permanent, "BonusGroupID") => Some("Lottery Group IDs"),
            (Book::MapSettings, "MapID") => Some("Map IDs"),
            (Book::MapSettings, "Level") => Some("Levels"),
            (Book::MapSettings, "PresetFileName") => Some("Lineup Preset File"),
            (Book::SpecialRules, "ContentsType") => Some("Trust Fund Draws On"),
            (Book::SpecialRules, "RuleNameLabel") => Some("Name Text Key"),
            (Book::SpecialRules, "RuleExplanationLabel") => Some("Explanation Text Key"),
            (Book::SpecialRules, "Parameters") => Some("Values"),
            (Book::SpecialRules, "ResetFlag") => Some("Resets"),
            (Book::SpecialRules, "PopupLabel") => Some("Reset Popup Text Key"),
            (Book::ScoreBonus, "BonusNameLabel") => Some("Name Text Key"),
            (Book::ScoreBonus, "BonusExplanationLabel") => Some("Explanation Text Key"),
            (Book::ScoreBonus, "Parameters") => Some("Values"),
            (Book::RuleBans, "InvalidNyancomboID") => Some("Disabled Combo IDs"),
            (Book::PointRules, "RuleType") => Some("Scoring"),
            (Book::PointRules, "PageHeadline") => Some("Page Headline Text Key"),
            (Book::PointRules, "RuleHeadlineLabel") => Some("Headline Text Keys"),
            (Book::PointRules, "RuleExplanationLabel") => Some("Explanation Text Keys"),
            (Book::PointRules, "RuleExplanationTypeLabel") => Some("Explanation Styles"),
            (Book::PointRules, "Parameters") => Some("Points per Kill, then Points & Score pairs"),
            (Book::PointRules, "RuleImageLabel") => Some("Image Keys"),
            (Book::Orbs, "gradeID") => Some("Grade"),
            (Book::Orbs, "content") => Some("Effect"),
            (Book::Orbs, "value") => Some("Values"),
            (Book::Orbs, "attribute") => Some("Trait"),
            (Book::Medals, "grade") => Some("Grade"),
            (Book::Medals, "line") => Some("Text Line"),
            (Book::Medals, "map") => Some("Map IDs"),
            (Book::Medals, "treasure") => Some("Treasure Grade"),
            (Book::Medals, "star") => Some("Crowns"),
            (Book::Medals, "stage") => Some("Stage"),
            (Book::Medals, "chara") => Some("Cat ID"),
            (Book::Medals, "medal") => Some("Medal IDs"),
            (Book::Medals, "action") => Some("Action"),
            (Book::Medals, "count") => Some("Count"),
            (Book::Medals, "limit") => Some("Limit"),
            (Book::Gauge, "MaxValue") => Some("Gauge Size"),
            (Book::Gauge, "StageClear") => Some("Fill Per Stage Clear"),
            (Book::Gauge, "RankUp") => Some("Fill Per Rank Up"),
            (Book::Gauge, "Gamatoto") => Some("Fill Per Gamatoto Trip"),
            (Book::MapConditions, "condition") => Some("Condition Codes"),
            (Book::MapConditions, "stage") => Some("Stage"),
            (Book::MapConditions, "count") => Some("Conditions Needed"),
            (Book::MapConditions, "serverSetting") => Some("Server Setting"),
            (Book::MapConditions, "hidden") => Some("Hidden"),
            (Book::MapConditions, "limitMessage") => Some("Locked Message Text Key"),
            (Book::ClearRewards, "DropItemID") => Some("Item"),
            (Book::ClearRewards, "Quantity") => Some("Quantity"),
            (Book::PointMaps, "MapID") => Some("Map IDs"),
            (Book::PointUnlocks, "serverSetting") => Some("Server Settings"),
            (Book::PointUnlocks, "ReleasePoint") => Some("Points to Unlock"),
            (Book::PointUnlocks, "Popup") => Some("Popup Text Key"),
            (Book::PointUnlocks, "PointReleaseHeadlineLabel") => Some("Headline Text Key"),
            (Book::PointUnlocks, "PointReleaseExplanationLabel") => Some("Explanation Text Key"),
            (Book::PointRewards, "ResetMapID") => Some("Reset Map ID"),
            (Book::PointRewards, "RewardID") => Some("Reward ID"),
            (Book::PointRewards, "Point") => Some("Points Needed"),
            (Book::PointRewards, "Type") => Some("Reward Type"),
            (Book::PointRewards, "ID") => Some("Reward Item ID"),
            (Book::PointRewards, "Quantity") => Some("Quantity"),
            (Book::PointRewards, "Label0") => Some("Text Key"),
            (Book::PointRewards, "Label1") => Some("Web Text Key"),
            (Book::EventDisplay, "eventWeight") => Some("Event Weight"),
            (Book::EventDisplay, "PercentBanner") => Some("Banner Scale %"),
            (Book::EventDisplay, "NekoBageImg") => Some("Badge Image"),
            (Book::MissionLimits, "Map") => Some("Map IDs"),
            (Book::MissionLimits, "RarityLimit") => Some("Rarity Limits"),
            (Book::MissionLimits, "SlotLimit") => Some("Slot Limits"),
            (Book::MissionLimits, "SlotOptionRule") => Some("Slot Rules"),
            (Book::GachaMissions, "Rarity") => Some("Rarities"),
            (Book::GachaMissions, "Type") | (Book::GachaBonus, "type") => Some("Item Type"),
            (Book::GachaMissions, "ItemID") | (Book::GachaBonus, "id") => Some("Item ID"),
            (Book::GachaBonus, "minCount") => Some("Minimum Rolls"),
            (Book::GachaBonus, "value") => Some("Quantity"),
            (Book::GachaBonus, "weight") => Some("Weight"),
            (Book::LoginStamps, "DailyLoginStampGroupID") => Some("Group"),
            (Book::LoginStamps, "DailyLoginStampGrade") => Some("Grade"),
            (Book::LoginStamps, "Parameters") => Some("Values"),
            (Book::Catseyes, "catseye") => Some("Catseye Item IDs"),
            (Book::Catseyes, "max") => Some("Up To Level"),
            (Book::Catseyes, "value") => Some("Levels Per Use"),
            (Book::UnlockPopups, "CharaID") => Some("Cat IDs"),
            (Book::UnlockPopups, "Category") => Some("Categories"),
            (Book::UnlockPopups, "Value") => Some("Value"),
            _ => None,
        };

        known.map_or_else(|| prettify(key), str::to_owned)
    }

    fn rule(self, key: &str) -> Rule {
        match (self, key) {
            (Book::Buffs, "TargetRarity") => Rule::Choice(RARITIES),
            (Book::Buffs, "BuffType") => Rule::Choice(BUFF_TYPES),
            (Book::LotteryGroups, "BonusTarget") => Rule::Choice(TARGETS),
            (Book::SpecialRules, "ContentsType") => Rule::Choice(FUNDS),
            (Book::PointRules, "RuleType") => Rule::Choice(SCORING),
            (Book::Gauge, "MaxValue") => Rule::Floor(1),
            (Book::ClearRewards, "DropItemID") => Rule::Floor(-1),
            (Book::Buffs, "TargetCharaID") => Rule::Floor(-1),
            _ => Rule::Floor(0),
        }
    }
}

impl Book {
    pub(super) fn entries(self, text: &str) -> Vec<Vec<String>> {
        match self.entry_root() {
            Some(root) => json::objects(text)
                .into_iter()
                .filter(|node| node.path.len() == ENTRY_DEPTH && node.path.first().is_some_and(|head| head == root))
                .map(|node| node.path)
                .collect(),
            None => json::records(text).into_iter().filter(|record| record.leaf).map(|record| record.path).collect(),
        }
    }

    fn entry_root(self) -> Option<&'static str> {
        match self {
            Book::Medals => Some("iconID"),
            Book::MissionLimits => Some("limitID"),
            Book::GachaMissions => Some("ID"),
            Book::GachaBonus => Some("GatyaID"),
            Book::LoginStamps => Some("DailyLoginStampID"),
            Book::Catseyes => Some("data"),
            Book::UnlockPopups => Some("popupID"),
            _ => None,
        }
    }

    fn shallow(self, path: &[String]) -> bool {
        self == Book::PointRewards && path.len() == POINT_DEPTH
    }

    pub(super) fn map_root(self) -> Option<&'static str> {
        match self {
            Book::SpecialRules | Book::ScoreBonus | Book::PointRules | Book::ClearRewards => Some(MAP_ROOT),
            Book::EventDisplay => Some(MAP_SET_ROOT),
            _ => None,
        }
    }

    fn section(self, suffix: &[String]) -> String {
        let named = match (self, suffix) {
            (Book::SpecialRules, [group, id]) if group == RULE_GROUP => id.parse::<u8>().ok().and_then(|id| rule_name(RuleType::from_parts(id, Vec::new()))),
            (Book::ScoreBonus, [group, id]) if group == BONUS_GROUP => id.parse::<u8>().ok().and_then(|id| bonus_name(BonusType::from_parts(id, Vec::new()))),
            (Book::PointRules, [group, id]) if group == STAGE_GROUP && id == EVERY_STAGE => Some(EVERY_STAGE_NAME),
            (Book::Catseyes, [group, index]) if group == CATSEYE_STEPS => {
                return format!("{CATSEYE_STEP} {index}");
            }
            (Book::PointRewards, [group, index]) if group == POINT_REWARDS => {
                return format!("{POINT_REWARD} {index}");
            }
            (Book::ClearRewards, [stage, group, count]) if group == REWARD_GROUP => {
                return format!("{STAGE_NAME} {stage:0>2}, {CLEAR_COUNT} {count}");
            }
            (Book::PointRules, [group, id]) if group == STAGE_GROUP => {
                return id.parse::<u32>().map_or_else(|_| format!("{STAGE_NAME} {id}"), |stage| format!("{STAGE_NAME} {stage:02}"));
            }
            _ => None,
        };

        named.map_or_else(|| suffix.iter().map(|segment| prettify(segment)).collect::<Vec<_>>().join(" "), str::to_owned)
    }
}

const RULE_GROUP: &str = "RuleType";
const MAP_ROOT: &str = "MapID";
const MAP_SET_ROOT: &str = "MapSet";
const ENTRY_DEPTH: usize = 2;
const BONUS_GROUP: &str = "BonusType";
const STAGE_GROUP: &str = "StageIndex";
const STAGE_NAME: &str = "Stage";
const REWARD_GROUP: &str = "data";
const POINT_REWARDS: &str = "Rewards";
const POINT_REWARD: &str = "Reward";
const POINT_DEPTH: usize = 2;
const CATSEYE_STEPS: &str = "a";
const CATSEYE_STEP: &str = "Step";
const CATSEYE_GROUP: &str = "Group";
const CATSEYE_RARITIES: [&str; 6] = ["Normal", "Special", "Rare", "Super Rare", "Uber Rare", "Legend Rare"];
const CATSEYE_ANY: &str = "Any Rarity";

pub(super) fn catseye_name(key: &str) -> Option<String> {
    let key = key.parse::<i32>().ok()?;
    let group = key / 10;
    let rarity = key - group * 10 - 1;

    let named = usize::try_from(rarity).ok().and_then(|at| CATSEYE_RARITIES.get(at)).copied().unwrap_or(CATSEYE_ANY);

    Some(format!("{CATSEYE_GROUP} {group}, {named}"))
}
const CLEAR_COUNT: &str = "Clear Count";
const EVERY_STAGE: &str = "-1";
const EVERY_STAGE_NAME: &str = "Every Stage";

fn bonus_name(bonus: BonusType) -> Option<&'static str> {
    match bonus {
        BonusType::Weaken(_) => Some("Weaken"),
        BonusType::Freeze(_) => Some("Freeze"),
        BonusType::Slow(_) => Some("Slow"),
        BonusType::Knockback(_) => Some("Knockback"),
        BonusType::StrongAttack(_) => Some("Strong Attack"),
        BonusType::MassiveDamage(_) => Some("Massive Damage"),
        BonusType::StrongDefense(_) => Some("Strong Defense"),
        BonusType::Resist(_) => Some("Resist"),
        BonusType::Unknown(..) => None,
    }
}

pub(super) fn rule_label(id: &str) -> Option<&'static str> {
    id.parse::<u8>().ok().and_then(|id| rule_name(RuleType::from_parts(id, Vec::new())))
}

fn rule_name(rule: RuleType) -> Option<&'static str> {
    match rule {
        RuleType::TrustFund(_) => Some("Trust Fund"),
        RuleType::CooldownEquality(_) => Some("Cooldown Equality"),
        RuleType::LineupLimit(_) => Some("Lineup Limit"),
        RuleType::RarityLimit(_) => Some("Rarity Limit"),
        RuleType::CheapLabor(_) => Some("Cheap Labor"),
        RuleType::CatCost(_) => Some("Cat Cost"),
        RuleType::CatProduction(_) => Some("Cat Production"),
        RuleType::TotalDeployLimit(_) => Some("Total Deploy Limit"),
        RuleType::MoreThanOne(_) => Some("More Than One"),
        RuleType::MegaCatCannon(_) => Some("Mega Cat Cannon"),
        RuleType::UniformMotion(_) => Some("Uniform Motion"),
        RuleType::CompoundingCost(_) => Some("Compounding Cost"),
        RuleType::Unknown(..) => None,
    }
}

fn prettify(key: &str) -> String {
    let mut spaced = String::with_capacity(key.len() + 4);
    let mut previous: Option<char> = None;

    for glyph in key.chars() {
        if glyph.is_uppercase() && previous.is_some_and(|before| before.is_lowercase()) {
            spaced.push(' ');
        }

        spaced.push(glyph);
        previous = Some(glyph);
    }

    spaced
}

#[derive(Clone, PartialEq)]
pub(crate) struct Plan {
    book: Book,
    label: String,
    file: String,
    game: PathBuf,
    target_mod: String,
    path: Vec<String>,
    values: EditorMode,
    named: Named,
}

pub(super) type Named = Vec<(&'static str, Vec<(i32, String)>)>;

pub(super) struct Seed {
    pub(super) book: Book,
    pub(super) label: String,
    pub(super) file: String,
    pub(super) game: PathBuf,
    pub(super) target_mod: String,
    pub(super) path: Vec<String>,
    pub(super) values: EditorMode,
    pub(super) named: Named,
}

pub(super) fn plan(seed: Seed) -> Plan {
    let Seed { book, label, file, game, target_mod, path, values, named } = seed;

    Plan { book, label, file, game, target_mod, path, values, named }
}

impl Plan {
    fn held(&self, vfs: &Vfs) -> Option<PathBuf> {
        mods::find(vfs, &self.target_mod, Path::new(&self.file))
    }

    fn source(&self, vfs: &Vfs) -> PathBuf {
        self.held(vfs).unwrap_or_else(|| self.game.clone())
    }
}

fn subtree(text: &str, path: &[String], shallow: bool) -> (Vec<Record>, Option<Range<usize>>) {
    let mut records: Vec<Record> = json::records(text)
        .into_iter()
        .filter(|record| match shallow {
            true => record.path == path,
            false => record.path.starts_with(path),
        })
        .collect();

    records.sort_by_key(|record| record.span.start);

    let span = json::objects(text).into_iter().find(|node| node.path == path).map(|node| node.span);

    (records, span)
}

fn object<'a>(text: &'a str, path: &[String]) -> Option<&'a str> {
    let (_, span) = subtree(text, path, false);

    text.get(span?)
}

fn own_fields(text: &str, path: &[String]) -> Option<Vec<(String, String)>> {
    let (records, _) = subtree(text, path, true);
    let record = records.into_iter().next()?;

    record.fields.iter().map(|field| Some((field.key.clone(), text.get(field.span.clone())?.to_owned()))).collect()
}

fn snapshot(text: &str, path: &[String], shallow: bool) -> Option<String> {
    match shallow {
        true => own_fields(text, path).map(|fields| fields.into_iter().map(|(key, value)| format!("{key}={value}")).collect::<Vec<_>>().join("\n")),
        false => object(text, path).map(str::to_owned),
    }
}

fn shown(value: &Value) -> String {
    match value {
        Value::Number(number) => number.clone(),
        Value::Text(words) => words.clone(),
        Value::Numbers(items) | Value::Texts(items) | Value::Tokens(items) => items.join(LIST_JOINT),
        Value::Flag(held) => held.to_string(),
        Value::Other => String::new(),
    }
}

struct Draft {
    plan: Plan,
    read_from: PathBuf,
    stamp: Stamp,
    text: String,
    records: Vec<Record>,
    span: Option<Range<usize>>,
    lines: Vec<(usize, usize)>,
    inputs: Vec<String>,
    failed: bool,
}

impl Draft {
    fn load(plan: Plan, vfs: &Vfs) -> Option<Draft> {
        let read_from = plan.source(vfs);

        let text = fs::read_to_string(&read_from)
            .inspect_err(|err| warn!(path = %read_from.display(), "Record editor could not read the file: {}", err))
            .ok()?;

        let stamp = preview::stamp(&read_from)?;
        let mut draft = Draft {
            plan,
            read_from,
            stamp,
            text,
            records: Vec::new(),
            span: None,
            lines: Vec::new(),
            inputs: Vec::new(),
            failed: false,
        };

        draft.index();

        Some(draft)
    }

    fn index(&mut self) {
        let (records, span) = subtree(&self.text, &self.plan.path, self.plan.book.shallow(&self.plan.path));

        self.records = records;
        self.span = span;
        self.lines.clear();
        self.inputs.clear();

        for (at, record) in self.records.iter().enumerate() {
            for (slot, field) in record.fields.iter().enumerate() {
                self.lines.push((at, slot));
                self.inputs.push(shown(&field.value));
            }
        }
    }

    fn field(&self, index: usize) -> Option<&Field> {
        let (at, slot) = *self.lines.get(index)?;

        self.records.get(at)?.fields.get(slot)
    }

    fn drifted(&self) -> bool {
        preview::stamp(&self.read_from) != Some(self.stamp)
    }

    fn rule(&self, key: &str) -> Rule {
        match self.plan.values {
            EditorMode::Raw => Rule::Plain,
            EditorMode::Resolved => self.plan.book.rule(key),
        }
    }

    fn edit(&mut self, index: usize, typed: &str) -> bool {
        let Some(field) = self.field(index).cloned() else {
            return false;
        };

        if let Some(slot) = self.inputs.get_mut(index) {
            typed.clone_into(slot);
        }

        let rendered = match &field.value {
            Value::Number(_) => {
                let digits = typed.strip_prefix('-').unwrap_or(typed);

                if digits.is_empty() || !digits.chars().all(|glyph| glyph.is_ascii_digit()) {
                    return false;
                }

                let Ok(number) = typed.parse::<i64>() else {
                    return false;
                };

                let number = match self.rule(&field.key) {
                    Rule::Floor(least) => number.max(i64::from(least)),
                    Rule::Plain | Rule::Choice(_) => number,
                };

                number.to_string()
            }
            Value::Text(_) => json::quote(typed),
            Value::Numbers(_) => {
                let parsed: Option<Vec<i64>> = typed
                    .split(',')
                    .map(str::trim)
                    .filter(|item| !item.is_empty())
                    .map(|item| item.parse::<i64>().ok())
                    .collect();

                match (parsed, typed.contains(QUOTE)) {
                    (Some(items), _) => json::numbers(&self.text, &field.span, &items),
                    (None, true) => match json::tokenize(typed) {
                        Some(items) => json::tokens(&self.text, &field.span, &items),
                        None => return false,
                    },
                    (None, false) => return false,
                }
            }
            Value::Texts(_) if typed.contains(QUOTE) => match json::tokenize(typed) {
                Some(items) => json::tokens(&self.text, &field.span, &items),
                None => return false,
            },
            Value::Texts(_) => {
                let items: Vec<String> = typed.split(',').map(str::trim).filter(|item| !item.is_empty()).map(str::to_owned).collect();

                json::texts(&self.text, &field.span, &items)
            }
            Value::Tokens(_) => match json::tokenize(typed) {
                Some(items) => json::tokens(&self.text, &field.span, &items),
                None => return false,
            },
            Value::Flag(_) => match typed.trim() {
                "true" => "true".to_owned(),
                "false" => "false".to_owned(),
                _ => return false,
            },
            Value::Other => return false,
        };

        let Some(text) = json::replace(&self.text, &field.span, &rendered) else {
            return false;
        };

        if text == self.text {
            return false;
        }

        self.text = text;
        self.index();

        if let Some(slot) = self.inputs.get_mut(index)
            && !matches!(field.value, Value::Number(_) | Value::Other)
        {
            typed.clone_into(slot);
        }

        true
    }

    fn restore(&mut self, vfs: &Vfs) -> bool {
        let Ok(vanilla) = fs::read_to_string(&self.plan.game) else {
            warn!(path = %self.plan.game.display(), "Record editor could not read the vanilla file");

            return false;
        };

        if self.plan.book.shallow(&self.plan.path) {
            let Some(fields) = own_fields(&vanilla, &self.plan.path) else {
                warn!(path = %self.plan.game.display(), "Record editor found no vanilla copy of this record");

                return false;
            };

            for (key, value) in fields {
                let span = self.records.first().and_then(|record| record.field(&key)).map(|field| field.span.clone());

                if let Some(text) = span.and_then(|span| json::replace(&self.text, &span, &value)) {
                    self.text = text;
                    self.index();
                }
            }

            return self.persist(vfs);
        }

        let Some(original) = object(&vanilla, &self.plan.path).map(str::to_owned) else {
            warn!(path = %self.plan.game.display(), "Record editor found no vanilla copy of this record");

            return false;
        };

        let Some(span) = self.span.clone() else {
            return false;
        };

        let Some(text) = json::replace(&self.text, &span, &original) else {
            return false;
        };

        self.text = text;
        self.index();

        self.persist(vfs)
    }

    fn destination(&self, vfs: &Vfs) -> Option<(PathBuf, Stamp)> {
        if self.read_from != self.plan.game {
            return Some((self.read_from.clone(), self.stamp));
        }

        let path = mods::ensure_as(vfs, &self.plan.target_mod, &self.plan.game, &self.plan.file)
            .inspect_err(|err| warn!(source = %self.plan.game.display(), "Record editor could not stage the file: {}", err))
            .ok()?;

        if let Err(err) = vfs.create((self.plan.target_mod.as_str(), path.as_path())) {
            warn!(path = %path.display(), "Record editor staged a file the index would not take: {}", err);
        }

        preview::stamp(&path).map(|stamp| (path, stamp))
    }

    fn persist(&mut self, vfs: &Vfs) -> bool {
        let Some((path, stamp)) = self.destination(vfs) else {
            self.failed = true;

            return false;
        };

        match preview::save(&path, self.text.as_bytes(), stamp) {
            Ok(fresh) => {
                self.read_from = path;
                self.stamp = fresh;
                self.failed = false;

                true
            }
            Err(err) => {
                warn!(path = %path.display(), "Record editor could not write the file: {}", err);
                self.failed = true;

                false
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct Choice {
    raw: i32,
    label: &'static str,
}

impl std::fmt::Display for Choice {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.label)
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct Pick {
    raw: i32,
    label: String,
}

impl std::fmt::Display for Pick {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.label)
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Popup(popup::Message),
    Changed(usize, String),
    Sync,
    ConfirmExpired,
    FailureExpired,
}

#[derive(Default)]
pub(super) struct State {
    draft: Option<Draft>,
    frame: popup::State,
    confirm: Slot<()>,
    failed: Slot<()>,
}

pub(super) fn kind() -> popup::Kind {
    POPUP.kind()
}

impl State {
    pub(super) fn begin(&mut self, plan: Plan, nudge: usize, vfs: &Vfs) {
        self.frame = popup::cascaded(nudge);
        self.confirm.clear();
        self.failed.clear();
        self.draft = Draft::load(plan, vfs);
    }

    pub(super) fn drafting(&self) -> bool {
        self.draft.is_some()
    }

    pub(super) fn raised(&self) -> u64 {
        self.frame.raised()
    }

    pub(super) fn drifted(&self) -> bool {
        self.draft.as_ref().is_some_and(Draft::drifted)
    }

    pub(super) fn sync(&mut self, plan: Option<Plan>, vfs: &Vfs) {
        let Some(current) = self.draft.as_ref() else {
            return;
        };

        let Some(plan) = plan else {
            self.draft = None;

            return;
        };

        let moved = current.plan.held(vfs).is_some() != (current.read_from != current.plan.game);

        if current.plan != plan || current.drifted() || moved {
            self.draft = Draft::load(plan, vfs);
        }
    }

    pub(super) fn update(&mut self, message: Message, vfs: &Vfs) -> Task<Message> {
        match message {
            Message::Popup(msg) => {
                if self.frame.update(msg, POPUP) {
                    self.draft = None;
                    self.confirm.clear();
                    self.failed.clear();
                }

                Task::none()
            }
            Message::Changed(index, typed) => {
                let Some(draft) = self.draft.as_mut() else {
                    return Task::none();
                };

                if draft.edit(index, &typed) && !draft.persist(vfs) {
                    return self.failed.set((), Message::FailureExpired);
                }

                Task::none()
            }
            Message::Sync => {
                if !self.confirm.take(&()) {
                    return self.confirm.set((), Message::ConfirmExpired);
                }

                let restored = self.draft.as_mut().is_some_and(|draft| draft.restore(vfs));

                match restored {
                    true => Task::none(),
                    false => self.failed.set((), Message::FailureExpired),
                }
            }
            Message::ConfirmExpired => {
                self.confirm.expire();

                Task::none()
            }
            Message::FailureExpired => {
                self.failed.expire();

                Task::none()
            }
        }
    }

    pub(super) fn view(&self, window: Size) -> Option<Element<'_, Message>> {
        let draft = self.draft.as_ref()?;

        Some(self.frame.view(&draft.plan.label, POPUP, window, Message::Popup, move || self.body(draft), None))
    }

    fn body<'a>(&'a self, draft: &'a Draft) -> Element<'a, Message> {
        let listing: Element<'_, Message> = match draft.records.is_empty() {
            true => container(theme::centered_text(MISSING_LABEL).size(LABEL_SIZE).style(text::danger))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into(),
            false => {
                let mut rows = Column::new().spacing(ROW_GAP).width(Length::Fill);
                let mut index = 0;
                let depth = draft.plan.path.len();

                for record in &draft.records {
                    if let Some(suffix) = record.path.get(depth..).filter(|suffix| !suffix.is_empty()) {
                        let heading = match draft.plan.values {
                            EditorMode::Raw => suffix.join(PATH_JOINT),
                            EditorMode::Resolved => draft.plan.book.section(suffix),
                        };

                        rows = rows.push(container(text(heading).size(HEADING_SIZE)).padding(Padding::default().top(SECTION_GAP)));
                    }

                    for field in &record.fields {
                        rows = rows.push(field_row(draft, index, &field.key, &field.value));
                        index += 1;
                    }
                }

                smooth_scroll(
                    scrollable(container(rows).padding(Padding::default().right(SCROLL_GAP)))
                        .width(Length::Fill)
                        .height(Length::Fill),
                )
                .into()
            }
        };

        let framed = container(listing)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(FRAME_INSET)
            .style(theme::card_container_outlined);

        let label = match (self.failed.is_set() || draft.failed, self.confirm.is_set()) {
            (true, _) => FAILURE_LABEL,
            (false, true) => CONFIRM_LABEL,
            (false, false) => SYNC_LABEL,
        };

        let sync = button(theme::button_label(label).size(BUTTON_SIZE))
            .width(Length::Fill)
            .padding(BUTTON_PADDING)
            .style(theme::danger_button)
            .on_press(Message::Sync);

        column![framed, sync]
            .spacing(GAP)
            .padding(BODY_PADDING)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

fn field_row<'a>(draft: &'a Draft, index: usize, key: &'a str, value: &'a Value) -> Element<'a, Message> {
    let label = match draft.plan.values {
        EditorMode::Raw => key.to_owned(),
        EditorMode::Resolved => draft.plan.book.label(key),
    };

    let input = draft.inputs.get(index).map_or("", String::as_str);

    let named = match draft.plan.values {
        EditorMode::Raw => None,
        EditorMode::Resolved => draft.plan.named.iter().find(|(held, _)| *held == key).map(|(_, options)| options),
    };

    let control: Element<'_, Message> = match (draft.rule(key), value) {
        (_, Value::Number(number)) if named.is_some() => {
            let options: Vec<Pick> = named.map_or_else(Vec::new, |options| options.iter().map(|(raw, label)| Pick { raw: *raw, label: label.clone() }).collect());
            let current = number.parse::<i32>().ok();
            let chosen = options.iter().find(|option| Some(option.raw) == current).cloned();

            match chosen {
                Some(chosen) => pick_list(options, Some(chosen), move |picked: Pick| Message::Changed(index, picked.raw.to_string()))
                    .text_size(INPUT_SIZE)
                    .width(Length::Fixed(INPUT_WIDTH))
                    .style(theme::combo_box)
                    .into(),
                None => numeric(index, input),
            }
        }
        (Rule::Choice(choices), Value::Number(number)) => {
            let options: Vec<Choice> = choices.iter().map(|(raw, label)| Choice { raw: *raw, label }).collect();
            let current = number.parse::<i32>().ok();
            let chosen = options.iter().find(|option| Some(option.raw) == current).cloned();

            match chosen {
                Some(chosen) => pick_list(options, Some(chosen), move |picked: Choice| Message::Changed(index, picked.raw.to_string()))
                    .text_size(INPUT_SIZE)
                    .width(Length::Fixed(INPUT_WIDTH))
                    .style(theme::combo_box)
                    .into(),
                None => numeric(index, input),
            }
        }
        (_, Value::Other) => text(input).size(INPUT_SIZE).width(Length::Fixed(INPUT_WIDTH)).into(),
        _ => numeric(index, input),
    };

    row![text(label).size(LABEL_SIZE).width(Length::Fill), control].spacing(GAP).align_y(Vertical::Center).into()
}

fn numeric(index: usize, input: &str) -> Element<'_, Message> {
    text_input("", input)
        .on_input(move |typed| Message::Changed(index, typed))
        .padding(INPUT_PADDING)
        .size(INPUT_SIZE)
        .width(Length::Fixed(INPUT_WIDTH))
        .style(theme::rounded_input)
        .into()
}

pub(super) fn mark(plan: &Plan, vfs: &Vfs, reader: &mut Reader) -> Mark {
    let Some(held) = plan.held(vfs) else {
        return Mark::Same;
    };

    let shallow = plan.book.shallow(&plan.path);
    let found = |path: &Path, reader: &mut Reader| reader.text(path).and_then(|body| snapshot(&body, &plan.path, shallow));

    let ours = found(&held, reader);
    let theirs = found(&plan.game, reader);

    Mark::of(ours, theirs)
}

pub(super) fn reset(plan: Plan, vfs: &Vfs) -> bool {
    Draft::load(plan, vfs).is_some_and(|mut draft| draft.restore(vfs))
}

pub(super) fn tag(path: &[String]) -> String {
    let numbered: Vec<&str> = path
        .iter()
        .map(String::as_str)
        .filter(|segment| segment.parse::<i64>().is_ok())
        .collect();

    match numbered.is_empty() {
        true => path.join("/"),
        false => numbered.join("/"),
    }
}

#[cfg(test)]
mod tests {
    use super::{prettify, tag};

    #[test]
    fn a_record_is_tagged_by_the_numbers_in_its_path() {
        let path: Vec<String> = ["MapID", "39001", "-1", "BonusNum", "0"].iter().map(|segment| (*segment).to_owned()).collect();

        assert_eq!(tag(&path), "39001/-1/0");
        assert_eq!(prettify("BonusTargetID"), "Bonus Target ID");
    }
}
