use std::cell::RefCell;
use std::fmt;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tracing::{error, info, trace, warn};

use emulator::runtime::{BattleOptions, Setup};
use foundry::Vfs;
use foundry::common::job::JobOutcome;
use foundry::domains::sandbox::replay as tape;
use foundry::domains::settings::{ReplaySource, SavedBcvName};

use super::assets::{DiskAssets, FileIndex, SheetCache};
use super::diagnostics::Diagnostics;
use super::driver::{Driver, Label};
use super::gpu;
use super::input::TouchQueue;
use super::sink::Frame;

const BLACK_FROM: i32 = 0xb;
const CLOSED_FRAME: i32 = 0xc;
const LAST_FRAME: i32 = 0x18;
const SAVE_PATIENCE: Duration = Duration::from_secs(5);
const GAME_FPS: u64 = 30;
const FRAME_TIME: Duration = STEP_TIME;
const STEP_TIME: Duration = Duration::from_nanos(1_000_000_000 / GAME_FPS);
const EARLY_STEP: Duration = Duration::from_millis(1);
const MOST_STEPS: u32 = 4;
const PRESENT_PATIENCE: Duration = Duration::from_secs(2);
const TURN_PATIENCE: Duration = Duration::from_secs(2);
const TURN_SETTLE: Duration = Duration::from_millis(300);
const SLOW_STEP: Duration = Duration::from_millis(100);

struct Opened {
    save: tape::Save,
    frames: Vec<Vec<tape::Cue>>,
    faults: Vec<tape::Forgiven>,
    index: FileIndex,
}

fn open(reel: &Reel) -> Result<Opened, String> {
    match reel {
        Reel::Latest => {
            let dir = tape::latest().ok_or("there is no state folder holding the latest battle")?;

            Ok(Opened {
                save: tape::read_save(&dir)?,
                frames: tape::read_input(&dir)?,
                faults: tape::read_faults(&dir),
                index: tape::index(&dir).map_err(|error| format!("the replay's asset list could not be read: {error}"))?,
            })
        }
        Reel::Bundle(bundle) => {
            let opened = tape::Bundle::open(bundle)?;

            Ok(Opened { save: opened.save()?, frames: opened.input()?, faults: opened.faults(), index: opened.index() })
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Lag {
    CatchUp,
    #[default]
    Slowdown,
}

impl Lag {
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

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Reel {
    Latest,
    Bundle(PathBuf),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Idle,
    Covering,
    Loading,
    Running,
    Faulted,
    Closing,
    Saving,
    Leaving,
}

struct Saver {
    done: Arc<AtomicBool>,
    began: Instant,
}

impl Saver {
    fn busy(&self) -> bool {
        !self.done.load(Ordering::Acquire)
    }
}

pub(super) type Shelved = Arc<dyn Fn() + Send + Sync>;

pub(super) struct Session {
    driver: Driver,
    frame: Rc<RefCell<Frame>>,
    phase: Phase,
    sweep: i32,
    started: Instant,
    entered: bool,
    frozen: usize,
    discard: bool,
    due: Option<Instant>,
    held_since: Option<Instant>,
    opens_at: Option<Instant>,
    curtain_seen: u64,
    curtain_stepped: Instant,
    lag: Lag,
    failure: Option<String>,
    stale: bool,
    again: bool,
    reel: Option<(Reel, ReplaySource)>,
    recording: bool,
    auto_save: Option<(SavedBcvName, Shelved)>,
    saver: Option<Saver>,
    cut: bool,
}

impl Session {
    pub(super) fn new() -> Self {
        let driver = Driver::new();
        let frame = Rc::clone(driver.frame());

        Self {
            driver,
            frame,
            phase: Phase::Idle,
            sweep: 0,
            stale: false,
            again: false,
            started: Instant::now(),
            entered: false,
            frozen: 0,
            discard: false,
            due: None,
            held_since: None,
            opens_at: None,
            curtain_seen: 0,
            curtain_stepped: Instant::now(),
            lag: Lag::default(),
            failure: None,
            reel: None,
            recording: true,
            auto_save: None,
            saver: None,
            cut: false,
        }
    }

    pub(super) fn set_lag(&mut self, lag: Lag) {
        self.lag = lag;
    }

    pub(super) fn set_auto_save(&mut self, auto_save: Option<(SavedBcvName, Shelved)>) {
        self.auto_save = auto_save;
    }

    fn start_saving(&mut self) -> bool {
        let Some((naming, shelved)) = self.auto_save.clone() else {
            return false;
        };

        if !self.driver.finish_recording() {
            return false;
        }

        let Some(latest) = tape::latest() else {
            return false;
        };
        let out = tape::saved_bundle(&tape::library(), naming, &latest);
        let done = Arc::new(AtomicBool::new(false));
        let landed = Arc::clone(&done);
        let spawned = thread::Builder::new().name("replay_auto_save".to_owned()).spawn(move || {
            if matches!(tape::pack(&latest, &out, |_| {}, &AtomicBool::new(false)), JobOutcome::Completed) {
                info!("The battle was saved to {}", out.display());
            }

            landed.store(true, Ordering::Release);
            shelved();
        });

        match spawned {
            Ok(_) => {
                self.saver = Some(Saver { done, began: Instant::now() });

                true
            }
            Err(err) => {
                error!("The auto-save thread could not be started: {err}");

                false
            }
        }
    }

    pub(super) fn set_recording(&mut self, recording: bool) {
        self.recording = recording;
    }

    pub(super) fn frame(&self) -> &Rc<RefCell<Frame>> {
        &self.frame
    }

    pub(super) fn sheets(&self) -> &Rc<RefCell<SheetCache>> {
        self.driver.sheets()
    }

    pub(super) fn touches(&self) -> &TouchQueue {
        self.driver.touches()
    }

    pub(super) fn phase(&self) -> Phase {
        self.phase
    }

    pub(super) fn surfaced(&self) -> bool {
        matches!(self.phase, Phase::Loading | Phase::Running | Phase::Faulted | Phase::Closing | Phase::Saving)
    }

    pub(super) fn covered(&self) -> bool {
        match self.phase {
            Phase::Idle | Phase::Faulted => false,
            Phase::Running => self.driver.fading(),
            Phase::Closing | Phase::Saving => true,
            Phase::Covering | Phase::Loading | Phase::Leaving => (BLACK_FROM..=CLOSED_FRAME).contains(&self.sweep),
        }
    }

    pub(super) fn running(&self) -> bool {
        self.phase != Phase::Idle
    }

    fn in_transition(&self) -> bool {
        matches!(self.phase, Phase::Covering | Phase::Loading | Phase::Closing | Phase::Saving | Phase::Leaving)
    }

    pub(super) fn watching(&self) -> bool {
        self.driver.watching()
    }

    pub(super) fn aspect(&self) -> f32 {
        if self.driver.watching() { self.driver.frame_aspect().max(0.0) } else { 0.0 }
    }

    pub(super) fn watch(&mut self, reel: Reel, source: ReplaySource) {
        if self.in_transition() {
            return;
        }

        self.begin();
        self.reel = Some((reel, source));
    }

    pub(super) fn begin(&mut self) {
        if self.in_transition() {
            return;
        }

        self.reel = None;
        self.phase = Phase::Covering;
        self.sweep = 0;
        self.started = Instant::now();
        self.entered = false;
        self.failure = None;
    }

    pub(super) fn forget_assets(&mut self) {
        if self.phase == Phase::Idle {
            self.driver.forget_assets();
        } else {
            self.stale = true;
        }
    }

    pub(super) fn equip(&mut self, setup: Setup) {
        self.driver.set_setup(setup);
    }

    pub(super) fn label(&mut self, label: Label) {
        self.driver.label(label);
    }

    pub fn set_collision(&mut self, look: Option<foundry::domains::settings::CombatLook>) {
        self.driver.set_collision(look);
    }

    pub(super) fn configure(&mut self, options: BattleOptions) {
        self.driver.set_options(options);
    }

    pub(super) fn take_options(&mut self) -> Option<BattleOptions> {
        self.driver.take_options()
    }

    pub(super) fn set_insets(&mut self, insets: Option<tape::Insets>) {
        self.driver.set_insets(insets);
    }

    pub(super) fn set_phone(&mut self, phone: bool) {
        self.driver.set_phone(phone);
    }

    pub(super) fn retune(&mut self, music: i32, effects: i32) {
        self.driver.set_volumes(music, effects);
    }

    pub(super) fn resume(&mut self) {
        if self.phase != Phase::Faulted {
            return;
        }

        self.driver.forgive();
        self.failure = None;
        self.entered = true;
        self.phase = Phase::Running;
        self.due = None;
    }

    pub(super) fn hold(&mut self, held: bool) {
        self.driver.hold_sound(held);

        if held {
            if self.held_since.is_none() && self.phase == Phase::Running {
                self.driver.focus_changed(false);
            }

            self.held_since.get_or_insert_with(Instant::now);
        } else if let Some(since) = self.held_since.take() {
            self.started += since.elapsed();
            self.due = None;

            if self.phase == Phase::Running {
                self.driver.focus_changed(true);
            }
        }
    }

    pub(super) fn turned(&mut self, rotated: bool) {
        if self.opens_at.is_some() {
            self.opens_at = Some(Instant::now() + if rotated { TURN_SETTLE } else { Duration::ZERO });
        }
    }

    pub(super) fn until_due(&self) -> Option<Duration> {
        (self.phase == Phase::Running).then_some(self.due?.saturating_duration_since(Instant::now()))
    }

    pub(super) fn hush(&self) {
        self.driver.hush();
    }

    pub(super) fn terminate(&mut self) {
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
        self.started = Instant::now();
    }

    pub(super) fn diagnose(&self) -> Option<Diagnostics> {
        matches!(self.phase, Phase::Running | Phase::Faulted).then(|| self.driver.diagnose())
    }

    pub(super) fn take_cut(&mut self) -> bool {
        std::mem::take(&mut self.cut)
    }

    pub(super) fn failure(&self) -> Option<&str> {
        self.failure.as_deref()
    }

    pub(super) fn resize(&mut self, width: f32, height: f32) {
        self.driver.resize(width, height);
    }

    pub(super) fn tick(&mut self, vfs: &Vfs) -> bool {
        let elapsed = self.started.elapsed().div_duration_f32(FRAME_TIME) as i32;

        match self.phase {
            Phase::Idle | Phase::Faulted => false,
            Phase::Closing => {
                self.sweep = elapsed.min(CLOSED_FRAME);
                self.frame.borrow_mut().quads.truncate(self.frozen);
                self.driver.draw_curtain_over(self.sweep);

                if self.sweep >= CLOSED_FRAME {
                    self.phase = match (self.start_saving(), self.again) {
                        (true, _) => Phase::Saving,
                        (false, true) => Phase::Loading,
                        (false, false) => Phase::Leaving,
                    };

                    if self.phase == Phase::Leaving {
                        self.opens_at = Some(Instant::now() + TURN_PATIENCE);
                    }

                    if self.driver.watching() {
                        self.driver.end_playback();
                        self.driver.draw_curtain(self.sweep);
                    }

                    self.started = Instant::now();

                    if self.phase == Phase::Loading {
                        self.await_curtain();
                    }
                }

                true
            }
            Phase::Running => self.pace(),
            Phase::Covering => {
                self.sweep = elapsed.min(CLOSED_FRAME);

                if self.sweep >= CLOSED_FRAME {
                    self.phase = Phase::Loading;
                }

                self.driver.draw_curtain(self.sweep);

                if self.phase == Phase::Loading {
                    self.started = Instant::now();
                    self.await_curtain();
                }

                true
            }
            Phase::Saving => {
                self.sweep = CLOSED_FRAME;
                self.driver.draw_curtain(self.sweep);

                if self.saver.as_ref().is_some_and(|saver| saver.busy() && saver.began.elapsed() < SAVE_PATIENCE) {
                    return true;
                }

                self.phase = if self.again { Phase::Loading } else { Phase::Leaving };
                self.started = Instant::now();

                if self.phase == Phase::Leaving {
                    self.opens_at = Some(Instant::now() + TURN_PATIENCE);
                } else {
                    self.await_curtain();
                }

                true
            }
            Phase::Loading => {
                if self.saver.as_ref().is_some_and(Saver::busy) {
                    return false;
                }

                let waited = self.started.elapsed();

                if !self.frame.borrow().presented.shown.load(Ordering::Acquire) {
                    if waited < PRESENT_PATIENCE {
                        return false;
                    }

                    warn!(ms = waited.as_millis(), "emu: the closed curtain never reached the screen; loading the battle anyway");
                }

                trace!(waited_ms = waited.as_millis(), "emu: loading the battle behind the closed curtain");

                if std::mem::take(&mut self.again) {
                    self.driver.renew();
                }

                match self.reel.take() {
                    Some((reel, source)) => match self.cue(&reel, source, vfs) {
                        Ok(()) => {
                            self.driver.draw_curtain(self.sweep);
                            self.load();
                        }
                        Err(reason) => {
                            warn!("emu: the replay could not be loaded: {reason}");
                            self.failure = Some(reason);
                        }
                    },
                    None => {
                        self.driver.reindex(vfs);

                        if self.recording {
                            self.driver.arm_recording();
                        }

                        self.load();
                        self.driver.stamp_deck();
                        self.driver.keep_assets();
                    }
                }

                trace!(entered = self.entered, failure = ?self.failure, "emu: battle load finished");

                if self.entered {
                    self.phase = Phase::Running;
                    self.due = None;
                } else {
                    self.phase = Phase::Leaving;
                    self.opens_at = Some(Instant::now() + TURN_PATIENCE);
                    self.started = Instant::now();
                    self.driver.end_playback();
                    self.driver.draw_curtain(self.sweep);
                }

                true
            }
            Phase::Leaving => {
                if let Some(opens_at) = self.opens_at {
                    if Instant::now() < opens_at {
                        self.sweep = CLOSED_FRAME;
                        self.driver.draw_curtain(self.sweep);

                        return true;
                    }

                    self.opens_at = None;
                    self.curtain_seen = gpu::presented().wrapping_sub(1);
                    self.curtain_stepped = Instant::now();
                }

                let waited = self.curtain_stepped.elapsed();
                let seen = gpu::presented();

                if !(waited >= PRESENT_PATIENCE || (waited >= FRAME_TIME && seen != self.curtain_seen)) {
                    return false;
                }

                let now = Instant::now();

                self.curtain_seen = seen;
                self.curtain_stepped = (self.curtain_stepped + FRAME_TIME).max(now.checked_sub(FRAME_TIME).unwrap_or(now));
                self.sweep += 1;

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

    fn await_curtain(&mut self) {
        self.frame.borrow().presented.shown.store(false, Ordering::Release);
    }

    fn pace(&mut self) -> bool {
        let now = Instant::now();
        let mut due = self.due.unwrap_or(now);
        let most = self.lag.most_steps();
        let mut steps = 0;

        while steps < most && self.phase == Phase::Running && now + EARLY_STEP >= due {
            self.step();
            due += STEP_TIME;
            steps += 1;
        }

        let behind = now + EARLY_STEP >= due;

        if behind && self.phase == Phase::Running && self.lag == Lag::CatchUp {
            trace!(steps, "emu: the sandbox fell too far behind and let the missed frames go");
        }

        self.due = Some(if behind { now + STEP_TIME } else { due });

        steps > 0
    }

    fn step(&mut self) {
        let began = Instant::now();
        let advanced = self.driver.advance();
        let spent = began.elapsed();

        if spent >= SLOW_STEP {
            trace!(ms = spent.as_millis(), in_battle = self.driver.in_battle(), fading = self.driver.fading(), "emu: slow battle frame");
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

            if self.start_saving() {
                self.phase = Phase::Saving;
            } else {
                self.phase = Phase::Leaving;
                self.opens_at = Some(Instant::now() + TURN_PATIENCE);
            }

            self.sweep = CLOSED_FRAME;
            self.driver.end_playback();
            self.started = Instant::now();
            self.driver.draw_curtain(self.sweep);
        }
    }

    fn cue(&mut self, reel: &Reel, source: ReplaySource, vfs: &Vfs) -> Result<(), String> {
        let opened = open(reel)?;
        let index = match source {
            ReplaySource::Bcv => opened.index,
            ReplaySource::Vfs => DiskAssets::index(vfs),
            ReplaySource::Hybrid => {
                let mut index: FileIndex = DiskAssets::index(vfs).into_iter().filter(|(name, _)| tape::unit_file(name).is_some()).collect();

                index.extend(opened.index);
                index
            }
        };

        self.driver.arm_playback(&opened.save, opened.frames, opened.faults, index);

        Ok(())
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

impl Drop for Session {
    fn drop(&mut self) {
        if self.in_transition() {
            warn!("emu: session dropped mid-transition");
        }
    }
}
