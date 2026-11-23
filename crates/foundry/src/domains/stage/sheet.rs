use std::collections::HashMap;

use codex::cat::unit::UnitBuy;
use codex::chapter::Category;
use codex::chapter::map::{BonusType, LockSkipDataEntry, RuleType, ScoreBonusMapEntry, SpecialRulesMapEntry};
use codex::chapter::stage::{BossType, DropOdds, EnemyAmount, RewardStructure, ScatCpuSetting};
use codex::common::{BreakHandling, strip_html_tags};

use crate::Vfs;
use crate::common::context::GlobalContext;
use crate::domains::enemy::scanner::EnemyEntry;
use crate::systems::statblock::{StageCrown, StageDrop, StageDropTable, StageGround, StageMaterial, StageMaterials, StageNote, StagePlate, StageSheet, StageSpawn};
use crate::systems::treasure;

use super::{Map, Stage, cost, files, materials, restrictions, treasure as drops};

const ENEMY_COLUMN: u16 = 4;
const COUNT_COLUMN: u16 = 5;
const MAG_COLUMN: u16 = 9;
const BASE_COLUMN: u16 = 6;
const SPAWN_COLUMN: u16 = 6;
const RESPAWN_COLUMN: u16 = 9;
const LAYER_COLUMN: u16 = 6;
const BOSS_COLUMN: u16 = 8;
const SCORE_COLUMN: u16 = 5;
const KILLS_COLUMN: u16 = 5;
const MATERIAL_ROW: usize = 4;
const MATERIAL_HALF: usize = 8;
const UNLIMITED_RULE: i32 = -2;
const SILENT_SWITCH: u32 = 100;

pub struct Subject<'a> {
    pub stage: &'a Stage,
    pub map: &'a Map,
    pub crown: u8,
    pub enemies: &'a HashMap<u32, EnemyEntry>,
    pub drop_charas: &'a HashMap<u32, u32>,
    pub unit_buys: &'a HashMap<u32, UnitBuy>,
    pub lock_skips: &'a HashMap<u32, LockSkipDataEntry>,
    pub cpu: &'a ScatCpuSetting,
    pub ctx: GlobalContext<'a>,
}

fn dash_unless(shown: bool, text: impl FnOnce() -> String) -> String {
    if shown { text() } else { "-".to_owned() }
}

fn span(low: impl ToString, high: impl ToString, suffix: &str, joint: &str) -> String {
    let (low, high) = (low.to_string(), high.to_string());

    if low == high { format!("{low}{suffix}") } else { format!("{low}{suffix}{joint}{high}{suffix}") }
}

fn base(stage: &Stage) -> (String, String) {
    if stage.anim_base_id != 0 {
        return ("Base Enemy".to_owned(), format!("{:03}-E", stage.anim_base_id.saturating_sub(2)));
    }

    ("Base Img".to_owned(), stage.base_id.to_string())
}

fn skip(subject: &Subject<'_>) -> String {
    let stage = subject.stage;
    let excluded = stage
        .category
        .global_map_id(stage.map_id)
        .is_some_and(|global| subject.lock_skips.get(&global).is_some_and(|entry| entry.excluded_map_id == global));

    if excluded {
        return "N/A".to_owned();
    }

    let consumed = subject.cpu.super_cpu_consume_amount;

    dash_unless(consumed > 0, || format!("{consumed} CPUs"))
}

fn percent(value: f64) -> String {
    let rounded = format!("{value:.2}");

    format!("{}%", rounded.trim_end_matches('0').trim_end_matches('.'))
}

fn odds(odds: DropOdds) -> String {
    let first = percent(odds.first);

    if odds.repeat <= 0.0 {
        return first;
    }

    let repeat = percent(odds.repeat);

    if first == repeat { first } else { format!("{first} -> {repeat}") }
}

fn treasure_rule(drop_rule: i32) -> &'static str {
    match drop_rule {
        1 | 2 => "First Drop Once",
        -2..=0 => "Unlimited",
        -3 => "Guaranteed (Once)",
        -4 => "Guaranteed (Unlimited)",
        _ => "Unknown Rule",
    }
}

fn filled(mut description: String, parameters: impl Iterator<Item = u32>) -> String {
    for parameter in parameters {
        description = description.replacen("%d", &parameter.to_string(), 1);
    }

    description
}

fn rule_parameters(rule: &RuleType) -> &[u32] {
    match rule {
        RuleType::TrustFund(parameters)
        | RuleType::CooldownEquality(parameters)
        | RuleType::LineupLimit(parameters)
        | RuleType::RarityLimit(parameters)
        | RuleType::CheapLabor(parameters)
        | RuleType::CatCost(parameters)
        | RuleType::CatProduction(parameters)
        | RuleType::TotalDeployLimit(parameters)
        | RuleType::MoreThanOne(parameters)
        | RuleType::MegaCatCannon(parameters)
        | RuleType::UniformMotion(parameters)
        | RuleType::CompoundingCost(parameters)
        | RuleType::Unknown(_, parameters) => parameters,
    }
}

fn bonus_parameters(bonus: &BonusType) -> &[u32] {
    match bonus {
        BonusType::Weaken(parameters)
        | BonusType::Freeze(parameters)
        | BonusType::Slow(parameters)
        | BonusType::Knockback(parameters)
        | BonusType::StrongAttack(parameters)
        | BonusType::MassiveDamage(parameters)
        | BonusType::StrongDefense(parameters)
        | BonusType::Resist(parameters)
        | BonusType::Unknown(_, parameters) => parameters,
    }
}

fn special_rule(rule: &SpecialRulesMapEntry, ctx: &GlobalContext<'_>) -> String {
    let key = if rule.explanation_label.is_empty() {
        rule.name_label.trim().strip_suffix("Name").map(|prefix| format!("{prefix}Explanation")).unwrap_or_default()
    } else {
        rule.explanation_label.clone()
    };
    let raw = if key.is_empty() { "" } else { ctx.localizable.lookup(&key).unwrap_or_default() };
    let description = strip_html_tags(raw, BreakHandling::Space);

    if description.is_empty() {
        return rule.rules.iter().map(|held| format!("{held:?}")).collect::<Vec<_>>().join("\n");
    }

    filled(description, rule.rules.iter().flat_map(|held| rule_parameters(held).iter().copied()))
}

fn score_bonus(bonus: &ScoreBonusMapEntry, ctx: &GlobalContext<'_>) -> String {
    let key = if bonus.explanation_label.is_empty() { &bonus.name_label } else { &bonus.explanation_label };
    let description = strip_html_tags(ctx.localizable.lookup(key).unwrap_or_default(), BreakHandling::Space);

    if description.is_empty() {
        return format!("\u{3010}{key}\u{3011} Localization data missing.");
    }

    filled(description, bonus.bonuses.iter().flat_map(|held| bonus_parameters(held).iter().copied()))
}

fn information(subject: &Subject<'_>) -> [Vec<String>; 4] {
    let stage = subject.stage;
    let (vfs, items) = (&subject.ctx.vault.vfs, &subject.ctx.vault.vds.items);
    let magnification = stage.crown_magnification(subject.crown.saturating_add(1)).unwrap_or(100);
    let hitpoints = if stage.anim_base_id != 0 { stage.base_hp * magnification / 100 } else { stage.base_hp };
    let dojo = matches!(stage.category, Category::DojoRankingEvents | Category::DojoHallOfInitiates);
    let (health_header, health) = if dojo { ("Time Limit", dash_unless(stage.time_limit != 0, || format!("{}m", stage.time_limit))) } else { ("Base HP", hitpoints.to_string()) };
    let energy = cost::resolve_cost(stage, items, vfs);
    let (base_header, base_value) = base(stage);
    let boss_music = boss_track(stage).map_or_else(|| "-".to_owned(), |track| track.to_string());
    let owned = |labels: [&str; 7]| labels.map(str::to_owned).to_vec();

    [
        owned([health_header, "Width", &energy.header, "XP", "Boss Guard", "Max Enemy", "Respawn"]),
        vec![
            health,
            stage.width.to_string(),
            energy.value,
            stage.xp.to_string(),
            dash_unless(stage.is_base_indestructible, || "Active".to_owned()),
            stage.max_enemies.to_string(),
            span(stage.min_spawn, stage.max_spawn, "f", " ~ "),
        ],
        owned([&base_header, "Background", "Music", "Boss Music", "Difficulty", "CPU Skip", "Continues"]),
        vec![
            base_value,
            stage.background_id.to_string(),
            stage.init_track.to_string(),
            boss_music,
            dash_unless(stage.difficulty != 0, || format!("\u{2605}{}", stage.difficulty)),
            skip(subject),
            if stage.is_no_continues { "No" } else { "Yes" }.to_owned(),
        ],
    ]
}

fn material_drops(subject: &Subject<'_>) -> Option<StageMaterials> {
    let held = subject.map.drop_items.as_ref().filter(|held| held.has_materials())?;
    let (vfs, items) = (&subject.ctx.vault.vfs, &subject.ctx.vault.vds.items);
    let mut chunks = Vec::new();

    for (half, chances) in held.material_drops.chunks(MATERIAL_HALF).enumerate().take(2) {
        if chances.iter().all(|chance| *chance == 0) {
            continue;
        }

        for (row, chunk) in chances.chunks(MATERIAL_ROW).enumerate() {
            let offset = half * MATERIAL_HALF + row * MATERIAL_ROW;

            chunks.push(
                chunk
                    .iter()
                    .enumerate()
                    .map(|(slot, chance)| {
                        let drop = materials::resolve(vfs, offset + slot, *chance, items);

                        StageMaterial { name: drop.name, icon: drop.image_path.as_deref().map(|path| vfs.source(path)), odds: materials::odds(held, offset + slot), empty: *chance == 0 }
                    })
                    .collect(),
            );
        }
    }

    Some(StageMaterials { heading: materials::heading(held, subject.stage, subject.crown), chunks })
}

fn drop_row(subject: &Subject<'_>, left: String, item: u32, amount: u32) -> StageDrop {
    let vfs = &subject.ctx.vault.vfs;
    let resolved = drops::resolve_drop(vfs, item, amount, &subject.ctx.vault.vds.items, subject.drop_charas, subject.unit_buys);

    StageDrop { left, icon: resolved.image_path.as_deref().map(|path| vfs.source(path)), name: resolved.name, amount: resolved.amount_display }
}

fn drop_tables(subject: &Subject<'_>) -> Vec<StageDropTable> {
    let rewards = &subject.stage.rewards;
    let rule = match rewards {
        RewardStructure::Treasure { drop_rule, .. } => *drop_rule,
        RewardStructure::Timed { .. } => UNLIMITED_RULE,
        RewardStructure::None => return Vec::new(),
    };
    let rolled: Vec<StageDrop> = rewards
        .drops()
        .iter()
        .zip(rewards.odds())
        .filter(|(_, held)| held.first > 0.0 || held.repeat > 0.0)
        .map(|(drop, held)| drop_row(subject, odds(held), drop.item_id, drop.amount))
        .collect();
    let mut tables = Vec::new();

    if !rolled.is_empty() {
        tables.push(StageDropTable { title: format!("Treasure | {}", treasure_rule(rule)), first_header: "Chance".to_owned(), rows: rolled });
    }

    if let RewardStructure::Timed { scores, .. } = rewards
        && !scores.is_empty()
    {
        let rows = scores.iter().map(|score| drop_row(subject, score.score.to_string(), score.item_id, score.amount)).collect();

        tables.push(StageDropTable { title: "Timed Score Rewards".to_owned(), first_header: "Score".to_owned(), rows });
    }

    tables
}

fn notes(subject: &Subject<'_>) -> Vec<StageNote> {
    let mut notes = Vec::new();
    let restricted = restrictions::parse_restrictions(subject.stage, subject.crown as i8, subject.ctx);

    if !restricted.is_empty() {
        notes.push(StageNote { title: "Restrictions".to_owned(), lines: restricted });
    }

    if let Some(rule) = &subject.map.special_rules {
        let mut lines = vec![special_rule(rule, &subject.ctx)];

        if !subject.map.invalid_combos.is_empty() {
            lines.push(format!("Disabled Combos: {} total", subject.map.invalid_combos.len()));
        }

        notes.push(StageNote { title: "Rules".to_owned(), lines });
    }

    if let Some(bonus) = &subject.map.score_bonuses {
        notes.push(StageNote { title: "Score Bonus".to_owned(), lines: vec![score_bonus(bonus, &subject.ctx)] });
    }

    notes
}

fn ground(subject: &Subject<'_>) -> StageGround {
    let stage = subject.stage;
    let crown = stage.crown_magnification(subject.crown.saturating_add(1)).unwrap_or(100);
    let chapter = treasure::chapter(&stage.category, stage.map_id);
    let scored = stage.enemies.iter().any(|enemy| enemy.score > 0);
    let dojo = stage.enemies.iter().any(|enemy| enemy.base_hp_perc > 100);
    let pair = |magnification: u32, attack: u32| (magnification * crown / 100, attack * crown / 100);
    let split = stage.enemies.iter().any(|enemy| {
        let (health, attack) = pair(enemy.magnification, enemy.atk_magnification);

        health != attack
    });
    let mut headers = vec![
        ("Enemy".to_owned(), ENEMY_COLUMN),
        ("Count".to_owned(), COUNT_COLUMN),
        (if split { "Magnification %\n(HP% / ATK%)" } else { "Magnification %" }.to_owned(), MAG_COLUMN),
        (if dojo { "Dmg #" } else { "Base %" }.to_owned(), BASE_COLUMN),
        ("Spawn".to_owned(), SPAWN_COLUMN),
        ("Respawn".to_owned(), RESPAWN_COLUMN),
        ("Layer".to_owned(), LAYER_COLUMN),
        ("Boss".to_owned(), BOSS_COLUMN),
    ];

    if scored {
        headers.push(("Score".to_owned(), SCORE_COLUMN));
    }

    headers.push(("Kills".to_owned(), KILLS_COLUMN));

    let spawns = stage
        .enemies
        .iter()
        .map(|spawn| {
            let entry = subject.enemies.get(&spawn.enemy_id);
            let (health, attack) = pair(spawn.magnification, spawn.atk_magnification);
            let gap = entry.map_or(0, |entry| subject.ctx.treasure.weakening(&entry.stats, chapter));
            let strengthened = |magnification: u32| i64::from(magnification) * i64::from(100 + gap) / 100;
            let single = spawn.amount == EnemyAmount::Limit(1);
            let mut cells = vec![
                match spawn.amount {
                    EnemyAmount::Infinite => "\u{221e}".to_owned(),
                    EnemyAmount::Limit(limit) => limit.to_string(),
                },
                span(strengthened(health), strengthened(attack), "%", " / "),
                if dojo { spawn.base_hp_perc.to_string() } else { dash_unless(spawn.base_hp_perc != 100, || format!("{}%", spawn.base_hp_perc)) },
                dash_unless(spawn.base_hp_perc == 100 || spawn.time_flag != 0, || format!("{}f", spawn.start_frame)),
                dash_unless(!single, || span(spawn.respawn_min, spawn.respawn_max, "f", " ~ ")),
                span(spawn.layer_min, spawn.layer_max, "", " ~ "),
                match spawn.boss_type {
                    BossType::None => "-",
                    BossType::Boss => "Yes",
                    BossType::ScreenShake => "Yes (Shake)",
                    BossType::Unknown(_) => "Unknown",
                }
                .to_owned(),
            ];

            if scored {
                cells.push(dash_unless(spawn.score != 0, || spawn.score.to_string()));
            }

            cells.push(dash_unless(spawn.kill_count != 0, || spawn.kill_count.to_string()));

            StageSpawn { icon: entry.and_then(|entry| entry.icon_path.as_deref()).map(|path| subject.ctx.vault.vfs.source(path)), fallback: format!("{:03}", spawn.enemy_id), cells }
        })
        .collect();

    StageGround { headers, spawns }
}

pub fn boss_track(stage: &Stage) -> Option<u32> {
    u32::try_from(stage.boss_track).ok().filter(|track| *track != stage.init_track && stage.bgm_change_percent != SILENT_SWITCH)
}

pub fn plates(stage: &Stage, map: &Map, vfs: &Vfs) -> (StagePlate, StagePlate) {
    let prefix = stage.category.image_prefix();
    let banner = |file: String| vfs.find(file.as_str()).map(|path| vfs.source(&path));

    (
        StagePlate { image: banner(files::map_banner_file(stage.map_id, &prefix)), name: map.name.clone() },
        StagePlate { image: banner(files::stage_banner_file(&stage.category, stage.map_id, stage.stage_id, &prefix)), name: stage.name.clone() },
    )
}

pub fn build(subject: Subject<'_>) -> StageSheet {
    let (stage, map) = (subject.stage, subject.map);
    let (map_plate, stage_plate) = plates(stage, map, &subject.ctx.vault.vfs);
    let crowns = if stage.max_crowns > 1 {
        (0..stage.max_crowns).map(|crown| StageCrown { selected: crown == subject.crown, enabled: stage.target_crowns == -1 || stage.target_crowns as u8 == crown }).collect()
    } else {
        Vec::new()
    };
    let [headers_1, values_1, headers_2, values_2] = information(&subject);

    StageSheet {
        id: format!("{}{:03}-{:02}", stage.category.map_prefix(), stage.map_id, stage.stage_id),
        crown: subject.crown,
        map: map_plate,
        stage: stage_plate,
        crowns,
        headers_1,
        values_1,
        headers_2,
        values_2,
        materials: material_drops(&subject),
        tables: drop_tables(&subject),
        notes: notes(&subject),
        ground: ground(&subject),
    }
}
