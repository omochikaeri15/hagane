use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread;
use std::time::Duration;

use slint::{ComponentHandle, Image, ModelRc, TimerMode, VecModel};
use tracing::{error, info, warn};

use emulator::runtime::VERSION;
use foundry::common::frames;
use foundry::common::job::JobOutcome;
use foundry::domains::cat::scanner::{self, CatEntry};
use foundry::domains::sandbox::replay::{self as tape, Save, Summary, Unit};

use crate::host::{Command, Reel};
use crate::pages::{Context, cats, detail};
use crate::{AppWindow, Cats, DeckCell, Field, OrbSlot, ReplayRow, Sandbox, Tone, bridge, delivery, paths};

use super::state::{Device, Volume};
use super::{Shared, battle, refresh, report};

const NP_ICON: &str = "gatyaitemD_07_f.png";
const LATEST_LABEL: &str = "Latest Battle";
const MISSING: &str = "Missing";
const UNKNOWN: &str = "Unknown";
const CROWN_GLYPH: &str = "\u{1F732}";
const MEGABYTE: f64 = 1024.0 * 1024.0;
const CONFIRM_MS: u64 = 2000;
const ENDED_MS: u64 = 2000;
const CONFIRM_LABEL: &str = "Continue?";
const DELETE_LABEL: &str = "Delete";
const SAVE_LABELS: [&str; 3] = ["Save to Library", "Saving...", "Saved!"];
const EXPORT_LABELS: [&str; 3] = ["Export to BCV", "Exporting...", "Exported!"];
const FAILURE_LABEL: &str = "Failed!";
const READY_LABEL: &str = "Ready";
const DONE_LABEL: &str = "Done";
const BUNDLE_STEM: &str = "Replay";
const FORM_LETTERS: [char; 4] = ['f', 'c', 's', 'u'];
const FIRST_TALENT_FORM: usize = 2;

#[derive(Clone, PartialEq, Eq, Debug)]
enum Pick {
    Latest,
    Bundle(PathBuf),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Work {
    Save,
    Export,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Ended {
    Done,
    Failed,
}

enum Update {
    Progress(f32),
    Finished(Work, JobOutcome),
}

#[derive(Default)]
pub struct Part {
    picks: Vec<Pick>,
    picked: Option<usize>,
    summary: Option<Summary>,
    inspecting: Option<Pick>,
    running: Option<Work>,
    abort: Option<Arc<AtomicBool>>,
    fraction: f32,
    status: String,
    ended: Option<(Work, Ended)>,
    confirm_delete: bool,
    rename: String,
    failure: Option<String>,
    foreign: Option<String>,
    ticket: Option<bridge::Ticket>,
    timer: slint::Timer,
    ended_timer: slint::Timer,
    opened: Option<usize>,
}

fn target_of(pick: &Pick) -> Option<PathBuf> {
    match pick {
        Pick::Latest => tape::latest(),
        Pick::Bundle(path) => Some(path.clone()),
    }
}

fn label_of(pick: &Pick) -> String {
    match pick {
        Pick::Latest => LATEST_LABEL.to_owned(),
        Pick::Bundle(path) => path.file_stem().map_or_else(String::new, |stem| stem.to_string_lossy().into_owned()),
    }
}

fn clamped(level: u32, plus: u32, altar_cap: Option<i32>) -> (u32, u32) {
    match altar_cap {
        Some(cap) => (u32::try_from(cap).map_or(level, |cap| level.min(cap.saturating_add(1))), 0),
        None => (level, plus),
    }
}

fn talent_levels(cat: &CatEntry, talents: &[(i32, i32)]) -> HashMap<u8, u8> {
    let Some(groups) = cat.talent_data.as_ref() else {
        return HashMap::new();
    };

    talents
        .iter()
        .filter_map(|(ability, level)| {
            let index = groups.groups.iter().position(|group| i32::from(group.ability_id) == *ability)?;

            Some((u8::try_from(index).ok()?, u8::try_from(*level).ok()?))
        })
        .collect()
}

fn picked(state: &Shared) -> Option<Pick> {
    let held = state.borrow();

    held.replay.picked.and_then(|index| held.replay.picks.get(index).cloned())
}

pub fn relist(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let library = tape::library();
    let mut picks = Vec::new();

    if tape::latest().is_some_and(|dir| dir.join("save").is_file()) {
        picks.push(Pick::Latest);
    }

    picks.extend(tape::list(&library).into_iter().map(Pick::Bundle));

    let current = picked(state);
    let index = current.and_then(|current| picks.iter().position(|pick| *pick == current)).or_else(|| (!picks.is_empty()).then_some(0));

    {
        let mut held = state.borrow_mut();

        held.replay.picks = picks;
        held.replay.picked = index;
    }

    inspect(ui, state, context);
}

fn inspect(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let Some(pick) = picked(state) else {
        let mut held = state.borrow_mut();

        held.replay.summary = None;
        held.replay.rename.clear();
        drop(held);
        refresh(ui, state, context);

        return;
    };
    let Some(target) = target_of(&pick) else {
        return;
    };

    state.borrow_mut().replay.inspecting = Some(pick.clone());
    state.borrow_mut().replay.rename = label_of(&pick);

    let landing = Rc::clone(state);
    let ctx = Rc::clone(context);
    let is_dir = matches!(pick, Pick::Latest);

    bridge::offload(
        ui,
        "replay_inspect",
        move || if is_dir { tape::inspect_dir(&target) } else { tape::inspect_bundle(&target) },
        move |ui, summary| {
            if landing.borrow().replay.inspecting.as_ref() != Some(&pick) {
                return;
            }

            {
                let mut held = landing.borrow_mut();

                held.replay.inspecting = None;
                held.replay.summary = Some(summary);
                held.replay.opened = None;
            }

            refresh(ui, &landing, &ctx);
        },
    );
}

pub fn watch(ui: &AppWindow, state: &Shared, context: &Rc<Context>, checked: bool) {
    let Some(pick) = picked(state) else {
        return;
    };
    let version = state.borrow().replay.summary.as_ref().and_then(|summary| summary.save.as_ref()).map(|save| save.version.clone());

    if !checked
        && let Some(recorded) = version.filter(|recorded| recorded != VERSION)
    {
        state.borrow_mut().replay.foreign = Some(if recorded.is_empty() { UNKNOWN.to_owned() } else { recorded });
        refresh(ui, state, context);

        return;
    }

    state.borrow_mut().replay.foreign = None;

    let reel = match pick {
        Pick::Latest => Reel::Latest,
        Pick::Bundle(path) => Reel::Bundle(path),
    };
    let (width, height) = battle::bounds(ui);
    let settings = context.state.settings();

    {
        let held = state.borrow();
        let saved = held.saved();

        if let Some(arena) = &held.arena {
            arena.send(Command::Display { scale: saved.render_scale(), padded: saved.padded() });
            arena.send(Command::Volumes(Volume::nearest(saved.music_volume).percent(), Volume::nearest(saved.effects_volume).percent()));
            arena.send(Command::Watch { reel, source: settings.sandbox.replay_source, phone: saved.device == Device::Phone, lag: saved.lag, width, height });
        }
    }

    state.borrow_mut().status.clear();
    refresh(ui, state, context);
}

fn begin(ui: &AppWindow, state: &Shared, context: &Rc<Context>, work: Work) {
    let Some(pick) = picked(state) else {
        return;
    };
    let Some(target) = target_of(&pick) else {
        return;
    };
    let Some(ticket) = state.borrow().replay.ticket else {
        return;
    };

    if state.borrow().replay.running.is_some() {
        return;
    }

    let abort = Arc::new(AtomicBool::new(false));
    let out = match (work, &pick) {
        (Work::Save, _) => {
            let library = tape::library();

            if let Err(err) = fs::create_dir_all(&library) {
                report(ui, state, context, format!("The replay library could not be created: {err}"));

                return;
            }

            tape::next_bundle(&library)
        }
        (Work::Export, Pick::Latest) => {
            let folder = Path::new(paths::EXPORTS);
            let _ = fs::create_dir_all(folder);

            (0usize..)
                .map(|copy| if copy == 0 { format!("{BUNDLE_STEM}.{}", tape::EXTENSION) } else { format!("{BUNDLE_STEM}{copy}.{}", tape::EXTENSION) })
                .map(|name| folder.join(name))
                .find(|path| !path.exists())
                .unwrap_or_else(|| folder.join(format!("{BUNDLE_STEM}.{}", tape::EXTENSION)))
        }
        (Work::Export, Pick::Bundle(_)) => PathBuf::new(),
    };

    {
        let mut held = state.borrow_mut();

        held.replay.running = Some(work);
        held.replay.abort = Some(Arc::clone(&abort));
        held.replay.fraction = 0.0;
        held.replay.status = format!("{} | 0%", if work == Work::Save { "Saving" } else { "Exporting" });
        held.replay.ended = None;
        held.replay.failure = None;
    }

    let weak = ui.as_weak();
    let progress = weak.clone();
    let spawned = thread::Builder::new().name("replay_job".to_owned()).spawn(move || {
        let outcome = match (&pick, work) {
            (Pick::Bundle(bundle), Work::Export) => tape::export(bundle).map_or_else(JobOutcome::Failed, |_| JobOutcome::Completed),
            _ => tape::pack(&target, &out, |fraction| bridge::post(&progress, ticket, Update::Progress(fraction)), &abort),
        };

        bridge::post(&weak, ticket, Update::Finished(work, outcome));
    });

    if let Err(err) = spawned {
        error!("The replay job thread could not be started: {err}");
        state.borrow_mut().replay.running = None;
    }

    refresh(ui, state, context);
}

fn update(ui: &AppWindow, state: &Shared, context: &Rc<Context>, update: Update) {
    match update {
        Update::Progress(fraction) => {
            let mut held = state.borrow_mut();
            let fraction = fraction.clamp(0.0, 1.0);

            held.replay.fraction = fraction;
            held.replay.status = format!("{} | {}%", if held.replay.running == Some(Work::Save) { "Saving" } else { "Exporting" }, (fraction * 100.0) as i32);
        }
        Update::Finished(work, outcome) => {
            let ended = match &outcome {
                JobOutcome::Completed => {
                    info!("The replay job finished");
                    Ended::Done
                }
                JobOutcome::Aborted => Ended::Failed,
                JobOutcome::Failed(reason) => {
                    warn!("The replay job failed: {reason}");
                    Ended::Failed
                }
            };

            {
                let mut held = state.borrow_mut();

                held.replay.running = None;
                held.replay.abort = None;
                held.replay.fraction = 1.0;
                held.replay.ended = Some((work, ended));
                held.replay.status = match &outcome {
                    JobOutcome::Completed => DONE_LABEL.to_owned(),
                    JobOutcome::Aborted => "Aborted".to_owned(),
                    JobOutcome::Failed(reason) => format!("Failed | {reason}"),
                };
            }

            if ended == Ended::Done {
                if work == Work::Export {
                    delivery::announce(ui, context);
                } else {
                    relist(ui, state, context);
                }
            }

            let landing = Rc::clone(state);
            let ctx = Rc::clone(context);
            let weak = ui.as_weak();

            state.borrow().replay.ended_timer.start(TimerMode::SingleShot, Duration::from_millis(ENDED_MS), move || {
                {
                    let mut held = landing.borrow_mut();

                    held.replay.ended = None;
                    held.replay.status = READY_LABEL.to_owned();
                }

                if let Some(ui) = weak.upgrade() {
                    refresh(&ui, &landing, &ctx);
                }
            });
        }
    }

    refresh(ui, state, context);
}

pub fn wire(window: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let sandbox = window.global::<Sandbox>();
    let on = |state: &Shared, context: &Rc<Context>, window: &AppWindow| (Rc::clone(state), Rc::clone(context), window.as_weak());

    {
        let (held, ctx) = (Rc::clone(state), Rc::clone(context));
        let ticket = bridge::subscribe::<Update>(move |ui, progress| update(ui, &held, &ctx, progress));

        state.borrow_mut().replay.ticket = Some(ticket);
        state.borrow_mut().replay.status = READY_LABEL.to_owned();
    }

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_replay_pick(move |index| {
        let index = index.max(0) as usize;

        if index < held.borrow().replay.picks.len() {
            held.borrow_mut().replay.picked = Some(index);
            held.borrow_mut().replay.confirm_delete = false;
        }

        if let Some(ui) = weak.upgrade() {
            inspect(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_replay_renamed(move |name| {
        held.borrow_mut().replay.rename = name.to_string();

        let Some(Pick::Bundle(bundle)) = picked(&held) else {
            return;
        };

        match tape::rename(&bundle, &name) {
            Ok(Some(renamed)) => {
                {
                    let mut state = held.borrow_mut();
                    let index = state.replay.picked;

                    if let Some(pick) = index.and_then(|index| state.replay.picks.get_mut(index)) {
                        *pick = Pick::Bundle(renamed);
                    }
                }

                if let Some(ui) = weak.upgrade() {
                    refresh(&ui, &held, &ctx);
                }
            }
            Ok(None) => {}
            Err(reason) => warn!("The replay could not be renamed: {reason}"),
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_replay_save(move || {
        if let Some(ui) = weak.upgrade() {
            begin(&ui, &held, &ctx, Work::Save);
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_replay_export(move || {
        if let Some(ui) = weak.upgrade() {
            begin(&ui, &held, &ctx, Work::Export);
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_replay_delete(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };

        if !held.borrow().replay.confirm_delete {
            held.borrow_mut().replay.confirm_delete = true;

            let landing = Rc::clone(&held);
            let ctx_inner = Rc::clone(&ctx);
            let weak_inner = ui.as_weak();

            held.borrow().replay.timer.start(TimerMode::SingleShot, Duration::from_millis(CONFIRM_MS), move || {
                landing.borrow_mut().replay.confirm_delete = false;

                if let Some(ui) = weak_inner.upgrade() {
                    refresh(&ui, &landing, &ctx_inner);
                }
            });
            refresh(&ui, &held, &ctx);

            return;
        }

        held.borrow_mut().replay.confirm_delete = false;

        let outcome = match picked(&held) {
            Some(Pick::Bundle(bundle)) => tape::delete(&bundle),
            Some(Pick::Latest) => tape::latest().map_or(Ok(()), |dir| fs::remove_dir_all(&dir).map_err(|err| format!("{} could not be deleted: {err}", dir.display()))),
            None => Ok(()),
        };

        if let Err(reason) = outcome {
            warn!("{reason}");
            held.borrow_mut().replay.failure = Some(reason);
        }

        held.borrow_mut().replay.picked = None;
        relist(&ui, &held, &ctx);
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_tile_tapped(move |slot| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let slot = slot.max(0) as usize;
        let unit = held.borrow().replay.summary.as_ref().and_then(|summary| summary.save.as_ref()).and_then(|save| save.setup.lineup.get(slot).cloned());
        let Some(unit) = unit else {
            return;
        };
        let Some(cat) = u32::try_from(unit.unit).ok().and_then(|id| held.borrow().cat(id)) else {
            return;
        };
        let Some(pages) = ctx.pages() else {
            return;
        };
        let altar_cap = held.borrow().replay.summary.as_ref().and_then(|summary| summary.save.as_ref()).and_then(|save| save.altar_cap);
        let (level, plus) = clamped(unit.level, unit.plus, altar_cap);
        let inspected = if plus == 0 { level.to_string() } else { format!("{level}+{plus}") };
        let form = usize::try_from(unit.form).unwrap_or(0);

        cats::seed_member(&ui, &pages.cats, &ctx, cats::Seed { cat: &cat, form, level: &inspected, talents: &talent_levels(&cat, &unit.talents), locked: true });
        held.borrow_mut().replay.opened = Some(slot);
        ui.global::<Sandbox>().set_unit_open(true);
        refresh(&ui, &held, &ctx);
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_version_close(move || {
        held.borrow_mut().replay.foreign = None;

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_version_continue(move || {
        held.borrow_mut().replay.foreign = None;

        if let Some(ui) = weak.upgrade() {
            watch(&ui, &held, &ctx, true);
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_version_ignore(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };

        held.borrow_mut().replay.foreign = None;

        let Some(target) = picked(&held).and_then(|pick| target_of(&pick)) else {
            return;
        };
        let landing = Rc::clone(&held);
        let ctx_inner = Rc::clone(&ctx);

        report(&ui, &held, &ctx, "Updating the replay's version...".to_owned());
        bridge::offload(
            &ui,
            "replay_restamp",
            move || tape::restamp(&target, VERSION),
            move |ui, outcome| match outcome {
                Ok(()) => {
                    landing.borrow_mut().status.clear();
                    relist(ui, &landing, &ctx_inner);
                    watch(ui, &landing, &ctx_inner, true);
                }
                Err(reason) => report(ui, &landing, &ctx_inner, format!("The replay's version could not be updated: {reason}")),
            },
        );
    });
}

fn tile(context: &Rc<Context>, cats: &[CatEntry], book: Option<&Rc<super::orbs::Book>>, unit: &Unit, save: &Save, slot: usize) -> DeckCell {
    let images = &context.images;
    let vault = context.engine.vault();
    let vfs = &vault.vfs;
    let id = u32::try_from(unit.unit).unwrap_or(0);
    let form = usize::try_from(unit.form).unwrap_or(0);
    let (level, plus) = clamped(unit.level, unit.plus, save.altar_cap);
    let cat = cats.iter().find(|cat| cat.id == id);
    let drawn = save.icons.get(slot).filter(|name| !name.is_empty()).and_then(|name| vfs.find(name.as_str()));
    let scanned = cat.and_then(|cat| scanner::deploy_icon(vfs, cat, form));
    let path = drawn.or(scanned).or_else(|| FORM_LETTERS.get(form).and_then(|letter| vfs.find(format!("uni{id:03}_{letter}00.png").as_str())));
    let icon = path.and_then(|path| images.trimmed(&vault, &path));
    let mut worn = unit.orbs.clone();

    worn.sort_unstable();

    let orbs: Vec<Image> = book.map(|book| worn.iter().filter_map(|(_, orb)| u32::try_from(*orb).ok()).filter_map(|orb| book.face(images, &vault, orb)).collect()).unwrap_or_default();
    let cost = save.costs.get(slot).copied().filter(|cost| *cost >= 0);
    let np = vault.vfs.find(NP_ICON).and_then(|path| images.trimmed(&vault, &path));

    DeckCell {
        filled: true,
        has_icon: icon.is_some(),
        icon: icon.unwrap_or_default(),
        fallback: format!("{id:03}-{}", form + 1).into(),
        level: if plus == 0 { format!("Lv{level}") } else { format!("Lv{level}+{plus}") }.into(),
        cost: cost.map_or_else(String::new, |cost| format!("{cost}\u{00a2}")).into(),
        has_cost: cost.is_some(),
        barred: false,
        talented: !unit.talents.is_empty(),
        has_np: np.is_some(),
        np: np.unwrap_or_default(),
        orbs: ModelRc::new(VecModel::from(orbs)),
    }
}

fn fields(pairs: Vec<(&str, String)>) -> ModelRc<Field> {
    ModelRc::new(VecModel::from(pairs.into_iter().map(|(label, value)| Field { label: label.into(), lines: detail::lines(&value) }).collect::<Vec<_>>()))
}

pub fn sync(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let sandbox = ui.global::<Sandbox>();
    let held = state.borrow();
    let part = &held.replay;
    let pick = part.picked.and_then(|index| part.picks.get(index));
    let rows: Vec<ReplayRow> = part.picks.iter().enumerate().map(|(index, pick)| ReplayRow { label: label_of(pick).into(), selected: part.picked == Some(index) }).collect();

    sandbox.set_replays(ModelRc::new(VecModel::from(rows)));
    sandbox.set_replay_title(pick.map(label_of).unwrap_or_default().into());
    sandbox.set_rename(part.rename.as_str().into());
    sandbox.set_replay_renamable(matches!(pick, Some(Pick::Bundle(_))));

    let summary = part.summary.as_ref();
    let save = summary.and_then(|summary| summary.save.as_ref());
    let ready = save.is_some() && part.inspecting.is_none();
    let time = summary.and_then(|summary| summary.frames).map_or_else(|| MISSING.to_owned(), |count| frames::label(i32::try_from(count).unwrap_or(i32::MAX)));
    let screen = save.map_or_else(|| MISSING.to_owned(), |save| format!("{} \u{00d7} {}{}", save.screen.width.round(), save.screen.height.round(), if save.screen.phone { " (Phone)" } else { "" }));
    let assets = summary.map_or_else(|| MISSING.to_owned(), |summary| format!("{} files", summary.assets));
    let size = summary.map_or_else(|| MISSING.to_owned(), |summary| format!("{:.1} MB", summary.bytes as f64 / MEGABYTE));

    sandbox.set_meta(fields(vec![("Time", time), ("Screen", screen), ("Assets", assets), ("Size", size)]));

    let game = match save {
        Some(save) => {
            let entry = &save.setup.stage;
            let map = if entry.map_name.is_empty() { format!("Map {}", entry.map_id) } else { entry.map_name.clone() };
            let named = if entry.stage_name.is_empty() { format!("Stage {}", entry.stage + 1) } else { entry.stage_name.clone() };
            let app = if save.app.is_empty() { UNKNOWN.to_owned() } else { save.app.clone() };
            let version = if save.version.is_empty() { UNKNOWN.to_owned() } else { save.version.clone() };

            vec![("App", app), ("Version", version), ("Map", map), ("Stage", format!("{named} [{}{CROWN_GLYPH}]", entry.crown + 1))]
        }
        None => vec![("App", MISSING.to_owned()), ("Version", MISSING.to_owned()), ("Map", MISSING.to_owned()), ("Stage", MISSING.to_owned())],
    };

    sandbox.set_game(fields(game));

    let book = held.book.clone();
    let tiles: Vec<DeckCell> = match (save, &held.cats) {
        (Some(save), Some(cats)) => (0..10).map(|slot| save.setup.lineup.get(slot).map(|unit| tile(context, cats, book.as_ref(), unit, save, slot)).unwrap_or_default()).collect(),
        _ => (0..10).map(|_| DeckCell::default()).collect(),
    };

    sandbox.set_tiles(ModelRc::new(VecModel::from(tiles)));
    sandbox.set_replay_ready(ready);
    sandbox.set_watchable(ready && held.battle.standing == crate::host::Standing::default());

    let busy = part.running.is_some();
    let face = |work: Work, labels: [&str; 3]| -> (slint::SharedString, Tone) {
        match (part.running, part.ended) {
            (Some(running), _) if running == work => (labels[1].into(), Tone::StatusWarning),
            (_, Some((ended, Ended::Done))) if ended == work => (labels[2].into(), Tone::StatusSuccess),
            (_, Some((ended, Ended::Failed))) if ended == work => (FAILURE_LABEL.into(), Tone::StatusDanger),
            _ => (labels[0].into(), Tone::Primary),
        }
    };
    let (save_label, save_tone) = face(Work::Save, SAVE_LABELS);
    let (export_label, export_tone) = face(Work::Export, EXPORT_LABELS);

    sandbox.set_save_label(save_label);
    sandbox.set_save_tone(save_tone);
    sandbox.set_save_enabled(ready && !busy && matches!(pick, Some(Pick::Latest)));
    sandbox.set_export_label(export_label);
    sandbox.set_export_tone(export_tone);
    sandbox.set_export_enabled(ready && !busy);
    sandbox.set_replay_delete_label(if part.confirm_delete { CONFIRM_LABEL.into() } else { DELETE_LABEL.into() });
    sandbox.set_replay_delete_enabled(pick.is_some() && !busy);
    sandbox.set_job_status(part.status.as_str().into());
    sandbox.set_job_progress(if busy { part.fraction } else { 1.0 });
    sandbox.set_replay_failure(part.failure.clone().unwrap_or_default().into());
    sandbox.set_version_open(part.foreign.is_some());

    if let Some(recorded) = &part.foreign {
        sandbox.set_version_text(format!("This replay was recorded in game version {recorded}, but Sandbox currently replicates game version {VERSION}.\n\nReplays recorded in a different game version may play out differently, desync, or crash partway through. If the replay crashes due to missing files, try setting Replay Source to VFS under Settings > Sandbox.\n\nIgnore marks this replay as working in game version {VERSION}, so you won't be asked again until the game version changes.").into());
    }

    if let (Some(slot), Some(save), Some(cats)) = (part.opened, save, &held.cats)
        && let Some(unit) = save.setup.lineup.get(slot)
    {
        let id = u32::try_from(unit.unit).unwrap_or(0);
        let form = usize::try_from(unit.form).unwrap_or(0);
        let cat = cats.iter().find(|cat| cat.id == id);

        sandbox.set_unit_title(cat.map_or_else(|| format!("Unit {id:03}"), |cat| cat.display_name(form)).into());
        sandbox.set_unit_locked(true);
        ui.global::<Cats>().set_has_orbs(form >= FIRST_TALENT_FORM && !unit.orbs.is_empty());

        if let Some(book) = &book {
            let vault = context.engine.vault();
            let slots: Vec<OrbSlot> = unit.orbs.iter().filter_map(|(_, orb)| u32::try_from(*orb).ok()).map(|orb| book.picture(&context.images, &vault, Some(orb))).collect();

            ui.global::<Cats>().set_orb_slots(ModelRc::new(VecModel::from(slots)));
        }
    } else if held.editing.is_some() {
        sandbox.set_unit_locked(false);
    }
}
