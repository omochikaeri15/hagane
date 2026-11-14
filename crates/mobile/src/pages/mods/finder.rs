use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use rustc_hash::{FxHashMap, FxHashSet};
use slint::{ComponentHandle, Model, ModelNotify, ModelRc, ModelTracker, Timer, TimerMode};
use tracing::warn;

use foundry::common::{architecture, game};
use foundry::domains::cat::scanner::CatEntry;
use foundry::domains::enemy::scanner::EnemyEntry;
use foundry::domains::mods;
use foundry::domains::settings::EditorMode;
use foundry::domains::stage::scanner::StageBundle;
use foundry::systems::editor::finder::{words, Catalog, Entry, Filter, Line};
use foundry::systems::editor::{figures, file_mark, file_reset, plans, prose, Mark, Pick, Reader, Sources};

use crate::pages::{files, stages, Context};
use crate::{AppWindow, Finder, FinderRow, Page, Shell, bridge};

use super::form;

const PLACE_FAILED: &str = "This file could not be added to the mod";
const SYNC_FAILED: &str = "This entry could not be synced";
const RAW_ACTION: &str = "Edit File";
const SYNC_LABEL: &str = "Sync With \"game\"";
const CONFIRM_LABEL: &str = "Continue?";
const CONFIRM_MS: u64 = 3000;
const UNMARKED: i32 = 0;
const CHANGED: i32 = 1;
const ADDED: i32 = 2;

type Scans = (Arc<Vec<CatEntry>>, Arc<Vec<EnemyEntry>>, Arc<StageBundle>);

#[derive(Default)]
pub struct State {
    catalog: Catalog,
    filter: Filter,
    lines: Vec<Line>,
    query: String,
    expanded: FxHashSet<u32>,
    collapsed: FxHashSet<u32>,
    picked: Option<u32>,
    values: EditorMode,
    open: bool,
    built: bool,
    building: bool,
    source: Option<Scans>,
    marks: FxHashMap<u32, Mark>,
    wanted: FxHashSet<u32>,
    marking: Timer,
    confirming: bool,
    confirm: Timer,
}

impl State {
    fn relayout(&mut self) {
        let searching = self.filter.searching();
        let (expanded, collapsed) = (&self.expanded, &self.collapsed);

        self.lines = self.catalog.lines(&self.filter, |id| match searching {
            true => !collapsed.contains(&id),
            false => expanded.contains(&id),
        });
    }

    fn position(&self, leaf: u32) -> Option<usize> {
        self.lines.iter().position(|line| line.entry == Entry::Leaf(leaf))
    }

    fn current(&self, scans: &Scans) -> bool {
        self.source
            .as_ref()
            .is_some_and(|held| Arc::ptr_eq(&held.0, &scans.0) && Arc::ptr_eq(&held.1, &scans.1) && Arc::ptr_eq(&held.2, &scans.2))
    }

    fn adopt(&mut self, catalog: Catalog) {
        let keyed = |ids: &FxHashSet<u32>| -> Vec<String> { ids.iter().filter_map(|id| self.catalog.folder_key(*id)).collect() };
        let (expanded, collapsed) = (keyed(&self.expanded), keyed(&self.collapsed));
        let picked = self.picked.and_then(|leaf| self.catalog.lead(leaf));

        self.catalog = catalog;

        let folders = self.catalog.folder_ids();
        let found = |keys: Vec<String>| -> FxHashSet<u32> { keys.iter().filter_map(|key| folders.get(key).copied()).collect() };

        self.expanded = found(expanded);
        self.collapsed = found(collapsed);
        self.filter = self.catalog.filter(&self.query);
        self.picked = picked.and_then(|lead| self.catalog.follow(&lead, &folders)).filter(|leaf| self.filter.keeps(*leaf));
        self.marks.clear();
        self.wanted.clear();
        self.relayout();
    }

    fn modded(&self) -> bool {
        self.picked.and_then(|leaf| self.marks.get(&leaf)).is_some_and(|mark| *mark != Mark::Same)
    }
}

pub struct Rows {
    state: RefCell<State>,
    notify: ModelNotify,
}

pub type Shared = Rc<Rows>;

impl Model for Rows {
    type Data = FinderRow;

    fn row_count(&self) -> usize {
        self.state.borrow().lines.len()
    }

    fn row_data(&self, row: usize) -> Option<FinderRow> {
        let mut state = self.state.borrow_mut();
        let line = state.lines.get(row)?;
        let entry = line.entry;
        let (depth, open) = (i32::from(line.depth), line.open);

        let mark = match entry {
            Entry::Folder(_) => UNMARKED,
            Entry::Leaf(leaf) => match state.marks.get(&leaf) {
                Some(Mark::Changed) => CHANGED,
                Some(Mark::Added) => ADDED,
                Some(Mark::Same) => UNMARKED,
                None => {
                    state.wanted.insert(leaf);

                    UNMARKED
                }
            },
        };

        Some(FinderRow {
            label: state.catalog.label(entry, state.values).into(),
            depth,
            folder: matches!(entry, Entry::Folder(_)),
            open,
            picked: matches!(entry, Entry::Leaf(leaf) if state.picked == Some(leaf)),
            mark,
        })
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.notify
    }
}

fn sourced<R>(context: &Rc<Context>, scans: &Scans, work: impl FnOnce(&Sources<'_>) -> R) -> Option<R> {
    let globals = context.globals()?;
    let vault = context.engine.vault();

    Some(work(&Sources {
        vault: &vault,
        cats: &scans.0,
        enemies: &scans.1,
        registry: &scans.2.registry,
        drops: &scans.2.dictionaries.drop_chara_registry,
        cat_names: &scans.2.dictionaries.cat_name_registry,
        names: &globals.names,
        localizable: &globals.localizable,
        category: stages::display_name,
        order: stages::sort_order,
    }))
}

fn scans(context: &Rc<Context>) -> Option<Scans> {
    let held = context.catalogs.cats().ready().zip(context.catalogs.enemies().ready()).zip(context.catalogs.stages().ready())?;

    Some((held.0.0, held.0.1, held.1))
}

fn planned(context: &Rc<Context>, pick: &Pick, target: &str) -> Option<form::Plan> {
    let values = context.state.settings().files.editor_mode;

    sourced(context, &scans(context)?, |sources| {
        plans::figures_plan(sources, pick, Some(target.to_owned()), values)
            .map(form::Plan::Figures)
            .or_else(|| plans::prose_plan(sources, pick, Some(target.to_owned())).map(form::Plan::Prose))
    })
    .flatten()
}

fn marked(context: &Rc<Context>, pick: &Pick, target: &str, reader: &mut Reader) -> Mark {
    let vault = context.engine.vault();

    match planned(context, pick, target) {
        Some(form::Plan::Figures(plan)) => figures::mark(&plan, &vault.vfs, reader),
        Some(form::Plan::Prose(plan)) => prose::mark(&plan, &vault.vfs, reader),
        None => file_mark(&vault.vfs, target, &pick.file),
    }
}

fn mark(ui: &AppWindow, rows: &Shared, context: &Rc<Context>) {
    let wanted: Vec<(u32, Pick)> = {
        let mut state = rows.state.borrow_mut();
        let leaves: Vec<u32> = state.wanted.drain().collect();

        leaves.into_iter().filter_map(|leaf| Some((leaf, state.catalog.pick(leaf)?))).collect()
    };

    let Some(target) = context.engine.mounted().filter(|_| !wanted.is_empty()) else {
        return;
    };

    let mut reader = Reader::default();
    let found: Vec<(u32, Mark)> = wanted.iter().map(|(leaf, pick)| (*leaf, marked(context, pick, &target, &mut reader))).collect();

    let touched: Vec<usize> = {
        let mut state = rows.state.borrow_mut();

        state.marks.extend(found.iter().copied());

        found.iter().filter(|(_, mark)| *mark != Mark::Same).filter_map(|(leaf, _)| state.position(*leaf)).collect()
    };

    for row in touched {
        rows.notify.row_changed(row);
    }

    present(ui, rows);
}

fn schedule(ui: &AppWindow, rows: &Shared, context: &Rc<Context>) {
    let (held, ctx, weak) = (Rc::clone(rows), Rc::clone(context), ui.as_weak());

    rows.state.borrow().marking.start(TimerMode::Repeated, Duration::from_millis(120), move || {
        let pending = !held.state.borrow().wanted.is_empty();

        if pending && let Some(ui) = weak.upgrade() {
            mark(&ui, &held, &ctx);
        }
    });
}

pub fn wire(window: &AppWindow, context: &Rc<Context>, forms: &form::Shared) -> Shared {
    let rows: Shared = Rc::new(Rows { state: RefCell::new(State::default()), notify: ModelNotify::default() });
    let finder = window.global::<Finder>();

    finder.set_rows(ModelRc::from(Rc::clone(&rows) as Rc<dyn Model<Data = FinderRow>>));

    let (held, weak) = (Rc::clone(&rows), window.as_weak());
    finder.on_searched(move |query| {
        {
            let mut state = held.state.borrow_mut();

            state.filter = state.catalog.filter(&query);
            state.query = query.to_string();
            state.collapsed.clear();
            state.picked = state.picked.filter(|leaf| state.filter.keeps(*leaf));
            state.relayout();
        }

        held.notify.reset();

        if let Some(ui) = weak.upgrade() {
            present(&ui, &held);
        }
    });

    let (held, weak) = (Rc::clone(&rows), window.as_weak());
    finder.on_activate(move |index| {
        activate(&held, index.max(0) as usize);

        if let Some(ui) = weak.upgrade() {
            present(&ui, &held);
        }
    });

    let (held, ctx, weak, forms) = (Rc::clone(&rows), Rc::clone(context), window.as_weak(), Rc::clone(forms));
    finder.on_edit(move || {
        if let Some(ui) = weak.upgrade() {
            edit(&ui, &held, &ctx, &forms);
        }
    });

    let (held, ctx, weak) = (Rc::clone(&rows), Rc::clone(context), window.as_weak());
    finder.on_sync(move || {
        if let Some(ui) = weak.upgrade() {
            reset(&ui, &held, &ctx);
        }
    });

    let (held, weak) = (Rc::clone(&rows), window.as_weak());
    finder.on_closed(move || {
        if let Some(ui) = weak.upgrade() {
            close(&ui, &held);
        }
    });

    let (held, ctx) = (Rc::clone(&rows), Rc::clone(context));
    context.catalogs.listen(Rc::new(move |ui: &AppWindow| settle(ui, &held, &ctx)));

    rows
}

pub fn open(ui: &AppWindow, rows: &Shared, context: &Rc<Context>) {
    {
        let mut state = rows.state.borrow_mut();

        state.open = true;
        state.values = context.state.settings().files.editor_mode;
    }

    schedule(ui, rows, context);
    present(ui, rows);
    ui.global::<Finder>().set_open(true);
    settle(ui, rows, context);
}

pub fn sync(ui: &AppWindow, rows: &Shared, context: &Rc<Context>) {
    let values = context.state.settings().files.editor_mode;

    {
        let mut state = rows.state.borrow_mut();

        if !state.open || state.values == values {
            return;
        }

        state.values = values;
        state.marks.clear();
    }

    rows.notify.reset();
    present(ui, rows);
}

pub fn touched(ui: &AppWindow, rows: &Shared) {
    let stale: Vec<usize> = {
        let mut state = rows.state.borrow_mut();

        if !state.open {
            return;
        }

        let leaves: Vec<u32> = state.marks.drain().map(|(leaf, _)| leaf).collect();

        leaves.into_iter().filter_map(|leaf| state.position(leaf)).collect()
    };

    for row in stale {
        rows.notify.row_changed(row);
    }

    present(ui, rows);
}

pub fn shut(ui: &AppWindow, rows: &Shared) {
    if rows.state.borrow().open {
        close(ui, rows);
    }
}

fn close(ui: &AppWindow, rows: &Shared) {
    *rows.state.borrow_mut() = State::default();
    rows.notify.reset();

    let finder = ui.global::<Finder>();

    finder.set_open(false);
    finder.set_query("".into());
    finder.set_building(false);
    finder.set_can_edit(false);
    finder.set_can_sync(false);
}

fn present(ui: &AppWindow, rows: &Shared) {
    let state = rows.state.borrow();
    let words = words(state.values);
    let finder = ui.global::<Finder>();

    finder.set_title(words.title.into());
    finder.set_note(words.note.into());
    finder.set_hint(words.hint.into());
    finder.set_empty(words.empty.into());
    finder.set_action(RAW_ACTION.into());
    finder.set_can_edit(state.built && state.picked.is_some());
    finder.set_can_sync(state.built && state.modded());
    finder.set_sync_label(if state.confirming { CONFIRM_LABEL.into() } else { SYNC_LABEL.into() });
    finder.set_building(!state.built);
}

fn settle(ui: &AppWindow, rows: &Shared, context: &Rc<Context>) {
    if !rows.state.borrow().open {
        return;
    }

    let Some((held, _)) = scans(context).zip(context.globals()) else {
        {
            let mut state = rows.state.borrow_mut();

            state.built = false;
            state.source = None;
            state.lines.clear();
            state.picked = None;
            state.marks.clear();
            state.wanted.clear();
        }

        rows.notify.reset();
        present(ui, rows);

        let config = context.state.settings().scanner_config(context.engine.mounted());

        context.catalogs.load_stages(ui, &context.engine, config);

        return;
    };

    {
        let mut state = rows.state.borrow_mut();

        if state.building || state.current(&held) {
            return;
        }

        state.building = true;
    }

    let vault = context.engine.vault();
    let Some(names) = context.globals().map(|globals| Arc::clone(&globals.names)) else {
        return;
    };
    let (landing, ctx) = (Rc::clone(rows), Rc::clone(context));
    let stamp = (Arc::clone(&held.0), Arc::clone(&held.1), Arc::clone(&held.2));

    bridge::offload(
        ui,
        "finder_catalog",
        move || {
            let localizable = game::localizable(&vault.vfs);

            Catalog::build(&Sources {
                vault: &vault,
                cats: &held.0,
                enemies: &held.1,
                registry: &held.2.registry,
                drops: &held.2.dictionaries.drop_chara_registry,
                cat_names: &held.2.dictionaries.cat_name_registry,
                names: &names,
                localizable: &localizable,
                category: stages::display_name,
                order: stages::sort_order,
            })
        },
        move |ui, catalog| {
            let reshaped = {
                let mut state = landing.state.borrow_mut();

                state.building = false;

                if !state.open {
                    return;
                }

                let before = state.lines.len();

                state.adopt(catalog);
                state.source = Some(stamp);
                state.built = true;

                state.lines.len() != before
            };

            if reshaped {
                landing.notify.reset();
            } else {
                touched(ui, &landing);
            }

            present(ui, &landing);
            settle(ui, &landing, &ctx);
        },
    );
}

fn edit(ui: &AppWindow, rows: &Shared, context: &Rc<Context>, forms: &form::Shared) {
    let picked = {
        let state = rows.state.borrow();

        state.picked.and_then(|leaf| state.catalog.pick(leaf))
    };

    let Some((pick, target)) = picked.zip(context.engine.mounted()) else {
        return;
    };

    if planned(context, &pick, &target).is_some_and(|plan| form::open(ui, forms, context, plan)) {
        return;
    }

    let vault = context.engine.vault();
    let vfs = &vault.vfs;

    let placed = vfs.rooted(&target, &pick.file).or_else(|| {
        let source = vfs.rooted(architecture::GAME, &pick.file).or_else(|| vfs.find(pick.file.as_str()))?;

        let placed = mods::ensure_as(vfs, &target, &source, &pick.file)
            .inspect_err(|err| warn!(file = pick.file, "The file could not be copied into the mod: {err}"))
            .ok()?;

        if let Err(err) = vfs.create((target.as_str(), placed.as_path())) {
            warn!(path = %placed.display(), "Failed to index the mod's new file: {err}");
        }

        Some(placed)
    });

    let Some((relative, pages)) = placed.and_then(|path| vfs.relative(&target, &path)).zip(context.pages()) else {
        context.toast(ui, PLACE_FAILED, true);

        return;
    };

    files::open(ui, &pages.files, context, target, relative);
    ui.global::<Shell>().invoke_navigate(Page::Files);
}

fn reset(ui: &AppWindow, rows: &Shared, context: &Rc<Context>) {
    let armed = std::mem::replace(&mut rows.state.borrow_mut().confirming, false);

    if !armed {
        let (landing, weak) = (Rc::clone(rows), ui.as_weak());

        {
            let mut state = rows.state.borrow_mut();

            state.confirming = true;
            state.confirm.start(TimerMode::SingleShot, Duration::from_millis(CONFIRM_MS), move || {
                landing.state.borrow_mut().confirming = false;

                if let Some(ui) = weak.upgrade() {
                    present(&ui, &landing);
                }
            });
        }

        present(ui, rows);

        return;
    }

    let picked = {
        let state = rows.state.borrow();

        state.confirm.stop();
        state.picked.and_then(|leaf| state.catalog.pick(leaf))
    };

    let Some((pick, target)) = picked.zip(context.engine.mounted()) else {
        present(ui, rows);

        return;
    };

    let vault = context.engine.vault();

    let synced = match planned(context, &pick, &target) {
        Some(form::Plan::Figures(plan)) => figures::reset(plan, &vault.vfs),
        Some(form::Plan::Prose(plan)) => prose::reset(plan, &vault.vfs),
        None => {
            let held = vault.vfs.rooted(&target, &pick.file);
            let removed = file_reset(&vault.vfs, &target, &pick.file);

            if let Some(path) = held.filter(|_| removed) {
                vault.vfs.prune(&target, &path);
            }

            removed
        }
    };

    if !synced {
        context.toast(ui, SYNC_FAILED, true);
        present(ui, rows);

        return;
    }

    context.changed(ui, &pick.file);
    present(ui, rows);
}

fn activate(rows: &Shared, index: usize) {
    let Some(entry) = rows.state.borrow().lines.get(index).map(|line| line.entry) else {
        return;
    };

    match entry {
        Entry::Folder(id) => {
            {
                let mut state = rows.state.borrow_mut();
                let searching = state.filter.searching();

                let held = match searching {
                    true => &mut state.collapsed,
                    false => &mut state.expanded,
                };

                if !held.remove(&id) {
                    held.insert(id);
                }

                state.relayout();
            }

            rows.notify.reset();
        }
        Entry::Leaf(leaf) => {
            let (before, after) = {
                let mut state = rows.state.borrow_mut();
                let before = state.picked.and_then(|held| state.position(held));

                state.picked = (state.picked != Some(leaf)).then_some(leaf);
                state.confirming = false;

                (before, state.picked.map(|_| index))
            };

            for row in before.into_iter().chain(after) {
                rows.notify.row_changed(row);
            }
        }
    }
}
