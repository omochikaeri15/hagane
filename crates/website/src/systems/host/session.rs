use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use serde::{Deserialize, Serialize};
use tracing::{trace, warn};

use emulator::runtime::{BattleOptions, Setup};
use foundry::domains::sandbox::replay::Footage;

use super::assets::SheetCache;
use super::diagnostics::Diagnostics;
use super::driver::{Clock, Driver, Equipment, Label};
use super::input::TouchQueue;
use super::keys::Action;
use super::sink::Frame;

const BLACK_FROM: i32 = 0xb;
pub const CLOSED_FRAME: i32 = 0xc;
const LAST_FRAME: i32 = 0x18;
const FRAME_MS: f64 = 33.0;
const STEP_MS: f64 = 1000.0 / 30.0;
const HALF_STEP_MS: f64 = STEP_MS / 2.0;
const MOST_STEPS: u32 = 4;
const PRESENT_PATIENCE_MS: f64 = 2000.0;
const TURN_PATIENCE_MS: f64 = 2000.0;
const TURN_SETTLE_MS: f64 = 100.0;
const SLOW_STEP_MS: f64 = 100.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Lag {
    #[default]
    CatchUp,
    Slowdown,
}

impl Lag {
    pub(crate) const ALL: [Self; 2] = [Self::CatchUp, Self::Slowdown];

    fn most_steps(self) -> u32 {
        match self {
            Self::CatchUp => MOST_STEPS,
            Self::Slowdown => 1,
        }
    }
}

impl fmt::Display for Lag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::CatchUp => "Catch-up",
            Self::Slowdown => "Slowdown",
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Phase {
    Idle,
    Covering,
    Loading,
    Running,
    Faulted,
    Closing,
    Leaving,
}

pub struct Session {
    driver: Driver,
    frame: Rc<RefCell<Frame>>,
    clock: Rc<dyn Clock>,
    phase: Phase,
    sweep: i32,
    swept: f64,
    started: f64,
    entered: bool,
    frozen: usize,
    discard: bool,
    due: Option<f64>,
    lag: Lag,
    failure: Option<String>,
    stale: bool,
    again: bool,
    presented: bool,
    held: Option<bool>,
    opens_at: Option<f64>,
    veiled: bool,
    primed: bool,
    warm: bool,
    reel: Option<Footage>,
    recording: bool,
    cut: bool,
}

impl Session {
    pub fn new(equipment: Equipment) -> Self {
        let clock = Rc::clone(&equipment.clock);
        let driver = Driver::new(equipment);
        let frame = Rc::clone(driver.frame());
        let now = clock.now_ms();

        Self {
            driver,
            frame,
            clock,
            phase: Phase::Idle,
            sweep: 0,
            swept: now,
            started: now,
            entered: false,
            frozen: 0,
            discard: false,
            due: None,
            lag: Lag::default(),
            failure: None,
            stale: false,
            again: false,
            presented: false,
            held: None,
            opens_at: None,
            veiled: false,
            primed: false,
            warm: true,
            reel: None,
            recording: true,
            cut: false,
        }
    }

    pub fn frame(&self) -> &Rc<RefCell<Frame>> {
        &self.frame
    }

    pub fn sheets(&self) -> &Rc<RefCell<SheetCache>> {
        self.driver.sheets()
    }

    pub fn touches(&self) -> &TouchQueue {
        self.driver.touches()
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn surfaced(&self) -> bool {
        match self.phase {
            Phase::Loading | Phase::Running | Phase::Faulted | Phase::Closing => true,
            Phase::Leaving => self.opens_at.is_some() || self.veiled,
            Phase::Idle | Phase::Covering => false,
        }
    }

    pub fn covered(&self) -> bool {
        match self.phase {
            Phase::Idle | Phase::Faulted => false,
            Phase::Running => self.driver.fading(),
            Phase::Closing => true,
            Phase::Covering | Phase::Loading | Phase::Leaving => (BLACK_FROM..=CLOSED_FRAME).contains(&self.sweep),
        }
    }

    fn in_transition(&self) -> bool {
        matches!(self.phase, Phase::Covering | Phase::Loading | Phase::Closing | Phase::Leaving)
    }

    pub fn set_recording(&mut self, recording: bool) {
        self.recording = recording;
    }

    pub fn label(&mut self, label: Label) {
        self.driver.label(label);
    }

    pub fn watching(&self) -> bool {
        self.driver.watching()
    }

    pub fn watch(&mut self, footage: Footage) {
        if self.in_transition() {
            return;
        }

        self.begin();
        self.reel = Some(footage);
    }

    pub fn take_cut(&mut self) -> bool {
        std::mem::take(&mut self.cut)
    }

    pub fn refit(&mut self) {
        if self.phase != Phase::Idle {
            self.stale = true;

            return;
        }

        self.driver.renew();
        self.driver.forget_assets();
    }

    pub fn take_footage(&mut self) -> Option<Footage> {
        self.driver.take_footage()
    }

    pub fn begin(&mut self) {
        if self.in_transition() {
            return;
        }

        self.reel = None;

        self.phase = Phase::Covering;
        self.sweep = 0;
        self.started = self.clock.now_ms();
        self.swept = self.started;
        self.entered = false;
        self.failure = None;
        self.presented = false;
        self.held = Some(false);
        self.primed = false;
    }

    pub fn forget_assets(&mut self) {
        if self.phase == Phase::Idle {
            self.driver.forget_assets();
        } else {
            self.stale = true;
        }
    }

    pub fn relocalize(&mut self, priority: Vec<String>) {
        self.driver.relocalize(priority);
        self.forget_assets();
    }

    pub fn equip(&mut self, setup: Setup) {
        self.driver.set_setup(setup);
    }

    pub fn set_collision(&mut self, look: Option<foundry::domains::settings::CombatLook>) {
        self.driver.set_collision(look);
    }

    pub fn configure(&mut self, options: BattleOptions) {
        self.driver.set_options(options);
    }

    pub fn take_options(&mut self) -> Option<BattleOptions> {
        self.driver.take_options()
    }

    pub fn set_phone(&mut self, phone: bool) {
        self.driver.set_phone(phone);
    }

    pub fn retune(&mut self, music: i32, effects: i32) {
        self.driver.set_volumes(music, effects);
    }

    pub fn set_lag(&mut self, lag: Lag) {
        self.lag = lag;
    }

    pub fn resume(&mut self) {
        if self.phase != Phase::Faulted {
            return;
        }

        self.driver.forgive();
        self.failure = None;
        self.entered = true;
        self.phase = Phase::Running;
        self.due = None;
    }

    pub fn terminate(&mut self) {
        if !matches!(self.phase, Phase::Faulted | Phase::Running) {
            return;
        }

        if self.phase == Phase::Running {
            self.entered = false;
            self.driver.silence();
            self.frozen = self.frame.borrow().quads.len();
        }

        self.phase = Phase::Closing;
        self.discard = true;
        self.sweep = 0;
        self.started = self.clock.now_ms();
        self.swept = self.started;
    }

    pub fn diagnose(&self) -> Option<Diagnostics> {
        matches!(self.phase, Phase::Running | Phase::Faulted).then(|| self.driver.diagnose())
    }

    pub fn failure(&self) -> Option<&str> {
        self.failure.as_deref()
    }

    pub fn resize(&mut self, width: f32, height: f32) {
        self.driver.resize(width, height);
    }

    pub fn presented(&mut self) {
        self.presented = true;
    }

    pub fn turned(&mut self, rotated: bool) {
        if self.phase != Phase::Leaving || self.opens_at.is_none() {
            return;
        }

        let now = self.clock.now_ms();

        if rotated {
            self.opens_at = Some(now + TURN_SETTLE_MS);
        } else {
            self.opens_at = None;
            self.sweep = CLOSED_FRAME + 1;
            self.swept = now;
        }
    }

    fn leave(&mut self) {
        let now = self.clock.now_ms();

        self.phase = Phase::Leaving;
        self.sweep = CLOSED_FRAME;
        self.started = now;
        self.swept = now;
        self.opens_at = Some(now + TURN_PATIENCE_MS);
        self.veiled = false;
    }

    pub fn warmed(&mut self) {
        self.warm = true;
    }

    pub fn expect_warmth(&mut self) {
        self.warm = false;
    }

    pub fn tick(&mut self) -> bool {
        let now = self.clock.now_ms();

        match self.phase {
            Phase::Idle | Phase::Faulted => false,
            Phase::Closing => {
                self.paced(now);
                self.sweep = self.sweep.min(CLOSED_FRAME);
                self.frame.borrow_mut().quads.truncate(self.frozen);
                self.driver.draw_curtain_over(self.sweep);

                if self.sweep >= CLOSED_FRAME {
                    if self.driver.watching() {
                        self.driver.end_playback();
                        self.driver.draw_curtain(self.sweep);
                    }

                    if self.again {
                        self.phase = Phase::Loading;
                        self.started = now;
                        self.swept = now;
                        self.presented = false;
                    } else {
                        self.leave();
                    }
                }

                true
            }
            Phase::Running => self.pace(now),
            Phase::Covering => {
                if let Some(posted) = self.held {
                    let waited = now - self.started;

                    if !self.presented && waited < PRESENT_PATIENCE_MS {
                        if !posted {
                            self.held = Some(true);
                            self.driver.draw_curtain(self.sweep);
                        }

                        return !posted;
                    }

                    if !self.presented {
                        warn!(ms = waited as u32, "emu: the first curtain frame never reached the screen; closing it anyway");
                    }

                    self.held = None;
                    self.swept = now;
                }

                self.paced(now);
                self.sweep = self.sweep.min(CLOSED_FRAME);

                if self.sweep >= CLOSED_FRAME {
                    self.phase = Phase::Loading;
                }

                self.driver.draw_curtain(self.sweep);

                if self.phase == Phase::Loading {
                    self.started = now;
                    self.presented = false;
                }

                true
            }
            Phase::Loading => {
                let waited = now - self.started;

                if !self.presented {
                    if waited < PRESENT_PATIENCE_MS {
                        return false;
                    }

                    if self.primed {
                        warn!(ms = waited as u32, "emu: the first battle frame never reached the screen; running anyway");
                    } else {
                        warn!(ms = waited as u32, "emu: the closed curtain never reached the screen; loading the battle anyway");
                    }
                }

                if std::mem::take(&mut self.primed) {
                    self.phase = Phase::Running;
                    self.due = None;

                    return false;
                }

                if !self.warm {
                    return false;
                }

                trace!(waited_ms = waited as u32, "emu: loading the battle behind the closed curtain");

                if std::mem::take(&mut self.again) {
                    self.driver.renew();
                }

                let mark = self.clock.now_ms();

                match self.reel.take() {
                    Some(footage) => match footage.cues() {
                        Ok(frames) => {
                            self.driver.arm_playback(&footage.save, frames, footage.faults());
                            self.driver.draw_curtain(self.sweep);
                            self.load();
                        }
                        Err(reason) => {
                            warn!("emu: the replay could not be loaded: {reason}");
                            self.failure = Some(reason);
                        }
                    },
                    None => {
                        if self.recording {
                            self.driver.arm_recording();
                        }

                        self.load();
                        self.driver.stamp_deck();
                        self.driver.keep_assets();
                    }
                }

                trace!(entered = self.entered, ms = (self.clock.now_ms() - mark) as u32, failure = ?self.failure, "emu: battle load finished");

                if self.entered {
                    self.step();

                    if self.phase == Phase::Loading {
                        self.primed = true;
                        self.presented = false;
                        self.started = self.clock.now_ms();
                    }
                } else {
                    self.leave();
                    self.driver.end_playback();
                    self.driver.draw_curtain(self.sweep);
                }

                true
            }
            Phase::Leaving => {
                if let Some(opens) = self.opens_at {
                    if now < opens {
                        self.driver.draw_curtain(CLOSED_FRAME);

                        return true;
                    }

                    self.opens_at = None;
                    self.veiled = true;
                    self.presented = false;
                    self.started = now;
                }

                if self.veiled {
                    if !self.presented && now - self.started < PRESENT_PATIENCE_MS {
                        self.driver.draw_curtain(CLOSED_FRAME);

                        return true;
                    }

                    self.veiled = false;
                    self.sweep = CLOSED_FRAME + 1;
                    self.swept = now;
                }

                self.paced(now);

                if self.sweep > LAST_FRAME {
                    self.phase = Phase::Idle;
                    self.driver.release_curtain();
                    self.frame.borrow_mut().clear();

                    if self.discard {
                        self.discard = false;
                        self.failure = None;
                    }

                    self.driver.renew();

                    if std::mem::take(&mut self.stale) {
                        self.driver.forget_assets();
                    }

                    return true;
                }

                self.driver.draw_curtain(self.sweep);

                true
            }
        }
    }

    pub fn key(&mut self, action: Action, pressed: bool) {
        if self.phase == Phase::Running {
            self.driver.key(action, pressed);
        }
    }

    pub fn restart(&mut self) {
        if self.phase != Phase::Running || self.driver.watching() {
            return;
        }

        self.again = true;
        self.entered = false;
        self.driver.silence();
        self.frozen = self.frame.borrow().quads.len();
        self.phase = Phase::Closing;
        self.sweep = 0;
        self.started = self.clock.now_ms();
        self.swept = self.started;
    }

    fn paced(&mut self, now: f64) {
        if now - self.swept >= FRAME_MS {
            self.sweep += 1;
            self.swept = now;
        }
    }

    fn pace(&mut self, now: f64) -> bool {
        let mut due = self.due.unwrap_or(now);
        let most = self.lag.most_steps();
        let mut steps = 0;

        while steps < most && self.phase == Phase::Running && now + HALF_STEP_MS >= due {
            self.step();
            due += STEP_MS;
            steps += 1;
        }

        let behind = now + HALF_STEP_MS >= due;

        if behind && self.phase == Phase::Running && self.lag == Lag::CatchUp {
            trace!(steps, "emu: the sandbox fell too far behind and let the missed frames go");
        }

        self.due = Some(if behind { now + STEP_MS } else { due });

        steps > 0
    }

    fn step(&mut self) {
        let began = self.clock.now_ms();
        let advanced = self.driver.advance();
        let spent = self.clock.now_ms() - began;

        if spent >= SLOW_STEP_MS {
            trace!(ms = spent as u32, in_battle = self.driver.in_battle(), fading = self.driver.fading(), "emu: slow battle frame");
        }

        if let Err(reason) = advanced {
            if self.driver.watching() {
                if self.driver.forgive_scripted() {
                    return;
                }

                warn!("emu: the replay was terminated by a fault: {reason}");
                self.cut = true;
                self.terminate();
                self.discard = false;

                return;
            }

            self.failure = Some(reason);
            self.phase = Phase::Faulted;
            self.entered = false;
            self.driver.silence();
            self.driver.dim();
            self.frozen = self.frame.borrow().quads.len();

            return;
        }

        if self.driver.exit_requested() || self.driver.reel_finished() {
            self.cut |= self.driver.reel_finished() && self.driver.cut();
            self.terminate();

            return;
        }

        if self.entered && !self.driver.in_battle() {
            self.entered = false;
            self.driver.silence();
            self.leave();
            self.driver.end_playback();
            self.driver.draw_curtain(self.sweep);
        }
    }

    fn load(&mut self) {
        if !self.driver.boot() {
            self.failure = Some(format!("game data did not load\n{} files visible to the emulator", self.driver.resolved()));

            return;
        }

        if !self.driver.enter_battle() {
            self.failure = Some("battle entry refused - see the log".to_owned());

            return;
        }

        self.entered = true;
    }
}
