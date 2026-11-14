use codex::chapter::map::{BonusType, RuleType};

use crate::common::formats::json;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Book {
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

pub(super) const BOOKS: [Book; 8] = [
    Book::Buffs,
    Book::BuffSkills,
    Book::Charas,
    Book::LotteryGroups,
    Book::StageLotteries,
    Book::Permanent,
    Book::MapSettings,
    Book::EffectAbilities,
];

impl Book {
    pub fn base(self) -> &'static str {
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

    pub fn title(self) -> &'static str {
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

    pub fn noun(self) -> &'static str {
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

    pub fn label(self, key: &str) -> String {
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

    pub fn rule(self, key: &str) -> Rule {
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
    pub fn entries(self, text: &str) -> Vec<Vec<String>> {
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

    pub fn shallow(self, path: &[String]) -> bool {
        self == Book::PointRewards && path.len() == POINT_DEPTH
    }

    pub fn map_root(self) -> Option<&'static str> {
        match self {
            Book::SpecialRules | Book::ScoreBonus | Book::PointRules | Book::ClearRewards => Some(MAP_ROOT),
            Book::EventDisplay => Some(MAP_SET_ROOT),
            _ => None,
        }
    }

    pub fn section(self, suffix: &[String]) -> String {
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

pub(super) fn catseye_name(key: &str) -> Option<String> {
    let key = key.parse::<i32>().ok()?;
    let group = key / 10;
    let rarity = key - group * 10 - 1;

    let named = usize::try_from(rarity).ok().and_then(|at| CATSEYE_RARITIES.get(at)).copied().unwrap_or(CATSEYE_ANY);

    Some(format!("{CATSEYE_GROUP} {group}, {named}"))
}

pub(super) fn rule_label(id: &str) -> Option<&'static str> {
    id.parse::<u8>().ok().and_then(|id| rule_name(RuleType::from_parts(id, Vec::new())))
}

pub type Named = Vec<(&'static str, Vec<(i32, String)>)>;

pub fn tag(path: &[String]) -> String {
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

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    Plain,
    Floor(i32),
    Choice(Choices),
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

pub fn prettify(key: &str) -> String {
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

pub(super) type Choices = &'static [(i32, &'static str)];
