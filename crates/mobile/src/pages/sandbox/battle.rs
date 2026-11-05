use std::rc::Rc;
use std::time::{Duration, Instant};

use slint::{ComponentHandle, Image, ModelRc, Rgba8Pixel, SharedPixelBuffer, Timer, TimerMode, VecModel};
use tracing::{info, warn};

use emulator::runtime::phone_activity;
use foundry::domains::sandbox::replay::Insets;

use crate::platform;
use crate::host::{self, Canvas, Command, Diagnostics, Event, Launch, Occupant, Phase, Standing, Touch};
use crate::pages::Context;
use crate::settings::Orientation;
use crate::{AppWindow, Sandbox, Shell, VitalRow};

use super::state::Volume;
use super::{Shared, refresh, report};

const LEAVE_HOLD_MS: u64 = 2000;
const LEAVE_HINT: &str = "Hold to Exit";
const REPLAY_TERMINATED: &str = "Replay Terminated!";
const PINCH_SPREAD: f32 = 300.0;
const TOUCH_DOWN: i32 = 0;
const TOUCH_MOVE: i32 = 1;
const TOUCH_UP: i32 = 2;
const DOUBLE_TAP: Duration = Duration::from_millis(400);

pub struct Part {
    pub standing: Standing,
    pub failure: Option<String>,
    pub frame: Option<Image>,
    pub scene_live: bool,
    pub immersed: bool,
    pub canvas: Canvas,
    pub bounds: (f32, f32),
    pub hint: String,
    pub diagnosing: bool,
    pub probe: Option<Diagnostics>,
    pub hold: Timer,
    pub blackout: bool,
    pub tapped: Option<Instant>,
    pub turning: Option<bool>,
}

impl Default for Part {
    fn default() -> Self {
        Self {
            standing: Standing::default(),
            failure: None,
            frame: None,
            scene_live: false,
            immersed: false,
            canvas: Canvas { surfaced: false, scale: 1.0, padded: false, aspect: 0.0 },
            bounds: (0.0, 0.0),
            hint: String::new(),
            diagnosing: false,
            probe: None,
            hold: Timer::default(),
            blackout: false,
            tapped: None,
            turning: None,
        }
    }
}

pub fn bounds(ui: &AppWindow) -> (f32, f32) {
    let shell = ui.global::<Shell>();

    (shell.get_app_width().max(1.0), shell.get_app_height().max(1.0))
}

pub fn device_insets(fullscreen: bool, width: f32, height: f32, density: f32) -> Insets {
    let pixels = |length: f32| (length * density.max(1.0)).round() as i32;
    let activity = platform::activity().unwrap_or_else(|| phone_activity(pixels(width), pixels(height)));
    let insets = host::insets_of(&activity);

    if fullscreen { insets } else { Insets { side: 0, top: 0, bottom: 0, ..insets } }
}

pub fn launch(ui: &AppWindow, state: &Shared, context: &Rc<Context>, launch: Launch) {
    {
        let mut held = state.borrow_mut();
        let (scale, padded) = {
            let saved = held.saved();

            (saved.render_scale(), saved.padded())
        };

        held.battle.failure = None;
        held.battle.frame = None;
        held.battle.scene_live = false;
        held.battle.bounds = (launch.width, launch.height);

        if let Some(arena) = &held.arena {
            arena.send(Command::Display { scale, padded });
            let density = ui.window().scale_factor();

            arena.send(Command::Density(density));
            arena.send(Command::Insets(device_insets(held.saved().fullscreen, launch.width, launch.height, density)));
            arena.send(Command::Start(Box::new(launch)));
        }
    }

    state.borrow_mut().status.clear();
    refresh(ui, state, context);
}

pub fn wire(window: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let sandbox = window.global::<Sandbox>();
    let on = |state: &Shared, context: &Rc<Context>, window: &AppWindow| (Rc::clone(state), Rc::clone(context), window.as_weak());

    let (held, _, _) = on(state, context, window);
    platform::on_shown(move |shown| {
        if let Some(arena) = &held.borrow().arena {
            arena.send(Command::Hold(!shown));
        }
    });

    let (held, _, weak) = on(state, context, window);
    sandbox.on_touch(move |kind, x, y| {
        let held_state = held.borrow();
        let battle = &held_state.battle;
        let (width, height) = battle.bounds;
        let (x, y) = battle.canvas.unpad(x, y, width, height);
        let inside = (0.0..=width).contains(&x) && (0.0..=height).contains(&y);
        let density = weak.upgrade().map_or(1.0, |ui| ui.window().scale_factor().max(1.0));
        let (x, y) = ((x.clamp(0.0, width) * density).round() as i32, (y.clamp(0.0, height) * density).round() as i32);
        let touch = match kind {
            TOUCH_DOWN if inside => Some(Touch::Pressed { x, y }),
            TOUCH_DOWN => None,
            TOUCH_MOVE => Some(Touch::Moved { x, y }),
            TOUCH_UP => Some(Touch::Released),
            _ => None,
        };

        if let (Some(touch), Some(arena)) = (touch, &held_state.arena) {
            arena.send(Command::Touches(vec![touch]));
        }
    });

    let (held, _, _) = on(state, context, window);
    sandbox.on_pinch(move |factor| {
        let spread = ((factor - 1.0) * PINCH_SPREAD) as i32;

        if spread != 0
            && let Some(arena) = &held.borrow().arena
        {
            arena.send(Command::Touches(vec![Touch::Pinched { spread }]));
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_hold_started(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };

        if !held.borrow().battle.standing.watching {
            return;
        }

        held.borrow_mut().battle.hint = LEAVE_HINT.to_owned();

        let landing = Rc::clone(&held);
        let ctx_inner = Rc::clone(&ctx);
        let weak_inner = ui.as_weak();

        held.borrow().battle.hold.start(TimerMode::SingleShot, Duration::from_millis(LEAVE_HOLD_MS), move || {
            {
                let mut state = landing.borrow_mut();

                state.battle.hint.clear();

                if let Some(arena) = &state.arena {
                    arena.send(Command::Stop);
                }
            }

            if let Some(ui) = weak_inner.upgrade() {
                refresh(&ui, &landing, &ctx_inner);
            }
        });
        refresh(&ui, &held, &ctx);
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_hold_ended(move || {
        {
            let mut state = held.borrow_mut();

            state.battle.hold.stop();
            state.battle.hint.clear();
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, _, weak) = on(state, context, window);
    sandbox.on_resized(move |width, height| {
        let mut state = held.borrow_mut();

        if width < 1.0 || height < 1.0 {
            return;
        }

        state.battle.bounds = (width, height);

        let turned = state.battle.turning == Some(width > height);

        if turned {
            state.battle.turning = None;
        }

        if let Some(arena) = &state.arena {
            if let Some(ui) = weak.upgrade() {
                let density = ui.window().scale_factor();

                arena.send(Command::Density(density));
                arena.send(Command::Insets(device_insets(state.saved().fullscreen, width, height, density)));
            }

            arena.send(Command::Resize(width, height));

            if turned {
                arena.send(Command::Turned(true));
            }
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_resume(move || {
        if let Some(arena) = &held.borrow().arena {
            arena.send(Command::Resume);
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_terminate(move || {
        if let Some(arena) = &held.borrow().arena {
            arena.send(Command::Stop);
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_close_diagnostics(move || {
        {
            let mut state = held.borrow_mut();

            state.battle.diagnosing = false;
            state.battle.probe = None;

            if let Some(arena) = &state.arena {
                arena.send(Command::Diagnose(false));
            }
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_tapped(move || {
        let now = Instant::now();
        let doubled = {
            let mut state = held.borrow_mut();
            let doubled = state.battle.standing.watching && state.battle.tapped.take().is_some_and(|at| now.duration_since(at) <= DOUBLE_TAP);

            if !doubled {
                state.battle.tapped = Some(now);
            }

            doubled
        };

        if !doubled {
            return;
        }

        {
            let mut state = held.borrow_mut();

            state.battle.hold.stop();
            state.battle.hint.clear();
            state.battle.diagnosing = true;

            if let Some(arena) = &state.arena {
                arena.send(Command::Diagnose(true));
            }
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });
}

pub fn event(ui: &AppWindow, state: &Shared, context: &Rc<Context>, event: Event) {
    match event {
        Event::Standing(standing) => {
            let was_faulted = state.borrow().battle.standing.phase == Phase::Faulted;
            let failed = state.borrow().battle.failure.clone();

            if standing.phase != Phase::Faulted
                && let Some(text) = failed
            {
                state.borrow_mut().battle.failure = None;

                if !was_faulted {
                    report(ui, state, context, text);
                }
            }

            if standing.phase == Phase::Idle {
                let mut held = state.borrow_mut();

                held.battle.frame = None;
                held.battle.scene_live = false;
                host::forget_scene();
                held.battle.probe = None;
                held.battle.diagnosing = false;
                held.battle.hint.clear();
                held.battle.turning = None;
            }

            let before = state.borrow().battle.standing.phase;

            if before != standing.phase {
                let wanted = match standing.phase {
                    Phase::Loading => Some(state.borrow().saved().battle_orientation),
                    Phase::Leaving | Phase::Idle => Some(context.state.orientation()),
                    _ => None,
                };

                if let Some(orientation) = wanted {
                    let mut held = state.borrow_mut();

                    held.battle.blackout = standing.phase == Phase::Loading;
                    held.battle.immersed = standing.phase == Phase::Loading;
                    drop(held);
                    platform::set_orientation(orientation);

                    if standing.phase == Phase::Leaving {
                        let mut held = state.borrow_mut();
                        let (width, height) = held.battle.bounds;
                        let wide = width > height;
                        let settled = !platform::ORIENTATION_ENFORCED
                            || match orientation {
                                Orientation::Automatic => true,
                                Orientation::Portrait => !wide,
                                Orientation::Landscape => wide,
                            };

                        held.battle.turning = (!settled).then_some(!wide);

                        if settled && let Some(arena) = &held.arena {
                            arena.send(Command::Turned(false));
                        }
                    }
                }
            }

            state.borrow_mut().battle.standing = standing;
            refresh(ui, state, context);
        }
        Event::Painted(painted) => {
            let image = Image::from_rgba8_premultiplied(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&painted.pixels, painted.width, painted.height));
            let mut held = state.borrow_mut();

            host::note_presented();
            held.battle.blackout = false;
            held.battle.frame = Some(image);
            held.battle.canvas = painted.canvas;
            drop(held);
            sync(ui, state);
        }
        Event::Scene(scene) => {
            {
                let mut held = state.borrow_mut();

                held.battle.blackout = false;
                held.battle.frame = None;
                held.battle.scene_live = true;
                held.battle.canvas = scene.canvas;
            }

            host::submit_scene(*scene);
            ui.window().request_redraw();
            sync(ui, state);
        }
        Event::Failure(failure) => {
            if let Some(text) = &failure {
                warn!("emu: {text}");
            }

            state.borrow_mut().battle.failure = failure;
            refresh(ui, state, context);
        }
        Event::Options(options) => {
            state.borrow().store.update(|saved| {
                saved.music_volume = options.music;
                saved.effects_volume = options.effects;
                saved.two_rows = options.two_rows;
                saved.vibrate = options.vibrate;
            });
            refresh(ui, state, context);
        }
        Event::Diagnostics(probe) => {
            state.borrow_mut().battle.probe = Some(probe);
            refresh(ui, state, context);
        }
        Event::Cut => context.toast(ui, REPLAY_TERMINATED, true),
        Event::Shelved => {
            if state.borrow().saved().tab == super::state::Tab::Replay {
                super::replay::relist(ui, state, context);
            }
        }
        Event::Ended => {
            info!("emu: the battle ended");

            if state.borrow().saved().tab == super::state::Tab::Replay {
                super::replay::relist(ui, state, context);
            }

            refresh(ui, state, context);
        }
    }
}

pub fn retune(state: &Shared) {
    let held = state.borrow();
    let saved = held.saved();

    if let Some(arena) = &held.arena {
        arena.send(Command::Volumes(Volume::nearest(saved.music_volume).percent(), Volume::nearest(saved.effects_volume).percent()));
        arena.send(Command::Phone(saved.device() == super::state::Device::Phone));
        arena.send(Command::Lag(saved.lag));
        arena.send(Command::Display { scale: saved.render_scale(), padded: saved.padded() });
    }
}

fn vitals(side: &[crate::host::Vitals], base: &str, fallback: &str) -> Vec<VitalRow> {
    side.iter()
        .map(|vitals| {
            let name = match &vitals.occupant {
                Occupant::Base => base.to_owned(),
                Occupant::Unit { id, name } if name.is_empty() => format!("{fallback} {id:03}"),
                Occupant::Unit { name, .. } => name.clone(),
            };

            VitalRow { name: name.into(), hp: vitals.hp.to_string().into() }
        })
        .collect()
}

pub fn sync(ui: &AppWindow, state: &Shared) {
    let sandbox = ui.global::<Sandbox>();
    let held = state.borrow();
    let battle = &held.battle;
    let standing = battle.standing;

    sandbox.set_active(standing.phase != Phase::Idle);
    sandbox.set_blackout(battle.blackout);
    sandbox.set_immersed(battle.immersed);
    sandbox.set_has_frame(battle.frame.is_some() || battle.scene_live);

    if let Some(frame) = &battle.frame {
        sandbox.set_frame(frame.clone());
    }

    sandbox.set_covered(standing.covered);
    sandbox.set_frozen(standing.phase == Phase::Faulted);
    sandbox.set_watching(standing.watching && standing.phase == Phase::Running);
    sandbox.set_hint(battle.hint.as_str().into());
    sandbox.set_fault_open(standing.phase == Phase::Faulted);
    sandbox.set_fault_reason(battle.failure.clone().unwrap_or_default().into());
    sandbox.set_diagnostics_open(battle.diagnosing && battle.probe.is_some());

    if let Some(probe) = &battle.probe {
        sandbox.set_seed(probe.rng.to_string().into());
        sandbox.set_vital_cats(ModelRc::new(VecModel::from(vitals(&probe.cats, "Cat Base", "Cat"))));
        sandbox.set_vital_enemies(ModelRc::new(VecModel::from(vitals(&probe.enemies, "Enemy Base", "Enemy"))));
    }
}
