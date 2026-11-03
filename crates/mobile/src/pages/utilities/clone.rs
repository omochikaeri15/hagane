use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::thread;

use slint::ComponentHandle;
use tracing::{error, info};

use foundry::common::job::{JobEvent, JobOutcome};
use foundry::systems::apk::clone::{self, CloneConfig};

use crate::pages::Context;
use crate::picker;
use crate::{AppWindow, Tone, Utilities, bridge};

use super::Shared;

const CLONE_IDLE: &str = "Clone App";
const CLONE_BUSY: &str = "Cloning...";

#[derive(Default)]
pub struct State {
    apk: Option<PathBuf>,
    icon: Option<PathBuf>,
    app_name: String,
    package: String,
    running: bool,
    log: String,
    ticket: Option<bridge::Ticket>,
}

impl State {
    fn config(&self) -> Option<CloneConfig> {
        Some(CloneConfig { input: self.apk.clone()?, app_name: self.app_name.clone(), package: self.package.clone(), icon: self.icon.clone() })
    }

    fn ready(&self) -> bool {
        !self.running && self.config().is_some_and(|config| config.is_actionable())
    }
}

pub fn wire(window: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let utilities = window.global::<Utilities>();
    let on = |state: &Shared, context: &Rc<Context>, window: &AppWindow| (Rc::clone(state), Rc::clone(context), window.as_weak());

    {
        let held = Rc::clone(state);
        let ticket = bridge::subscribe::<JobEvent>(move |ui, event| job_event(ui, &held, event));

        state.borrow_mut().clone.ticket = Some(ticket);
    }

    let (held, ctx, weak) = on(state, context, window);
    utilities.on_pick_apk(move || {
        if let Some(ui) = weak.upgrade() {
            let landing = Rc::clone(&held);

            picker::open(
                &ui,
                &ctx,
                "Select an APK",
                &["apk", "xapk", "apkm", "apks"],
                Box::new(move |ui, _, path| {
                    landing.borrow_mut().clone.apk = Some(path);
                    refresh(ui, &landing);
                }),
            );
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    utilities.on_pick_icon(move || {
        if let Some(ui) = weak.upgrade() {
            let landing = Rc::clone(&held);

            picker::open(
                &ui,
                &ctx,
                "Select a PNG",
                &["png"],
                Box::new(move |ui, _, path| {
                    landing.borrow_mut().clone.icon = Some(path);
                    refresh(ui, &landing);
                }),
            );
        }
    });

    let (held, _, weak) = on(state, context, window);
    utilities.on_clear_icon(move || {
        held.borrow_mut().clone.icon = None;

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held);
        }
    });

    let (held, _, weak) = on(state, context, window);
    utilities.on_name_changed(move |value| {
        held.borrow_mut().clone.app_name = value.to_string();

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held);
        }
    });

    let (held, _, weak) = on(state, context, window);
    utilities.on_package_changed(move |value| {
        held.borrow_mut().clone.package = value.to_string();

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held);
        }
    });

    let (held, _, weak) = on(state, context, window);
    utilities.on_start_clone(move || {
        if let Some(ui) = weak.upgrade() {
            start(&ui, &held);
        }
    });
}

fn start(ui: &AppWindow, state: &Shared) {
    let (config, ticket) = {
        let held = state.borrow();

        if !held.clone.ready() {
            return;
        }

        (held.clone.config(), held.clone.ticket)
    };
    let (Some(config), Some(ticket)) = (config, ticket) else {
        return;
    };

    {
        let mut held = state.borrow_mut();

        held.clone.running = true;
        held.clone.log.clear();
    }

    let weak = ui.as_weak();
    let emit = move |event: JobEvent| bridge::post(&weak, ticket, event);
    let spawned = thread::Builder::new().name("apk_clone".to_owned()).spawn(move || {
        let outcome = clone::run(config, &emit);

        emit(JobEvent::Finished(outcome.map_or_else(JobOutcome::Failed, |_| JobOutcome::Completed)));
    });

    if let Err(err) = spawned {
        error!("The clone thread could not be started: {err}");
        state.borrow_mut().clone.running = false;
    }

    refresh(ui, state);
}

fn job_event(ui: &AppWindow, state: &Shared, event: JobEvent) {
    {
        let mut held = state.borrow_mut();

        match event {
            JobEvent::Log(line) => {
                held.clone.log.push_str(&line);
                held.clone.log.push('\n');
            }
            JobEvent::Progress { .. } => {}
            JobEvent::Finished(outcome) => {
                held.clone.running = false;

                match outcome {
                    JobOutcome::Completed => info!("Clone job completed"),
                    JobOutcome::Aborted => info!("Clone job aborted"),
                    JobOutcome::Failed(message) => {
                        error!("Clone Error: {message}");
                        held.clone.log.push_str(&format!("!! ERROR: {message}\n"));
                    }
                }
            }
        }
    }

    refresh(ui, state);
}

pub fn refresh(ui: &AppWindow, state: &Shared) {
    let utilities = ui.global::<Utilities>();
    let held = state.borrow();
    let clone = &held.clone;
    let face = |picked: Option<&Path>, idle: &str| picked.map_or((idle.to_owned(), Tone::Neutral), |path| (picker::label(path), Tone::Success));
    let (apk_label, apk_tone) = face(clone.apk.as_deref(), "Add APK");
    let (icon_label, icon_tone) = face(clone.icon.as_deref(), "Add PNG");

    utilities.set_clone_apk_label(apk_label.into());
    utilities.set_clone_apk_tone(apk_tone);
    utilities.set_clone_icon_label(icon_label.into());
    utilities.set_clone_icon_tone(icon_tone);
    utilities.set_clone_icon_set(clone.icon.is_some());
    utilities.set_clone_name(clone.app_name.as_str().into());
    utilities.set_clone_package(clone.package.as_str().into());
    utilities.set_clone_label(if clone.running { CLONE_BUSY.into() } else { CLONE_IDLE.into() });
    utilities.set_clone_enabled(clone.ready());
    utilities.set_clone_log(clone.log.as_str().into());
}
