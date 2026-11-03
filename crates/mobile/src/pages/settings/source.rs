use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::Duration;

use slint::{ComponentHandle, Timer, TimerMode};
use tracing::{error, info};

use foundry::domains::import;
use foundry::domains::settings::source::{self, ErrorKind, Partial, Step};
use foundry::domains::settings::{Stamp, UpdateMode};

use crate::pages::Context;
use crate::{AppWindow, Settings, Shell, Source, Tone, bridge};

use super::format_size;

const AGREEMENT: &str = "v1";
const ACKNOWLEDGMENT: &str = "The purpose of this agreement is to ensure that you, the User, are aware of the potential quirks regarding the use of a Provider.\nBy accepting this agreement, you must have read, understood, and accepted the following facts about Providers:\n\n\u{2022} Providers wipe the import manifest, so future imports will import every file.\n\u{2022} Providers always overwrite every game file, so edits to vanilla will be lost.\n\nYou can accept the agreement by tapping the \"Agree\" button below. Selecting \"Disagree\" will close this pop-up.";
const BANNER_MS: u64 = 2000;
const CONFIRM_MS: u64 = 3000;

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

#[derive(Clone, Copy)]
struct Tickets {
    step: bridge::Ticket,
    finished: bridge::Ticket,
}

struct State {
    status: Status,
    abort: Arc<AtomicBool>,
    partial: Arc<Mutex<Option<Partial>>>,
    confirm_ignore: bool,
    open: bool,
    pending: Option<String>,
    timer: Timer,
    tickets: Option<Tickets>,
}

type Shared = Rc<RefCell<State>>;

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

impl State {
    fn new() -> Self {
        Self {
            status: Status::Idle,
            abort: Arc::default(),
            partial: Arc::default(),
            confirm_ignore: false,
            open: false,
            pending: None,
            timer: Timer::default(),
            tickets: None,
        }
    }

    fn kept(&self) -> bool {
        self.partial.lock().unwrap_or_else(PoisonError::into_inner).is_some()
    }

    fn settle(&mut self) {
        self.open = false;
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

pub(super) fn wire(window: &AppWindow, context: &Rc<Context>) {
    let state: Shared = Rc::new(RefCell::new(State::new()));
    let panel = window.global::<Source>();

    panel.set_terms(ACKNOWLEDGMENT.into());

    {
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
        let finished = bridge::subscribe::<(Result<Stamp, source::Error>, bool)>(move |ui, (result, touched)| finished(ui, &held, &ctx, result, touched));

        state.borrow_mut().tickets = Some(Tickets { step, finished });
    }

    let on = |state: &Shared, context: &Rc<Context>, window: &AppWindow| (Rc::clone(state), Rc::clone(context), window.as_weak());

    let (held, ctx, weak) = on(&state, context, window);
    panel.on_link_changed(move |link| {
        let Some(ui) = weak.upgrade() else {
            return;
        };

        if held.borrow().pending.is_none() && link.as_str() == ctx.state.settings().files.source_link {
            return;
        }

        if held.borrow().busy() {
            refresh(&ui, &held, &ctx);
            return;
        }

        if ctx.memory.get().source_agreement != AGREEMENT {
            held.borrow_mut().pending = Some(link.to_string());
            ask_terms(&ui);
            return;
        }

        ctx.state.apply(|settings| settings.files.source_link = link.to_string());
        held.borrow_mut().settle();
        refresh(&ui, &held, &ctx);
    });

    let (held, ctx, weak) = on(&state, context, window);
    panel.on_check(move || {
        if let Some(ui) = weak.upgrade() {
            if held.borrow().prompts() {
                held.borrow_mut().open = true;
                refresh(&ui, &held, &ctx);
            } else {
                check(&ui, &held, &ctx, true);
            }
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    panel.on_accept(move || {
        if let Some(ui) = weak.upgrade()
            && matches!(held.borrow().status, Status::Found(_))
        {
            let link = ctx.state.settings().files.source_link.trim().to_owned();

            install(&ui, &held, &ctx, link);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    panel.on_decline(move || {
        held.borrow_mut().settle();

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    panel.on_ignore(move || {
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
        ui.global::<Settings>().set_source_mode(1);
        held.borrow_mut().settle();
        refresh(&ui, &held, &ctx);
    });

    let (held, _, _) = on(&state, context, window);
    panel.on_cancel(move || held.borrow().abort.store(true, Ordering::Relaxed));

    let (held, ctx, weak) = on(&state, context, window);
    panel.on_resume(move || {
        if let Some(ui) = weak.upgrade()
            && matches!(held.borrow().status, Status::Interrupted { .. })
        {
            info!("Resuming the game data download");

            let link = ctx.state.settings().files.source_link.trim().to_owned();

            install(&ui, &held, &ctx, link);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    panel.on_dismiss(move || {
        held.borrow_mut().settle();

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    panel.on_terms_agree(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };

        ctx.memory.update(|memory| memory.source_agreement = AGREEMENT.to_owned());
        ui.global::<Source>().set_terms_open(false);

        let pending = held.borrow_mut().pending.take();

        if let Some(link) = pending {
            ctx.state.apply(|settings| settings.files.source_link = link);
            held.borrow_mut().settle();
        }

        refresh(&ui, &held, &ctx);
    });

    let (held, ctx, weak) = on(&state, context, window);
    panel.on_terms_disagree(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };

        ui.global::<Source>().set_terms_open(false);
        held.borrow_mut().pending = None;
        ctx.state.apply(|settings| settings.files.source_link.clear());
        held.borrow_mut().settle();
        refresh(&ui, &held, &ctx);
    });

    refresh(window, &state, context);

    let files = context.state.settings().files.clone();

    if files.source_mode != UpdateMode::Ignore && !files.source_link.trim().is_empty() {
        info!("Checking the game data source at startup");
        check(window, &state, context, false);
    }
}

fn ask_terms(ui: &AppWindow) {
    let shell = ui.global::<Shell>();

    ui.global::<Source>().set_terms_open(true);
    shell.set_taps(shell.get_taps() + 1);
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
        if manual {
            ask_terms(ui);
        }

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
                    state.open = true;
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

            if !held.open && matches!(held.status, Status::Current | Status::Installed | Status::Failed(_)) {
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

        held.abort.store(false, Ordering::Relaxed);
        held.confirm_ignore = false;
        (Arc::clone(&held.abort), Arc::clone(&held.partial))
    };
    let weak = ui.as_weak();
    let steps = weak.clone();
    let spawned = thread::Builder::new().name("source_install".to_owned()).spawn(move || {
        let partial = slot.lock().unwrap_or_else(PoisonError::into_inner).take();
        let result = source::install(&link, partial, |step| bridge::post(&steps, tickets.step, step), &abort);
        let touched = result.as_ref().map_or_else(|failure| failure.touched, |_| true);

        if touched {
            import::forget_imports();
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
            held.open = true;
        }
        Err(err) => {
            error!("Failed to spawn the game data download thread: {err}");

            let kept = state.borrow().kept();

            state.borrow_mut().status = Status::Interrupted { kind: ErrorKind::Disk, kept };
        }
    }

    refresh(ui, state, context);
}

fn finished(ui: &AppWindow, state: &Shared, context: &Rc<Context>, result: Result<Stamp, source::Error>, touched: bool) {
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
        if state.borrow().open {
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

fn refresh(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let panel = ui.global::<Source>();
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

    panel.set_link(held.pending.as_deref().unwrap_or(settings.files.source_link.as_str()).into());
    panel.set_link_enabled(!held.busy());
    panel.set_label(label.into());
    panel.set_tone(tone);
    panel.set_enabled(enabled);
    panel.set_open(held.open && held.prompts());
    panel.set_ignore_label(if held.confirm_ignore { "Continue?".into() } else { "Ignore".into() });

    match &held.status {
        Status::Found(summary) => {
            panel.set_kind(0);
            panel.set_title("New Files Available".into());
            panel.set_summary(summary.as_str().into());
        }
        Status::Installing(progress) => {
            panel.set_kind(1);
            panel.set_title("Downloading Files".into());
            panel.set_heading(progress.heading.into());
            panel.set_fraction(progress.fraction);
            panel.set_detail(progress.detail.as_str().into());
            panel.set_sealed(progress.sealed);
        }
        Status::Installed => {
            panel.set_kind(2);
            panel.set_title("Files Updated".into());
        }
        Status::Interrupted { kind, kept } => {
            panel.set_kind(3);
            panel.set_title("Download Failed".into());
            panel.set_resumable(resumable(*kind, *kept));
            panel.set_failure(failure(*kind, *kept).into());
        }
        Status::Idle | Status::Checking | Status::Current | Status::Failed(_) => {}
    }
}
