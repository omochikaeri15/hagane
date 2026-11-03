use std::rc::Rc;

use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use codex::chapter::Category;
use foundry::domains::sandbox::base;
use foundry::domains::sandbox::config::{CatGod, FILIBUSTER_MAP, StartSpeed, Tutorial};
use foundry::domains::sandbox::{ITEMS, TECHS};
use foundry::domains::stage::{GlobalMapId, StageRegistry, navigate};

use crate::settings::Orientation;
use crate::host::Lag;
use crate::pages::{Context, settings};
use crate::{AppWindow, ConfigEntry, ItemRow, Sandbox};

use super::state::{Device, Scaling, Volume};
use super::{Shared, Staging, battle, refresh};

const LEVEL_DIGITS: usize = 6;
const TUTORIAL_STEPS: [(&str, &str); 5] = [
    ("First Battle", "Off replays the first battle's guided deployment and continue prompts"),
    ("Deck Guide", "Off shows the deck guide popup when a battle starts"),
    ("Two Rows Guide", "Off shows the two-row deck guide popup when a battle starts"),
    ("Cat God Guide", "Off shows the Cat God guide popup on the first Cat God open"),
    ("Shop Guide", "Off shows the Cat Food shop guide popup on the first shop open"),
];
const BASE_LEVELS: [&str; 4] = ["Cannon Level", "Style Level", "Foundation Level", "Base Health Level"];

fn typable(entry: &str) -> bool {
    entry.len() <= LEVEL_DIGITS && entry.chars().all(|glyph| glyph.is_ascii_digit() || glyph == '+' || glyph == '%')
}

fn tutorial_step(tutorial: &Tutorial, step: usize) -> bool {
    match step {
        0 => tutorial.battle_cleared,
        1 => tutorial.deck_seen,
        2 => tutorial.two_rows_seen,
        3 => tutorial.cat_god_seen,
        _ => tutorial.shop_seen,
    }
}

fn set_tutorial_step(tutorial: &mut Tutorial, step: usize, cleared: bool) {
    match step {
        0 => tutorial.battle_cleared = cleared,
        1 => tutorial.deck_seen = cleared,
        2 => tutorial.two_rows_seen = cleared,
        3 => tutorial.cat_god_seen = cleared,
        _ => tutorial.shop_seen = cleared,
    }
}

fn invadable(registry: &StageRegistry, category: Category) -> Vec<(u32, String)> {
    let map = GlobalMapId { category, map: FILIBUSTER_MAP };

    navigate::invadable_stages(registry, &map).into_iter().map(|stage| (stage.stage_id, stage.name.clone())).collect()
}

type Labeled = Vec<(i32, String)>;

fn labeled(state: &Shared, context: &Rc<Context>) -> (Labeled, Labeled, Labeled) {
    let parts = super::parts(state, context);
    let Some(globals) = context.globals() else {
        return (Vec::new(), Vec::new(), Vec::new());
    };
    let names = &globals.names;
    let cannons = parts.cannons.keys().map(|id| (*id, names.cannon(*id).map_or_else(|| base::name(*id), str::to_owned))).collect();
    let styles = parts.styles.keys().map(|id| (*id, parts.style_label(*id, names))).collect();
    let foundations = parts.foundations.keys().map(|id| (*id, parts.foundation_label(*id, names))).collect();

    (cannons, styles, foundations)
}

fn apply(ui: &AppWindow, state: &Shared, context: &Rc<Context>, change: impl FnOnce(&mut super::state::Saved)) {
    state.borrow().store.update(change);
    battle::retune(state);
    refresh(ui, state, context);
}

pub fn wire(window: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let sandbox = window.global::<Sandbox>();
    let on = |state: &Shared, context: &Rc<Context>, window: &AppWindow| (Rc::clone(state), Rc::clone(context), window.as_weak());

    macro_rules! setter {
        ($callback:ident, |$saved:ident, $value:ident| $body:expr) => {{
            let (held, ctx, weak) = on(state, context, window);

            sandbox.$callback(move |$value| {
                if let Some(ui) = weak.upgrade() {
                    apply(&ui, &held, &ctx, |$saved| $body);
                }
            });
        }};
    }

    setter!(on_panel_set, |saved, index| saved.panel = index.clamp(0, 6) as u8);
    setter!(on_music_set, |saved, index| saved.music_volume = Volume::at(index).percent());
    setter!(on_effects_set, |saved, index| saved.effects_volume = Volume::at(index).percent());
    setter!(on_vibration_set, |saved, index| saved.vibrate = index == 0);
    setter!(on_formation_set, |saved, index| saved.two_rows = index == 1);
    setter!(on_device_set, |saved, index| saved.device = if index == 1 { Device::Phone } else { Device::Tablet });
    setter!(on_scaling_set, |saved, index| saved.scaling = Some(if index == 1 { Scaling::Pad } else { Scaling::Stretch }));
    setter!(on_lag_set, |saved, index| saved.lag = if index == 0 { Lag::CatchUp } else { Lag::Slowdown });
    setter!(on_battle_orientation_set, |saved, index| saved.battle_orientation = match index { 1 => Orientation::Portrait, 2 => Orientation::Landscape, _ => Orientation::Automatic });
    setter!(on_quality_set, |saved, value| saved.image_quality = Some(value.clamp(15, 100) as u8));
    setter!(on_fullscreen_set, |saved, value| saved.fullscreen = value);
    setter!(on_cat_god_set, |saved, index| saved.config.cat_god = CatGod::ALL.get(index.max(0) as usize).copied().unwrap_or_default());
    setter!(on_speed_set, |saved, index| saved.config.start_speed = StartSpeed::SELECTABLE.get(index.max(0) as usize).copied().unwrap_or_default());

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_altar_edited(move |entry| {
        if typable(&entry)
            && let Some(ui) = weak.upgrade()
        {
            apply(&ui, &held, &ctx, |saved| saved.config.altar_level = entry.to_string());
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_arc_edited(move |arc, value| {
        if let Some(ui) = weak.upgrade() {
            apply(&ui, &held, &ctx, |saved| saved.config.treasure.set_arc(arc.max(0) as usize, value.to_string()));
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_fold(move |slot| {
        let slot = slot.max(0) as usize;

        {
            let mut held_state = held.borrow_mut();

            if !held_state.folds.remove(&slot) {
                held_state.folds.insert(slot);
            }
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_grade(move |slot, stage, level| {
        if let Some(ui) = weak.upgrade() {
            apply(&ui, &held, &ctx, |saved| saved.config.treasure.set_level(slot.max(0) as usize, stage.max(0) as usize, level));
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_tech_edited(move |index, entry| {
        if typable(&entry)
            && let Some(ui) = weak.upgrade()
        {
            apply(&ui, &held, &ctx, |saved| {
                if let Some(held) = saved.config.techs.get_mut(index.max(0) as usize) {
                    *held = entry.to_string();
                }
            });
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_cannon_set(move |index| {
        let (cannons, _, _) = labeled(&held, &ctx);

        if let (Some(ui), Some((id, _))) = (weak.upgrade(), cannons.get(index.max(0) as usize)) {
            let id = *id;

            apply(&ui, &held, &ctx, |saved| saved.config.cannon = Some(id));
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_style_set(move |index| {
        let (_, styles, _) = labeled(&held, &ctx);

        if let (Some(ui), Some((id, _))) = (weak.upgrade(), styles.get(index.max(0) as usize)) {
            let id = *id;

            apply(&ui, &held, &ctx, |saved| saved.config.style = Some(id));
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_foundation_set(move |index| {
        let (_, _, foundations) = labeled(&held, &ctx);

        if let (Some(ui), Some((id, _))) = (weak.upgrade(), foundations.get(index.max(0) as usize)) {
            let id = *id;

            apply(&ui, &held, &ctx, |saved| saved.config.foundation = Some(id));
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_base_level_edited(move |index, entry| {
        if !typable(&entry) {
            return;
        }

        if let Some(ui) = weak.upgrade() {
            apply(&ui, &held, &ctx, |saved| match index {
                0 => saved.config.cannon_level = entry.to_string(),
                1 => saved.config.style_level = entry.to_string(),
                2 => saved.config.foundation_level = entry.to_string(),
                _ => saved.config.castle_level = entry.to_string(),
            });
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_item_toggled(move |item, on| {
        if let Some(ui) = weak.upgrade() {
            apply(&ui, &held, &ctx, |saved| {
                if let Some(off) = saved.config.items_off.get_mut(item.max(0) as usize) {
                    *off = !on;
                }
            });
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_filibuster_set(move |index| {
        let slots = held.borrow().bundle.as_ref().map(|bundle| invadable(&bundle.registry, Category::CatsOfTheCosmos)).unwrap_or_default();

        if let (Some(ui), Some((slot, _))) = (weak.upgrade(), slots.get(index.max(0) as usize)) {
            let slot = *slot;

            apply(&ui, &held, &ctx, |saved| saved.config.invasions.filibuster = slot);
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_outbreak_set(move |index| {
        let slots = held.borrow().bundle.as_ref().map(|bundle| invadable(&bundle.registry, Category::ZombieOutbreaks)).unwrap_or_default();

        if let (Some(ui), Some((slot, _))) = (weak.upgrade(), slots.get(index.max(0) as usize)) {
            let slot = *slot;

            apply(&ui, &held, &ctx, |saved| saved.config.invasions.filibuster_outbreak = slot);
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_tutorial_toggled(move |step, cleared| {
        if let Some(ui) = weak.upgrade() {
            apply(&ui, &held, &ctx, |saved| set_tutorial_step(&mut saved.config.tutorial, step.max(0) as usize, cleared));
        }
    });
}

fn strings(items: impl IntoIterator<Item = String>) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(items.into_iter().map(SharedString::from).collect::<Vec<_>>()))
}

pub fn sync(ui: &AppWindow, state: &Shared, context: &Rc<Context>, staging: Option<&Staging>) {
    let sandbox = ui.global::<Sandbox>();
    let held = state.borrow();
    let saved = held.saved();

    sandbox.set_panel(i32::from(saved.panel));
    sandbox.set_music(Volume::nearest(saved.music_volume).index());
    sandbox.set_effects(Volume::nearest(saved.effects_volume).index());
    sandbox.set_vibration(if saved.vibrate { 0 } else { 1 });
    sandbox.set_formation(if saved.two_rows { 1 } else { 0 });
    sandbox.set_device(if saved.device == Device::Phone { 1 } else { 0 });
    sandbox.set_scaling_enabled(!saved.native());
    sandbox.set_scaling(if saved.scaling() == Scaling::Pad { 1 } else { 0 });
    sandbox.set_lag(if saved.lag == Lag::CatchUp { 0 } else { 1 });
    sandbox.set_battle_orientation(match saved.battle_orientation {
        Orientation::Automatic => 0,
        Orientation::Portrait => 1,
        Orientation::Landscape => 2,
    });
    sandbox.set_quality(i32::from(saved.quality()));
    sandbox.set_fullscreen(saved.fullscreen);

    if let Some(globals) = context.globals() {
        let (arcs, chapters, grades) = settings::treasure_models(&saved.config.treasure, &globals.catalog, &held.folds);

        sandbox.set_arcs(ModelRc::new(VecModel::from(arcs)));
        sandbox.set_chapters(ModelRc::new(VecModel::from(chapters)));
        sandbox.set_grades(ModelRc::new(VecModel::from(grades)));
    }

    let techs: Vec<ConfigEntry> = TECHS.iter().enumerate().map(|(index, tech)| ConfigEntry { label: tech.name.into(), hint: tech.hint().into(), value: saved.config.techs[index].as_str().into() }).collect();

    sandbox.set_techs(ModelRc::new(VecModel::from(techs)));
    sandbox.set_cat_god(CatGod::ALL.iter().position(|god| *god == saved.config.cat_god).map_or(1, |index| index as i32));
    sandbox.set_altar(saved.config.altar_level.as_str().into());

    let config = saved.config.clone();

    drop(saved);
    drop(held);

    let (cannons, styles, foundations) = labeled(state, context);
    let parts = super::parts(state, context);
    let position = |offered: &[(i32, String)], id: Option<i32>| offered.iter().position(|(part, _)| *part == id.unwrap_or(0)).map_or(0, |index| index as i32);
    let highest = |parts: &std::collections::BTreeMap<i32, i32>, id: Option<i32>| parts.get(&id.unwrap_or(0)).copied().unwrap_or(1).to_string();

    sandbox.set_cannon(position(&cannons, config.cannon));
    sandbox.set_style(position(&styles, config.style));
    sandbox.set_foundation(position(&foundations, config.foundation));
    sandbox.set_cannons(strings(cannons.into_iter().map(|(_, label)| label)));
    sandbox.set_styles(strings(styles.into_iter().map(|(_, label)| label)));
    sandbox.set_foundations(strings(foundations.into_iter().map(|(_, label)| label)));

    let hints = [highest(&parts.cannons, config.cannon), highest(&parts.styles, config.style), highest(&parts.foundations, config.foundation), parts.castle.to_string()];
    let values = [&config.cannon_level, &config.style_level, &config.foundation_level, &config.castle_level];
    let levels: Vec<ConfigEntry> = BASE_LEVELS.iter().enumerate().map(|(index, label)| ConfigEntry { label: (*label).into(), hint: hints[index].as_str().into(), value: values[index].as_str().into() }).collect();

    sandbox.set_base_levels(ModelRc::new(VecModel::from(levels)));

    let usable = staging.map_or([true; 6], |staging| staging.usable);
    let speed_up = usable[0] && !config.items_off[0];
    let shown = if speed_up { StartSpeed::Triple } else { config.start_speed };

    sandbox.set_speed_enabled(!speed_up);
    sandbox.set_speeds(strings(if speed_up { vec![shown.to_string()] } else { StartSpeed::SELECTABLE.iter().map(ToString::to_string).collect() }));
    sandbox.set_speed(if speed_up { 0 } else { StartSpeed::SELECTABLE.iter().position(|speed| *speed == shown).map_or(0, |index| index as i32) });

    let items: Vec<ItemRow> = ITEMS
        .iter()
        .enumerate()
        .map(|(item, name)| ItemRow { label: (*name).into(), on: usable[item] && !config.items_off[item], allowed: usable[item] })
        .collect();

    sandbox.set_items(ModelRc::new(VecModel::from(items)));

    let (filibuster, outbreak) = state.borrow().bundle.as_ref().map_or((Vec::new(), Vec::new()), |bundle| {
        (invadable(&bundle.registry, Category::CatsOfTheCosmos), invadable(&bundle.registry, Category::ZombieOutbreaks))
    });
    let slot_index = |offered: &[(u32, String)], slot: u32| offered.iter().position(|(held, _)| *held == slot).map_or(-1, |index| index as i32);

    sandbox.set_filibuster(slot_index(&filibuster, config.invasions.filibuster));
    sandbox.set_outbreak(slot_index(&outbreak, config.invasions.filibuster_outbreak));
    sandbox.set_filibusters(strings(filibuster.into_iter().map(|(_, label)| label)));
    sandbox.set_outbreaks(strings(outbreak.into_iter().map(|(_, label)| label)));

    let tutorials: Vec<ItemRow> = TUTORIAL_STEPS
        .iter()
        .enumerate()
        .map(|(step, (label, _))| ItemRow { label: (*label).into(), on: tutorial_step(&config.tutorial, step), allowed: true })
        .collect();

    sandbox.set_tutorials(ModelRc::new(VecModel::from(tutorials)));
}
