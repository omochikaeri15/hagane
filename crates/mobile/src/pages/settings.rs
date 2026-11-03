use std::cell::RefCell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use slint::{ComponentHandle, ModelRc, SharedString, Timer, TimerMode, VecModel};
use tracing::{debug, error, info};

use codex::chapter::treasure::ARC_SLOTS;
use foundry::common::architecture;
use foundry::common::dirs;
use foundry::common::io::APP_LANGUAGES;
use foundry::common::keys::sanitize;
use foundry::domains::cat::files as cat_files;
use foundry::domains::import;
use foundry::domains::settings::{ExceptionList, ExceptionRule, FrameCount, ImportStructure, ReplaySource, RuleHandling, UpdateMode, UserKeys, Utf8Mode, lang};
use foundry::systems::treasure::{self, ARCS, Catalog};

use crate::pages::Context;
use crate::settings::{Exports, Orientation};
use crate::{AppWindow, ArcRow, ChapterFold, KeyRow, LangRow, LangToggle, RuleRow, Settings, Tone, TreasureItem, TreasureSet, bridge, delivery, picker, platform};

const FEEDBACK_MS: u64 = 2000;
const CONFIRM_MS: u64 = 3000;
const ENTRY_DIGITS: usize = 4;
const LICENSES: &str = include_str!("../../../foundry/assets/licenses.txt");
const CHAPTER_TITLES: [[&str; 3]; 3] = [
    ["Empire of Cats Ch. 1", "Empire of Cats Ch. 2", "Empire of Cats Ch. 3"],
    ["Into the Future Ch. 1", "Into the Future Ch. 2", "Into the Future Ch. 3"],
    ["Cats of the Cosmos Ch. 1", "Cats of the Cosmos Ch. 2", "Cats of the Cosmos Ch. 3"],
];
const REGION_LABELS: [&str; 4] = ["Japan", "Global", "Taiwan", "Korea"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Target {
    Game,
    Cache,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    Deleting,
    Done,
}

pub struct State {
    tab: usize,
    folds: HashSet<usize>,
    keys: UserKeys,
    validation: Option<[(bool, bool); 4]>,
    keys_load: Option<bool>,
    keys_export: Option<bool>,
    keys_confirm: bool,
    rules: Vec<ExceptionRule>,
    rules_load: Option<bool>,
    rules_export: Option<bool>,
    rules_confirm: bool,
    languages_for: Option<usize>,
    game: Phase,
    cache: Phase,
    confirm: Option<Target>,
    sizes: Option<(u64, u64)>,
    scanning: bool,
    default_level: String,
    showcase: [String; 3],
    timers: Vec<Timer>,
}

pub type Shared = Rc<RefCell<State>>;

fn format_size(size: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let size = size as f64;

    if size >= GB {
        format!("{:.1} GB", size / GB)
    } else if size >= MB {
        format!("{:.1} MB", size / MB)
    } else if size >= KB {
        format!("{:.1} KB", size / KB)
    } else {
        format!("{size:.1} B")
    }
}

fn folder_size(path: &Path) -> u64 {
    let mut size = 0;

    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                if metadata.is_dir() {
                    size += folder_size(&entry.path());
                } else {
                    size += metadata.len();
                }
            }
        }
    }

    size
}

fn typable(entry: &str) -> bool {
    entry.len() <= ENTRY_DIGITS && entry.chars().all(|glyph| glyph.is_ascii_digit() || glyph == '%')
}

fn later(state: &Shared, ui: &AppWindow, context: &Rc<Context>, ms: u64, change: impl FnOnce(&mut State) + 'static) {
    let timer = Timer::default();
    let landing = Rc::clone(state);
    let ctx = Rc::clone(context);
    let weak = ui.as_weak();

    let mut pending = Some(change);

    timer.start(TimerMode::SingleShot, Duration::from_millis(ms), move || {
        if let Some(change) = pending.take() {
            change(&mut landing.borrow_mut());
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &landing, &ctx);
        }
    });

    let mut held = state.borrow_mut();

    held.timers.retain(Timer::running);
    held.timers.push(timer);
}

impl State {
    fn new(context: &Rc<Context>) -> Self {
        let settings = context.state.settings();

        Self {
            tab: usize::from(context.memory.get().settings_tab).min(9),
            folds: HashSet::new(),
            keys: UserKeys::load(),
            validation: None,
            keys_load: None,
            keys_export: None,
            keys_confirm: false,
            rules: ExceptionList::load_or_default().rules,
            rules_load: None,
            rules_export: None,
            rules_confirm: false,
            languages_for: None,
            game: Phase::Idle,
            cache: Phase::Idle,
            confirm: None,
            sizes: None,
            scanning: false,
            default_level: settings.cat_data.default_level.to_string(),
            showcase: [settings.animation.default_showcase_walk.to_string(), settings.animation.default_showcase_idle.to_string(), settings.animation.default_showcase_kb.to_string()],
            timers: Vec::new(),
        }
    }

    fn region_mut(&mut self, slot: usize) -> &mut foundry::domains::settings::RegionKey {
        match slot {
            0 => &mut self.keys.ja,
            1 => &mut self.keys.en,
            2 => &mut self.keys.tw,
            _ => &mut self.keys.ko,
        }
    }

    fn save_rules(&mut self) {
        let mut list = ExceptionList { rules: self.rules.clone(), ..Default::default() };

        list.save();
    }
}

pub fn treasure_models(treasures: &treasure::Config, catalog: &Catalog, folds: &HashSet<usize>) -> (Vec<ArcRow>, Vec<ChapterFold>, Vec<SharedString>) {
    let arcs: Vec<ArcRow> = ARCS.iter().enumerate().map(|(arc, name)| ArcRow { name: (*name).into(), value: treasures.arcs[arc].as_str().into(), tuned: treasures.is_tuned(arc) }).collect();
    let mut chapters = Vec::new();

    for (arc, slots) in ARC_SLOTS.iter().enumerate() {
        for (chapter, slot) in slots.iter().copied().enumerate() {
            let open = folds.contains(&slot);
            let groups = catalog.groups(slot);
            let sets: Vec<TreasureSet> = if open {
                groups
                    .iter()
                    .enumerate()
                    .filter(|(_, set)| set.members().next().is_some())
                    .map(|(group, set)| {
                        let items: Vec<TreasureItem> = set
                            .members()
                            .filter_map(|stage| {
                                let index = usize::try_from(stage).ok()?;
                                let label = catalog.item(slot, stage).map_or_else(|| format!("Treasure {}", stage + 1), str::to_owned);

                                Some(TreasureItem { label: label.into(), level: treasures.level(slot, index), stage: index as i32 })
                            })
                            .collect();

                        TreasureSet {
                            name: catalog.set(slot, group).map_or_else(|| format!("Set {}", group + 1), str::to_owned).into(),
                            effect: catalog.effect(slot, group).unwrap_or_default().into(),
                            items: ModelRc::new(VecModel::from(items)),
                        }
                    })
                    .collect()
            } else {
                Vec::new()
            };

            chapters.push(ChapterFold { title: CHAPTER_TITLES[arc][chapter].into(), slot: slot as i32, open, sets: ModelRc::new(VecModel::from(sets)), empty: groups.is_empty() });
        }
    }

    let grades = catalog.grades.iter().map(|grade| SharedString::from(grade.as_str())).collect();

    (arcs, chapters, grades)
}

pub fn wire(window: &AppWindow, context: &Rc<Context>) -> Shared {
    let state: Shared = Rc::new(RefCell::new(State::new(context)));
    let settings = window.global::<Settings>();

    settings.set_licenses(LICENSES.into());
    settings.set_forms(ModelRc::new(VecModel::from((0..cat_files::FORM_COUNT).map(|form| SharedString::from(cat_files::form_name(form))).collect::<Vec<_>>())));
    settings.set_data_path(std::env::current_dir().map_or_else(|_| "unknown".into(), |path| path.display().to_string().into()));

    let on = |state: &Shared, context: &Rc<Context>, window: &AppWindow| (Rc::clone(state), Rc::clone(context), window.as_weak());

    let (held, ctx, weak) = on(&state, context, window);
    settings.on_tab_set(move |index| {
        held.borrow_mut().tab = index.max(0) as usize;
        ctx.memory.update(|memory| memory.settings_tab = index.clamp(0, 9) as u8);

        if let Some(ui) = weak.upgrade() {
            if index == 5 {
                measure(&ui, &held, &ctx);
            }

            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    settings.on_source_mode_set(move |index| {
        ctx.state.apply(|settings| settings.files.source_mode = if index == 0 { UpdateMode::Prompt } else { UpdateMode::Ignore });

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    settings.on_toggle_logging(move |value| {
        ctx.state.apply(|settings| settings.general.enable_logging = value);

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    settings.on_lang_move(move |from, to| {
        let (held, ctx, weak) = (held.clone(), ctx.clone(), weak.clone());

        Timer::single_shot(Duration::ZERO, move || {
            if let Some(ui) = weak.upgrade() {
                ui.global::<Settings>().set_lang_drag(-1);
                reorder(&ui, &held, &ctx, from.max(0) as usize, to.max(0) as usize);
            }
        });
    });

    let (held, ctx, weak) = on(&state, context, window);
    settings.on_lang_default(move || {
        let restored = lang::default_priority();
        let changed = ctx.state.settings().general.language_priority != restored;

        if changed {
            ctx.state.apply(|settings| settings.general.language_priority = restored);

            if let Some(ui) = weak.upgrade() {
                relanguage(&ui, &ctx);
                refresh(&ui, &held, &ctx);
            }
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    settings.on_arc_edited(move |arc, value| {
        let Some(ui) = weak.upgrade() else {
            return;
        };

        if typable(&value) {
            ctx.state.apply(|settings| settings.general.treasures.set_arc(arc.max(0) as usize, value.to_string()));
            ctx.retreasure(&ui);
        }

        refresh(&ui, &held, &ctx);
    });

    let (held, ctx, weak) = on(&state, context, window);
    settings.on_fold(move |slot| {
        {
            let mut state = held.borrow_mut();
            let slot = slot.max(0) as usize;

            if !state.folds.remove(&slot) {
                state.folds.insert(slot);
            }
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    settings.on_grade(move |slot, stage, level| {
        ctx.state.apply(|settings| settings.general.treasures.set_level(slot.max(0) as usize, stage.max(0) as usize, level));

        if let Some(ui) = weak.upgrade() {
            ctx.retreasure(&ui);
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    settings.on_toggle_enforce(move |value| {
        ctx.state.apply(|settings| settings.game_data.enforce_key_validation = value);

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    settings.on_open_keys(move || {
        {
            let mut state = held.borrow_mut();

            state.keys = UserKeys::load();
            state.validation = None;
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
            ui.global::<Settings>().set_keys_open(true);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    settings.on_key_changed(move |slot, value| {
        {
            let mut state = held.borrow_mut();

            state.region_mut(slot.max(0) as usize).key = sanitize(&value);
            state.validation = None;
            state.keys.save();
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    settings.on_iv_changed(move |slot, value| {
        {
            let mut state = held.borrow_mut();

            state.region_mut(slot.max(0) as usize).iv = sanitize(&value);
            state.validation = None;
            state.keys.save();
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    settings.on_keys_load(move || {
        if let Some(ui) = weak.upgrade() {
            let landing = Rc::clone(&held);

            ui.global::<Settings>().set_keys_open(false);
            picker::open(
                &ui,
                &ctx,
                "Select a Keys File",
                &["json"],
                Box::new(move |ui, ctx, path| {
                    let success = std::fs::read_to_string(&path)
                        .ok()
                        .and_then(|data| serde_json::from_str::<UserKeys>(&data).ok())
                        .map(|mut parsed| {
                            for region in [&mut parsed.ja, &mut parsed.en, &mut parsed.tw, &mut parsed.ko] {
                                region.key = sanitize(&region.key);
                                region.iv = sanitize(&region.iv);
                            }

                            let mut state = landing.borrow_mut();

                            state.keys = parsed;
                            state.validation = None;
                            state.keys.save();
                        })
                        .is_some();

                    landing.borrow_mut().keys_load = Some(success);
                    later(&landing, ui, ctx, FEEDBACK_MS, |state| state.keys_load = None);
                    ui.global::<Settings>().set_keys_open(true);
                    refresh(ui, &landing, ctx);
                }),
            );
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    settings.on_keys_export(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let export_dir = Path::new("exports");
        let _ = std::fs::create_dir_all(export_dir);
        let success = serde_json::to_string_pretty(&held.borrow().keys).ok().is_some_and(|json| std::fs::write(export_dir.join("keys.json"), json).is_ok());

        if success {
            delivery::announce(&ui, &ctx);
        }

        held.borrow_mut().keys_export = Some(success);
        later(&held, &ui, &ctx, FEEDBACK_MS, |state| state.keys_export = None);
        refresh(&ui, &held, &ctx);
    });

    let (held, ctx, weak) = on(&state, context, window);
    settings.on_keys_validate(move || {
        {
            let mut state = held.borrow_mut();

            state.validation = Some(state.keys.validate());
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    settings.on_keys_delete(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let armed = held.borrow().keys_confirm;

        if armed {
            let mut state = held.borrow_mut();

            state.keys = UserKeys::default();
            state.validation = None;
            state.keys.save();
            state.keys_confirm = false;
        } else {
            held.borrow_mut().keys_confirm = true;
            later(&held, &ui, &ctx, CONFIRM_MS, |state| state.keys_confirm = false);
        }

        refresh(&ui, &held, &ctx);
    });

    let (held, ctx, weak) = on(&state, context, window);
    settings.on_banner_form_set(move |index| {
        ctx.state.apply(|settings| settings.cat_data.preferred_banner_form = index.max(0) as usize);

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    macro_rules! toggle {
        ($callback:ident, $field:expr) => {{
            let (held, ctx, weak) = on(&state, context, window);

            settings.$callback(move |value| {
                ctx.state.apply(|settings| $field(settings, value));

                if let Some(ui) = weak.upgrade() {
                    refresh(&ui, &held, &ctx);
                }
            });
        }};
    }

    toggle!(on_toggle_invalid_cats, |settings: &mut foundry::domains::settings::Settings, value| settings.cat_data.show_invalid_cats = value);
    toggle!(on_toggle_expand_spirit, |settings: &mut foundry::domains::settings::Settings, value| settings.cat_data.expand_spirit_details = value);
    toggle!(on_toggle_auto_level, |settings: &mut foundry::domains::settings::Settings, value| settings.cat_data.auto_level_calculations = value);
    toggle!(on_toggle_bump_ultra, |settings: &mut foundry::domains::settings::Settings, value| settings.cat_data.bump_ultra_60 = value);
    toggle!(on_toggle_invalid_enemies, |settings: &mut foundry::domains::settings::Settings, value| settings.enemy_data.show_invalid_enemies = value);
    toggle!(on_toggle_force_names, |settings: &mut foundry::domains::settings::Settings, value| settings.mods.force_name_fallbacks = value);
    toggle!(on_toggle_unlock_game, |settings: &mut foundry::domains::settings::Settings, value| settings.files.unlock_game_mount = value);
    toggle!(on_toggle_ignore_modified, |settings: &mut foundry::domains::settings::Settings, value| settings.game_data.ignore_modified_app = value);
    toggle!(on_toggle_auto_camera, |settings: &mut foundry::domains::settings::Settings, value| settings.animation.auto_set_camera_region = value);

    let (held, ctx, weak) = on(&state, context, window);
    settings.on_default_level_edited(move |value| {
        held.borrow_mut().default_level = value.to_string();

        if let Ok(parsed) = value.parse::<i32>() {
            ctx.state.apply(|settings| settings.cat_data.default_level = parsed);
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    settings.on_utf8_mode_set(move |index| {
        ctx.state.apply(|settings| settings.files.utf8_mode = if index == 1 { Utf8Mode::Virtual } else { Utf8Mode::Fill });

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    settings.on_structure_set(move |index| {
        ctx.state.apply(|settings| settings.game_data.import_structure = if index == 1 { ImportStructure::Flat } else { ImportStructure::Bcc });

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    settings.on_frame_count_set(move |index| {
        ctx.state.apply(|settings| settings.utilities.frame_count = if index == 1 { FrameCount::Continuous } else { FrameCount::Automatic });

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    settings.on_showcase_edited(move |which, value| {
        if let Some(slot) = held.borrow_mut().showcase.get_mut(which.clamp(0, 2) as usize) {
            *slot = value.to_string();
        }

        if let Ok(parsed) = value.parse::<i32>() {
            ctx.state.apply(|settings| match which {
                0 => settings.animation.default_showcase_walk = parsed,
                1 => settings.animation.default_showcase_idle = parsed,
                _ => settings.animation.default_showcase_kb = parsed,
            });
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    settings.on_open_exceptions(move || {
        held.borrow_mut().rules = ExceptionList::load_or_default().rules;

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
            ui.global::<Settings>().set_exceptions_open(true);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    settings.on_rule_add(move || {
        {
            let mut state = held.borrow_mut();

            state.rules.push(ExceptionRule::default());
            state.save_rules();
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    settings.on_rule_remove(move |index| {
        {
            let mut state = held.borrow_mut();
            let index = index.max(0) as usize;

            if index < state.rules.len() {
                state.rules.remove(index);
                state.save_rules();
            }
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, _, _) = on(&state, context, window);
    settings.on_rule_pattern(move |index, value| {
        let mut state = held.borrow_mut();

        if let Some(rule) = state.rules.get_mut(index.max(0) as usize) {
            rule.pattern = value.to_string();
        }

        state.save_rules();
    });

    let (held, _, _) = on(&state, context, window);
    settings.on_rule_extension(move |index, value| {
        let mut state = held.borrow_mut();

        if let Some(rule) = state.rules.get_mut(index.max(0) as usize) {
            rule.extension = value.to_string();
        }

        state.save_rules();
    });

    let (held, ctx, weak) = on(&state, context, window);
    settings.on_rule_handling(move |index, handling| {
        {
            let mut state = held.borrow_mut();

            if let Some(rule) = state.rules.get_mut(index.max(0) as usize) {
                rule.handling = match handling {
                    1 => RuleHandling::Only,
                    2 => RuleHandling::Ignore,
                    _ => RuleHandling::Include,
                };
            }

            state.save_rules();
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    settings.on_open_rule_languages(move |index| {
        held.borrow_mut().languages_for = Some(index.max(0) as usize);

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
            ui.global::<Settings>().set_languages_open(true);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    settings.on_rule_language_toggled(move |index, enabled| {
        {
            let mut state = held.borrow_mut();
            let Some(rule_index) = state.languages_for else {
                return;
            };
            let Some((code, _)) = APP_LANGUAGES.get(index.max(0) as usize) else {
                return;
            };

            if let Some(rule) = state.rules.get_mut(rule_index) {
                rule.languages.insert((*code).to_owned(), enabled);
            }

            state.save_rules();
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    settings.on_rules_load(move || {
        if let Some(ui) = weak.upgrade() {
            let landing = Rc::clone(&held);

            ui.global::<Settings>().set_exceptions_open(false);
            picker::open(
                &ui,
                &ctx,
                "Select an Exceptions File",
                &["json"],
                Box::new(move |ui, ctx, path| {
                    let success = ExceptionList::load_from_file(&path)
                        .map(|list| {
                            let mut state = landing.borrow_mut();

                            state.rules = list.rules;
                            state.save_rules();
                        })
                        .is_ok();

                    landing.borrow_mut().rules_load = Some(success);
                    later(&landing, ui, ctx, FEEDBACK_MS, |state| state.rules_load = None);
                    ui.global::<Settings>().set_exceptions_open(true);
                    refresh(ui, &landing, ctx);
                }),
            );
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    settings.on_rules_export(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let export_dir = Path::new("exports");
        let _ = std::fs::create_dir_all(export_dir);
        let mut list = ExceptionList { rules: held.borrow().rules.clone(), ..Default::default() };
        let success = list.save_to_file(&export_dir.join("exceptions.json")).is_ok();

        if success {
            delivery::announce(&ui, &ctx);
        }

        held.borrow_mut().rules_export = Some(success);
        later(&held, &ui, &ctx, FEEDBACK_MS, |state| state.rules_export = None);
        refresh(&ui, &held, &ctx);
    });

    let (held, ctx, weak) = on(&state, context, window);
    settings.on_rules_reset(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let armed = held.borrow().rules_confirm;

        if armed {
            let mut state = held.borrow_mut();

            state.rules = ExceptionList::default().rules;
            state.save_rules();
            state.rules_confirm = false;
        } else {
            held.borrow_mut().rules_confirm = true;
            later(&held, &ui, &ctx, CONFIRM_MS, |state| state.rules_confirm = false);
        }

        refresh(&ui, &held, &ctx);
    });

    let (held, ctx, weak) = on(&state, context, window);
    settings.on_delete_game(move || {
        if let Some(ui) = weak.upgrade() {
            delete(&ui, &held, &ctx, Target::Game);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    settings.on_clear_cache(move || {
        if let Some(ui) = weak.upgrade() {
            delete(&ui, &held, &ctx, Target::Cache);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    settings.on_replay_source_set(move |index| {
        ctx.state.apply(|settings| settings.sandbox.replay_source = ReplaySource::ALL.get(index.max(0) as usize).copied().unwrap_or_default());

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    settings.on_sandbox_banner_form_set(move |index| {
        ctx.state.apply(|settings| settings.sandbox.banner_form = index.clamp(0, 3) as usize);

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    settings.on_toggle_disable_replays(move |value| {
        ctx.state.apply(|settings| settings.sandbox.disable_replays = value);

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    settings.on_exports_set(move |index| {
        ctx.state.set_exports(if index == 1 { Exports::Ask } else { Exports::AppFolder });

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    settings.on_orientation_set(move |index| {
        let orientation = match index {
            1 => Orientation::Portrait,
            2 => Orientation::Landscape,
            _ => Orientation::Automatic,
        };

        ctx.state.set_orientation(orientation);
        platform::set_orientation(orientation);

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    refresh(window, &state, context);

    state
}

fn reorder(ui: &AppWindow, state: &Shared, context: &Rc<Context>, from: usize, to: usize) {
    let mut changed = false;

    context.state.apply(|settings| {
        let priority = &mut settings.general.language_priority;

        if from != to && from < priority.len() && to < priority.len() {
            let code = priority.remove(from);
            priority.insert(to, code);
            changed = true;
        }
    });

    if changed {
        relanguage(ui, context);
    }

    refresh(ui, state, context);
}

fn relanguage(ui: &AppWindow, context: &Rc<Context>) {
    let config = context.state.settings().scanner_config(context.engine.mounted());

    info!("Language priority changed, the file index is rebuilt");
    context.engine.rebuild(ui, config);
}

fn measure(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    if state.borrow().scanning {
        return;
    }

    state.borrow_mut().scanning = true;

    let landing = Rc::clone(state);
    let ctx = Rc::clone(context);

    bridge::offload(
        ui,
        "disk_sizes",
        || (folder_size(Path::new(architecture::GAME)), dirs::cache_path().map_or(0, |dir| folder_size(&dir))),
        move |ui, sizes| {
            {
                let mut held = landing.borrow_mut();

                held.sizes = Some(sizes);
                held.scanning = false;
            }

            refresh(ui, &landing, &ctx);
        },
    );
}

fn delete(ui: &AppWindow, state: &Shared, context: &Rc<Context>, target: Target) {
    let armed = state.borrow().confirm == Some(target);

    if !armed {
        state.borrow_mut().confirm = Some(target);
        later(state, ui, context, CONFIRM_MS, |held| held.confirm = None);
        refresh(ui, state, context);
        return;
    }

    let path = match target {
        Target::Game => PathBuf::from(architecture::GAME),
        Target::Cache => match dirs::cache_path() {
            Some(dir) => dir,
            None => return,
        },
    };

    {
        let mut held = state.borrow_mut();

        held.confirm = None;

        match target {
            Target::Game => held.game = Phase::Deleting,
            Target::Cache => held.cache = Phase::Deleting,
        }
    }

    refresh(ui, state, context);
    debug!(path = %path.display(), "Deleting a folder");

    let landing = Rc::clone(state);
    let ctx = Rc::clone(context);

    bridge::offload(
        ui,
        "disk_delete",
        move || {
            if let Err(err) = std::fs::remove_dir_all(&path) {
                error!(path = %path.display(), "Failed to delete folder: {err}");
                return false;
            }

            if target == Target::Game {
                import::forget_imports();
            }

            true
        },
        move |ui, _| {
            {
                let mut held = landing.borrow_mut();

                match target {
                    Target::Game => held.game = Phase::Done,
                    Target::Cache => held.cache = Phase::Done,
                }
            }

            if target == Target::Game {
                ctx.state.apply(|settings| settings.files.source_stamp = None);
                ctx.images.forget_all();

                let config = ctx.state.settings().scanner_config(ctx.engine.mounted());

                ctx.engine.rebuild(ui, config);
            }

            later(&landing, ui, &ctx, FEEDBACK_MS, move |held| match target {
                Target::Game => held.game = Phase::Idle,
                Target::Cache => held.cache = Phase::Idle,
            });
            measure(ui, &landing, &ctx);
            refresh(ui, &landing, &ctx);
        },
    );
}

fn feedback(face: Option<bool>, idle: &str, done: &str) -> (SharedString, Tone) {
    match face {
        Some(true) => (done.into(), Tone::StatusSuccess),
        Some(false) => ("Failed!".into(), Tone::StatusDanger),
        None => (idle.into(), Tone::Primary),
    }
}

pub fn invalidate(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    refresh(ui, state, context);
}

fn refresh(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let settings = ui.global::<Settings>();
    let held = state.borrow();
    let core = context.state.settings();

    settings.set_tab(held.tab as i32);
    settings.set_logging(core.general.enable_logging);

    let none_index = core.general.language_priority.iter().position(|code| code == "--");
    let languages: Vec<LangRow> = core
        .general
        .language_priority
        .iter()
        .enumerate()
        .map(|(index, code)| LangRow { label: lang::get_label_for_code(code).into(), weak: none_index.is_some_and(|none| index > none), none: Some(index) == none_index })
        .collect();

    settings.set_languages(ModelRc::new(VecModel::from(languages)));

    if let Some(globals) = context.globals() {
        let (arcs, chapters, grades) = treasure_models(&core.general.treasures, &globals.catalog, &held.folds);

        settings.set_arcs(ModelRc::new(VecModel::from(arcs)));
        settings.set_chapters(ModelRc::new(VecModel::from(chapters)));
        settings.set_grades(ModelRc::new(VecModel::from(grades)));
    }

    settings.set_enforce_keys(core.game_data.enforce_key_validation);

    let validation = held.validation.unwrap_or([(true, true); 4]);
    let regions = [&held.keys.ja, &held.keys.en, &held.keys.tw, &held.keys.ko];
    let keys: Vec<KeyRow> = regions
        .iter()
        .enumerate()
        .map(|(index, region)| KeyRow { region: REGION_LABELS[index].into(), key: region.key.as_str().into(), iv: region.iv.as_str().into(), key_valid: validation[index].0, iv_valid: validation[index].1 })
        .collect();
    let (load_label, load_tone) = feedback(held.keys_load, "Load Keys", "Loaded!");
    let (export_label, export_tone) = feedback(held.keys_export, "Export Keys", "Exported!");

    settings.set_keys(ModelRc::new(VecModel::from(keys)));
    settings.set_keys_validated(held.validation.is_some());
    settings.set_keys_load_label(load_label);
    settings.set_keys_load_tone(load_tone);
    settings.set_keys_export_label(export_label);
    settings.set_keys_export_tone(export_tone);
    settings.set_keys_delete_label(if held.keys_confirm { "Continue?".into() } else { "Delete Keys".into() });

    settings.set_banner_form(core.cat_data.preferred_banner_form as i32);
    settings.set_invalid_cats(core.cat_data.show_invalid_cats);
    settings.set_expand_spirit(core.cat_data.expand_spirit_details);
    settings.set_default_level(held.default_level.as_str().into());
    settings.set_auto_level(core.cat_data.auto_level_calculations);
    settings.set_bump_ultra(core.cat_data.bump_ultra_60);
    settings.set_invalid_enemies(core.enemy_data.show_invalid_enemies);
    settings.set_force_names(core.mods.force_name_fallbacks);
    settings.set_unlock_game(core.files.unlock_game_mount);
    settings.set_utf8_mode(if core.files.utf8_mode == Utf8Mode::Virtual { 1 } else { 0 });
    settings.set_source_mode(if core.files.source_mode == UpdateMode::Prompt { 0 } else { 1 });
    settings.set_import_structure(if core.game_data.import_structure == ImportStructure::Flat { 1 } else { 0 });
    settings.set_ignore_modified(core.game_data.ignore_modified_app);
    settings.set_frame_count(if core.utilities.frame_count == FrameCount::Continuous { 1 } else { 0 });
    settings.set_auto_camera(core.animation.auto_set_camera_region);
    settings.set_showcase_walk(held.showcase[0].as_str().into());
    settings.set_showcase_idle(held.showcase[1].as_str().into());
    settings.set_showcase_kb(held.showcase[2].as_str().into());

    let rules: Vec<RuleRow> = held
        .rules
        .iter()
        .map(|rule| RuleRow {
            pattern: rule.pattern.as_str().into(),
            extension: rule.extension.as_str().into(),
            handling: match rule.handling {
                RuleHandling::Include => 0,
                RuleHandling::Only => 1,
                RuleHandling::Ignore => 2,
            },
            languages: format!("Languages ({})", rule.languages.values().filter(|enabled| **enabled).count()).into(),
        })
        .collect();
    let (rules_load_label, rules_load_tone) = feedback(held.rules_load, "Load List", "Imported!");
    let (rules_export_label, rules_export_tone) = feedback(held.rules_export, "Export List", "Exported!");
    let toggles: Vec<LangToggle> = held
        .languages_for
        .and_then(|index| held.rules.get(index))
        .map(|rule| APP_LANGUAGES.iter().map(|(code, _)| LangToggle { code: code.to_uppercase().into(), enabled: rule.languages.get(*code).copied().unwrap_or(false) }).collect())
        .unwrap_or_default();

    settings.set_rules(ModelRc::new(VecModel::from(rules)));
    settings.set_rules_load_label(rules_load_label);
    settings.set_rules_load_tone(rules_load_tone);
    settings.set_rules_export_label(rules_export_label);
    settings.set_rules_export_tone(rules_export_tone);
    settings.set_rules_reset_label(if held.rules_confirm { "Continue?".into() } else { "Set to Default".into() });
    settings.set_rule_languages(ModelRc::new(VecModel::from(toggles)));

    let game_exists = architecture::game_present();
    let cache_present = dirs::cache_path().is_some_and(|dir| architecture::has_content(&dir));
    let size_text = |size: Option<u64>| -> SharedString {
        match (held.scanning, size) {
            (true, _) => "Scanning file sizes...".into(),
            (false, Some(bytes)) => format_size(bytes).into(),
            (false, None) => "Unknown file size".into(),
        }
    };
    let (game_label, game_tone, game_enabled) = match held.game {
        Phase::Deleting => ("Deleting \"game\"...".to_owned(), Tone::StatusWarning, false),
        Phase::Done => ("Deleted \"game\"!".to_owned(), Tone::StatusSuccess, false),
        Phase::Idle if game_exists => (if held.confirm == Some(Target::Game) { "Continue?".to_owned() } else { "Delete \"game\"".to_owned() }, Tone::Danger, true),
        Phase::Idle => ("No \"game\"".to_owned(), Tone::Neutral, false),
    };
    let (cache_label, cache_tone, cache_enabled) = match held.cache {
        Phase::Deleting => ("Clearing Cache...".to_owned(), Tone::StatusWarning, false),
        Phase::Done => ("Cache Cleared!".to_owned(), Tone::StatusSuccess, false),
        Phase::Idle if cache_present => (if held.confirm == Some(Target::Cache) { "Continue?".to_owned() } else { "Clear Cache".to_owned() }, Tone::Danger, true),
        Phase::Idle => ("Cache Empty".to_owned(), Tone::Neutral, false),
    };

    settings.set_game_label(game_label.into());
    settings.set_game_tone(game_tone);
    settings.set_game_enabled(game_enabled);
    settings.set_game_size(size_text(held.sizes.map(|sizes| sizes.0)));
    settings.set_cache_label(cache_label.into());
    settings.set_cache_tone(cache_tone);
    settings.set_cache_enabled(cache_enabled);
    settings.set_cache_size(size_text(held.sizes.map(|sizes| sizes.1)));
    settings.set_orientation(match context.state.orientation() {
        Orientation::Automatic => 0,
        Orientation::Portrait => 1,
        Orientation::Landscape => 2,
    });
    settings.set_exports(if context.state.exports() == Exports::Ask { 1 } else { 0 });
    settings.set_replay_source(ReplaySource::ALL.iter().position(|source| *source == core.sandbox.replay_source).map_or(0, |index| index as i32));
    settings.set_sandbox_banner_form(core.sandbox.banner_form.min(3) as i32);
    settings.set_disable_replays(core.sandbox.disable_replays);
}
