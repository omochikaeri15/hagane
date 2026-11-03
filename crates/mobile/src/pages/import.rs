use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use slint::{ComponentHandle, Timer, TimerMode};
use tracing::{error, info, warn};

use foundry::common::job::{JobEvent, JobOutcome, ProgressCounter};
use foundry::common::region::Region;
use foundry::domains::import::device::{self, Installed};
use foundry::common::architecture;
use foundry::domains::import::{AdbImportType, AdbTarget, ImportMode, pack, raw};

use crate::pages::Context;
use crate::{AppWindow, Import, Tone, bridge, picker, platform};

const BANNER_MS: u64 = 2000;
const POLL_MS: u64 = 100;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Device,
    Pack,
    Raw,
}

impl Tab {
    fn from_index(index: i32) -> Self {
        match index {
            1 => Self::Pack,
            2 => Self::Raw,
            _ => Self::Device,
        }
    }

    fn index(self) -> i32 {
        match self {
            Self::Device => 0,
            Self::Pack => 1,
            Self::Raw => 2,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Banner {
    Completed,
    Aborted,
}

pub struct State {
    tab: Tab,
    device_type: AdbImportType,
    device_region: AdbTarget,
    pack_region: AdbTarget,
    pack_path: Option<PathBuf>,
    raw_mode: ImportMode,
    raw_game: bool,
    raw_path: Option<PathBuf>,
    running: bool,
    aborting: bool,
    log: String,
    progress: Arc<ProgressCounter>,
    abort_job: Arc<AtomicBool>,
    banner: Option<Banner>,
    timer: Timer,
    poll: Timer,
    tickets: Option<Tickets>,
}

#[derive(Clone, Copy)]
struct Tickets {
    job: bridge::Ticket,
}

pub type Shared = Rc<RefCell<State>>;

fn region_index(target: AdbTarget) -> i32 {
    match target {
        AdbTarget::Specific(Region::En) => 0,
        AdbTarget::Specific(Region::Ja) => 1,
        AdbTarget::Specific(Region::Tw) => 2,
        AdbTarget::Specific(Region::Ko) => 3,
        AdbTarget::All => 4,
    }
}

fn region_of(index: i32) -> AdbTarget {
    match index {
        1 => AdbTarget::Specific(Region::Ja),
        2 => AdbTarget::Specific(Region::Tw),
        3 => AdbTarget::Specific(Region::Ko),
        4 => AdbTarget::All,
        _ => AdbTarget::Specific(Region::En),
    }
}

fn job_outcome(result: Result<(), String>, abort: &AtomicBool) -> JobOutcome {
    if abort.load(Ordering::Relaxed) {
        return JobOutcome::Aborted;
    }

    result.map_or_else(JobOutcome::Failed, |()| JobOutcome::Completed)
}

impl State {
    fn new() -> Self {
        Self {
            tab: Tab::Device,
            device_type: AdbImportType::Update,
            device_region: AdbTarget::Specific(Region::En),
            pack_region: AdbTarget::Specific(Region::En),
            pack_path: None,
            raw_mode: ImportMode::Folder,
            raw_game: false,
            raw_path: None,
            running: false,
            aborting: false,
            log: String::new(),
            progress: Arc::default(),
            abort_job: Arc::default(),
            banner: None,
            timer: Timer::default(),
            poll: Timer::default(),
            tickets: None,
        }
    }
}

pub fn wire(window: &AppWindow, context: &Rc<Context>) -> Shared {
    let state: Shared = Rc::new(RefCell::new(State::new()));
    let import = window.global::<Import>();

    {
        let remembered = context.memory.get();
        let mut held = state.borrow_mut();

        held.tab = Tab::from_index(i32::from(remembered.import_tab));
        held.device_type = if remembered.device_type == 1 { AdbImportType::All } else { AdbImportType::Update };
        held.device_region = region_of(i32::from(remembered.device_region));
    }

    import.set_device_available(platform::DEVICE_IMPORT);


    {
        let (held, ctx) = (Rc::clone(&state), Rc::clone(context));
        let job = bridge::subscribe::<JobEvent>(move |ui, event| job_event(ui, &held, &ctx, event));

        state.borrow_mut().tickets = Some(Tickets { job });
    }

    let on = |state: &Shared, context: &Rc<Context>, window: &AppWindow| (Rc::clone(state), Rc::clone(context), window.as_weak());

    let (held, ctx, weak) = on(&state, context, window);
    import.on_tab_set(move |index| {
        if held.borrow().running {
            return;
        }

        held.borrow_mut().tab = Tab::from_index(index);
        ctx.memory.update(|memory| memory.import_tab = index.clamp(0, 2) as u8);

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    import.on_device_type_set(move |index| {
        held.borrow_mut().device_type = if index == 1 { AdbImportType::All } else { AdbImportType::Update };
        ctx.memory.update(|memory| memory.device_type = index.clamp(0, 1) as u8);

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    import.on_device_region_set(move |index| {
        held.borrow_mut().device_region = region_of(index);
        ctx.memory.update(|memory| memory.device_region = index.clamp(0, 4) as u8);

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held);
        }
    });

    let (held, _, weak) = on(&state, context, window);
    import.on_pack_region_set(move |index| {
        held.borrow_mut().pack_region = region_of(index);

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    import.on_pick_pack(move || {
        if let Some(ui) = weak.upgrade() {
            let landing = Rc::clone(&held);

            picker::open_many(
                &ui,
                &ctx,
                "Select Pack Files",
                &["pack", "list"],
                Box::new(move |ui, _, path| {
                    landing.borrow_mut().pack_path = Some(path);
                    refresh(ui, &landing);
                }),
            );
        }
    });

    let (held, _, weak) = on(&state, context, window);
    import.on_raw_mode_set(move |index| {
        {
            let mut state = held.borrow_mut();

            state.raw_mode = if index == 1 { ImportMode::Zip } else { ImportMode::Folder };
            state.raw_game = index == 2;
            state.raw_path = state.raw_game.then(|| PathBuf::from(architecture::GAME));
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    import.on_pick_raw(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let landing = Rc::clone(&held);
        let then: picker::Then = Box::new(move |ui, _, path| {
            landing.borrow_mut().raw_path = Some(path);
            refresh(ui, &landing);
        });

        if held.borrow().raw_mode == ImportMode::Zip {
            picker::open(&ui, &ctx, "Select an Archive", &["zst", "tar", "zip"], then);
        } else {
            picker::open_many(&ui, &ctx, "Select Files", &[], then);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    import.on_action(move || {
        if let Some(ui) = weak.upgrade() {
            action(&ui, &held, &ctx);
        }
    });

    refresh(window, &state);

    state
}

fn regions(target: AdbTarget) -> Vec<Region> {
    match target {
        AdbTarget::Specific(region) => vec![region],
        AdbTarget::All => vec![Region::En, Region::Ja, Region::Tw, Region::Ko],
    }
}

fn installed(target: AdbTarget, emit: &dyn Fn(JobEvent)) -> Vec<Installed> {
    regions(target)
        .into_iter()
        .filter_map(|region| {
            let package = device::package_name(region.metadata().package_suffix);

            match platform::installed(&package) {
                Ok(Some(found)) => Some(found),
                Ok(None) => {
                    emit(JobEvent::Log(format!("Skipping {package}: Not installed.")));
                    None
                }
                Err(reason) => {
                    warn!("The installed app {package} could not be read: {reason}");
                    emit(JobEvent::Log(format!("Skipping {package}: {reason}")));
                    None
                }
            }
        })
        .collect()
}

fn action(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let Some(tickets) = state.borrow().tickets else {
        return;
    };

    {
        let mut held = state.borrow_mut();

        if held.running {
            if !held.aborting {
                warn!("Aborting import job");
                held.aborting = true;
                held.abort_job.store(true, Ordering::Relaxed);
            }

            drop(held);
            refresh(ui, state);
            return;
        }

        held.running = true;
        held.aborting = false;
        held.log.clear();
        held.banner = None;
        held.progress.reset(0);
        held.abort_job.store(false, Ordering::Relaxed);
    }

    let import_config = context.state.settings().import_config();
    let (tab, abort, progress) = {
        let held = state.borrow();

        (held.tab, Arc::clone(&held.abort_job), Arc::clone(&held.progress))
    };
    let weak = ui.as_weak();
    let emit = move |event: JobEvent| bridge::post(&weak, tickets.job, event);
    let spawned = match tab {
        Tab::Device => {
            let (device_type, device_region) = {
                let held = state.borrow();

                (held.device_type, held.device_region)
            };

            thread::Builder::new().name("device_import".to_owned()).spawn(move || {
                let found = installed(device_region, &emit);
                let result = device::run(found, device_type, import_config, &emit, &abort, &progress);

                emit(JobEvent::Finished(job_outcome(result, &abort)));
            })
        }
        Tab::Pack => {
            let Some(folder) = state.borrow().pack_path.clone() else {
                state.borrow_mut().running = false;
                return;
            };

            thread::Builder::new().name("pack_import".to_owned()).spawn(move || {
                let result = pack::run(&folder.to_string_lossy(), ImportMode::Folder, import_config, &emit, &abort, &progress);

                emit(JobEvent::Finished(job_outcome(result, &abort)));
            })
        }
        Tab::Raw => {
            let Some(source) = state.borrow().raw_path.clone() else {
                state.borrow_mut().running = false;
                return;
            };
            let priority = context.state.settings().general.language_priority.clone();

            thread::Builder::new().name("raw_import".to_owned()).spawn(move || {
                let result = raw::run(&source.to_string_lossy(), import_config, &emit, &abort, &priority, &progress);

                emit(JobEvent::Finished(job_outcome(result, &abort)));
            })
        }
    };

    if let Err(err) = spawned {
        error!("The import thread could not be started: {err}");
        state.borrow_mut().running = false;
    } else {
        let landing = Rc::clone(state);
        let weak = ui.as_weak();

        state.borrow().poll.start(TimerMode::Repeated, Duration::from_millis(POLL_MS), move || {
            if let Some(ui) = weak.upgrade() {
                if !landing.borrow().running {
                    landing.borrow().poll.stop();
                }

                refresh(&ui, &landing);
            }
        });
    }

    refresh(ui, state);
}

fn job_event(ui: &AppWindow, state: &Shared, context: &Rc<Context>, event: JobEvent) {
    let mut completed = false;

    {
        let mut held = state.borrow_mut();

        match event {
            JobEvent::Log(line) => {
                if !held.log.is_empty() {
                    held.log.push('\n');
                }

                held.log.push_str(&line);
            }
            JobEvent::Progress { .. } => {}
            JobEvent::Finished(outcome) => {
                held.running = false;
                held.aborting = false;
                held.abort_job.store(false, Ordering::Relaxed);

                match outcome {
                    JobOutcome::Completed => {
                        info!("Import job completed");
                        held.banner = Some(Banner::Completed);
                        completed = true;
                    }
                    JobOutcome::Aborted => {
                        info!("Import job aborted");
                        held.banner = Some(Banner::Aborted);
                    }
                    JobOutcome::Failed(message) => {
                        warn!("Import job failed: {message}");

                        if !held.log.is_empty() {
                            held.log.push('\n');
                        }

                        held.log.push_str(&format!("Error: {message}"));
                    }
                }

                if held.banner.is_some() {
                    let landing = Rc::clone(state);
                    let weak = ui.as_weak();

                    held.timer.start(TimerMode::SingleShot, Duration::from_millis(BANNER_MS), move || {
                        landing.borrow_mut().banner = None;

                        if let Some(ui) = weak.upgrade() {
                            refresh(&ui, &landing);
                        }
                    });
                }
            }
        }
    }

    if completed {
        let config = context.state.settings().scanner_config(context.engine.mounted());

        context.engine.rebuild(ui, config);
    }

    refresh(ui, state);
}

fn refresh(ui: &AppWindow, state: &Shared) {
    let import = ui.global::<Import>();
    let held = state.borrow();
    import.set_tab(held.tab.index());
    import.set_running(held.running);
    import.set_device_type(if held.device_type == AdbImportType::All { 1 } else { 0 });
    import.set_device_region(region_index(held.device_region));
    import.set_pack_region(region_index(held.pack_region));

    let pick_many = if platform::SYSTEM_PICKER { "Select Files" } else { "Select Folder" };
    let (pack_label, pack_tone) = held.pack_path.as_deref().map_or((pick_many.to_owned(), Tone::Neutral), |path| (picker::label(path), Tone::Success));
    let raw_default = if held.raw_mode == ImportMode::Zip { "Select Archive" } else { pick_many };
    let (raw_label, raw_tone) = held.raw_path.as_deref().map_or((raw_default.to_owned(), Tone::Neutral), |path| (picker::label(path), Tone::Success));

    import.set_pack_label(pack_label.into());
    import.set_pack_tone(pack_tone);
    import.set_raw_mode(if held.raw_game { 2 } else if held.raw_mode == ImportMode::Zip { 1 } else { 0 });
    import.set_raw_label(raw_label.into());
    import.set_raw_tone(raw_tone);

    let can_run = match held.tab {
        Tab::Device => platform::DEVICE_IMPORT,
        Tab::Pack => held.pack_path.is_some(),
        Tab::Raw => held.raw_path.is_some(),
    };
    let (action_label, action_tone, action_enabled) = match (held.banner, held.running, held.aborting) {
        (Some(Banner::Completed), _, _) => ("Job Complete!", Tone::Success, can_run),
        (Some(Banner::Aborted), _, _) => ("Job Aborted!", Tone::Danger, can_run),
        (None, true, true) => ("Aborting Job...", Tone::Danger, false),
        (None, true, false) => ("Abort Job", Tone::Danger, true),
        (None, false, _) if can_run => ("Start Job", Tone::Primary, true),
        (None, false, _) => (
            match held.tab {
                Tab::Device => "Android Only",
                Tab::Pack => "Select Source Folder",
                Tab::Raw => "Select Source Data",
            },
            Tone::Neutral,
            false,
        ),
    };

    import.set_action_label(action_label.into());
    import.set_action_tone(action_tone);
    import.set_action_enabled(action_enabled);
    import.set_progress(if held.running && held.progress.total() > 0 { held.progress.fraction() } else { 1.0 });
    import.set_log(held.log.as_str().into());
}
