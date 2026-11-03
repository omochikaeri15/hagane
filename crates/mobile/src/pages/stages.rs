use std::cell::RefCell;
use std::collections::HashSet;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::time::Instant;
use std::sync::Arc;

use slint::{ComponentHandle, Image, ModelRc, SharedString, VecModel};
use tracing::warn;

use codex::chapter::Category;
use codex::chapter::map::{BonusType, LockSkipDataEntry, RuleType, ScoreBonusMapEntry, SpecialRulesMapEntry};
use codex::chapter::stage::{AbilityType, BossType, CannonType, CataminGrade, CertificationPreset, CostType, DropOdds, EnemyAmount, EvolutionForm, RewardStructure, ScatCpuSetting, TreasureType};
use codex::combat::Identity;
use codex::common::{BreakHandling, strip_html_tags};
use foundry::Vault;
use foundry::common::context::GlobalContext;
use foundry::domains::cat::waiter::unitexplanation;
use foundry::domains::enemy::filter::EnemyFilterState;
use foundry::domains::enemy::scanner::EnemyEntry;
use foundry::domains::stage::filter::enemy::EnemyFilter;
use foundry::domains::stage::filter::lineup::LineupFilter;
use foundry::domains::stage::filter::material::MaterialFilter;
use foundry::domains::stage::filter::range::StatRange;
use foundry::domains::stage::filter::treasure::TreasureFilter;
use foundry::domains::stage::filter::{StageFilterState, StageLookupContext};
use foundry::domains::stage::fixedlineup::{self as core_fixedlineup, ResolvedSlot};
use foundry::domains::stage::scanner::StageBundle;
use foundry::domains::stage::{GlobalMapId, GlobalStageId, Map, Stage, StageDataState, cost, files as stage_files, materials, navigate, restrictions, treasure as stage_treasure};
use foundry::systems::combat::{NameBook, present_identities};
use foundry::systems::treasure;

use crate::catalog::Load;
use crate::context::Globals;
use crate::images::{self, Images};
use crate::pages::cats::text_cell;
use crate::pages::{Context, enemies};
use crate::{AppWindow, Crown, DropRow, FilterCard, FlagRow, GroundRow, LineupSlot, MaterialCell, Page, Pair, PickRow, RangeField, Shell, Stages};

const MAX_ICON_SIZE: u32 = 32;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Flag {
    Continues,
    BossGuard,
    UseSuperCpu,
    RuleTrustFund,
    RuleCooldownEquality,
    RuleRarityLimit,
    RuleCheapLabor,
    RuleCatCost,
    RuleCatProduction,
    RuleTotalDeployLimit,
    RuleMoreThanOne,
    RuleMegaCatCannon,
    RuleUniformMotion,
    InvalidCombos,
    BonusWeaken,
    BonusFreeze,
    BonusSlow,
    BonusKnockback,
    BonusStrongAttack,
    BonusMassiveDamage,
    BonusStrongDefense,
    BonusResist,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Range {
    Width,
    BaseHp,
    MaxEnemies,
    TimeLimit,
    Energy,
    ItemCost,
    ItemId,
    CataminCost,
    Xp,
    MinSpawn,
    MaxSpawn,
    Difficulty,
    MaxCrowns,
    TargetCrowns,
    MinCost,
    MaxCost,
    DeployLimit,
    AllowedRows,
    BaseId,
    AnimBaseId,
    BackgroundId,
    InitTrack,
    BossTrack,
    BgmChangePercent,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EnemyRange {
    Amount,
    StartFrame,
    RespawnMin,
    RespawnMax,
    BaseHpPerc,
    LayerMin,
    LayerMax,
    Magnification,
    AtkMagnification,
    Score,
    TimeFlag,
    KillCount,
}

const FLAGS: [Flag; 22] = [
    Flag::Continues,
    Flag::BossGuard,
    Flag::UseSuperCpu,
    Flag::RuleTrustFund,
    Flag::RuleCooldownEquality,
    Flag::RuleRarityLimit,
    Flag::RuleCheapLabor,
    Flag::RuleCatCost,
    Flag::RuleCatProduction,
    Flag::RuleTotalDeployLimit,
    Flag::RuleMoreThanOne,
    Flag::RuleMegaCatCannon,
    Flag::RuleUniformMotion,
    Flag::InvalidCombos,
    Flag::BonusWeaken,
    Flag::BonusFreeze,
    Flag::BonusSlow,
    Flag::BonusKnockback,
    Flag::BonusStrongAttack,
    Flag::BonusMassiveDamage,
    Flag::BonusStrongDefense,
    Flag::BonusResist,
];
const GENERAL_RULES: std::ops::Range<usize> = 0..3;
const SPECIAL_RULES: std::ops::Range<usize> = 3..14;
const SCORE_BONUSES: std::ops::Range<usize> = 14..22;
const STATS: [Range; 10] = [Range::BaseHp, Range::Width, Range::TimeLimit, Range::MaxEnemies, Range::Xp, Range::Difficulty, Range::MaxCrowns, Range::TargetCrowns, Range::MinSpawn, Range::MaxSpawn];
const RESTRICTIONS: [Range; 4] = [Range::DeployLimit, Range::AllowedRows, Range::MinCost, Range::MaxCost];
const IDS_AUDIO: [Range; 6] = [Range::BaseId, Range::AnimBaseId, Range::BackgroundId, Range::InitTrack, Range::BossTrack, Range::BgmChangePercent];
const ENEMY_RANGES: [EnemyRange; 12] = [
    EnemyRange::Amount,
    EnemyRange::StartFrame,
    EnemyRange::RespawnMin,
    EnemyRange::RespawnMax,
    EnemyRange::BaseHpPerc,
    EnemyRange::LayerMin,
    EnemyRange::LayerMax,
    EnemyRange::Magnification,
    EnemyRange::AtkMagnification,
    EnemyRange::Score,
    EnemyRange::TimeFlag,
    EnemyRange::KillCount,
];
const ABILITIES: [(AbilityType, &str); 10] = [
    (AbilityType::CatCannonAttack, "Cat Cannon Attack"),
    (AbilityType::CatCannonRange, "Cat Cannon Range"),
    (AbilityType::CatCannonCharge, "Cat Cannon Charge"),
    (AbilityType::WorkerCatRate, "Worker Cat Efficiency"),
    (AbilityType::WorkerCatWallet, "Worker Cat Wallet"),
    (AbilityType::BaseDefense, "Cat Base Health"),
    (AbilityType::Research, "Research"),
    (AbilityType::BountyUp, "Accounting"),
    (AbilityType::Study, "Study"),
    (AbilityType::CatEnergy, "Cat Energy"),
];
const TREASURES: [(TreasureType, &str); 9] = [
    (TreasureType::EoC1, "EoC Ch. 1"),
    (TreasureType::EoC2, "EoC Ch. 2"),
    (TreasureType::EoC3, "EoC Ch. 3"),
    (TreasureType::ItF1, "ItF Ch. 1"),
    (TreasureType::ItF2, "ItF Ch. 2"),
    (TreasureType::ItF3, "ItF Ch. 3"),
    (TreasureType::CotC1, "CotC Ch. 1"),
    (TreasureType::CotC2, "CotC Ch. 2"),
    (TreasureType::CotC3, "CotC Ch. 3"),
];

pub fn display_name(category: &Category) -> &str {
    match category {
        Category::StoriesOfLegend => "Stories of Legend",
        Category::EventStages => "Event Stages",
        Category::CollabStages => "Collab Stages",
        Category::EmpireOfCats => "Empire of Cats",
        Category::IntoTheFuture => "Into the Future",
        Category::CatsOfTheCosmos => "Cats of the Cosmos",
        Category::ExtraStages => "Extra Stages",
        Category::DojoHallOfInitiates => "Dojo Hall of Initiates",
        Category::TowersAndCitadels => "Towers & Citadels",
        Category::DojoRankingEvents => "Dojo Ranking Events",
        Category::ChallengeBattle => "Challenge Battle",
        Category::UncannyLegends => "Uncanny Legends",
        Category::CataminStages => "Catamin Stages",
        Category::LegendQuest => "Legend Quest",
        Category::ZombieOutbreaks => "Zombie Outbreaks",
        Category::GauntletStages => "Gauntlet Stages",
        Category::EnigmaStages => "Enigma Stages",
        Category::CollabGauntletStages => "Collab Gauntlet Stages",
        Category::AkuRealms => "Aku Realms",
        Category::BehemothCulling => "Behemoth Culling",
        Category::Labyrinth => "Labyrinth",
        Category::ZeroLegends => "Zero Legends",
        Category::OtherworldColosseum => "Otherworld Colosseum",
        Category::CatclawChampionships => "Catclaw Championships",
        Category::FeverRush => "Fever Rush",
        Category::Unknown(prefix) => {
            if prefix == "PT" {
                "Legacy Princess Punt"
            } else {
                prefix.as_str()
            }
        }
    }
}

fn sort_order(category: &Category) -> u32 {
    match category {
        Category::EmpireOfCats => 0,
        Category::IntoTheFuture => 1,
        Category::CatsOfTheCosmos => 2,
        Category::ZombieOutbreaks => 3,
        Category::AkuRealms => 4,
        Category::StoriesOfLegend => 10,
        Category::UncannyLegends => 11,
        Category::ZeroLegends => 12,
        Category::EventStages => 20,
        Category::ExtraStages => 21,
        Category::GauntletStages => 22,
        Category::CataminStages => 24,
        Category::ChallengeBattle => 25,
        Category::FeverRush => 26,
        Category::TowersAndCitadels => 31,
        Category::OtherworldColosseum => 32,
        Category::Labyrinth => 33,
        Category::LegendQuest => 34,
        Category::BehemothCulling => 41,
        Category::EnigmaStages => 42,
        Category::DojoHallOfInitiates => 51,
        Category::DojoRankingEvents => 52,
        Category::CatclawChampionships => 53,
        Category::CollabStages => 61,
        Category::CollabGauntletStages => 62,
        Category::Unknown(prefix) => {
            if prefix == "PT" {
                63
            } else {
                99
            }
        }
    }
}

fn flag_mut(state: &mut StageFilterState, flag: Flag) -> &mut Option<bool> {
    match flag {
        Flag::Continues => &mut state.continues,
        Flag::BossGuard => &mut state.boss_guard,
        Flag::UseSuperCpu => &mut state.use_super_cpu,
        Flag::RuleTrustFund => &mut state.rule_trust_fund,
        Flag::RuleCooldownEquality => &mut state.rule_cooldown_equality,
        Flag::RuleRarityLimit => &mut state.rule_rarity_limit,
        Flag::RuleCheapLabor => &mut state.rule_cheap_labor,
        Flag::RuleCatCost => &mut state.rule_cat_cost,
        Flag::RuleCatProduction => &mut state.rule_cat_production,
        Flag::RuleTotalDeployLimit => &mut state.rule_total_deploy_limit,
        Flag::RuleMoreThanOne => &mut state.rule_more_than_one,
        Flag::RuleMegaCatCannon => &mut state.rule_mega_cat_cannon,
        Flag::RuleUniformMotion => &mut state.rule_uniform_motion,
        Flag::InvalidCombos => &mut state.invalid_combos,
        Flag::BonusWeaken => &mut state.bonus_weaken,
        Flag::BonusFreeze => &mut state.bonus_freeze,
        Flag::BonusSlow => &mut state.bonus_slow,
        Flag::BonusKnockback => &mut state.bonus_knockback,
        Flag::BonusStrongAttack => &mut state.bonus_strong_attack,
        Flag::BonusMassiveDamage => &mut state.bonus_massive_damage,
        Flag::BonusStrongDefense => &mut state.bonus_strong_defense,
        Flag::BonusResist => &mut state.bonus_resist,
    }
}

fn flag_label(flag: Flag, names: &NameBook) -> &str {
    match flag {
        Flag::Continues => "Continues",
        Flag::BossGuard => "Boss Guard",
        Flag::UseSuperCpu => "Use Super CPU",
        Flag::RuleTrustFund => "Trust Fund",
        Flag::RuleCooldownEquality => "Cooldown Equality",
        Flag::RuleRarityLimit => "Rarity Limit",
        Flag::RuleCheapLabor => "Cheap Labor",
        Flag::RuleCatCost => "Cat Cost",
        Flag::RuleCatProduction => "Cat Production",
        Flag::RuleTotalDeployLimit => "Total Deploy Limit",
        Flag::RuleMoreThanOne => "More Than One",
        Flag::RuleMegaCatCannon => "Mega Cat Cannon",
        Flag::RuleUniformMotion => "Uniform Motion",
        Flag::InvalidCombos => "Invalid Combos",
        Flag::BonusWeaken => names.bonus(0).unwrap_or("Weaken"),
        Flag::BonusKnockback => names.bonus(1).unwrap_or("Knockback"),
        Flag::BonusStrongAttack => names.bonus(2).unwrap_or("Strong Attack"),
        Flag::BonusFreeze => names.bonus(3).unwrap_or("Freeze"),
        Flag::BonusMassiveDamage => names.bonus(4).unwrap_or("Massive Damage"),
        Flag::BonusSlow => names.bonus(5).unwrap_or("Slow"),
        Flag::BonusStrongDefense => names.bonus(6).unwrap_or("Strong Defense"),
        Flag::BonusResist => names.bonus(7).unwrap_or("Resist"),
    }
}

fn range_mut(state: &mut StageFilterState, range: Range) -> &mut StatRange {
    match range {
        Range::Width => &mut state.width,
        Range::BaseHp => &mut state.base_hp,
        Range::MaxEnemies => &mut state.max_enemies,
        Range::TimeLimit => &mut state.time_limit,
        Range::Energy => &mut state.cost.energy,
        Range::ItemCost => &mut state.cost.item_cost,
        Range::ItemId => &mut state.cost.item_id,
        Range::CataminCost => &mut state.cost.catamin_cost,
        Range::Xp => &mut state.xp,
        Range::MinSpawn => &mut state.min_spawn,
        Range::MaxSpawn => &mut state.max_spawn,
        Range::Difficulty => &mut state.difficulty,
        Range::MaxCrowns => &mut state.max_crowns,
        Range::TargetCrowns => &mut state.target_crowns,
        Range::MinCost => &mut state.min_cost,
        Range::MaxCost => &mut state.max_cost,
        Range::DeployLimit => &mut state.deploy_limit,
        Range::AllowedRows => &mut state.allowed_rows,
        Range::BaseId => &mut state.base_id,
        Range::AnimBaseId => &mut state.anim_base_id,
        Range::BackgroundId => &mut state.background_id,
        Range::InitTrack => &mut state.init_track,
        Range::BossTrack => &mut state.boss_track,
        Range::BgmChangePercent => &mut state.bgm_change_percent,
    }
}

fn range_label(range: Range) -> &'static str {
    match range {
        Range::Width => "Width",
        Range::BaseHp => "Base HP",
        Range::MaxEnemies => "Max Enemies",
        Range::TimeLimit => "Time Limit (f)",
        Range::Energy | Range::ItemCost | Range::CataminCost => "Cost",
        Range::ItemId => "ID",
        Range::Xp => "XP Reward",
        Range::MinSpawn => "Min Spawn",
        Range::MaxSpawn => "Max Spawn",
        Range::Difficulty => "Difficulty",
        Range::MaxCrowns => "Max Crowns",
        Range::TargetCrowns => "Target Crowns",
        Range::MinCost => "Min Cost",
        Range::MaxCost => "Max Cost",
        Range::DeployLimit => "Deploy Limit",
        Range::AllowedRows => "Allowed Rows",
        Range::BaseId => "Base ID",
        Range::AnimBaseId => "Anim Base ID",
        Range::BackgroundId => "Background ID",
        Range::InitTrack => "Init Track",
        Range::BossTrack => "Boss Track",
        Range::BgmChangePercent => "BGM Change (%)",
    }
}

fn enemy_range_mut(filter: &mut EnemyFilter, range: EnemyRange) -> &mut StatRange {
    match range {
        EnemyRange::Amount => &mut filter.amount,
        EnemyRange::StartFrame => &mut filter.start_frame,
        EnemyRange::RespawnMin => &mut filter.respawn_min,
        EnemyRange::RespawnMax => &mut filter.respawn_max,
        EnemyRange::BaseHpPerc => &mut filter.base_hp_perc,
        EnemyRange::LayerMin => &mut filter.layer_min,
        EnemyRange::LayerMax => &mut filter.layer_max,
        EnemyRange::Magnification => &mut filter.magnification,
        EnemyRange::AtkMagnification => &mut filter.atk_magnification,
        EnemyRange::Score => &mut filter.score,
        EnemyRange::TimeFlag => &mut filter.time_flag,
        EnemyRange::KillCount => &mut filter.kill_count,
    }
}

fn enemy_range_label(range: EnemyRange) -> &'static str {
    match range {
        EnemyRange::Amount => "Amount",
        EnemyRange::StartFrame => "Start Frame",
        EnemyRange::RespawnMin => "Respawn Min",
        EnemyRange::RespawnMax => "Respawn Max",
        EnemyRange::BaseHpPerc => "Base HP (%)",
        EnemyRange::LayerMin => "Layer Min",
        EnemyRange::LayerMax => "Layer Max",
        EnemyRange::Magnification => "Magnification",
        EnemyRange::AtkMagnification => "Atk Mag",
        EnemyRange::Score => "Score",
        EnemyRange::TimeFlag => "Time Flag",
        EnemyRange::KillCount => "Kill Count",
    }
}

fn cycle(value: Option<bool>) -> Option<bool> {
    match value {
        None => Some(true),
        Some(true) => Some(false),
        Some(false) => None,
    }
}

fn tristate(value: Option<bool>) -> i32 {
    match value {
        None => 0,
        Some(true) => 1,
        Some(false) => 2,
    }
}

fn cost_ranges(cost_type: Option<CostType>) -> &'static [Range] {
    match cost_type {
        None => &[],
        Some(CostType::Item) => &[Range::ItemCost, Range::ItemId],
        Some(CostType::Catamin) => &[Range::CataminCost],
        Some(_) => &[Range::Energy],
    }
}

fn group_ranges(group: i32, cost_type: Option<CostType>) -> &'static [Range] {
    match group {
        0 => cost_ranges(cost_type),
        1 => &STATS,
        2 => &RESTRICTIONS,
        _ => &IDS_AUDIO,
    }
}

fn format_diff(difficulty: u16) -> String {
    if difficulty == 0 { "-".to_owned() } else { format!("\u{2605}{difficulty}") }
}

fn format_base(anim_id: u32, standard_id: i32) -> (String, String) {
    if anim_id != 0 {
        return ("Base Enemy".to_owned(), format!("{:03}-E", anim_id.saturating_sub(2)));
    }

    ("Base Img".to_owned(), standard_id.to_string())
}

fn format_bool(status: bool, yes: &str, no: &str) -> String {
    if status { yes.to_owned() } else { no.to_owned() }
}

fn format_respawn(minimum: u32, maximum: u32) -> String {
    if minimum == maximum { format!("{minimum}f") } else { format!("{minimum}f ~ {maximum}f") }
}

fn format_boss_bgm(boss_track: i16, init_track: u32, change_percent: u32) -> String {
    if boss_track < 0 || boss_track as u32 == init_track || change_percent == 100 {
        return "-".to_owned();
    }

    boss_track.to_string()
}

fn format_time(time_limit: u32) -> String {
    if time_limit == 0 { "-".to_owned() } else { format!("{time_limit}m") }
}

fn skip_status(category: &Category, map_id: u32, lock_registry: &std::collections::HashMap<u32, LockSkipDataEntry>, cpu_setting: &ScatCpuSetting) -> String {
    if let Some(global_id) = category.global_map_id(map_id)
        && let Some(entry) = lock_registry.get(&global_id)
        && entry.excluded_map_id == global_id
    {
        return "N/A".to_owned();
    }

    if cpu_setting.super_cpu_consume_amount > 0 {
        return format!("{} CPUs", cpu_setting.super_cpu_consume_amount);
    }

    "-".to_owned()
}

fn format_percent(value: f64) -> String {
    let rounded = format!("{value:.2}");

    format!("{}%", rounded.trim_end_matches('0').trim_end_matches('.'))
}

fn format_odds(odds: DropOdds) -> String {
    let first = format_percent(odds.first);

    if odds.repeat <= 0.0 {
        return first;
    }

    let repeat = format_percent(odds.repeat);

    if first == repeat { first } else { format!("{first} -> {repeat}") }
}

fn format_rule(drop_rule: i32) -> &'static str {
    match drop_rule {
        1 | 2 => "First Drop Once",
        -2..=0 => "Unlimited",
        -3 => "Guaranteed (Once)",
        -4 => "Guaranteed (Unlimited)",
        _ => "Unknown Rule",
    }
}

fn format_amount(amount: &EnemyAmount) -> String {
    match amount {
        EnemyAmount::Infinite => "\u{221e}".to_owned(),
        EnemyAmount::Limit(limit) => limit.to_string(),
    }
}

fn format_spawn(time_flag: u32, start_frame: u32, base_hp_perc: u32) -> String {
    if base_hp_perc != 100 && time_flag == 0 { "-".to_owned() } else { format!("{start_frame}f") }
}

fn format_enemy_respawn(amount: &EnemyAmount, min: u32, max: u32) -> String {
    if amount == &EnemyAmount::Limit(1) {
        return "-".to_owned();
    }

    if min == max { format!("{min}f") } else { format!("{min}f ~ {max}f") }
}

fn format_layer(min: i32, max: i32) -> String {
    if min == max { min.to_string() } else { format!("{min} ~ {max}") }
}

fn format_boss(boss: &BossType) -> String {
    match boss {
        BossType::None => "-".to_owned(),
        BossType::Boss => "Yes".to_owned(),
        BossType::ScreenShake => "Yes (Shake)".to_owned(),
        BossType::Unknown(_) => "Unknown".to_owned(),
    }
}

fn format_count(count: u32) -> String {
    if count == 0 { "-".to_owned() } else { count.to_string() }
}

fn format_base_hp(base_hp_perc: u32, dojo: bool) -> String {
    if dojo {
        return base_hp_perc.to_string();
    }

    if base_hp_perc == 100 { "-".to_owned() } else { format!("{base_hp_perc}%") }
}

fn rule_parameters(rule: &RuleType) -> &[u32] {
    match rule {
        RuleType::TrustFund(params)
        | RuleType::CooldownEquality(params)
        | RuleType::LineupLimit(params)
        | RuleType::RarityLimit(params)
        | RuleType::CheapLabor(params)
        | RuleType::CatCost(params)
        | RuleType::CatProduction(params)
        | RuleType::TotalDeployLimit(params)
        | RuleType::MoreThanOne(params)
        | RuleType::MegaCatCannon(params)
        | RuleType::UniformMotion(params)
        | RuleType::CompoundingCost(params)
        | RuleType::Unknown(_, params) => params,
    }
}

fn rule_name(rule: &RuleType) -> String {
    match rule {
        RuleType::TrustFund(params) => format!("Trust Fund {params:?}"),
        RuleType::CooldownEquality(params) => format!("Cooldown Equality {params:?}"),
        RuleType::LineupLimit(params) => format!("Lineup Limit {params:?}"),
        RuleType::RarityLimit(params) => format!("Rarity Limit {params:?}"),
        RuleType::CheapLabor(params) => format!("Cheap Labor {params:?}"),
        RuleType::CatCost(params) => format!("Cat Cost {params:?}"),
        RuleType::CatProduction(params) => format!("Cat Production {params:?}"),
        RuleType::TotalDeployLimit(params) => format!("Total Deploy Limit {params:?}"),
        RuleType::MoreThanOne(params) => format!("More Than One {params:?}"),
        RuleType::MegaCatCannon(params) => format!("Mega Cat Cannon {params:?}"),
        RuleType::UniformMotion(params) => format!("Uniform Motion {params:?}"),
        RuleType::CompoundingCost(params) => format!("Compounding Cost {params:?}"),
        RuleType::Unknown(id, params) => format!("Unknown Rule {id} {params:?}"),
    }
}

fn special_rule(rule: &SpecialRulesMapEntry, ctx: &GlobalContext<'_>) -> String {
    let key = if !rule.explanation_label.is_empty() {
        rule.explanation_label.clone()
    } else if let Some(prefix) = rule.name_label.trim().strip_suffix("Name") {
        format!("{prefix}Explanation")
    } else {
        String::new()
    };
    let raw = if key.is_empty() { "" } else { ctx.localizable.lookup(&key).unwrap_or("") };
    let mut description = strip_html_tags(raw, BreakHandling::Space);

    if description.is_empty() {
        if !key.is_empty() {
            warn!(key = %key, name_label = %rule.name_label, "missing localization for special rule explanation, falling back to raw enum parsing");
        }

        return rule.rules.iter().map(rule_name).collect::<Vec<_>>().join("\n");
    }

    for target in &rule.rules {
        for param in rule_parameters(target) {
            description = description.replacen("%d", &param.to_string(), 1);
        }
    }

    description
}

fn bonus_parameters(bonus: &BonusType) -> &[u32] {
    match bonus {
        BonusType::Weaken(params)
        | BonusType::Freeze(params)
        | BonusType::Slow(params)
        | BonusType::Knockback(params)
        | BonusType::StrongAttack(params)
        | BonusType::MassiveDamage(params)
        | BonusType::StrongDefense(params)
        | BonusType::Resist(params)
        | BonusType::Unknown(_, params) => params,
    }
}

fn score_bonus(bonus: &ScoreBonusMapEntry, ctx: &GlobalContext<'_>) -> String {
    let key = if bonus.explanation_label.is_empty() { &bonus.name_label } else { &bonus.explanation_label };
    let raw = ctx.localizable.lookup(key).unwrap_or("");
    let mut description = strip_html_tags(raw, BreakHandling::Space);

    if description.is_empty() {
        return format!("\u{3010}{key}\u{3011} Localization data missing.");
    }

    for held in &bonus.bonuses {
        for param in bonus_parameters(held) {
            description = description.replacen("%d", &param.to_string(), 1);
        }
    }

    description
}

fn assemble(bundle: &StageBundle, enemies: Option<&[EnemyEntry]>) -> StageDataState {
    let mut data = StageDataState {
        registry: bundle.registry.clone(),
        enemy_name_registry: bundle.dictionaries.enemy_name_registry.clone(),
        drop_chara_registry: bundle.dictionaries.drop_chara_registry.clone(),
        unit_buy_registry: bundle.dictionaries.unit_buy_registry.clone(),
        cat_name_registry: bundle.dictionaries.cat_name_registry.clone(),
        lock_skip_registry: bundle.dictionaries.lock_skip_registry.clone(),
        scat_cpu_setting: bundle.dictionaries.scat_cpu_setting.clone(),
        ..StageDataState::default()
    };

    if let Some(enemies) = enemies {
        data.sync_enemies(enemies);
    }

    data
}

fn digest(filter: &StageFilterState) -> u64 {
    let mut hasher = DefaultHasher::new();

    filter.hash(&mut hasher);
    hasher.finish()
}

struct Matching {
    active: bool,
    hash: u64,
    stages: HashSet<GlobalStageId>,
    maps: HashSet<GlobalMapId>,
    categories: HashSet<Category>,
}

pub struct State {
    bundle: Option<Rc<StageBundle>>,
    enemies: Option<Rc<Vec<EnemyEntry>>>,
    data: Option<Rc<StageDataState>>,
    category: Option<Category>,
    map: Option<GlobalMapId>,
    stage: Option<GlobalStageId>,
    arriving: Option<GlobalStageId>,
    crown: u8,
    filter: StageFilterState,
    matching: Option<Matching>,
    attributes_for: Option<usize>,
    available: Option<Rc<HashSet<Identity>>>,
    shown_categories: Vec<Category>,
    shown_maps: Vec<GlobalMapId>,
    shown_stages: Vec<GlobalStageId>,
}

pub type Shared = Rc<RefCell<State>>;

impl State {
    fn new() -> Self {
        Self {
            bundle: None,
            enemies: None,
            data: None,
            category: None,
            map: None,
            stage: None,
            arriving: None,
            crown: 0,
            filter: StageFilterState::default(),
            matching: None,
            attributes_for: None,
            available: None,
            shown_categories: Vec::new(),
            shown_maps: Vec::new(),
            shown_stages: Vec::new(),
        }
    }

    fn rebuild(&mut self) {
        let Some(bundle) = &self.bundle else {
            self.data = None;
            return;
        };

        self.data = Some(Rc::new(assemble(bundle, self.enemies.as_deref().map(Vec::as_slice))));
        self.matching = None;
        self.available = None;
    }

    fn refresh_matching(&mut self, vault: &Vault, globals: &Globals) {
        let hash = digest(&self.filter);

        if self.matching.as_ref().is_some_and(|held| held.hash == hash) {
            return;
        }

        let Some(data) = &self.data else {
            return;
        };
        let compiled = self.filter.compile();
        let mut stages = HashSet::new();
        let mut maps = HashSet::new();
        let mut categories = HashSet::new();

        if compiled.is_active() {
            let lookup = StageLookupContext::from_data(data, vault, &globals.treasure);

            for (key, stage) in &data.registry.stages {
                let map_key = GlobalMapId { category: key.category.clone(), map: key.map };
                let Some(map) = data.registry.maps.get(&map_key) else {
                    continue;
                };

                if compiled.matches(display_name(&key.category), map, stage, &lookup) {
                    stages.insert(key.clone());
                    maps.insert(map_key);
                    categories.insert(key.category.clone());
                }
            }
        }

        self.matching = Some(Matching { active: compiled.is_active(), hash, stages, maps, categories });
    }

    pub fn picked(&self) -> Option<(GlobalStageId, u8)> {
        let id = self.stage.clone()?;
        let registry = &self.data.as_ref()?.registry;

        registry.stages.contains_key(&id).then_some((id, self.crown))
    }

    fn stage(&self) -> Option<(&Stage, &Map)> {
        let data = self.data.as_ref()?;
        let stage = data.registry.stages.get(self.stage.as_ref()?)?;
        let map = data.registry.maps.get(&GlobalMapId { category: stage.category.clone(), map: stage.map_id })?;

        Some((stage, map))
    }
}

pub fn wire(window: &AppWindow, context: &Rc<Context>) -> Shared {
    let state: Shared = Rc::new(RefCell::new(State::new()));
    let stages = window.global::<Stages>();

    {
        let (held, ctx) = (Rc::clone(&state), Rc::clone(context));

        context.catalogs.listen(Rc::new(move |ui: &AppWindow| sync(ui, &held, &ctx)));
    }

    {
        let (held, ctx) = (Rc::clone(&state), Rc::clone(context));

        context.images.watch(Rc::new(move |ui: &AppWindow, key: &str| {
            if (key.starts_with("trim:") || key.starts_with("box")) && held.borrow().stage.is_some() && matches!(ui.global::<Shell>().get_page(), Page::Stages | Page::Sandbox) {
                refresh(ui, &held, &ctx);
            }
        }));
    }

    let on = |state: &Shared, context: &Rc<Context>, window: &AppWindow| (Rc::clone(state), Rc::clone(context), window.as_weak());

    let (held, ctx, weak) = on(&state, context, window);
    stages.on_pick_category(move |index| {
        {
            let mut state = held.borrow_mut();
            let Some(category) = state.shown_categories.get(index.max(0) as usize).cloned() else {
                return;
            };

            state.category = Some(category);
            state.map = None;
            state.stage = None;
        }

        if let Some(ui) = weak.upgrade() {
            refresh_browser(&ui, &held, &ctx);
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    stages.on_pick_map(move |index| {
        {
            let mut state = held.borrow_mut();
            let Some(map) = state.shown_maps.get(index.max(0) as usize).cloned() else {
                return;
            };

            state.map = Some(map);
            state.stage = None;
        }

        if let Some(ui) = weak.upgrade() {
            refresh_browser(&ui, &held, &ctx);
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    stages.on_pick_stage(move |index| {
        let picked = held.borrow().shown_stages.get(index.max(0) as usize).cloned();

        if let (Some(ui), Some(id)) = (weak.upgrade(), picked) {
            select(&ui, &held, &ctx, id);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    stages.on_pick_crown(move |crown| {
        held.borrow_mut().crown = crown.clamp(0, 3) as u8;

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    stages.on_open_filter(move || {
        if let Some(ui) = weak.upgrade() {
            refresh_filter(&ui, &held, &ctx);
            ui.global::<Stages>().set_filter_open(true);
        }
    });

    let (_, ctx, weak) = on(&state, context, window);
    stages.on_jump_enemy(move |id, magnification| {
        if let Some(ui) = weak.upgrade()
            && let Some(pages) = ctx.pages()
        {
            enemies::jump(&ui, &pages.enemies, &ctx, id.max(0) as u32, magnification.to_string());
            ui.global::<Shell>().invoke_navigate(Page::Enemies);
        }
    });

    let (_, ctx, weak) = on(&state, context, window);
    stages.on_jump_cat(move |id, form, level| {
        if let Some(ui) = weak.upgrade()
            && let Some(pages) = ctx.pages()
        {
            crate::pages::cats::jump(&ui, &pages.cats, &ctx, id.max(0) as u32, form.max(0) as usize, Some(level.to_string()));
            ui.global::<Shell>().invoke_navigate(Page::Cats);
        }
    });

    wire_filter(window, &state, context);

    state
}

fn edited(ui: &AppWindow, held: &Shared, ctx: &Rc<Context>, change: impl FnOnce(&mut StageFilterState)) {
    change(&mut held.borrow_mut().filter);
    ui.global::<Stages>().set_filter_active(held.borrow().filter.is_active());
    refresh_browser(ui, held, ctx);
    refresh_filter(ui, held, ctx);
}

fn wire_filter(window: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let stages = window.global::<Stages>();
    let on = |state: &Shared, context: &Rc<Context>, window: &AppWindow| (Rc::clone(state), Rc::clone(context), window.as_weak());

    let (held, ctx, weak) = on(state, context, window);
    stages.on_filter_name(move |which, value| {
        if let Some(ui) = weak.upgrade() {
            edited(&ui, &held, &ctx, |filter| match which {
                0 => filter.category_name = value.to_string(),
                1 => filter.map_name = value.to_string(),
                _ => filter.stage_name = value.to_string(),
            });
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    stages.on_filter_flag(move |id| {
        if let (Some(ui), Some(flag)) = (weak.upgrade(), FLAGS.get(id.max(0) as usize).copied()) {
            edited(&ui, &held, &ctx, |filter| {
                let slot = flag_mut(filter, flag);

                *slot = cycle(*slot);
            });
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    stages.on_filter_cost_type(move |index| {
        if let Some(ui) = weak.upgrade() {
            edited(&ui, &held, &ctx, |filter| {
                filter.cost.cost_type = match index {
                    1 => Some(CostType::Energy),
                    2 => Some(CostType::Item),
                    3 => Some(CostType::Catamin),
                    _ => None,
                }
            });
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    stages.on_filter_cost_grade(move |index| {
        if let Some(ui) = weak.upgrade() {
            edited(&ui, &held, &ctx, |filter| {
                filter.cost.catamin_grade = match index {
                    1 => Some(CataminGrade::A),
                    2 => Some(CataminGrade::B),
                    3 => Some(CataminGrade::C),
                    _ => None,
                }
            });
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    stages.on_filter_range(move |group, index, value, min| {
        if let Some(ui) = weak.upgrade() {
            let cost_type = held.borrow().filter.cost.cost_type;
            let Some(range) = group_ranges(group, cost_type).get(index.max(0) as usize).copied() else {
                return;
            };

            edited(&ui, &held, &ctx, |filter| {
                let slot = range_mut(filter, range);

                if min {
                    slot.min = value.to_string();
                } else {
                    slot.max = value.to_string();
                }
            });
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    stages.on_filter_card_exclude(move |kind, index, exclude| {
        if let Some(ui) = weak.upgrade() {
            let index = index.max(0) as usize;
            let exclude = exclude == 1;

            edited(&ui, &held, &ctx, |filter| match kind {
                0 => {
                    if let Some(card) = filter.enemies.get_mut(index) {
                        card.is_exclude = exclude;
                    }
                }
                1 => {
                    if let Some(card) = filter.lineup_cats.get_mut(index) {
                        card.is_exclude = exclude;
                    }
                }
                2 => {
                    if let Some(card) = filter.treasures.get_mut(index) {
                        card.is_exclude = exclude;
                    }
                }
                _ => {
                    if let Some(card) = filter.materials.get_mut(index) {
                        card.is_exclude = exclude;
                    }
                }
            });
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    stages.on_filter_card_remove(move |kind, index| {
        if let Some(ui) = weak.upgrade() {
            let index = index.max(0) as usize;

            edited(&ui, &held, &ctx, |filter| match kind {
                0 => {
                    if index < filter.enemies.len() {
                        filter.enemies.remove(index);
                    }
                }
                1 => {
                    if index < filter.lineup_cats.len() {
                        filter.lineup_cats.remove(index);
                    }
                }
                2 => {
                    if index < filter.treasures.len() {
                        filter.treasures.remove(index);
                    }
                }
                _ => {
                    if index < filter.materials.len() {
                        filter.materials.remove(index);
                    }
                }
            });
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    stages.on_filter_card_name(move |kind, index, value| {
        if let Some(ui) = weak.upgrade() {
            let index = index.max(0) as usize;
            let value = value.to_string();

            edited(&ui, &held, &ctx, |filter| match kind {
                0 => {
                    if let Some(card) = filter.enemies.get_mut(index) {
                        card.name_or_id = value;
                    }
                }
                1 => {
                    if let Some(card) = filter.lineup_cats.get_mut(index) {
                        card.name_or_id = value;
                    }
                }
                2 => {
                    if let Some(card) = filter.treasures.get_mut(index) {
                        card.name_or_id = value;
                    }
                }
                _ => {
                    if let Some(card) = filter.materials.get_mut(index) {
                        card.name_or_id = value;
                    }
                }
            });
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    stages.on_filter_card_range(move |kind, index, field, value, min| {
        if let Some(ui) = weak.upgrade() {
            let index = index.max(0) as usize;
            let field = field.max(0) as usize;
            let value = value.to_string();
            let apply = |slot: &mut StatRange| {
                if min {
                    slot.min = value;
                } else {
                    slot.max = value;
                }
            };

            edited(&ui, &held, &ctx, |filter| match kind {
                0 => {
                    if let (Some(card), Some(range)) = (filter.enemies.get_mut(index), ENEMY_RANGES.get(field).copied()) {
                        apply(enemy_range_mut(card, range));
                    }
                }
                1 => {
                    if let Some(card) = filter.lineup_cats.get_mut(index) {
                        apply(&mut card.level);
                    }
                }
                2 => {
                    if let Some(card) = filter.treasures.get_mut(index) {
                        apply(if field == 0 { &mut card.amount } else { &mut card.chance });
                    }
                }
                _ => {
                    if let Some(card) = filter.materials.get_mut(index) {
                        apply(&mut card.amount);
                    }
                }
            });
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    stages.on_filter_card_boss(move |index, boss| {
        if let Some(ui) = weak.upgrade() {
            edited(&ui, &held, &ctx, |filter| {
                if let Some(card) = filter.enemies.get_mut(index.max(0) as usize) {
                    card.boss_type = match boss {
                        1 => Some(0),
                        2 => Some(1),
                        3 => Some(2),
                        _ => None,
                    };
                }
            });
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    stages.on_filter_card_base(move |index| {
        if let Some(ui) = weak.upgrade() {
            edited(&ui, &held, &ctx, |filter| {
                if let Some(card) = filter.enemies.get_mut(index.max(0) as usize) {
                    card.is_base = cycle(card.is_base);
                }
            });
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    stages.on_filter_card_attributes(move |index| {
        if let Some(ui) = weak.upgrade() {
            held.borrow_mut().attributes_for = Some(index.max(0) as usize);
            refresh_attributes(&ui, &held, &ctx);
            ui.global::<Stages>().set_attributes_open(true);
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    stages.on_filter_card_add(move |kind| {
        if let Some(ui) = weak.upgrade() {
            edited(&ui, &held, &ctx, |filter| match kind {
                0 => filter.enemies.push(EnemyFilter::default()),
                1 => filter.lineup_cats.push(LineupFilter::default()),
                2 => filter.treasures.push(TreasureFilter::default()),
                _ => filter.materials.push(MaterialFilter::default()),
            });
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    stages.on_filter_clear(move || {
        if let Some(ui) = weak.upgrade() {
            edited(&ui, &held, &ctx, |filter| *filter = StageFilterState { is_open: filter.is_open, ..StageFilterState::default() });
        }
    });

    let attribute = |held: &Shared, change: &dyn Fn(&mut EnemyFilterState)| {
        let mut state = held.borrow_mut();
        let Some(index) = state.attributes_for else {
            return;
        };

        if let Some(card) = state.filter.enemies.get_mut(index) {
            change(&mut card.attributes);
        }
    };

    let (held, ctx, weak) = on(state, context, window);
    stages.on_attr_mode_set(move |index| {
        attribute(&held, &|filter| filter.match_mode = if index == 0 { foundry::domains::enemy::filter::MatchMode::And } else { foundry::domains::enemy::filter::MatchMode::Or });

        if let Some(ui) = weak.upgrade() {
            edited(&ui, &held, &ctx, |_| {});
            refresh_attributes(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    stages.on_attr_mag_edited(move |value| {
        attribute(&held, &|filter| filter.mag_input = value.to_string());

        if let Some(ui) = weak.upgrade() {
            edited(&ui, &held, &ctx, |_| {});
            refresh_attributes(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    stages.on_attr_stat(move |index, value, min| {
        if let Some(stat) = enemies::STAT_KEYS.get(index.max(0) as usize) {
            attribute(&held, &|filter| {
                let slot = filter.stat_ranges.entry(stat).or_default();

                if min {
                    slot.min = value.to_string();
                } else {
                    slot.max = value.to_string();
                }
            });
        }

        if let Some(ui) = weak.upgrade() {
            edited(&ui, &held, &ctx, |_| {});
            refresh_attributes(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    stages.on_attr_icon(move |code| {
        if let Some(identity) = enemies::identity_from_code(code) {
            attribute(&held, &|filter| {
                if !filter.active_identities.remove(&identity) {
                    filter.active_identities.insert(identity);
                }
            });
        }

        if let Some(ui) = weak.upgrade() {
            edited(&ui, &held, &ctx, |_| {});
            refresh_attributes(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    stages.on_attr_range(move |code, index, value, min| {
        if let Some(identity) = enemies::identity_from_code(code)
            && let Some((attr, _)) = enemies::ability_schema(identity).get(index.max(0) as usize).copied()
        {
            attribute(&held, &|filter| {
                let slot = filter.adv_ranges.entry(identity).or_default().entry(attr).or_default();

                if min {
                    slot.min = value.to_string();
                } else {
                    slot.max = value.to_string();
                }
            });
        }

        if let Some(ui) = weak.upgrade() {
            edited(&ui, &held, &ctx, |_| {});
            refresh_attributes(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    stages.on_attr_clear(move || {
        attribute(&held, &|filter| *filter = EnemyFilterState { is_open: filter.is_open, treasure: filter.treasure, ..EnemyFilterState::default() });

        if let Some(ui) = weak.upgrade() {
            edited(&ui, &held, &ctx, |_| {});
            refresh_attributes(&ui, &held, &ctx);
        }
    });
}

fn sync(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let stages = ui.global::<Stages>();
    let load = context.catalogs.stages();
    let enemies = context.catalogs.enemies().ready();

    match &load {
        Load::Ready(bundle) => {
            let changed = {
                let held = state.borrow();
                let bundle_changed = held.bundle.as_ref().is_none_or(|known| !Rc::ptr_eq(known, bundle));
                let enemies_changed = match (&held.enemies, &enemies) {
                    (Some(known), Some(found)) => !Rc::ptr_eq(known, found),
                    (None, None) => false,
                    _ => true,
                };

                bundle_changed || enemies_changed
            };

            if changed {
                {
                    let mut held = state.borrow_mut();

                    held.bundle = Some(Rc::clone(bundle));
                    held.enemies = enemies;
                    held.rebuild();
                }

                stages.set_ready(true);
                stages.set_empty(bundle.registry.stages.is_empty());

                if let Some(id) = context.appearances.take() {
                    let mut held = state.borrow_mut();

                    held.filter = StageFilterState { enemies: vec![EnemyFilter { name_or_id: format!("{id:03}"), ..EnemyFilter::default() }], ..StageFilterState::default() };
                    held.category = None;
                    held.map = None;
                    held.stage = None;
                    stages.set_filter_active(true);
                    stages.set_browser_open(true);
                    ui.global::<Shell>().set_left_open(true);
                }

                let remembered = context.memory.get().last_stage.map(|(prefix, map, stage)| GlobalStageId { category: Category::from_prefix(&prefix), map, stage });
                let current = state.borrow().stage.clone();
                let wanted = current.or(remembered).filter(|id| bundle.registry.stages.contains_key(id));

                match wanted {
                    Some(id) => select(ui, state, context, id),
                    None => {
                        if state.borrow().stage.is_none() && !bundle.registry.stages.is_empty() && context.appearances.get().is_none() {
                            stages.set_browser_open(true);
                            ui.global::<Shell>().set_left_open(true);
                        }

                        refresh_browser(ui, state, context);
                        refresh(ui, state, context);
                    }
                }
            }
        }
        Load::Loading(..) | Load::Idle => {
            stages.set_ready(false);
            stages.set_status_title(if load.scanning() { "Scanning Stages...".into() } else { "Loading Stages...".into() });
            stages.set_status_detail(load.detail().unwrap_or_default().into());
        }
    }
}

fn apply(state: &Shared, id: &GlobalStageId) {
    let mut held = state.borrow_mut();

    held.category = Some(id.category.clone());
    held.map = Some(GlobalMapId { category: id.category.clone(), map: id.map });
    held.stage = Some(id.clone());
}

fn select(ui: &AppWindow, state: &Shared, context: &Rc<Context>, id: GlobalStageId) {
    let previous = state.borrow().stage.clone().filter(|held| *held != id);

    apply(state, &id);
    context.memory.update(|memory| memory.last_stage = Some((id.category.map_prefix(), id.map, id.stage)));
    refresh_browser(ui, state, context);
    refresh(ui, state, context);

    match previous {
        Some(shown) if context.images.pending() > 0 => {
            apply(state, &shown);
            refresh(ui, state, context);
            state.borrow_mut().arriving = Some(id.clone());

            let (held, ctx) = (Rc::clone(state), Rc::clone(context));

            context.images.settled(
                ui,
                Instant::now() + images::SETTLE_LIMIT,
                Rc::new(move |ui| {
                    if held.borrow().arriving.as_ref() != Some(&id) {
                        return;
                    }

                    held.borrow_mut().arriving = None;
                    apply(&held, &id);
                    refresh(ui, &held, &ctx);
                }),
            );
        }
        _ => state.borrow_mut().arriving = None,
    }
}

fn refresh_browser(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let stages = ui.global::<Stages>();
    let Some(globals) = context.globals() else {
        return;
    };
    let vault = context.engine.vault();
    let mut held = state.borrow_mut();

    held.refresh_matching(&vault, &globals);

    let Some(data) = held.data.clone() else {
        return;
    };
    let registry = &data.registry;
    let active = held.matching.as_ref().is_some_and(|matching| matching.active);
    let mut categories = navigate::get_categories(registry);

    categories.sort_by_key(sort_order);

    let shown_categories: Vec<Category> = categories.into_iter().filter(|category| !active || held.matching.as_ref().is_some_and(|matching| matching.categories.contains(category))).collect();
    let shown_maps: Vec<GlobalMapId> = held
        .category
        .as_ref()
        .map(|category| {
            navigate::get_maps(registry, category)
                .into_iter()
                .map(|map| GlobalMapId { category: category.clone(), map: map.map_id })
                .filter(|key| !active || held.matching.as_ref().is_some_and(|matching| matching.maps.contains(key)))
                .collect()
        })
        .unwrap_or_default();
    let shown_stages: Vec<GlobalStageId> = held
        .map
        .as_ref()
        .filter(|map_id| registry.maps.contains_key(map_id))
        .map(|map_id| {
            navigate::get_stages(registry, map_id)
                .into_iter()
                .map(|stage| GlobalStageId { category: map_id.category.clone(), map: map_id.map, stage: stage.stage_id })
                .filter(|key| !active || held.matching.as_ref().is_some_and(|matching| matching.stages.contains(key)))
                .collect()
        })
        .unwrap_or_default();
    let rows = |labels: Vec<(String, bool)>| ModelRc::new(VecModel::from(labels.into_iter().map(|(label, selected)| PickRow { label: label.into(), selected }).collect::<Vec<_>>()));

    stages.set_categories(rows(shown_categories.iter().map(|category| (display_name(category).to_owned(), held.category.as_ref() == Some(category))).collect()));
    stages.set_maps(rows(shown_maps.iter().map(|key| (registry.maps.get(key).map_or_else(|| format!("Map {}", key.map), |map| map.name.clone()), held.map.as_ref() == Some(key))).collect()));
    stages.set_stages(rows(shown_stages.iter().map(|key| (registry.stages.get(key).map_or_else(|| format!("Stage {}", key.stage), |stage| stage.name.clone()), held.stage.as_ref() == Some(key))).collect()));
    stages.set_has_category(held.category.is_some());
    stages.set_has_map(held.map.as_ref().is_some_and(|map_id| registry.maps.contains_key(map_id)));

    held.shown_categories = shown_categories;
    held.shown_maps = shown_maps;
    held.shown_stages = shown_stages;
}

fn banner(images: &Images, vault: &Arc<Vault>, name: &str) -> Option<Image> {
    vault.vfs.find(name).and_then(|path| images.trimmed(vault, &path))
}

fn refresh(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let stages = ui.global::<Stages>();
    let Some(globals) = context.globals() else {
        return;
    };
    let vault = context.engine.vault();
    let images = &context.images;
    let mut held = state.borrow_mut();
    let Some(data) = held.data.clone() else {
        stages.set_selected(false);
        return;
    };
    let max_crowns = held.stage().map_or(1, |(stage, _)| stage.max_crowns.max(1));

    if held.crown >= max_crowns {
        held.crown = 0;
    }

    let crown = held.crown;
    let Some((stage, map)) = held.stage() else {
        stages.set_selected(false);
        return;
    };
    let ctx = globals.context(&vault);
    let prefix = stage.category.image_prefix();
    let map_banner = banner(images, &vault, &stage_files::map_banner_file(stage.map_id, &prefix));
    let stage_banner = banner(images, &vault, &stage_files::stage_banner_file(&stage.category, stage.map_id, stage.stage_id, &prefix));

    stages.set_selected(true);
    stages.set_has_map_banner(map_banner.is_some());
    stages.set_map_banner(map_banner.unwrap_or_default());
    stages.set_map_name(map.name.as_str().into());
    stages.set_has_stage_banner(stage_banner.is_some());
    stages.set_stage_banner(stage_banner.unwrap_or_default());
    stages.set_stage_name(stage.name.as_str().into());
    stages.set_crowns(ModelRc::new(VecModel::from(
        (0..stage.max_crowns).map(|held| Crown { label: (held + 1).to_string().into(), enabled: stage.target_crowns == -1 || stage.target_crowns as u8 == held }).collect::<Vec<_>>(),
    )));
    stages.set_crown(i32::from(crown));

    let magnification = stage.crown_magnification(crown.saturating_add(1)).unwrap_or(100);
    let final_hp = if stage.anim_base_id != 0 { (stage.base_hp * magnification) / 100 } else { stage.base_hp };
    let dojo = stage.category == Category::DojoRankingEvents || stage.category == Category::DojoHallOfInitiates;
    let hp_header = if dojo { "Time Limit" } else { "Base HP" };
    let hp_value = if dojo { format_time(stage.time_limit) } else { final_hp.to_string() };
    let resolved_cost = cost::resolve_cost(stage, &vault.vds.items, &vault.vfs);
    let (base_header, base_value) = format_base(stage.anim_base_id, stage.base_id);
    let skip = skip_status(&stage.category, stage.map_id, &data.lock_skip_registry, &data.scat_cpu_setting);

    stages.set_info_headers_1(headers_of(&[hp_header, "Width", &resolved_cost.header, "XP", "Boss Guard", "Max Enemy", "Respawn"]));
    stages.set_info_values_1(ModelRc::new(VecModel::from(vec![
        text_cell(hp_header, &hp_value),
        text_cell("Width", &stage.width.to_string()),
        text_cell(&resolved_cost.header, &resolved_cost.value),
        text_cell("XP", &stage.xp.to_string()),
        text_cell("Boss Guard", &format_bool(stage.is_base_indestructible, "Active", "-")),
        text_cell("Max Enemy", &stage.max_enemies.to_string()),
        text_cell("Respawn", &format_respawn(stage.min_spawn, stage.max_spawn)),
    ])));
    stages.set_info_headers_2(headers_of(&[&base_header, "Background", "Music", "Boss Music", "Difficulty", "CPU Skip", "Continues"]));
    stages.set_info_values_2(ModelRc::new(VecModel::from(vec![
        text_cell(&base_header, &base_value),
        text_cell("Background", &stage.background_id.to_string()),
        text_cell("Music", &stage.init_track.to_string()),
        text_cell("Boss Music", &format_boss_bgm(stage.boss_track, stage.init_track, stage.bgm_change_percent)),
        text_cell("Difficulty", &format_diff(stage.difficulty)),
        text_cell("CPU Skip", &skip),
        text_cell("Continues", &format_bool(stage.is_no_continues, "No", "Yes")),
    ])));

    refresh_materials(ui, images, &vault, stage, map, crown);
    refresh_treasure(ui, images, &vault, stage, &data);
    refresh_lineup(ui, images, &vault, &globals, stage, crown);
    refresh_battleground(ui, images, &vault, &globals, &ctx, stage, map, crown, &data);
}

fn headers_of(labels: &[&str]) -> ModelRc<crate::StatCell> {
    ModelRc::new(VecModel::from(labels.iter().map(|label| crate::StatCell { header: (*label).into(), value: ModelRc::default(), tip: SharedString::default() }).collect::<Vec<_>>()))
}

fn refresh_materials(ui: &AppWindow, images: &Images, vault: &Arc<Vault>, stage: &Stage, map: &Map, crown: u8) {
    let stages = ui.global::<Stages>();
    let drops = map.drop_items.as_ref().filter(|drops| drops.material_drops.iter().any(|&count| count > 0));
    let Some(drops) = drops else {
        stages.set_has_materials(false);
        return;
    };
    let base_amount = drops.stage_drops.get(stage.stage_id as usize).copied().unwrap_or(0);
    let multiplier = drops.crown_multipliers.get(usize::from(crown)).copied().unwrap_or(1.0);
    let final_amount = (base_amount as f32 * multiplier).round() as u32;
    let mut chunks: Vec<ModelRc<MaterialCell>> = Vec::new();

    for (offset, slice) in [(0usize, &drops.material_drops[0..8]), (8, &drops.material_drops[8..16])] {
        if !slice.iter().any(|&count| count > 0) {
            continue;
        }

        for (chunk_index, chunk) in slice.chunks(4).enumerate() {
            let chunk_offset = offset + chunk_index * 4;
            let cells: Vec<MaterialCell> = chunk
                .iter()
                .enumerate()
                .map(|(index, &chance)| {
                    let drop = materials::resolve(&vault.vfs, chunk_offset + index, chance, &vault.vds.items);
                    let icon = drop.image_path.as_ref().and_then(|path| images.boxed(vault, path, MAX_ICON_SIZE));

                    MaterialCell { name: drop.name.as_str().into(), has_icon: icon.is_some(), icon: icon.unwrap_or_default(), chance: format!("{chance}%").into(), faint: chance == 0 }
                })
                .collect();

            chunks.push(ModelRc::new(VecModel::from(cells)));
        }
    }

    stages.set_has_materials(true);
    stages.set_materials_title(format!("Materials | Amount: {final_amount} ({base_amount}\u{d7}{multiplier:.2})").into());
    stages.set_material_chunks(ModelRc::new(VecModel::from(chunks)));
}

fn drop_rows(images: &Images, vault: &Arc<Vault>, data: &StageDataState, rows: Vec<(String, u32, u32)>) -> ModelRc<DropRow> {
    let built: Vec<DropRow> = rows
        .into_iter()
        .map(|(left, item_id, amount)| {
            let resolved = stage_treasure::resolve_drop(&vault.vfs, item_id, amount, &vault.vds.items, &data.drop_chara_registry, &data.unit_buy_registry);
            let icon = resolved.image_path.as_ref().and_then(|path| images.boxed(vault, path, MAX_ICON_SIZE));

            DropRow { left: left.into(), has_icon: icon.is_some(), icon: icon.unwrap_or_default(), name: resolved.name.as_str().into(), amount: resolved.amount_display.as_str().into() }
        })
        .collect();

    ModelRc::new(VecModel::from(built))
}

fn refresh_treasure(ui: &AppWindow, images: &Images, vault: &Arc<Vault>, stage: &Stage, data: &StageDataState) {
    let stages = ui.global::<Stages>();
    let table = |rewards: &RewardStructure, rule: &str| {
        let rows: Vec<(String, u32, u32)> = rewards
            .drops()
            .iter()
            .zip(rewards.odds())
            .filter(|(_, odds)| odds.first > 0.0 || odds.repeat > 0.0)
            .map(|(drop, odds)| (format_odds(odds), drop.item_id, drop.amount))
            .collect();

        (!rows.is_empty()).then(|| (format!("Treasure | {rule}"), rows))
    };
    let (drops, timed) = match &stage.rewards {
        RewardStructure::Treasure { drop_rule, .. } => (table(&stage.rewards, format_rule(*drop_rule)), None),
        RewardStructure::Timed { scores, .. } => {
            let rows: Vec<(String, u32, u32)> = scores.iter().map(|score| (score.score.to_string(), score.item_id, score.amount)).collect();

            (table(&stage.rewards, format_rule(-2)), (!rows.is_empty()).then_some(rows))
        }
        RewardStructure::None => (None, None),
    };

    match drops {
        Some((title, rows)) => {
            stages.set_has_treasure(true);
            stages.set_treasure_title(title.into());
            stages.set_treasure_first("Chance".into());
            stages.set_treasure_rows(drop_rows(images, vault, data, rows));
        }
        None => stages.set_has_treasure(false),
    }

    match timed {
        Some(rows) => {
            stages.set_has_timed(true);
            stages.set_timed_rows(drop_rows(images, vault, data, rows));
        }
        None => stages.set_has_timed(false),
    }
}

fn slot_view(images: &Images, vault: &Arc<Vault>, slot: &ResolvedSlot, preset: &CertificationPreset) -> LineupSlot {
    let icon = slot.image_path.as_ref().and_then(|path| images.trimmed(vault, path));
    let (Some(unit_id), Some(level)) = (slot.unit_id, slot.level) else {
        return LineupSlot { has_icon: icon.is_some(), icon: icon.unwrap_or_default(), tip: SharedString::default(), cat_id: -1, form: 0, level: SharedString::default(), jumpable: false };
    };
    let explanation = unitexplanation(&vault.vfs, unit_id);
    let form = preset.characters.get(&unit_id).map_or(0, |chara| match chara.evolution_form {
        EvolutionForm::Normal => 0,
        EvolutionForm::Evolved => 1,
        EvolutionForm::True => 2,
        EvolutionForm::Ultra => 3,
        _ => 0,
    });
    let name = explanation.names.get(form).or_else(|| explanation.names.first()).and_then(|name| name.clone()).unwrap_or_else(|| format!("Unit {unit_id:03}"));
    let plus = slot.plus_level.unwrap_or(0);
    let level_text = if plus > 0 { format!("{level}+{plus}") } else { level.to_string() };

    LineupSlot {
        has_icon: icon.is_some(),
        icon: icon.unwrap_or_default(),
        tip: format!("[Name] {name}\n[ID] {unit_id:03}\n[Level] {level_text}").into(),
        cat_id: unit_id as i32,
        form: form as i32,
        level: level_text.into(),
        jumpable: true,
    }
}

fn pairs(rows: Vec<(String, String)>) -> ModelRc<Pair> {
    ModelRc::new(VecModel::from(rows.into_iter().map(|(left, right)| Pair { left: left.into(), right: right.into() }).collect::<Vec<_>>()))
}

fn refresh_lineup(ui: &AppWindow, images: &Images, vault: &Arc<Vault>, globals: &Globals, stage: &Stage, crown: u8) {
    let stages = ui.global::<Stages>();
    let Some(preset) = stage.fixed_lineups.get(&crown) else {
        stages.set_has_lineup(false);
        return;
    };
    let lineup = core_fixedlineup::resolve_lineup(&vault.vfs, preset);
    let slots: Vec<LineupSlot> = lineup.slots.iter().take(10).map(|slot| slot_view(images, vault, slot, preset)).collect();
    let (cannon_id, fallback) = match preset.slot_cannon_type {
        CannonType::Basic => (0, "Basic"),
        CannonType::SlowBeam => (1, "Slow Beam"),
        CannonType::IronWall => (2, "Iron Wall"),
        CannonType::Thunderbolt => (3, "Thunderbolt"),
        CannonType::Waterblast => (4, "Waterblast"),
        CannonType::HolyBlast => (5, "Holy Blast"),
        CannonType::Breakerblast => (6, "Breakerblast"),
        CannonType::Curseblast => (7, "Curseblast"),
        CannonType::Unknown(id) => (i32::from(id), "Unknown"),
    };
    let cannon_name = globals.names.cannon(cannon_id).unwrap_or(fallback).to_owned();
    let cannon_level = preset.cannon_levels.get(&preset.slot_cannon_type).copied().unwrap_or(0);
    let abilities: Vec<(String, String)> = ABILITIES
        .iter()
        .map(|(kind, name)| {
            let level = preset.abilities.get(kind).map_or_else(|| "0".to_owned(), |ability| if ability.plus_level > 0 { format!("{} +{}", ability.level, ability.plus_level) } else { ability.level.to_string() });

            ((*name).to_owned(), level)
        })
        .collect();
    let treasures: Vec<(String, String)> = TREASURES
        .iter()
        .map(|(kind, name)| {
            let grades = preset.treasures.get(kind).map_or_else(|| "0/0/0".to_owned(), |treasure| format!("{}/{}/{}", treasure.inferior_count, treasure.normal_count, treasure.superior_count));

            ((*name).to_owned(), grades)
        })
        .collect();

    stages.set_has_lineup(true);
    stages.set_lineup(ModelRc::new(VecModel::from(slots)));
    stages.set_cannon(pairs(vec![(cannon_name, cannon_level.to_string())]));
    stages.set_upgrades(pairs(abilities));
    stages.set_treasures(pairs(treasures));
}

#[allow(clippy::too_many_arguments)]
fn refresh_battleground(ui: &AppWindow, images: &Images, vault: &Arc<Vault>, globals: &Globals, ctx: &GlobalContext<'_>, stage: &Stage, map: &Map, crown: u8, data: &StageDataState) {
    let stages = ui.global::<Stages>();
    let restrictions = restrictions::parse_restrictions(stage, crown as i8, *ctx);
    let rule = map.special_rules.as_ref().map(|rule| special_rule(rule, ctx));
    let bonus = map.score_bonuses.as_ref().map(|bonus| score_bonus(bonus, ctx));
    let crown_mag = stage.crown_magnification(crown.saturating_add(1)).unwrap_or(100);
    let chapter = treasure::chapter(&stage.category, stage.map_id);
    let show_score = stage.enemies.iter().any(|enemy| enemy.score > 0);
    let dojo = stage.enemies.iter().any(|enemy| enemy.base_hp_perc > 100);
    let split = stage.enemies.iter().any(|enemy| (enemy.magnification * crown_mag) / 100 != (enemy.atk_magnification * crown_mag) / 100);
    let mag_header = if split { "Magnification %\n(HP% / ATK%)" } else { "Magnification %" };
    let base_header = if dojo { "Dmg #" } else { "Base %" };
    let mut headers: Vec<SharedString> = vec!["Count".into(), mag_header.into(), base_header.into(), "Spawn".into(), "Respawn".into(), "Layer".into(), "Boss".into()];

    if show_score {
        headers.push("Score".into());
    }

    headers.push("Kills".into());

    let rows: Vec<GroundRow> = stage
        .enemies
        .iter()
        .map(|spawn| {
            let name = data.enemy_name_registry.get(spawn.enemy_id as usize).filter(|name| !name.is_empty()).cloned().unwrap_or_else(|| format!("{:03}-E", spawn.enemy_id));
            let hp_mag = (spawn.magnification * crown_mag) / 100;
            let atk_mag = (spawn.atk_magnification * crown_mag) / 100;
            let same = hp_mag == atk_mag;
            let entry = data.enemy_registry.get(&spawn.enemy_id);
            let icon = entry.and_then(|entry| entry.icon_path.as_ref()).and_then(|path| images.trimmed(vault, path));
            let gap = entry.map_or(0, |entry| globals.treasure.weakening(&entry.stats, chapter));
            let strengthened = |magnification: u32| (i64::from(magnification) * i64::from(100 + gap) / 100).to_string();
            let formatted = if same { format!("{}%", strengthened(hp_mag)) } else { format!("{}% / {}%", strengthened(hp_mag), strengthened(atk_mag)) };
            let magnification_input = if same { hp_mag.to_string() } else { format!("{hp_mag}/{atk_mag}") };
            let mut cells: Vec<SharedString> = vec![
                format_amount(&spawn.amount).into(),
                formatted.into(),
                format_base_hp(spawn.base_hp_perc, dojo).into(),
                format_spawn(spawn.time_flag, spawn.start_frame, spawn.base_hp_perc).into(),
                format_enemy_respawn(&spawn.amount, spawn.respawn_min, spawn.respawn_max).into(),
                format_layer(spawn.layer_min, spawn.layer_max).into(),
                format_boss(&spawn.boss_type).into(),
            ];

            if show_score {
                cells.push(format_count(spawn.score).into());
            }

            cells.push(format_count(spawn.kill_count).into());

            GroundRow {
                has_icon: icon.is_some(),
                icon: icon.unwrap_or_default(),
                id: format!("{:03}", spawn.enemy_id).into(),
                name: name.into(),
                enemy_id: spawn.enemy_id as i32,
                magnification: magnification_input.into(),
                cells: ModelRc::new(VecModel::from(cells)),
            }
        })
        .collect();

    stages.set_restrictions(ModelRc::new(VecModel::from(restrictions.into_iter().map(SharedString::from).collect::<Vec<_>>())));
    stages.set_has_rule(rule.is_some());
    stages.set_rule(rule.unwrap_or_default().into());
    stages.set_combos_note(if map.invalid_combos.is_empty() { SharedString::default() } else { format!("Disabled Combos: {} total", map.invalid_combos.len()).into() });
    stages.set_has_bonus(bonus.is_some());
    stages.set_bonus(bonus.unwrap_or_default().into());
    stages.set_ground_headers(ModelRc::new(VecModel::from(headers)));
    stages.set_no_enemies(stage.enemies.is_empty());
    stages.set_ground(ModelRc::new(VecModel::from(rows)));
}

fn range_fields(filter: &mut StageFilterState, ranges: &[Range]) -> ModelRc<RangeField> {
    let fields: Vec<RangeField> = ranges
        .iter()
        .enumerate()
        .map(|(index, range)| {
            let value = range_mut(filter, *range).clone();

            RangeField { label: range_label(*range).into(), min: value.min.into(), max: value.max.into(), owner: index as i32 }
        })
        .collect();

    ModelRc::new(VecModel::from(fields))
}

fn flag_rows(filter: &mut StageFilterState, names: &NameBook, span: std::ops::Range<usize>) -> ModelRc<FlagRow> {
    let rows: Vec<FlagRow> = span
        .map(|index| {
            let flag = FLAGS[index];
            let value = *flag_mut(filter, flag);

            FlagRow { label: flag_label(flag, names).into(), value: tristate(value), id: index as i32 }
        })
        .collect();

    ModelRc::new(VecModel::from(rows))
}

fn card(kind: i32, index: usize, exclude: bool, name: &str, fields: Vec<RangeField>, (boss, base, attributes): (i32, i32, bool)) -> FilterCard {
    FilterCard { kind, index: index as i32, exclude, name: name.into(), fields: ModelRc::new(VecModel::from(fields)), boss, base, attributes }
}

fn field(label: &str, range: &StatRange, owner: usize) -> RangeField {
    RangeField { label: label.into(), min: range.min.as_str().into(), max: range.max.as_str().into(), owner: owner as i32 }
}

fn refresh_filter(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let stages = ui.global::<Stages>();
    let Some(globals) = context.globals() else {
        return;
    };
    let mut filter = state.borrow().filter.clone();
    let names = &globals.names;

    stages.set_filter_category(filter.category_name.as_str().into());
    stages.set_filter_map(filter.map_name.as_str().into());
    stages.set_filter_stage(filter.stage_name.as_str().into());
    stages.set_general_flags(flag_rows(&mut filter, names, GENERAL_RULES));
    stages.set_rule_flags(flag_rows(&mut filter, names, SPECIAL_RULES));
    stages.set_bonus_flags(flag_rows(&mut filter, names, SCORE_BONUSES));
    stages.set_cost_type(match filter.cost.cost_type {
        None => 0,
        Some(CostType::Item) => 2,
        Some(CostType::Catamin) => 3,
        Some(_) => 1,
    });
    stages.set_cost_grade(match filter.cost.catamin_grade {
        None => 0,
        Some(CataminGrade::A) => 1,
        Some(CataminGrade::B) => 2,
        Some(_) => 3,
    });

    let cost_type = filter.cost.cost_type;

    stages.set_cost_fields(range_fields(&mut filter, cost_ranges(cost_type)));
    stages.set_stat_fields(range_fields(&mut filter, &STATS));
    stages.set_restriction_fields(range_fields(&mut filter, &RESTRICTIONS));
    stages.set_id_fields(range_fields(&mut filter, &IDS_AUDIO));

    let enemy_cards: Vec<FilterCard> = filter
        .enemies
        .iter_mut()
        .enumerate()
        .map(|(index, enemy)| {
            let fields: Vec<RangeField> = ENEMY_RANGES.iter().enumerate().map(|(at, range)| field(enemy_range_label(*range), &enemy_range_mut(enemy, *range).clone(), at)).collect();
            let boss = match enemy.boss_type {
                None => 0,
                Some(0) => 1,
                Some(1) => 2,
                Some(2) => 3,
                Some(_) => 0,
            };

            card(0, index, enemy.is_exclude, &enemy.name_or_id, fields, (boss, tristate(enemy.is_base), enemy.attributes.is_active()))
        })
        .collect();
    let lineup_cards: Vec<FilterCard> = filter.lineup_cats.iter().enumerate().map(|(index, cat)| card(1, index, cat.is_exclude, &cat.name_or_id, vec![field("Total Level", &cat.level, 0)], (0, 0, false))).collect();
    let treasure_cards: Vec<FilterCard> = filter
        .treasures
        .iter()
        .enumerate()
        .map(|(index, treasure)| card(2, index, treasure.is_exclude, &treasure.name_or_id, vec![field("Amount", &treasure.amount, 0), field("Chance (%)", &treasure.chance, 1)], (0, 0, false)))
        .collect();
    let material_cards: Vec<FilterCard> = filter.materials.iter().enumerate().map(|(index, material)| card(3, index, material.is_exclude, &material.name_or_id, vec![field("Amount", &material.amount, 0)], (0, 0, false))).collect();

    stages.set_enemy_cards(ModelRc::new(VecModel::from(enemy_cards)));
    stages.set_lineup_cards(ModelRc::new(VecModel::from(lineup_cards)));
    stages.set_treasure_cards(ModelRc::new(VecModel::from(treasure_cards)));
    stages.set_material_cards(ModelRc::new(VecModel::from(material_cards)));
}

fn refresh_attributes(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let stages = ui.global::<Stages>();
    let Some(globals) = context.globals() else {
        return;
    };
    let vault = context.engine.vault();
    let (filter, available) = {
        let mut held = state.borrow_mut();
        let Some(index) = held.attributes_for else {
            return;
        };
        let Some(filter) = held.filter.enemies.get(index).map(|enemy| enemy.attributes.clone()) else {
            return;
        };
        let available = match (&held.available, &held.data) {
            (Some(available), _) => Rc::clone(available),
            (None, Some(data)) => {
                let found = Rc::new(present_identities(data.enemy_registry.values().map(|enemy| &enemy.stats)));

                held.available = Some(Rc::clone(&found));
                found
            }
            (None, None) => return,
        };

        (filter, available)
    };
    let view = enemies::filter_view(&filter, &available, &globals, &vault, &context.images);

    stages.set_attr_mode(view.mode);
    stages.set_attr_mag(view.mag);
    stages.set_attr_stats(view.stats);
    stages.set_attr_traits(view.traits);
    stages.set_attr_attack_types(view.attack_types);
    stages.set_attr_headline_1(view.headline_1);
    stages.set_attr_headline_2(view.headline_2);
    stages.set_attr_body_1(view.body_1);
    stages.set_attr_body_2(view.body_2);
    stages.set_attr_footer(view.footer);
}
