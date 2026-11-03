use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::Duration;

use slint::{ComponentHandle, Timer, TimerMode};
use tracing::{error, info, warn};

use foundry::common::job::{JobEvent, JobOutcome, ProgressCounter};
use foundry::common::region::Region;
use foundry::domains::import::device::{self, Installed};
use foundry::common::architecture;
use foundry::domains::import::{self as game_import, AdbImportType, AdbTarget, ImportMode, pack, raw};
use foundry::domains::settings::source::{self, ErrorKind, Partial, Step};
use foundry::domains::settings::{Stamp, UpdateMode};

use crate::pages::Context;
use crate::{AppWindow, Import, Tone, bridge, picker, platform};

const AGREEMENT: &str = "v1";
const BANNER_MS: u64 = 2000;
const CONFIRM_MS: u64 = 3000;
const POLL_MS: u64 = 100;
const ACKNOWLEDGMENT: &str = "The purpose of this agreement is to ensure that you, the User, are aware of the potential quirks regarding the use of a Provider.\nBy accepting this agreement, you must have read, understood, and accepted the following facts about Providers:\n\n\u{2022} Providers wipe the import manifest, so future imports will import every file.\n\u{2022} Providers always overwrite every game file, so edits to vanilla will be lost.\n\nYou can accept the agreement by tapping the \"Agree\" button below. Selecting \"Disagree\" will close this pop-up.";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Download,
    Device,
    Pack,
    Raw,
}

impl Tab {
    fn from_index(index: i32) -> Self {
        match index {
            1 => Self::Device,
            2 => Self::Pack,
            3 => Self::Raw,
            _ => Self::Download,
        }
    }

    fn index(self) -> i32 {
        match self {
            Self::Download => 0,
            Self::Device => 1,
            Self::Pack => 2,
            Self::Raw => 3,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Banner {
    Completed,
    Aborted,
}

struct Progress {
    heading: &'static str,
    fraction: f32,
    detail: String,
    sealed: bool,
}

enum Status {
    Idle,
    Checking,
    Current,
    Found(String),
    Installing(Progress),
    Installed,
    Interrupted { kind: ErrorKind, kept: bool },
    Failed(ErrorKind),
}

pub struct State {
    tab: Tab,
    status: Status,
    abort_source: Arc<AtomicBool>,
    partial: Arc<Mutex<Option<Partial>>>,
    confirm_ignore: bool,
    pending_link: Option<String>,
    source_open: bool,
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
    step: bridge::Ticket,
    finished: bridge::Ticket,
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

fn uploaded(modified: &str) -> &str {
    let dated = modified.split_once(", ").map_or(modified, |(_, rest)| rest);

    dated.match_indices(' ').nth(2).map_or(dated, |(end, _)| &dated[..end])
}

fn summary(stamp: &Stamp) -> String {
    match (stamp.modified.is_empty(), stamp.size) {
        (true, 0) => "Uploaded at an unknown time".to_owned(),
        (true, size) => format!("{} download", format_size(size)),
        (false, 0) => format!("Uploaded {}", uploaded(&stamp.modified)),
        (false, size) => format!("Uploaded {} ({})", uploaded(&stamp.modified), format_size(size)),
    }
}

fn progress(step: Step) -> Progress {
    match step {
        Step::Downloading { done, total: 0 } => Progress { heading: "Downloading game files", fraction: 0.0, detail: format_size(done), sealed: false },
        Step::Downloading { done, total } => Progress { heading: "Downloading game files", fraction: done as f32 / total as f32, detail: format!("{} of {}", format_size(done), format_size(total)), sealed: false },
        Step::Unpacking { left, done, total } => Progress {
            heading: "Writing game files",
            fraction: if total == 0 { 1.0 } else { done as f32 / total as f32 },
            detail: match left {
                0 => "Finishing the last chunk".to_owned(),
                1 => "1 chunk is still being written to disk".to_owned(),
                left => format!("{left} chunks are still being written to disk"),
            },
            sealed: false,
        },
        Step::Replacing => Progress { heading: "Finishing up", fraction: 1.0, detail: "Clearing out files the new \"game\" folder no longer needs".to_owned(), sealed: true },
    }
}

fn resumable(kind: ErrorKind, kept: bool) -> bool {
    kept && matches!(kind, ErrorKind::Network | ErrorKind::Disk)
}

fn failure(kind: ErrorKind, kept: bool) -> &'static str {
    match (kind, kept) {
        (ErrorKind::Link, _) => "That is not an https link this app can read, so your current \"game\" folder was left as it was until the link is fixed",
        (ErrorKind::Missing, _) => "Nothing is published at that link, so your current \"game\" folder was left as it was until the link is fixed",
        (ErrorKind::Network, true) => "The download was cut short, the files that already arrived are kept in your \"game\" folder for when it resumes",
        (ErrorKind::Network, false) => "The download was cut short, the next download fetches whatever your \"game\" folder is still missing",
        (ErrorKind::Archive, _) => "The download is not a payload this app can read and cannot be resumed, the next download fetches whatever your \"game\" folder is still missing",
        (ErrorKind::Disk, true) => "The new files could not be written to disk, the files that already arrived are kept in your \"game\" folder for when it resumes",
        (ErrorKind::Disk, false) => "The new files could not be written to disk, the next download fetches whatever your \"game\" folder is still missing",
        (ErrorKind::Aborted, _) => "The download was canceled, the next download fetches whatever your \"game\" folder is still missing",
    }
}

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
            tab: Tab::Download,
            status: Status::Idle,
            abort_source: Arc::default(),
            partial: Arc::default(),
            confirm_ignore: false,
            pending_link: None,
            source_open: false,
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

    fn kept(&self) -> bool {
        self.partial.lock().unwrap_or_else(PoisonError::into_inner).is_some()
    }

    fn settle(&mut self) {
        self.source_open = false;
        self.confirm_ignore = false;
        self.status = Status::Idle;
        self.partial.lock().unwrap_or_else(PoisonError::into_inner).take();
    }

    fn prompts(&self) -> bool {
        matches!(self.status, Status::Found(_) | Status::Installing(_) | Status::Installed | Status::Interrupted { .. })
    }

    fn busy(&self) -> bool {
        matches!(self.status, Status::Checking | Status::Installing(_))
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

    import.set_terms(ACKNOWLEDGMENT.into());

    {
        let (held, ctx) = (Rc::clone(&state), Rc::clone(context));
        let job = bridge::subscribe::<JobEvent>(move |ui, event| job_event(ui, &held, &ctx, event));
        let (held, ctx) = (Rc::clone(&state), Rc::clone(context));
        let step = bridge::subscribe::<Step>(move |ui, step| {
            {
                let mut state = held.borrow_mut();

                if matches!(state.status, Status::Installing(_)) {
                    state.status = Status::Installing(progress(step));
                }
            }

            refresh(ui, &held, &ctx);
        });
        let (held, ctx) = (Rc::clone(&state), Rc::clone(context));
        let finished = bridge::subscribe::<(Result<Stamp, source::Error>, bool)>(move |ui, (result, touched)| source_finished(ui, &held, &ctx, result, touched));

        state.borrow_mut().tickets = Some(Tickets { job, step, finished });
    }

    let on = |state: &Shared, context: &Rc<Context>, window: &AppWindow| (Rc::clone(state), Rc::clone(context), window.as_weak());

    let (held, ctx, weak) = on(&state, context, window);
    import.on_tab_set(move |index| {
        if held.borrow().running {
            return;
        }

        held.borrow_mut().tab = Tab::from_index(index);
        ctx.memory.update(|memory| memory.import_tab = index.clamp(0, 3) as u8);

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    import.on_link_changed(move |link| {
        let Some(ui) = weak.upgrade() else {
            return;
        };

        if held.borrow().busy() {
            refresh(&ui, &held, &ctx);
            return;
        }

        if ctx.memory.get().source_agreement != AGREEMENT {
            held.borrow_mut().pending_link = Some(link.to_string());
            ui.global::<Import>().set_terms_open(true);
            return;
        }

        ctx.state.apply(|settings| settings.files.source_link = link.to_string());
        held.borrow_mut().settle();
        refresh(&ui, &held, &ctx);
    });


    let (held, ctx, weak) = on(&state, context, window);
    import.on_source_check(move || {
        if let Some(ui) = weak.upgrade() {
            if held.borrow().prompts() {
                held.borrow_mut().source_open = true;
                refresh(&ui, &held, &ctx);
            } else {
                check(&ui, &held, &ctx, true);
            }
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    import.on_source_accept(move || {
        if let Some(ui) = weak.upgrade()
            && matches!(held.borrow().status, Status::Found(_))
        {
            let link = ctx.state.settings().files.source_link.trim().to_owned();

            install(&ui, &held, &ctx, link);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    import.on_source_decline(move || {
        held.borrow_mut().settle();

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    import.on_source_ignore(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let armed = held.borrow().confirm_ignore;

        if !armed {
            held.borrow_mut().confirm_ignore = true;

            let landing = Rc::clone(&held);
            let ctx_inner = Rc::clone(&ctx);
            let weak_inner = ui.as_weak();

            held.borrow().timer.start(TimerMode::SingleShot, Duration::from_millis(CONFIRM_MS), move || {
                landing.borrow_mut().confirm_ignore = false;

                if let Some(ui) = weak_inner.upgrade() {
                    refresh(&ui, &landing, &ctx_inner);
                }
            });
            refresh(&ui, &held, &ctx);
            return;
        }

        info!("User chose to ignore the game data source, changing its mode to Ignore");
        ctx.state.apply(|settings| settings.files.source_mode = UpdateMode::Ignore);
        held.borrow_mut().settle();
        refresh(&ui, &held, &ctx);
    });

    let (held, _, _) = on(&state, context, window);
    import.on_source_cancel(move || held.borrow().abort_source.store(true, Ordering::Relaxed));

    let (held, ctx, weak) = on(&state, context, window);
    import.on_source_resume(move || {
        if let Some(ui) = weak.upgrade()
            && matches!(held.borrow().status, Status::Interrupted { .. })
        {
            info!("Resuming the game data download");

            let link = ctx.state.settings().files.source_link.trim().to_owned();

            install(&ui, &held, &ctx, link);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    import.on_source_dismiss(move || {
        held.borrow_mut().settle();

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    import.on_terms_agree(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };

        ctx.memory.update(|memory| memory.source_agreement = AGREEMENT.to_owned());
        ui.global::<Import>().set_terms_open(false);

        if let Some(link) = held.borrow_mut().pending_link.take() {
            ctx.state.apply(|settings| settings.files.source_link = link);
            held.borrow_mut().settle();
        }

        refresh(&ui, &held, &ctx);
    });

    let (held, ctx, weak) = on(&state, context, window);
    import.on_terms_disagree(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };

        ui.global::<Import>().set_terms_open(false);
        held.borrow_mut().pending_link = None;
        ctx.state.apply(|settings| settings.files.source_link.clear());
        held.borrow_mut().settle();
        refresh(&ui, &held, &ctx);
    });

    let (held, ctx, weak) = on(&state, context, window);
    import.on_device_type_set(move |index| {
        held.borrow_mut().device_type = if index == 1 { AdbImportType::All } else { AdbImportType::Update };
        ctx.memory.update(|memory| memory.device_type = index.clamp(0, 1) as u8);

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    import.on_device_region_set(move |index| {
        held.borrow_mut().device_region = region_of(index);
        ctx.memory.update(|memory| memory.device_region = index.clamp(0, 4) as u8);

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    import.on_pack_region_set(move |index| {
        held.borrow_mut().pack_region = region_of(index);

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
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
                Box::new(move |ui, ctx, path| {
                    landing.borrow_mut().pack_path = Some(path);
                    refresh(ui, &landing, ctx);
                }),
            );
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    import.on_raw_mode_set(move |index| {
        {
            let mut state = held.borrow_mut();

            state.raw_mode = if index == 1 { ImportMode::Zip } else { ImportMode::Folder };
            state.raw_game = index == 2;
            state.raw_path = state.raw_game.then(|| PathBuf::from(architecture::GAME));
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    import.on_pick_raw(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let landing = Rc::clone(&held);
        let then: picker::Then = Box::new(move |ui, ctx, path| {
            landing.borrow_mut().raw_path = Some(path);
            refresh(ui, &landing, ctx);
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

    refresh(window, &state, context);

    if context.state.settings().files.source_mode == UpdateMode::Prompt {
        check(window, &state, context, false);
    }

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

fn check(ui: &AppWindow, state: &Shared, context: &Rc<Context>, manual: bool) {
    if !matches!(state.borrow().status, Status::Idle | Status::Current | Status::Failed(_)) {
        return;
    }

    let link = context.state.settings().files.source_link.trim().to_owned();

    if link.is_empty() {
        return;
    }

    if context.memory.get().source_agreement != AGREEMENT {
        ui.global::<Import>().set_terms_open(manual);
        return;
    }

    info!("Checking the game data source for newer files");
    state.borrow_mut().settle();
    state.borrow_mut().status = Status::Checking;
    refresh(ui, state, context);

    let landing = Rc::clone(state);
    let ctx = Rc::clone(context);
    let probed = link.clone();

    bridge::offload(
        ui,
        "source_probe",
        move || source::probe(&probed),
        move |ui, result| {
            if !matches!(landing.borrow().status, Status::Checking) {
                return;
            }

            landing.borrow_mut().status = Status::Idle;

            if link != ctx.state.settings().files.source_link.trim() {
                refresh(ui, &landing, &ctx);
                return;
            }

            match result {
                Ok(stamp) if source::current(&stamp, ctx.state.settings().files.source_stamp.as_ref()) => {
                    info!("Game data already matches its source");

                    if manual {
                        flash(ui, &landing, &ctx, Status::Current);
                    }
                }
                Ok(stamp) => {
                    info!(modified = stamp.modified, "The game data source holds newer files");

                    let mut state = landing.borrow_mut();

                    state.status = Status::Found(summary(&stamp));
                    state.source_open = true;
                }
                Err(err) if manual => flash(ui, &landing, &ctx, Status::Failed(err.kind)),
                Err(_) => {}
            }

            refresh(ui, &landing, &ctx);
        },
    );
}

fn flash(ui: &AppWindow, state: &Shared, context: &Rc<Context>, status: Status) {
    state.borrow_mut().status = status;

    let landing = Rc::clone(state);
    let ctx = Rc::clone(context);
    let weak = ui.as_weak();

    state.borrow().timer.start(TimerMode::SingleShot, Duration::from_millis(BANNER_MS), move || {
        {
            let mut held = landing.borrow_mut();

            if !held.source_open && matches!(held.status, Status::Current | Status::Installed | Status::Failed(_)) {
                held.status = Status::Idle;
            }
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &landing, &ctx);
        }
    });
    refresh(ui, state, context);
}

fn install(ui: &AppWindow, state: &Shared, context: &Rc<Context>, link: String) {
    let Some(tickets) = state.borrow().tickets else {
        return;
    };
    let (abort, slot) = {
        let mut held = state.borrow_mut();

        held.abort_source.store(false, Ordering::Relaxed);
        held.confirm_ignore = false;
        (Arc::clone(&held.abort_source), Arc::clone(&held.partial))
    };
    let weak = ui.as_weak();
    let steps = weak.clone();
    let spawned = thread::Builder::new().name("source_install".to_owned()).spawn(move || {
        let partial = slot.lock().unwrap_or_else(PoisonError::into_inner).take();
        let result = source::install(&link, partial, |step| bridge::post(&steps, tickets.step, step), &abort);
        let touched = result.as_ref().map_or_else(|failure| failure.touched, |_| true);

        if touched {
            game_import::forget_imports();
        }

        let result = result.map_err(|failure| {
            *slot.lock().unwrap_or_else(PoisonError::into_inner) = failure.partial;

            failure.error
        });

        bridge::post(&weak, tickets.finished, (result, touched));
    });

    match spawned {
        Ok(_) => {
            let mut held = state.borrow_mut();

            held.status = Status::Installing(progress(Step::Downloading { done: 0, total: 0 }));
            held.source_open = true;
        }
        Err(err) => {
            error!("Failed to spawn the game data download thread: {err}");

            let kept = state.borrow().kept();

            state.borrow_mut().status = Status::Interrupted { kind: ErrorKind::Disk, kept };
        }
    }

    refresh(ui, state, context);
}

fn source_finished(ui: &AppWindow, state: &Shared, context: &Rc<Context>, result: Result<Stamp, source::Error>, touched: bool) {
    let landed = match result {
        Ok(stamp) => {
            context.state.apply(|settings| settings.files.source_stamp = Some(stamp));
            Some(Status::Installed)
        }
        Err(err) if err.kind == ErrorKind::Aborted => {
            state.borrow_mut().settle();
            None
        }
        Err(err) => {
            let kept = state.borrow().kept();

            state.borrow_mut().status = Status::Interrupted { kind: err.kind, kept };
            None
        }
    };

    if let Some(landed) = landed {
        if state.borrow().source_open {
            state.borrow_mut().status = landed;
        } else {
            flash(ui, state, context, landed);
        }
    }

    if touched {
        let config = context.state.settings().scanner_config(context.engine.mounted());

        context.engine.rebuild(ui, config);
    }

    refresh(ui, state, context);
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
            refresh(ui, state, context);
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
        Tab::Download => {
            state.borrow_mut().running = false;
            return;
        }
    };

    if let Err(err) = spawned {
        error!("The import thread could not be started: {err}");
        state.borrow_mut().running = false;
    } else {
        let landing = Rc::clone(state);
        let ctx = Rc::clone(context);
        let weak = ui.as_weak();

        state.borrow().poll.start(TimerMode::Repeated, Duration::from_millis(POLL_MS), move || {
            if let Some(ui) = weak.upgrade() {
                if !landing.borrow().running {
                    landing.borrow().poll.stop();
                }

                refresh(&ui, &landing, &ctx);
            }
        });
    }

    refresh(ui, state, context);
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
                    let ctx = Rc::clone(context);
                    let weak = ui.as_weak();

                    held.timer.start(TimerMode::SingleShot, Duration::from_millis(BANNER_MS), move || {
                        landing.borrow_mut().banner = None;

                        if let Some(ui) = weak.upgrade() {
                            refresh(&ui, &landing, &ctx);
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

    refresh(ui, state, context);
}

fn refresh(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let import = ui.global::<Import>();
    let held = state.borrow();
    let settings = context.state.settings();
    let linked = !settings.files.source_link.trim().is_empty();
    let (label, tone, enabled): (&str, Tone, bool) = match &held.status {
        Status::Idle if linked => ("Check for Files Now", Tone::Primary, true),
        Status::Idle => ("No Source Link", Tone::Neutral, false),
        Status::Checking => ("Checking for Files...", Tone::StatusWarning, false),
        Status::Current => ("Up to Date!", Tone::StatusSuccess, false),
        Status::Found(_) => ("Files Found!", Tone::Success, true),
        Status::Installing(_) => ("Downloading Files...", Tone::Warning, true),
        Status::Installed => ("Files Updated!", Tone::StatusSuccess, false),
        Status::Interrupted { .. } => ("Download Failed!", Tone::Danger, true),
        Status::Failed(ErrorKind::Link) => ("Invalid Link!", Tone::StatusDanger, false),
        Status::Failed(ErrorKind::Missing) => ("Source Not Found!", Tone::StatusDanger, false),
        Status::Failed(ErrorKind::Network) => ("Failed to Check!", Tone::StatusDanger, false),
        Status::Failed(_) => ("Download Failed!", Tone::StatusDanger, false),
    };

    import.set_tab(held.tab.index());
    import.set_running(held.running);
    import.set_link(settings.files.source_link.as_str().into());
    import.set_link_enabled(!held.busy());
    import.set_source_label(label.into());
    import.set_source_tone(tone);
    import.set_source_enabled(enabled);
    import.set_source_open(held.source_open && held.prompts());
    import.set_ignore_label(if held.confirm_ignore { "Continue?".into() } else { "Ignore".into() });

    match &held.status {
        Status::Found(summary) => {
            import.set_source_kind(0);
            import.set_source_title("New Files Available".into());
            import.set_source_summary(summary.as_str().into());
        }
        Status::Installing(progress) => {
            import.set_source_kind(1);
            import.set_source_title("Downloading Files".into());
            import.set_source_heading(progress.heading.into());
            import.set_source_fraction(progress.fraction);
            import.set_source_detail(progress.detail.as_str().into());
            import.set_source_sealed(progress.sealed);
        }
        Status::Installed => {
            import.set_source_kind(2);
            import.set_source_title("Files Updated".into());
        }
        Status::Interrupted { kind, kept } => {
            import.set_source_kind(3);
            import.set_source_title("Download Failed".into());
            import.set_source_resumable(resumable(*kind, *kept));
            import.set_source_failure(failure(*kind, *kept).into());
        }
        Status::Idle | Status::Checking | Status::Current | Status::Failed(_) => {}
    }

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
        Tab::Download => false,
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
                Tab::Raw | Tab::Download => "Select Source Data",
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
