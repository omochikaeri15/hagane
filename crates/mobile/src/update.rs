use std::cell::RefCell;
use std::fs;
use std::rc::Rc;
use std::thread;
use std::time::Duration;

use slint::{ComponentHandle, Timer, TimerMode};
use tracing::{error, info, warn};

use foundry::common::architecture::Workspace;
use foundry::common::github::{self, Offering};
use foundry::domains::settings::UpdateMode;

use crate::pages::Context;
use crate::{AppWindow, Tone, Updater, bridge, platform};

const OWNER: &str = "omochikaeri15";
const REPO: &str = "hagane";
const APK: &str = "apk";
const APK_MIME: &str = "application/vnd.android.package-archive";
const CURRENT: &str = env!("CARGO_PKG_VERSION");
const BANNER_MS: u64 = 2000;
const CONFIRM_MS: u64 = 3000;

enum Status {
    Idle,
    Checking,
    Current,
    Found(Offering),
    Downloading(Offering, f32),
    Ready(Offering, String),
    Failed(String),
}

pub struct State {
    status: Status,
    open: bool,
    confirm_never: bool,
    timer: Timer,
    tickets: Option<Tickets>,
}

#[derive(Clone, Copy)]
struct Tickets {
    progress: bridge::Ticket,
    finished: bridge::Ticket,
}

pub type Shared = Rc<RefCell<State>>;

impl State {
    fn prompts(&self) -> bool {
        matches!(self.status, Status::Found(_) | Status::Downloading(..) | Status::Ready(..) | Status::Failed(_))
    }
}

fn found() -> Result<Option<Offering>, String> {
    github::newest_offering(OWNER, REPO, CURRENT).map_err(|err| err.to_string())
}

fn fetch(target: &Offering, progress: &dyn Fn(f32)) -> Result<String, String> {
    let workspace = Workspace::claim("update").map_err(|err| format!("the update folder could not be prepared: {err}"))?;
    let archive = workspace.path().join(github::TARGET.asset);
    let mut file = fs::File::create(&archive).map_err(|err| format!("the update could not be saved: {err}"))?;

    github::download_asset(OWNER, REPO, &target.tag, github::TARGET.asset, &mut file, &|done, total| {
        progress(total.map_or(0.0, |total| done as f32 / total.max(1) as f32));
    })
    .map_err(|err| err.to_string())?;
    drop(file);

    let apk = github::extract_asset(&archive, APK, workspace.path()).map_err(|err| err.to_string())?;

    platform::publish(&apk, "", APK_MIME)
}

pub fn wire(window: &AppWindow, context: &Rc<Context>) -> Shared {
    let state: Shared = Rc::new(RefCell::new(State { status: Status::Idle, open: false, confirm_never: false, timer: Timer::default(), tickets: None }));
    let updater = window.global::<Updater>();

    updater.set_current(CURRENT.into());
    updater.set_installable(platform::DOWNLOADS);

    {
        let (held, ctx) = (Rc::clone(&state), Rc::clone(context));
        let progress = bridge::subscribe::<f32>(move |ui, fraction| {
            {
                let mut state = held.borrow_mut();

                if let Status::Downloading(_, held) = &mut state.status {
                    *held = fraction;
                }
            }

            refresh(ui, &held, &ctx);
        });
        let (held, ctx) = (Rc::clone(&state), Rc::clone(context));
        let finished = bridge::subscribe::<Result<String, String>>(move |ui, outcome| {
            {
                let mut state = held.borrow_mut();
                let Status::Downloading(target, _) = &state.status else {
                    return;
                };
                let target = target.clone();

                state.status = match outcome {
                    Ok(uri) => {
                        info!(version = target.version, "The update is downloaded and ready to install");
                        Status::Ready(target, uri)
                    }
                    Err(reason) => {
                        error!("The update could not be downloaded: {reason}");
                        Status::Failed(reason)
                    }
                };
                state.open = true;
            }

            refresh(ui, &held, &ctx);
        });

        state.borrow_mut().tickets = Some(Tickets { progress, finished });
    }

    let on = |state: &Shared, context: &Rc<Context>, window: &AppWindow| (Rc::clone(state), Rc::clone(context), window.as_weak());

    let (held, ctx, weak) = on(&state, context, window);
    updater.on_check(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };

        if held.borrow().prompts() {
            held.borrow_mut().open = true;
            refresh(&ui, &held, &ctx);
        } else {
            check(&ui, &held, &ctx, true);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    updater.on_mode_set(move |index| {
        ctx.state.apply(|settings| settings.general.update_mode = if index == 0 { UpdateMode::Prompt } else { UpdateMode::Ignore });

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    updater.on_accept(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let target = match &held.borrow().status {
            Status::Found(target) => target.clone(),
            _ => return,
        };

        if !platform::DOWNLOADS {
            platform::open_url(&github::release_page(OWNER, REPO, &target.tag));
            held.borrow_mut().open = false;
            refresh(&ui, &held, &ctx);
            return;
        }

        download(&ui, &held, &ctx, target);
    });

    let (held, ctx, weak) = on(&state, context, window);
    updater.on_decline(move || {
        {
            let mut state = held.borrow_mut();

            state.open = false;
            state.confirm_never = false;

            if matches!(state.status, Status::Found(_) | Status::Failed(_)) {
                state.status = Status::Idle;
            }
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    updater.on_never(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };

        if !held.borrow().confirm_never {
            held.borrow_mut().confirm_never = true;

            let landing = Rc::clone(&held);
            let ctx_inner = Rc::clone(&ctx);
            let weak_inner = ui.as_weak();

            held.borrow().timer.start(TimerMode::SingleShot, Duration::from_millis(CONFIRM_MS), move || {
                landing.borrow_mut().confirm_never = false;

                if let Some(ui) = weak_inner.upgrade() {
                    refresh(&ui, &landing, &ctx_inner);
                }
            });
            refresh(&ui, &held, &ctx);
            return;
        }

        info!("User chose to never check for updates");
        ctx.state.apply(|settings| settings.general.update_mode = UpdateMode::Ignore);

        {
            let mut state = held.borrow_mut();

            state.open = false;
            state.confirm_never = false;
            state.status = Status::Idle;
        }

        refresh(&ui, &held, &ctx);
    });

    let (held, ctx, weak) = on(&state, context, window);
    updater.on_install(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let uri = match &held.borrow().status {
            Status::Ready(_, uri) => uri.clone(),
            _ => return,
        };

        if let Err(reason) = platform::install(&uri) {
            error!("The package installer could not be opened: {reason}");
            held.borrow_mut().status = Status::Failed(reason);
        } else {
            held.borrow_mut().open = false;
        }

        refresh(&ui, &held, &ctx);
    });

    refresh(window, &state, context);

    if context.state.settings().general.update_mode == UpdateMode::Prompt {
        check(window, &state, context, false);
    }

    state
}

fn check(ui: &AppWindow, state: &Shared, context: &Rc<Context>, manual: bool) {
    if !matches!(state.borrow().status, Status::Idle | Status::Current | Status::Failed(_)) {
        return;
    }

    info!("Checking GitHub for a newer release");
    state.borrow_mut().status = Status::Checking;
    refresh(ui, state, context);

    let landing = Rc::clone(state);
    let ctx = Rc::clone(context);

    bridge::offload(
        ui,
        "update_check",
        found,
        move |ui, outcome| {
            {
                let mut held = landing.borrow_mut();

                if !matches!(held.status, Status::Checking) {
                    return;
                }

                held.status = match outcome {
                    Ok(Some(target)) => {
                        info!(version = target.version, "A newer release is available");
                        held.open = true;
                        Status::Found(target)
                    }
                    Ok(None) => {
                        info!("This build is up to date");
                        if manual { Status::Current } else { Status::Idle }
                    }
                    Err(reason) => {
                        warn!("The update check failed: {reason}");
                        if manual { Status::Failed(reason) } else { Status::Idle }
                    }
                };
            }

            if matches!(landing.borrow().status, Status::Current) {
                flash(ui, &landing, &ctx);
            }

            refresh(ui, &landing, &ctx);
        },
    );
}

fn flash(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let landing = Rc::clone(state);
    let ctx = Rc::clone(context);
    let weak = ui.as_weak();

    state.borrow().timer.start(TimerMode::SingleShot, Duration::from_millis(BANNER_MS), move || {
        {
            let mut held = landing.borrow_mut();

            if matches!(held.status, Status::Current) {
                held.status = Status::Idle;
            }
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &landing, &ctx);
        }
    });
}

fn download(ui: &AppWindow, state: &Shared, context: &Rc<Context>, target: Offering) {
    let Some(tickets) = state.borrow().tickets else {
        return;
    };

    info!(version = target.version, "Downloading the update");

    {
        let mut held = state.borrow_mut();

        held.status = Status::Downloading(target.clone(), 0.0);
        held.open = true;
        held.confirm_never = false;
    }

    let weak = ui.as_weak();
    let progress = weak.clone();
    let spawned = thread::Builder::new().name("update_download".to_owned()).spawn(move || {
        let outcome = fetch(&target, &|fraction| bridge::post(&progress, tickets.progress, fraction));

        bridge::post(&weak, tickets.finished, outcome);
    });

    if let Err(err) = spawned {
        error!("The update download thread could not be started: {err}");
        state.borrow_mut().status = Status::Failed(err.to_string());
    }

    refresh(ui, state, context);
}

fn refresh(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let updater = ui.global::<Updater>();
    let held = state.borrow();
    let (label, tone, enabled): (&str, Tone, bool) = match &held.status {
        Status::Idle => ("Check for Updates", Tone::Primary, true),
        Status::Checking => ("Checking...", Tone::StatusWarning, false),
        Status::Current => ("Up to Date!", Tone::StatusSuccess, false),
        Status::Found(_) => ("Update Available!", Tone::Success, true),
        Status::Downloading(..) => ("Downloading...", Tone::Warning, true),
        Status::Ready(..) => ("Ready to Install!", Tone::Success, true),
        Status::Failed(_) => ("Failed to Check!", Tone::StatusDanger, true),
    };

    updater.set_label(label.into());
    updater.set_tone(tone);
    updater.set_enabled(enabled);
    updater.set_mode(if context.state.settings().general.update_mode == UpdateMode::Prompt { 0 } else { 1 });
    updater.set_open(held.open && held.prompts());
    updater.set_never_label(if held.confirm_never { "Continue?".into() } else { "Never".into() });

    match &held.status {
        Status::Found(target) => {
            updater.set_kind(0);
            updater.set_version(target.version.as_str().into());
        }
        Status::Downloading(target, fraction) => {
            updater.set_kind(1);
            updater.set_version(target.version.as_str().into());
            updater.set_fraction(*fraction);
        }
        Status::Ready(target, _) => {
            updater.set_kind(2);
            updater.set_version(target.version.as_str().into());
        }
        Status::Failed(reason) => {
            updater.set_kind(3);
            updater.set_failure(reason.as_str().into());
        }
        Status::Idle | Status::Checking | Status::Current => {}
    }
}
