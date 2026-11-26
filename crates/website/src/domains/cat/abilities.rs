use dioxus::prelude::*;

use codex::combat::{Foe, Mode, Switches};
use codex::files::img015;
use foundry::common::context::GlobalContext;
use foundry::domains::cat::game::stats::get_final_stats;
use foundry::domains::cat::versus::{self, Felt};
use foundry::domains::cat::waiter::unitid;
use foundry::systems::combat::abilities::collect_ability_data;
use foundry::systems::combat::lens::{self, Pace};
use foundry::systems::combat::registry::{
    Magnification, STAT_ATK_CYCLE, STAT_ATTACK, STAT_COOLDOWN, STAT_COST, STAT_DPS, STAT_HITPOINTS, STAT_KNOCKBACKS, STAT_RANGE, STAT_RARITY, STAT_SPEED,
    StatContext, format_stat,
};
use foundry::systems::combat::{AbilityItem, CustomIcon, Mark, RenderContext};
use foundry::systems::treasure::Bonus;

use crate::systems::context::{self, Shared};
use crate::systems::images::{self, Pictures};
use crate::widget::{FitWidth, StatFrames, StatHeader, StatValue, Superscript, Tip, span};

use super::icons::{self, ICON_SIZE, IMG015};
use super::{Picked, state};

pub(crate) const DESCRIPTION_TEXT_SIZE: f32 = 13.0;
const STAT_COLUMNS: usize = 5;

#[component]
pub(crate) fn Abilities(selected: Picked, globals: Shared) -> Element {
    let settings = context::settings();
    let mut state = state();
    let pictures = images::pictures();
    let cat = &selected.0.cat;
    let form = (state.form)();
    let level = (state.level)();
    let Some(base) = selected.0.stats.as_ref().and_then(|stats| stats.get(form)) else {
        return rsx! { div { class: "note", "Stats data not found" } };
    };
    let allows_talents = state.talented(form);
    let talent_data = if allows_talents { cat.talent_data.as_ref() } else { None };
    let levels = allows_talents.then(|| state.levels_for(cat.id));
    let treasure = state.treasure_or(globals.0.treasure);
    let mut final_stats = get_final_stats(base, cat.curve.as_ref(), level, talent_data, levels.as_ref(), &treasure);

    if let Some(grants) = state.granted.read().as_ref() {
        grants.apply(&mut final_stats);
    }
    let ctx = RenderContext {
        global: GlobalContext { treasure: &treasure, ..globals.0.context() },
        base_stats: base,
        final_stats: &final_stats,
        magnification: Magnification::default(),
        current_level: level,
        level_curve: cat.curve.as_ref(),
        talent_data,
        talent_levels: levels.as_ref(),
        is_conjure_unit: false,
    };
    let versus = (state.versus)().then(|| (state.foe)());
    let stat_ctx = StatContext::cat(&final_stats, cat.atk_anim_frames[form], Some(&cat.unitbuy), &treasure);
    let held = (state.lens)();
    let on = versus.is_some();
    let switches = if on { (state.switches)() } else { Switches::default() };
    let lens = lens::Lens { hits: lens::settled(&final_stats, held.hits), switches, switching: on, ..held };
    let (traits, headline_1, headline_2, body_1, body_2, footer) = match versus {
        Some(foe) => versus::palette(&ctx, foe, versus::spirit_controls(&ctx), Pace::of(&stat_ctx), lens),
        None => lens::plain(&ctx, Pace::of(&stat_ctx), lens),
    };
    let felt = versus::felt(&ctx, &stat_ctx, versus.unwrap_or_default(), lens);
    let conjure_id = base.conjure_unit_id;
    let conjure_open = state.conjure_expanded.read().get(&cat.id).copied().unwrap_or(settings.read().cat_data.expand_spirit_details);
    let cat_id = cat.id;

    rsx! {
        FitWidth { natural: span(STAT_COLUMNS),
        div { class: "abilities",
            {stats_grid(&stat_ctx, &felt)}
            div { class: "ability-groups",
                if versus.is_some() {
                    {pick_row(&pictures, &traits, state.foe)}
                } else if !traits.is_empty() {
                    {icon_row(&pictures, &traits)}
                }
                if !headline_1.is_empty() {
                    {icon_row(&pictures, &headline_1)}
                }
                if !headline_2.is_empty() {
                    {icon_row(&pictures, &headline_2)}
                }
                for items in [&body_1, &body_2] {
                    if !items.is_empty() {
                        div { class: "ability-list",
                            for item in items.iter() {
                                if item.icon_id == Some(img015::ICON_CONJURE) && item.custom_icon == CustomIcon::None {
                                    div { class: "ability-row",
                                        {shown_icon(&pictures, item)}
                                        Superscript { text: item.text.clone(), size: DESCRIPTION_TEXT_SIZE }
                                        button {
                                            class: if conjure_open { "btn btn-toggle active conjure-toggle" } else { "btn btn-toggle conjure-toggle" },
                                            onclick: move |_| {
                                                let next = !conjure_open;
                                                state.conjure_expanded.write().insert(cat_id, next);
                                            },
                                            "Details"
                                        }
                                    }
                                    if conjure_open {
                                        {spirit_card(&pictures, &globals, Spirited { treasure: &treasure, conjure_id, curve: cat.curve.as_ref(), level, foe: versus })}
                                    }
                                } else if item.switchable {
                                    {switch_row(&pictures, item, state.switches, held.mode)}
                                } else {
                                    {ability_row(&pictures, item)}
                                }
                            }
                        }
                    }
                }
                if !footer.is_empty() {
                    {icon_row(&pictures, &footer)}
                }
            }
        }
        }
    }
}

fn tone_style(mark: Mark) -> String {
    let color = |tone: [u8; 4]| format!("rgba({}, {}, {}, {})", tone[0], tone[1], tone[2], f32::from(tone[3]) / 255.0);

    mark.tones().map_or_else(String::new, |(upper, lower)| format!("--mark-upper: {}; --mark-lower: {};", color(upper), color(lower)))
}

fn pair(header: Element, value: Element, felt: Option<&(String, Mark)>) -> Element {
    let mark = felt.map_or(Mark::None, |(_, mark)| *mark);
    let class = if mark == Mark::None { "stat-pair" } else { "stat-pair marked" };

    rsx! {
        div { class, style: tone_style(mark), {header} {value} }
    }
}

fn stats_grid(ctx: &StatContext<'_>, felt: &Felt) -> Element {
    let cycle = (STAT_ATK_CYCLE.get_value)(ctx);
    let cooldown = (STAT_COOLDOWN.get_value)(ctx);
    let shown = |held: &Option<(String, Mark)>, plain: String| held.as_ref().map_or(plain, |(text, _)| text.clone());
    let head = |label: &'static str| rsx! { StatHeader { label } };
    let value = |header: &'static str, value: String| rsx! { StatValue { header, value } };
    let frames = |header: &'static str, frames: i32| rsx! { StatFrames { header, frames } };

    rsx! {
        div { class: "stat-grid",
            div { class: "stat-row",
                {pair(head(STAT_ATTACK.display_name), value(STAT_ATTACK.display_name, shown(&felt.attack, format_stat(&STAT_ATTACK, ctx))), felt.attack.as_ref())}
                {pair(head(STAT_DPS.display_name), value(STAT_DPS.display_name, shown(&felt.dps, format_stat(&STAT_DPS, ctx))), felt.dps.as_ref())}
                {pair(head(STAT_RANGE.display_name), value(STAT_RANGE.display_name, format_stat(&STAT_RANGE, ctx)), None)}
                {pair(head(STAT_ATK_CYCLE.display_name), frames(STAT_ATK_CYCLE.display_name, cycle), None)}
                {pair(head(STAT_RARITY.display_name), value(STAT_RARITY.display_name, format_stat(&STAT_RARITY, ctx)), None)}
            }
            div { class: "stat-row",
                {pair(head(STAT_HITPOINTS.display_name), value(STAT_HITPOINTS.display_name, shown(&felt.hitpoints, format_stat(&STAT_HITPOINTS, ctx))), felt.hitpoints.as_ref())}
                {pair(head(STAT_KNOCKBACKS.display_name), value(STAT_KNOCKBACKS.display_name, format_stat(&STAT_KNOCKBACKS, ctx)), None)}
                {pair(head(STAT_SPEED.display_name), value(STAT_SPEED.display_name, format_stat(&STAT_SPEED, ctx)), None)}
                {pair(head(STAT_COOLDOWN.display_name), frames(STAT_COOLDOWN.display_name, cooldown), None)}
                {pair(head(STAT_COST.display_name), value(STAT_COST.display_name, format_stat(&STAT_COST, ctx)), None)}
            }
        }
    }
}

fn shown_icon(pictures: &Pictures, item: &AbilityItem) -> Element {
    let icon = icons::item_icon(pictures, item, ICON_SIZE);
    let classes: Vec<&str> = [item.mark.tones().map(|_| "marked"), item.dimmed.then_some("dimmed")].into_iter().flatten().collect();

    if classes.is_empty() {
        return icon;
    }

    rsx! {
        div { class: classes.join(" "), style: tone_style(item.mark), {icon} }
    }
}

pub(crate) fn icon_row(pictures: &Pictures, items: &[AbilityItem]) -> Element {
    rsx! {
        div { class: "icon-row",
            for item in items {
                Tip { text: item.text.clone(), {shown_icon(pictures, item)} }
            }
        }
    }
}

fn pick_row(pictures: &Pictures, items: &[AbilityItem], mut foe: Signal<Foe>) -> Element {
    rsx! {
        div { class: "icon-row",
            for item in items {
                Tip { text: item.text.clone(),
                    button {
                        class: "icon-pick",
                        onclick: {
                            let label = item.identity;

                            move |_| {
                                let held = *foe.peek();

                                foe.set(if held.has(label) { held.without(label) } else { held.with(label) });
                            }
                        },
                        {shown_icon(pictures, item)}
                    }
                }
            }
        }
    }
}

fn switch_row(pictures: &Pictures, item: &AbilityItem, mut switches: Signal<Switches>, mode: Mode) -> Element {
    let switch = lens::switch_of(item.identity);

    rsx! {
        button {
            class: "row-pick",
            onclick: move |_| {
                if let Some(switch) = switch {
                    let flipped = switches.peek().flipped(switch, mode);

                    switches.set(flipped);
                }
            },
            {ability_row(pictures, item)}
        }
    }
}

pub(crate) fn ability_row(pictures: &Pictures, item: &AbilityItem) -> Element {
    rsx! {
        div { class: "ability-row",
            {shown_icon(pictures, item)}
            Superscript { text: item.text.clone(), size: DESCRIPTION_TEXT_SIZE }
        }
    }
}

struct Spirited<'a> {
    treasure: &'a Bonus,
    conjure_id: i32,
    curve: Option<&'a codex::cat::unit::LevelCurve>,
    level: i32,
    foe: Option<Foe>,
}

fn spirit_card(pictures: &Pictures, globals: &Shared, spirited: Spirited<'_>) -> Element {
    let Spirited { treasure, conjure_id, curve, level, foe } = spirited;
    let vfs = &globals.0.vault().vfs;
    let Some(stats) = unitid(vfs, conjure_id).and_then(|forms| forms.into_iter().next()) else {
        return rsx! { div { class: "note", "Spirit data not found" } };
    };
    let final_stats = get_final_stats(&stats, curve, level, None, None, treasure);
    let ctx = RenderContext {
        global: GlobalContext { treasure, ..globals.0.context() },
        base_stats: &stats,
        final_stats: &final_stats,
        magnification: Magnification::default(),
        current_level: level,
        level_curve: curve,
        talent_data: None,
        talent_levels: None,
        is_conjure_unit: true,
    };
    let (traits, headline_1, headline_2, body_1, body_2, footer) = match foe {
        Some(foe) => versus::spirit_abilities(&ctx, foe),
        None => collect_ability_data(&ctx),
    };
    let area = icons::sheet_url(pictures, IMG015, img015::ICON_AREA_ATTACK);
    let damage = format!("Damage {}\nRange {}", final_stats.attack_1_damage, final_stats.standing_range);

    rsx! {
        div { class: "card spirit-card",
            div { class: "ability-row",
                {area.map_or_else(|| icons::fallback("AoE", ICON_SIZE), |url| icons::image(url, ICON_SIZE))}
                Superscript { text: damage, size: DESCRIPTION_TEXT_SIZE }
            }
            if !traits.is_empty() {
                {icon_row(pictures, &traits)}
            }
            if !headline_1.is_empty() {
                {icon_row(pictures, &headline_1)}
            }
            if !headline_2.is_empty() {
                {icon_row(pictures, &headline_2)}
            }
            for items in [&body_1, &body_2] {
                if !items.is_empty() {
                    div { class: "ability-list",
                        for item in items.iter() {
                            {ability_row(pictures, item)}
                        }
                    }
                }
            }
            if !footer.is_empty() {
                {icon_row(pictures, &footer)}
            }
        }
    }
}
