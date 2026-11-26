use std::collections::HashMap;

use codex::combat::{Entity, Faction, Foe, Identity, get_talent};

use crate::common::context::GlobalContext;
use crate::common::io;
use crate::domains::cat::combo;
use crate::domains::cat::game::talents as talent_logic;
use crate::systems::combat::abilities::collect_ability_data_against;
use crate::systems::combat::registry::{get_display_def, AbilityIcon, format_stat, Magnification, StatContext, STAT_ATK_CYCLE, STAT_ATTACK, STAT_COOLDOWN, STAT_COST, STAT_DPS, STAT_HITPOINTS, STAT_KNOCKBACKS, STAT_RANGE, STAT_RARITY, STAT_SPEED};
use crate::systems::combat::{AbilityItem, CustomIcon, Mark, RenderContext};
use crate::systems::statblock::{ComboCard, ComboSeat, DetailsData, EvolveCost, EvolveData, EvolveItem, SheetHead, SpiritData, StatCell, StatblockData, TalentCard, TalentsData};

use super::files;
use super::game::stats::get_final_stats;
use super::grants::Grants;
use super::scanner::CatEntry;
use super::versus;
use super::waiter::unitid;

pub struct Subject<'a> {
    pub cat: &'a CatEntry,
    pub form: usize,
    pub current_level: i32,
    pub level_input: &'a str,
    pub talent_levels: Option<&'a HashMap<u8, u8>>,
    pub is_conjure_expanded: bool,
    pub global: GlobalContext<'a>,
    pub grants: Option<&'a Grants>,
}

const NO_DESCRIPTION: &str = "No description available";
const XP_ITEM: i32 = 6;
const LEVEL_LABEL: &str = "Level:";

fn head(cat: &CatEntry, form: usize, global: GlobalContext<'_>, top: Option<(String, String)>) -> SheetHead {
    SheetHead {
        id_str: cat.id_str(form),
        name: cat.display_name(form),
        icon: files::portrait(&global.vault.vfs, cat.deploy_icon_paths.get(form).and_then(Option::as_ref)),
        top,
    }
}

fn spoken(lines: Option<&Vec<String>>) -> Vec<String> {
    let held: Vec<String> = lines.map_or_else(Vec::new, |lines| lines.iter().map(|line| line.trim_end().to_owned()).collect());
    let last = held.iter().rposition(|line| !line.is_empty());

    last.map_or_else(Vec::new, |last| held.into_iter().take(last + 1).collect())
}

fn evolve(cat: &CatEntry, form: usize, global: GlobalContext<'_>) -> Option<EvolveData> {
    let vfs = &global.vault.vfs;
    let (materials, cost) = match form {
        2 => (cat.unitbuy.true_form_materials().collect::<Vec<_>>(), cat.unitbuy.true_form_xp_cost),
        3 => (cat.unitbuy.ultra_form_materials().collect::<Vec<_>>(), cat.unitbuy.ultra_form_xp_cost),
        _ => return None,
    };
    let lines = spoken(cat.evolve_text.texts.get(form).and_then(Option::as_ref));

    if materials.is_empty() && lines.is_empty() && cost <= 0 {
        return None;
    }

    let icon = |id: i32| io::gatya_item_icon(vfs, id).map(|path| vfs.source(&path));
    let items = materials.iter().map(|material| EvolveItem { icon: icon(material.item_id), fallback: format!("ID {}", material.item_id), amount: material.quantity }).collect();

    Some(EvolveData { lines, items, cost: (cost > 0).then(|| EvolveCost { icon: icon(XP_ITEM), amount: cost }) })
}

pub fn details(cat: &CatEntry, form: usize, global: GlobalContext<'_>) -> DetailsData {
    let mut description = spoken(cat.description.get(form).and_then(Option::as_ref));

    if description.is_empty() {
        description.push(NO_DESCRIPTION.to_owned());
    }

    let combos = combo::combos(global, cat.id, form)
        .into_iter()
        .map(|held| ComboCard {
            seats: held.members.iter().map(|member| ComboSeat { icon: member.icon.clone(), label: member.label() }).collect(),
            name: held.name,
            effect: held.effect,
            note: held.restriction,
        })
        .collect();

    DetailsData { head: head(cat, form, global, None), description, evolve: evolve(cat, form, global), combos }
}

pub fn talents(subject: Subject<'_>) -> Option<TalentsData> {
    let cat = subject.cat;
    let global = subject.global;
    let vfs = &global.vault.vfs;
    let talent_data = cat.talent_data.as_ref()?;
    let dynamic_stats = unitid(vfs, cat.id as i32);
    let base_stats = dynamic_stats.as_ref().and_then(|forms| forms.get(subject.form))?;
    let none = HashMap::new();
    let levels = subject.talent_levels.unwrap_or(&none);
    let traits = get_final_stats(base_stats, cat.curve.as_ref(), subject.current_level, Some(talent_data), Some(levels), global.treasure);
    let combo_names = global.vault.vds.cats.combo_names(vfs);
    let scaling = talent_logic::Scaling { curve: cat.curve.as_ref(), level: subject.current_level, treasure: global.treasure };

    let cards = talent_data
        .groups
        .iter()
        .enumerate()
        .map(|(index, group)| {
            let level = levels.get(&(index as u8)).copied().unwrap_or(0);
            let known = get_talent(group.ability_id);
            let shown = known.map(|def| get_display_def(def.identity));
            let name = global.names.talent(group.ability_id).map(str::to_owned).unwrap_or_else(|| {
                known.map_or_else(|| format!("Unknown Skill (ID: {})", group.ability_id), |def| global.names.ability(def.identity, Faction::Cat).to_owned())
            });
            let (icon_id, custom_icon) = match shown.as_ref().map(|shown| &shown.icon) {
                Some(AbilityIcon::Standard(id)) => (Some(*id), CustomIcon::None),
                Some(AbilityIcon::Custom(custom)) => (None, *custom),
                Some(AbilityIcon::None) | None => (None, CustomIcon::None),
            };
            let reading = talent_logic::calculate_linked_display(talent_data, levels, index, base_stats, scaling, &traits).unwrap_or_default();
            let picture = group.name_image();

            TalentCard {
                item: AbilityItem { identity: known.map_or(Identity::Unknown, |def| def.identity), icon_id, text: name, custom_icon, border_id: None, mark: Mark::None, added: false },
                plate: (picture > 0).then(|| vfs.find(&format!("Skill_name_{picture:03}.png"))).flatten().map(|path| vfs.source(&path)),
                description: talent_logic::describe(group, &cat.skill_descriptions, &combo_names),
                ultra: group.is_ultra(),
                level,
                max_level: group.max_level.max(1),
                cost: talent_logic::get_talent_np_cost(group.cost_id, level, &cat.talent_costs),
                lines: reading.lines().map(str::to_owned).collect(),
            }
        })
        .collect();

    Some(TalentsData {
        head: head(cat, subject.form, global, Some((LEVEL_LABEL.to_owned(), subject.level_input.to_owned()))),
        cost: talent_logic::get_total_np_cost(talent_data, levels, &cat.talent_costs),
        cards,
    })
}

pub fn build(subject: Subject<'_>) -> Option<StatblockData> {
    build_against(subject, |_| Foe::default())
}

pub fn build_against(subject: Subject<'_>, versus: impl FnOnce(&Entity) -> Foe) -> Option<StatblockData> {
    let cat = subject.cat;
    let dynamic_stats = unitid(&subject.global.vault.vfs, cat.id as i32);
    let base_stats = dynamic_stats.as_ref().and_then(|forms| forms.get(subject.form))?;

    let form_allows_talents = subject.grants.is_some() || subject.form >= 2;
    let talent_data = if form_allows_talents { cat.talent_data.as_ref() } else { None };
    let talent_levels = if form_allows_talents { subject.talent_levels } else { None };
    let mut final_stats = get_final_stats(base_stats, cat.curve.as_ref(), subject.current_level, talent_data, talent_levels, subject.global.treasure);

    if let Some(grants) = subject.grants {
        grants.apply(&mut final_stats);
    }

    let foe = versus(&final_stats);
    let ctx = RenderContext {
        global: subject.global,
        base_stats,
        final_stats: &final_stats,
        magnification: Magnification::default(),
        current_level: subject.current_level,
        level_curve: cat.curve.as_ref(),
        talent_data,
        talent_levels,
        is_conjure_unit: false,
    };

    Some(assemble(&ctx, cat, subject.form, subject.level_input.to_string(), subject.is_conjure_expanded, foe))
}

fn assemble(
    ctx: &RenderContext<'_>,
    cat_entry: &CatEntry,
    current_form: usize,
    level_input: String,
    is_conjure_expanded: bool,
    foe: Foe,
) -> StatblockData {
    let (mut traits, mut h1, mut h2, mut b1, mut b2, mut footer) = collect_ability_data_against(ctx, versus::against(ctx, foe));
    let spirit = if is_conjure_expanded { build_spirit_data(ctx, foe) } else { None };

    versus::tag(ctx, foe, spirit.as_ref().is_some_and(|(_, controls)| *controls), [&mut traits, &mut h1, &mut h2, &mut b1, &mut b2, &mut footer]);

    let spirit_data = spirit.map(|(data, _)| data);

    let anim_frames = cat_entry.atk_anim_frames[current_form];
    let stat_ctx = StatContext::cat(ctx.final_stats, anim_frames, Some(&cat_entry.unitbuy), ctx.global.treasure);
    let cycle = (STAT_ATK_CYCLE.get_value)(&stat_ctx);

    let headers_1 = vec![
        STAT_ATTACK.display_name.to_string(),
        STAT_DPS.display_name.to_string(),
        STAT_RANGE.display_name.to_string(),
        STAT_ATK_CYCLE.display_name.to_string(),
        STAT_RARITY.display_name.to_string(),
    ];

    let felt = versus::felt(ctx, cycle, foe);
    let shown = |held: &Option<(String, Mark)>, plain: String| held.as_ref().map_or(plain, |(text, _)| text.clone());
    let marked = |held: &Option<(String, Mark)>| held.as_ref().map_or(Mark::None, |(_, mark)| *mark);

    let data_1 = vec![
        StatCell::Text(shown(&felt.attack, format_stat(&STAT_ATTACK, &stat_ctx))),
        StatCell::Text(shown(&felt.dps, format_stat(&STAT_DPS, &stat_ctx))),
        StatCell::Text(ctx.final_stats.standing_range.to_string()),
        StatCell::Frames(cycle),
        StatCell::Text(format_stat(&STAT_RARITY, &stat_ctx)),
    ];
    let marks_1 = vec![marked(&felt.attack), marked(&felt.dps), Mark::None, Mark::None, Mark::None];

    let headers_2 = vec![
        STAT_HITPOINTS.display_name.to_string(),
        STAT_KNOCKBACKS.display_name.to_string(),
        STAT_SPEED.display_name.to_string(),
        STAT_COOLDOWN.display_name.to_string(),
        STAT_COST.display_name.to_string(),
    ];

    let cd_frames = (STAT_COOLDOWN.get_value)(&stat_ctx);

    let marks_2 = vec![marked(&felt.hitpoints), Mark::None, Mark::None, Mark::None, Mark::None];
    let data_2 = vec![
        StatCell::Text(shown(&felt.hitpoints, ctx.final_stats.hitpoints.to_string())),
        StatCell::Text(ctx.final_stats.knockbacks.to_string()),
        StatCell::Text(ctx.final_stats.speed.to_string()),
        StatCell::Frames(cd_frames),
        StatCell::Text(format_stat(&STAT_COST, &stat_ctx)),
    ];

    StatblockData {
        is_cat: true,
        id_str: cat_entry.id_str(current_form),
        name: cat_entry.display_name(current_form),
        icon: files::portrait(&ctx.global.vault.vfs, cat_entry.deploy_icon_paths[current_form].as_ref()),
        top_label: LEVEL_LABEL.to_string(),
        top_value: level_input,
        headers_1,
        data_1,
        headers_2,
        data_2,
        marks_1,
        marks_2,
        traits, h1, h2, b1, b2, footer, spirit_data,
    }
}

fn build_spirit_data(ctx: &RenderContext<'_>, foe: Foe) -> Option<(SpiritData, bool)> {
    if ctx.base_stats.conjure_unit_id <= 0 {
        return None;
    }

    let conjure_stats_vec = unitid(&ctx.global.vault.vfs, ctx.base_stats.conjure_unit_id)?;
    let conjure_stats = conjure_stats_vec.first()?;

    let conjure_final = get_final_stats(conjure_stats, ctx.level_curve, ctx.current_level, None, None, ctx.global.treasure);

    let spirit_ctx = RenderContext {
        base_stats: conjure_stats,
        final_stats: &conjure_final,
        talent_data: None,
        talent_levels: None,
        is_conjure_unit: true,
        ..*ctx
    };

    let (mut s_traits, mut s_h1, mut s_h2, mut s_b1, mut s_b2, mut s_footer) = collect_ability_data_against(&spirit_ctx, versus::against(&spirit_ctx, foe));

    versus::mark(&spirit_ctx, foe, [&mut s_traits, &mut s_h1, &mut s_h2, &mut s_b1, &mut s_b2, &mut s_footer]);

    let data = SpiritData {
        dmg_text: format!("Damage {}\nRange {}", conjure_final.attack_1_damage, conjure_final.standing_range),
        traits: s_traits,
        h1: s_h1,
        h2: s_h2,
        b1: s_b1,
        b2: s_b2,
        footer: s_footer,
    };

    Some((data, conjure_final.controls()))
}
