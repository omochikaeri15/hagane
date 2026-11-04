use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

use slint::Weak;
use tracing::{error, info};

use emulator::runtime::{BattleOptions, Setup};
use foundry::Vault;
use foundry::domains::settings::ReplaySource;

use crate::{AppWindow, bridge};

use super::diagnostics::Diagnostics;
use super::driver::Label;
use super::gpu::{self, Scene};
use super::input::Touch;
use super::session::{Lag, Phase, Reel, Session};
use super::viewport::{self, Canvas};

const TICK: Duration = Duration::from_millis(16);
const LEAST_SIDE: u32 = 1;

pub struct Launch {
    pub setup: Setup,
    pub options: BattleOptions,
    pub phone: bool,
    pub lag: Lag,
    pub width: f32,
    pub height: f32,
    pub label: Label,
    pub record: bool,
}

pub enum Command {
    Start(Box<Launch>),
    Watch { reel: Reel, source: ReplaySource, phone: bool, lag: Lag, width: f32, height: f32 },
    Stop,
    Resume,
    Turned(bool),
    Touches(Vec<Touch>),
    Resize(f32, f32),
    Phone(bool),
    Volumes(i32, i32),
    Lag(Lag),
    Display { scale: f32, padded: bool },
    Density(f32),
    Hold(bool),
    Diagnose(bool),
    Vault(Arc<Vault>),
    Quit,
}

pub struct Painted {
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub canvas: Canvas,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Standing {
    pub phase: Phase,
    pub covered: bool,
    pub surfaced: bool,
    pub watching: bool,
}

impl Default for Standing {
    fn default() -> Self {
        Self { phase: Phase::Idle, covered: false, surfaced: false, watching: false }
    }
}

pub enum Event {
    Standing(Standing),
    Painted(Painted),
    Scene(Box<Scene>),
    Failure(Option<String>),
    Options(BattleOptions),
    Diagnostics(Diagnostics),
    Cut,
    Ended,
}

pub struct Arena {
    sender: Sender<Command>,
}

struct Stage {
    session: Option<Session>,
    vault: Arc<Vault>,
    weak: Weak<AppWindow>,
    ticket: bridge::Ticket,
    bounds: (f32, f32),
    scale: f32,
    padded: bool,
    density: f32,
    held: bool,
    diagnosing: bool,
    standing: Standing,
    failure: Option<String>,
    was_running: bool,
}

impl Stage {
    fn post(&self, event: Event) {
        bridge::post(&self.weak, self.ticket, event);
    }

    fn session(&mut self) -> &mut Session {
        self.session.get_or_insert_with(Session::new)
    }

    fn handle(&mut self, command: Command) -> bool {
        match command {
            Command::Start(launch) => {
                let Launch { setup, options, phone, lag, width, height, label, record } = *launch;

                self.bounds = (width, height);

                let session = self.session();

                session.set_recording(record);
                session.label(label);
                session.equip(setup);
                session.set_phone(phone);
                session.set_lag(lag);
                session.resize(width, height);
                session.configure(options);
                session.begin();
                self.failure = None;
            }
            Command::Watch { reel, source, phone, lag, width, height } => {
                self.bounds = (width, height);

                let session = self.session();

                session.set_phone(phone);
                session.set_lag(lag);
                session.resize(width, height);
                session.watch(reel, source);
                self.failure = None;
            }
            Command::Stop => {
                if let Some(session) = self.session.as_mut() {
                    session.terminate();
                }
            }
            Command::Resume => {
                if let Some(session) = self.session.as_mut() {
                    session.resume();
                }
            }
            Command::Touches(touches) => {
                if let Some(session) = self.session.as_ref() {
                    session.touches().borrow_mut().extend(touches);
                }
            }
            Command::Resize(width, height) => {
                self.bounds = (width, height);

                if let Some(session) = self.session.as_mut() {
                    session.resize(width, height);
                }
            }
            Command::Phone(phone) => {
                if let Some(session) = self.session.as_mut() {
                    session.set_phone(phone);
                }
            }
            Command::Volumes(music, effects) => {
                if let Some(session) = self.session.as_mut() {
                    session.retune(music, effects);
                }
            }
            Command::Lag(lag) => {
                if let Some(session) = self.session.as_mut() {
                    session.set_lag(lag);
                }
            }
            Command::Display { scale, padded } => {
                self.scale = scale;
                self.padded = padded;
            }
            Command::Density(density) => self.density = density.max(1.0),
            Command::Turned(rotated) => {
                if let Some(session) = self.session.as_mut() {
                    session.turned(rotated);
                }
            }
            Command::Hold(held) => {
                self.held = held;

                if let Some(session) = self.session.as_mut() {
                    session.hold(held);
                }
            }
            Command::Diagnose(on) => self.diagnosing = on,
            Command::Vault(vault) => {
                self.vault = vault;

                if let Some(session) = self.session.as_mut() {
                    session.forget_assets();
                }
            }
            Command::Quit => return false,
        }

        true
    }

    fn wait(&self) -> Duration {
        self.session.as_ref().and_then(Session::until_due).map_or(TICK, |left| left.min(TICK))
    }

    fn running(&self) -> bool {
        self.session.as_ref().is_some_and(Session::running)
    }

    fn tick(&mut self) {
        let diagnosing = self.diagnosing;
        let Some(session) = self.session.as_mut() else {
            return;
        };
        let changed = session.tick(&self.vault.vfs);
        let standing = Standing { phase: session.phase(), covered: session.covered(), surfaced: session.surfaced(), watching: session.watching() };
        let failure = session.failure().map(str::to_owned);
        let options = session.take_options();
        let cut = session.take_cut();
        let probe = if diagnosing { session.diagnose() } else { None };
        let running = session.running();

        #[cfg(target_os = "ios")]
        if self.was_running && !running {
            super::offscreen::forget();
        }

        if changed {
            self.paint();
        }

        if standing != self.standing {
            self.standing = standing;
            self.post(Event::Standing(standing));
        }

        if failure != self.failure {
            self.failure.clone_from(&failure);
            self.post(Event::Failure(failure));
        }

        if let Some(options) = options {
            self.post(Event::Options(options));
        }

        if cut {
            self.post(Event::Cut);
        }

        if let Some(probe) = probe {
            self.post(Event::Diagnostics(probe));
        }

        if self.was_running && !running {
            if let Some(session) = self.session.as_ref() {
                session.hush();
            }

            self.post(Event::Ended);
        }

        self.was_running = running;
    }

    fn paint(&mut self) {
        let Some(session) = self.session.as_ref() else {
            return;
        };
        let (width, height) = self.bounds;

        if width < 1.0 || height < 1.0 {
            return;
        }

        let surfaced = session.surfaced();
        let canvas = Canvas { surfaced, scale: if surfaced { self.scale } else { 1.0 }, padded: surfaced && self.padded, aspect: session.aspect() };
        let pixel_width = ((width * canvas.scale * self.density).round() as u32).max(LEAST_SIDE);
        let pixel_height = ((height * canvas.scale * self.density).round() as u32).max(LEAST_SIDE);
        let frame = session.frame().borrow();

        if frame.quads.is_empty() {
            return;
        }

        let sheets = session.sheets().borrow();
        let event = if gpu::available() {
            Event::Scene(Box::new(viewport::scene(&frame, &sheets, canvas, width, height, pixel_width, pixel_height)))
        } else {
            let pixels = viewport::paint(&frame, &sheets, canvas, width, height, pixel_width, pixel_height);

            Event::Painted(Painted { pixels, width: pixel_width, height: pixel_height, canvas })
        };

        frame.presented.shown.store(true, Ordering::Release);
        drop(sheets);
        drop(frame);
        self.post(event);
    }
}

impl Arena {
    pub fn spawn(weak: Weak<AppWindow>, ticket: bridge::Ticket, vault: Arc<Vault>) -> Option<Self> {
        let (sender, receiver) = mpsc::channel::<Command>();
        let spawned = thread::Builder::new().name("battle".to_owned()).spawn(move || {
            let mut stage = Stage { session: None, vault, weak, ticket, bounds: (0.0, 0.0), scale: 1.0, padded: false, density: 1.0, held: false, diagnosing: false, standing: Standing::default(), failure: None, was_running: false };

            info!("emu: the battle thread is up");

            loop {
                let command = if stage.running() && !stage.held { receiver.recv_timeout(stage.wait()) } else { receiver.recv().map_err(|_| RecvTimeoutError::Disconnected) };

                match command {
                    Ok(command) => {
                        if !stage.handle(command) {
                            break;
                        }

                        while let Ok(command) = receiver.try_recv() {
                            if !stage.handle(command) {
                                return;
                            }
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => break,
                }

                if !stage.held {
                    stage.tick();
                }
            }

            info!("emu: the battle thread is down");
        });

        match spawned {
            Ok(_) => Some(Self { sender }),
            Err(err) => {
                error!("The battle thread could not be started: {err}");

                None
            }
        }
    }

    pub fn send(&self, command: Command) {
        if self.sender.send(command).is_err() {
            error!("The battle thread is gone, a command was dropped");
        }
    }
}

impl Drop for Arena {
    fn drop(&mut self) {
        let _ = self.sender.send(Command::Quit);
    }
}
