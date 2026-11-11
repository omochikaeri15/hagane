mod export;
mod scene;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::time::{Duration, Instant};

use slint::{ComponentHandle, Image, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, Timer, TimerMode, VecModel};
use tracing::warn;

use codex::graphics::animate::{FrameData, resolve_frame};
use codex::graphics::rig::{Animation, BoundingBox, Rig};
use foundry::domains::cat::animation as cat_animation;
use foundry::domains::cat::scanner::CatEntry;
use foundry::domains::enemy::animation as enemy_animation;
use foundry::domains::enemy::scanner::EnemyEntry;
use foundry::domains::settings::{AnimSettings, CombatLook};
use foundry::systems::animation::collision;
use foundry::systems::animation::{Loop, MotionSet, NO_OFFSET, Placement, Rigging, cycle, loop_frame, restart, restart_frame};

use crate::host;
use crate::pages::Context;
use crate::{AppWindow, MotionTile, Sandbox, Theme, Viewer, bridge};

use scene::View;

const ZOOM_MIN: f32 = 0.1;
const ZOOM_MAX: f32 = 10.0;
const ZOOM_RATE_PER_PIXEL: f32 = 0.0016;
const ZOOM_SCROLL_STRENGTH: f32 = 2.0;
const ZOOM_DECAY_RATE: f32 = 16.0;
const ZOOM_EPSILON: f32 = 0.05;
const WHEEL_LINE_PIXELS: f32 = 0.6;
const FRAME_ADVANCE_PER_TICK: f32 = 0.5;
const TICK_MS: f64 = 1000.0 / 60.0;
const LONGEST_TICK_MS: f64 = 100.0;
const HOLD_OLD_FRAME: Duration = Duration::from_millis(500);
const HOLD_DELAY_MS: f64 = 200.0;
const HOLD_FRAMES_PER_SEC: f32 = 30.0;
const CULL_SCALE: f32 = 100.0;
const MIN_SELECTION_AREA: f32 = 25.0;
const COLUMNS: usize = 4;
const DEFAULT_SLOTS: usize = 8;
const GRID_ROWS: usize = 2;
const DISPLAY_CEILING: i32 = 999_999;
const MAX_SCALE: f32 = 2.0;
const MAX_SIDE: u32 = 4096;
const TILE_W: f32 = 70.0;
const TILE_H: f32 = 25.0;
const TILE_INSET: f32 = 2.0;
const TILE_STEPS: [f32; 6] = [13.0, 12.0, 11.0, 10.0, 9.0, 8.0];
const GLYPH_ADVANCE: f32 = 0.52;
const LINE_HEIGHT: f32 = 1.1;
const RIG_JOB: &str = "rig";

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Region {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

struct Reach {
    rig: usize,
    anim: usize,
    offset: Option<usize>,
    bounds: Option<BoundingBox>,
}

type Lit = (bool, bool, bool, Option<u32>, CombatLook);
type Stamp = (usize, usize, i32, (i32, i32), i32, (u32, u32), Lit, bool, Option<Region>);

pub struct State {
    key: Option<String>,
    set: Option<Rc<MotionSet>>,
    slots: Vec<Option<usize>>,
    selected_label: Option<String>,
    rigs: HashMap<String, Option<Arc<Rig>>>,
    inflight: HashSet<String>,
    anims: HashMap<String, Arc<Animation>>,
    frame: f32,
    playing: bool,
    pan: (f32, f32),
    zoom: f32,
    zoom_pending: f32,
    loop_start: Option<f32>,
    loop_end: Option<f32>,
    range_inputs: (String, String),
    placement: Placement,
    hold: Option<(i8, f64)>,
    size: (f32, f32),
    scale: f32,
    selecting: bool,
    region: Option<Region>,
    last_time: Option<Instant>,
    last_stamp: Option<Stamp>,
    reach: Option<Reach>,
    pixels: Vec<u8>,
    gpu_sheet: Option<(Arc<Rig>, Arc<[u8]>)>,
    waiting_since: Option<Instant>,
    active: bool,
    timer: Timer,
    export: export::Form,
}

pub type Shared = Rc<RefCell<State>>;

fn unit_of(set: &str) -> &str {
    set.split('_').next().unwrap_or(set)
}

fn fitted_size(label: &str) -> f32 {
    let room_w = TILE_W - TILE_INSET * 2.0;
    let room_h = TILE_H - TILE_INSET * 2.0;
    let glyphs = label.chars().count() as f32;

    TILE_STEPS
        .into_iter()
        .find(|size| {
            let per_line = (room_w / (size * GLYPH_ADVANCE)).floor().max(1.0);
            let lines = (room_h / (size * LINE_HEIGHT)).floor().max(1.0);

            glyphs <= per_line * lines
        })
        .unwrap_or(8.0)
}

fn display_bound(bound: Option<i32>) -> String {
    match bound {
        Some(value) if value <= DISPLAY_CEILING => value.to_string(),
        _ => "???".to_owned(),
    }
}

#[derive(Clone, Copy)]
struct Request {
    slot: Option<usize>,
    trailing: bool,
}

fn place(set: &MotionSet) -> Vec<Option<usize>> {
    let requests: Vec<Request> = set.motions.iter().map(|motion| Request { slot: motion.slot, trailing: motion.file.is_none() }).collect();
    let mut slots: Vec<Option<usize>> = vec![None; DEFAULT_SLOTS];
    let settled = |request: &Request| !request.trailing;
    let mut anchored: Vec<(usize, usize)> = requests.iter().enumerate().filter(|(_, request)| settled(request)).filter_map(|(index, request)| Some((request.slot?, index))).collect();

    anchored.sort_by_key(|(slot, _)| *slot);

    for (slot, index) in anchored {
        settle(&mut slots, slot, 1, index);
    }

    for (index, _) in requests.iter().enumerate().filter(|(_, request)| settled(request) && request.slot.is_none()) {
        settle(&mut slots, 0, 1, index);
    }

    for (index, request) in requests.iter().enumerate().filter(|(_, request)| request.trailing) {
        settle(&mut slots, request.slot.unwrap_or(0), COLUMNS, index);
    }

    while !slots.len().is_multiple_of(COLUMNS) {
        slots.push(None);
    }

    slots
}

fn settle(slots: &mut Vec<Option<usize>>, from: usize, stride: usize, index: usize) {
    let mut at = from;

    loop {
        while at >= slots.len() {
            slots.push(None);
        }

        if slots[at].is_none() {
            slots[at] = Some(index);
            return;
        }

        at += stride;
    }
}

fn selected_index(set: &MotionSet, slots: &[Option<usize>], label: Option<&str>) -> Option<usize> {
    label.and_then(|label| set.motions.iter().position(|motion| motion.label() == label)).or_else(|| slots.iter().find_map(|slot| *slot))
}

fn offset_of(rig: Option<&Rig>, placement: Placement) -> Option<usize> {
    let rows = rig.map_or(0, |rig| rig.model.alignment.len());

    match placement {
        Placement::Bare => None,
        Placement::Row(row) if row < rows => Some(row),
        Placement::Row(_) => (rows > 0).then_some(0),
    }
}

fn offset_choices(set: &MotionSet, rig: Option<&Rig>) -> Vec<(String, Placement)> {
    let bare = set.offsets.iter().find(|held| held.row.is_none()).map_or(NO_OFFSET, |held| held.name).to_owned();
    let rows = rig.map_or(0, |rig| rig.model.alignment.len());
    let name = |row: usize| set.offsets.iter().find(|held| held.row == Some(row)).map_or_else(|| format!("Row {row}"), |held| held.name.to_owned());

    std::iter::once((bare, Placement::Bare)).chain((0..rows).map(|row| (name(row), Placement::Row(row)))).collect()
}

fn placement_matches(choice: Placement, current: Option<usize>) -> bool {
    match (choice, current) {
        (Placement::Bare, None) => true,
        (Placement::Row(row), Some(current)) => row == current,
        _ => false,
    }
}

fn loop_bound(looping: Loop, model: bool, anim: Option<&Animation>) -> Option<i32> {
    if model {
        return Some(0);
    }

    anim.map_or(Some(0), |anim| match looping {
        Loop::Exact => cycle(anim).map(|frames| frames - 1),
        Loop::Frames => Some(anim.declared_frames() - 1),
        Loop::Auto => restart(anim).map(|frames| frames - 1),
        Loop::Continuous => None,
    })
}

fn playback_frame(looping: Loop, anim: Option<&Animation>, frame: f32) -> f32 {
    let Some(anim) = anim else {
        return frame;
    };

    match looping {
        Loop::Exact | Loop::Continuous => frame,
        Loop::Frames => loop_frame(anim, frame),
        Loop::Auto => restart_frame(anim, frame),
    }
}

impl State {
    pub fn shared() -> Shared {
        Rc::new(RefCell::new(Self {
            key: None,
            set: None,
            slots: Vec::new(),
            selected_label: None,
            rigs: HashMap::new(),
            inflight: HashSet::new(),
            anims: HashMap::new(),
            frame: 0.0,
            playing: true,
            pan: (0.0, 0.0),
            zoom: 1.0,
            zoom_pending: 0.0,
            loop_start: None,
            loop_end: None,
            range_inputs: (String::new(), String::new()),
            placement: Placement::default(),
            hold: None,
            size: (1.0, 1.0),
            scale: 1.0,
            selecting: false,
            region: None,
            last_time: None,
            last_stamp: None,
            reach: None,
            pixels: Vec::new(),
            gpu_sheet: None,
            waiting_since: None,
            active: false,
            timer: Timer::default(),
            export: export::Form::default(),
        }))
    }

    fn selected(&self) -> Option<usize> {
        self.set.as_ref().and_then(|set| selected_index(set, &self.slots, self.selected_label.as_deref()))
    }

    fn motion(&self) -> Option<&foundry::systems::animation::Motion> {
        self.set.as_ref().and_then(|set| set.motions.get(self.selected()?))
    }

    fn rigging(&self) -> Option<&Arc<Rigging>> {
        self.motion().map(|motion| &motion.rig)
    }

    pub fn rig(&self) -> Option<Arc<Rig>> {
        self.rigs.get(&self.rigging()?.id).cloned().flatten()
    }

    fn rig_failed(&self) -> bool {
        self.rigging().is_some_and(|rigging| matches!(self.rigs.get(&rigging.id), Some(None)))
    }

    fn anim_key(motion: &foundry::systems::animation::Motion) -> String {
        format!("{}:{}", motion.rig.id, motion.label())
    }

    pub fn anim(&self) -> Option<Arc<Animation>> {
        let motion = self.motion()?;

        motion.file.as_ref()?;
        self.anims.get(&Self::anim_key(motion)).cloned()
    }

    fn is_model(&self) -> bool {
        self.motion().is_some_and(|motion| motion.file.is_none())
    }

    pub fn selected_file(&self) -> Option<std::path::PathBuf> {
        self.motion().and_then(|motion| motion.file.as_ref()).map(|file| file.path.clone())
    }

    fn looping(&self) -> Loop {
        self.motion().map_or(Loop::Frames, |motion| motion.looping)
    }

    pub fn loop_bound(&self) -> Option<i32> {
        loop_bound(self.looping(), self.is_model(), self.anim().as_deref())
    }

    pub fn playback(&self, frame: f32) -> f32 {
        playback_frame(self.looping(), self.anim().as_deref(), frame)
    }

    pub fn loop_supported(&self) -> bool {
        match self.looping() {
            Loop::Exact => true,
            Loop::Auto => self.anim().is_some_and(|anim| cycle(&anim).is_some()),
            Loop::Frames | Loop::Continuous => false,
        }
    }

    pub fn offset(&self) -> Option<usize> {
        offset_of(self.rig().as_deref(), self.placement)
    }

    pub fn view(&self) -> View {
        View { size: self.size, pan: self.pan, zoom: self.zoom }
    }

    fn frame_bounds(&self) -> (f32, Option<f32>) {
        if self.is_model() {
            return (0.0, Some(0.0));
        }

        let min = self.loop_start.unwrap_or(0.0).max(0.0);
        let max = self.loop_end.or_else(|| self.loop_bound().map(|bound| bound.max(0) as f32)).map(|max| max.max(min));

        (min, max)
    }

    fn clamp_frame(&mut self) {
        let (min, max) = self.frame_bounds();
        let clamped = max.map_or(self.frame.max(min), |max| self.frame.clamp(min, max));

        self.frame = clamped;
    }

    fn step(&mut self, direction: f32) {
        let (min, max) = self.frame_bounds();
        let stepped = self.frame + direction;

        self.frame = if stepped < min {
            max.unwrap_or(min)
        } else if max.is_some_and(|max| stepped > max) {
            min
        } else {
            stepped
        };
    }

    fn to_world(&self, point: (f32, f32)) -> (f32, f32) {
        let (width, height) = self.size;

        ((point.0 - width / 2.0) / self.zoom - self.pan.0, (point.1 - height / 2.0) / self.zoom - self.pan.1)
    }

    fn settle_zoom(&mut self, delta_ms: f64) {
        if self.zoom_pending == 0.0 {
            return;
        }

        let pending = self.zoom_pending;
        let step = if pending.abs() < ZOOM_EPSILON { pending } else { pending * (1.0 - (-ZOOM_DECAY_RATE * (delta_ms / 1000.0) as f32).exp()) };
        let factor = (step * ZOOM_RATE_PER_PIXEL).exp();

        self.zoom_pending = pending - step;
        self.zoom = (self.zoom * factor).clamp(ZOOM_MIN, ZOOM_MAX);
    }

    fn pinch(&mut self, factor: f32, center: (f32, f32)) {
        let (width, height) = self.size;
        let held = ((center.0 - width / 2.0) / self.zoom - self.pan.0, (center.1 - height / 2.0) / self.zoom - self.pan.1);
        let zoomed = (self.zoom * factor).clamp(ZOOM_MIN, ZOOM_MAX);

        self.zoom = zoomed;
        self.pan = ((center.0 - width / 2.0) / zoomed - held.0, (center.1 - height / 2.0) / zoomed - held.1);
    }

    fn pixel_size(&self) -> (u32, u32) {
        let scale = self.scale.min(MAX_SCALE);

        (((self.size.0 * scale).round() as u32).clamp(1, MAX_SIDE), ((self.size.1 * scale).round() as u32).clamp(1, MAX_SIDE))
    }
}

pub fn wire(window: &AppWindow, context: &Rc<Context>) {
    let viewer = window.global::<Viewer>();
    let on = |context: &Rc<Context>, window: &AppWindow| (Rc::clone(context), window.as_weak());

    let (ctx, _) = on(context, window);
    viewer.on_resized(move |width, height, _| {
        let mut state = ctx.viewer.borrow_mut();

        state.size = (width.max(1.0), height.max(1.0));
    });

    let (ctx, _) = on(context, window);
    viewer.on_pan(move |dx, dy| {
        let mut state = ctx.viewer.borrow_mut();
        let zoom = state.zoom;

        state.pan.0 += dx / zoom;
        state.pan.1 += dy / zoom;
    });

    let (ctx, _) = on(context, window);
    viewer.on_pinch(move |factor, x, y| {
        if factor > 0.0 {
            ctx.viewer.borrow_mut().pinch(factor, (x, y));
        }
    });

    let (ctx, _) = on(context, window);
    viewer.on_zoom_wheel(move |delta| {
        if delta != 0.0 {
            ctx.viewer.borrow_mut().zoom_pending += delta * WHEEL_LINE_PIXELS * ZOOM_SCROLL_STRENGTH;
        }
    });

    let (ctx, _) = on(context, window);
    viewer.on_orient(move || {
        let mut state = ctx.viewer.borrow_mut();

        state.pan = (0.0, 0.0);
        state.zoom = 1.0;
        state.zoom_pending = 0.0;
    });

    let (ctx, weak) = on(context, window);
    viewer.on_toggle_play(move || {
        {
            let mut state = ctx.viewer.borrow_mut();

            state.playing = !state.playing;
            state.hold = None;
        }

        if let Some(ui) = weak.upgrade() {
            sync_controls(&ui, &ctx);
        }
    });

    let (ctx, weak) = on(context, window);
    viewer.on_step(move |direction| {
        ctx.viewer.borrow_mut().step(direction as f32);

        if let Some(ui) = weak.upgrade() {
            sync_controls(&ui, &ctx);
        }
    });

    let (ctx, _) = on(context, window);
    viewer.on_hold(move |direction| {
        ctx.viewer.borrow_mut().hold = Some((direction.clamp(-1, 1) as i8, 0.0));
    });

    let (ctx, weak) = on(context, window);
    viewer.on_release(move || {
        {
            let mut state = ctx.viewer.borrow_mut();

            if let Some((direction, held_ms)) = state.hold.take()
                && held_ms <= HOLD_DELAY_MS
            {
                state.step(f32::from(direction));
            }
        }

        if let Some(ui) = weak.upgrade() {
            sync_controls(&ui, &ctx);
        }
    });

    let (ctx, weak) = on(context, window);
    viewer.on_set_frame(move |value| {
        {
            let mut state = ctx.viewer.borrow_mut();
            let trimmed = value.trim();
            let target = if trimmed.is_empty() { Some(0.0) } else { trimmed.parse::<f32>().ok() };

            if let Some(target) = target {
                state.frame = target;
                state.clamp_frame();
            }
        }

        if let Some(ui) = weak.upgrade() {
            sync_controls(&ui, &ctx);
        }
    });

    let (ctx, weak) = on(context, window);
    viewer.on_set_range_start(move |value| {
        {
            let mut state = ctx.viewer.borrow_mut();

            if value.is_empty() {
                state.loop_start = None;
            } else if let Ok(parsed) = value.parse::<f32>() {
                let start = parsed.max(0.0);

                state.loop_start = Some(start);

                if state.frame < start {
                    state.frame = start;
                }
            }

            state.range_inputs.0 = value.to_string();
        }

        if let Some(ui) = weak.upgrade() {
            sync_controls(&ui, &ctx);
        }
    });

    let (ctx, weak) = on(context, window);
    viewer.on_set_range_end(move |value| {
        {
            let mut state = ctx.viewer.borrow_mut();

            if value.is_empty() {
                state.loop_end = None;
            } else if let Ok(parsed) = value.parse::<f32>() {
                let end = parsed.max(0.0);

                state.loop_end = Some(end);

                if state.frame > end {
                    state.frame = end;
                }
            }

            state.range_inputs.1 = value.to_string();
        }

        if let Some(ui) = weak.upgrade() {
            sync_controls(&ui, &ctx);
        }
    });

    let (ctx, weak) = on(context, window);
    viewer.on_pick_tile(move |at| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let label = {
            let state = ctx.viewer.borrow();

            state.slots.get(at.max(0) as usize).copied().flatten().and_then(|index| state.set.as_ref()?.motions.get(index).map(|motion| motion.label()))
        };

        if let Some(label) = label {
            {
                let mut state = ctx.viewer.borrow_mut();

                state.selected_label = Some(label);
                state.clamp_frame();
            }

            load_selected(&ui, &ctx);
            sync_controls(&ui, &ctx);
        }
    });

    let (ctx, weak) = on(context, window);
    viewer.on_pick_offset(move |index| {
        {
            let mut state = ctx.viewer.borrow_mut();
            let choices = state.set.as_ref().map(|set| offset_choices(set, state.rig().as_deref())).unwrap_or_default();

            if let Some((_, placement)) = choices.get(index.max(0) as usize) {
                state.placement = *placement;
            }
        }

        if let Some(ui) = weak.upgrade() {
            sync_controls(&ui, &ctx);
        }
    });

    let (ctx, weak) = on(context, window);
    viewer.on_export_pressed(move || {
        if let Some(ui) = weak.upgrade() {
            export::open(&ui, &ctx);
        }
    });

    let (ctx, weak) = on(context, window);
    viewer.on_region_picked(move |x, y, w, h| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let picked = {
            let mut state = ctx.viewer.borrow_mut();

            state.selecting = false;

            if w * h <= MIN_SELECTION_AREA {
                None
            } else {
                let a = state.to_world((x, y));
                let b = state.to_world((x + w, y + h));
                let region = Region { x: a.0.min(b.0), y: a.1.min(b.1), w: (b.0 - a.0).abs(), h: (b.1 - a.1).abs() };

                state.region = Some(region);
                Some(region)
            }
        };

        ui.global::<Viewer>().set_selecting(false);
        export::region_picked(&ui, &ctx, picked);
    });

    let (ctx, weak) = on(context, window);
    viewer.on_region_canceled(move || {
        ctx.viewer.borrow_mut().selecting = false;

        if let Some(ui) = weak.upgrade() {
            ui.global::<Viewer>().set_selecting(false);
            export::region_picked(&ui, &ctx, None);
        }
    });

    let (ctx, weak) = on(context, window);
    viewer.on_toggle_debug(move |which, value| {
        ctx.state.apply(|settings| match which {
            0 => settings.animation.show_rig = value,
            1 => settings.animation.show_origin = value,
            2 => settings.animation.show_world = value,
            _ => settings.animation.show_combat = value,
        });

        if let Some(ui) = weak.upgrade() {
            sync_controls(&ui, &ctx);
        }
    });

    export::wire(window, context);
}

pub fn forget(context: &Rc<Context>) {
    let mut state = context.viewer.borrow_mut();

    state.key = None;
    state.rigs.clear();
    state.inflight.clear();
    state.anims.clear();
    state.last_stamp = None;
    state.reach = None;
    state.gpu_sheet = None;
}

pub fn hide(context: &Rc<Context>) {
    let mut state = context.viewer.borrow_mut();

    if state.active {
        state.active = false;
        state.timer.stop();
        state.last_time = None;
        state.last_stamp = None;
    }
}

pub fn show_cat(ui: &AppWindow, context: &Rc<Context>, cat: &CatEntry, form: usize) {
    let vault = context.engine.vault();
    let key = format!("{}:{}", cat_animation::set_id(cat, form), context.engine.shared().generation());

    show(ui, context, key, || cat_animation::motions(cat, form, &vault.vfs));
}

pub fn show_enemy(ui: &AppWindow, context: &Rc<Context>, enemy: &EnemyEntry) {
    let vault = context.engine.vault();
    let key = format!("{}:{}", enemy_animation::set_id(enemy), context.engine.shared().generation());

    show(ui, context, key, || enemy_animation::motions(enemy, &vault.vfs));
}

pub fn show_set(ui: &AppWindow, context: &Rc<Context>, key: String, build: impl FnOnce() -> MotionSet) {
    show(ui, context, key, build);
}

fn show(ui: &AppWindow, context: &Rc<Context>, key: String, build: impl FnOnce() -> MotionSet) {
    let fresh = context.viewer.borrow().key.as_deref() != Some(key.as_str());

    if fresh {
        let set = build();

        {
            let mut state = context.viewer.borrow_mut();
            let previous = state.set.as_ref().map(|held| held.name.clone());

            if previous.as_deref().is_none_or(|last| unit_of(last) != unit_of(&set.name)) {
                state.frame = 0.0;
            }

            state.slots = place(&set);
            state.set = Some(Rc::new(set));
            state.key = Some(key);
            state.last_stamp = None;
            state.region = None;
            state.clamp_frame();
        }

        load_selected(ui, context);
        export::synced(ui, context);
    }

    {
        let mut state = context.viewer.borrow_mut();

        if !state.active {
            state.active = true;
            state.last_time = None;

            let weak = ui.as_weak();
            let ticking: Weak<Context> = Rc::downgrade(context);

            state.timer.start(TimerMode::Repeated, Duration::from_millis(16), move || {
                if let (Some(ui), Some(context)) = (weak.upgrade(), ticking.upgrade()) {
                    tick(&ui, &context);
                }
            });
        }
    }

    sync_controls(ui, context);
}

fn load_selected(ui: &AppWindow, context: &Rc<Context>) {
    let (rigging, anim_source) = {
        let state = context.viewer.borrow();
        let Some(motion) = state.motion() else {
            return;
        };

        (Arc::clone(&motion.rig), motion.file.clone().map(|file| (State::anim_key(motion), file)))
    };

    if let Some((key, file)) = anim_source
        && !context.viewer.borrow().anims.contains_key(&key)
    {
        let parsed = file.read().ok().and_then(|bytes| Animation::parse(&bytes).inspect_err(|err| warn!("An animation of {} could not be parsed: {err}", rigging.id)).ok());

        if let Some(anim) = parsed {
            context.viewer.borrow_mut().anims.insert(key, Arc::new(anim));
        }
    }

    {
        let mut state = context.viewer.borrow_mut();

        if state.rigs.contains_key(&rigging.id) || !state.inflight.insert(rigging.id.clone()) {
            return;
        }
    }

    let id = rigging.id.clone();
    let landing = Rc::clone(context);

    bridge::offload(
        ui,
        RIG_JOB,
        move || {
            let read = |source: &foundry::Source| source.read().inspect_err(|err| warn!(path = %source.path.display(), "A rig file could not be read: {err}")).ok();
            let (Some(png), Some(cut), Some(model)) = (read(&rigging.png), read(&rigging.cut), read(&rigging.model)) else {
                return None;
            };

            Rig::parse(png, cut, model).inspect_err(|err| warn!("The rig {} could not be parsed: {err}", rigging.id)).ok().map(Arc::new)
        },
        move |ui, rig| {
            {
                let mut state = landing.viewer.borrow_mut();

                state.inflight.remove(&id);
                state.rigs.insert(id, rig);
                state.last_stamp = None;
                state.clamp_frame();
            }

            sync_controls(ui, &landing);
            export::synced(ui, &landing);
        },
    );
}

pub fn sync_controls(ui: &AppWindow, context: &Rc<Context>) {
    let viewer = ui.global::<Viewer>();
    let state = context.viewer.borrow();
    let rig = state.rig();
    let has_rig = rig.is_some();
    let motion = state.motion();
    let has_anim = has_rig && motion.is_some();
    let model = state.is_model();
    let bound = display_bound(state.loop_bound());
    let effective = if model { "0".to_owned() } else { state.loop_end.map_or_else(|| bound.clone(), |end| (end.trunc() as i32).to_string()) };
    let selected = state.selected();
    let tiles: Vec<MotionTile> = state
        .slots
        .iter()
        .map(|slot| {
            let held = slot.and_then(|index| state.set.as_ref()?.motions.get(index));

            match held {
                Some(held) => {
                    let label = held.label();

                    MotionTile { size: fitted_size(&label), label: label.into(), present: true, active: *slot == selected }
                }
                None => MotionTile { label: SharedString::default(), present: false, active: false, size: 12.0 },
            }
        })
        .collect();
    let choices = state.set.as_ref().map(|set| offset_choices(set, rig.as_deref())).unwrap_or_default();
    let current = state.offset();
    let offset_index = choices.iter().position(|(_, choice)| placement_matches(*choice, current)).unwrap_or(0);
    let show = state_settings(context);

    viewer.set_has_rig(has_rig);
    viewer.set_has_anim(has_anim);
    viewer.set_missing(state.rig_failed());
    viewer.set_playing(state.playing);
    viewer.set_frame_text((state.frame.trunc() as i32).to_string().into());
    viewer.set_max_text(bound.into());
    viewer.set_effective_max(effective.into());
    viewer.set_range_start(state.range_inputs.0.as_str().into());
    viewer.set_range_end(state.range_inputs.1.as_str().into());
    viewer.set_tiles_scroll(state.slots.len() > COLUMNS * GRID_ROWS);
    viewer.set_tiles(ModelRc::new(VecModel::from(tiles)));
    viewer.set_offsets(ModelRc::new(VecModel::from(choices.iter().map(|(name, _)| SharedString::from(name.as_str())).collect::<Vec<_>>())));
    viewer.set_offset_current(offset_index as i32);
    viewer.set_show_rig(show.show_rig);
    viewer.set_show_origin(show.show_origin);
    viewer.set_show_world(show.show_world);
    viewer.set_show_combat(show.show_combat);
}

fn state_settings(context: &Rc<Context>) -> AnimSettings {
    context.state.settings().animation.clone()
}

pub fn begin_selecting(ui: &AppWindow, context: &Rc<Context>) {
    context.viewer.borrow_mut().selecting = true;
    ui.global::<Viewer>().set_selecting(true);
}

fn tick(ui: &AppWindow, context: &Rc<Context>) {
    if ui.global::<Sandbox>().get_surfaced() {
        let mut state = context.viewer.borrow_mut();

        state.last_time = None;
        state.last_stamp = None;

        return;
    }

    let now = Instant::now();
    let show = state_settings(context);
    let theme = ui.global::<Theme>();
    let bg = theme.get_bg();
    let clear = [bg.red(), bg.green(), bg.blue(), 255];
    let mut state = context.viewer.borrow_mut();
    let delta = state.last_time.map_or(TICK_MS, |last| (now.duration_since(last).as_secs_f64() * 1000.0).clamp(0.0, LONGEST_TICK_MS));

    state.last_time = Some(now);
    state.scale = ui.window().scale_factor();
    state.settle_zoom(delta);

    let rig = state.rig();
    let anim = state.anim();
    let mut controls_dirty = false;

    if state.playing && rig.is_some() {
        let advance = FRAME_ADVANCE_PER_TICK * (delta / TICK_MS) as f32;
        let current = state.frame + advance;
        let start = state.loop_start.unwrap_or(0.0);
        let end = state.loop_end.or_else(|| state.loop_bound().map(|bound| bound.max(0) as f32));
        let next = match end {
            Some(end) if current > end => start,
            _ => current,
        };

        if next.trunc() != state.frame.trunc() {
            controls_dirty = true;
        }

        state.frame = next;
    } else if let Some((direction, held_ms)) = state.hold {
        let total = held_ms + delta;

        state.hold = Some((direction, total));

        if total > HOLD_DELAY_MS {
            state.step(f32::from(direction) * HOLD_FRAMES_PER_SEC * (delta / 1000.0) as f32);
            controls_dirty = true;
        }
    }

    let pixels = state.pixel_size();
    let frame = state.playback(state.frame).floor() as i32;
    let rig_key = rig.as_ref().map_or(0, |rig| Arc::as_ptr(rig) as usize);
    let anim_key = anim.as_ref().map_or(0, |anim| Arc::as_ptr(anim) as usize);
    let offset = state.offset();
    let combat = state.motion().and_then(|motion| motion.combat.clone()).filter(|_| show.show_combat);
    let look = show.combat_look();
    let played = state.frame;
    let stamp: Stamp = (
        rig_key,
        anim_key,
        frame,
        ((state.pan.0 * 100.0) as i32, (state.pan.1 * 100.0) as i32),
        (state.zoom * 1000.0) as i32,
        pixels,
        (show.show_rig, show.show_origin, show.show_world, combat.is_some().then(|| played.to_bits()), look),
        state.selecting,
        state.region,
    );
    let offset_stale = state.reach.as_ref().is_some_and(|held| held.offset != offset);

    if state.last_stamp == Some(stamp) && !offset_stale {
        drop(state);

        if controls_dirty {
            sync_controls(ui, context);
        }

        return;
    }

    let complete = anim.is_some() || state.is_model();

    if rig.is_some() && complete {
        state.waiting_since = None;
    } else if now.duration_since(*state.waiting_since.get_or_insert(now)) < HOLD_OLD_FRAME {
        drop(state);

        if controls_dirty {
            sync_controls(ui, context);
        }

        return;
    }

    state.last_stamp = Some(stamp);

    let view = state.view();
    let (width, height) = pixels;
    let camera = scene::camera(&view, state.selecting, state.region);
    let mut frames: Vec<FrameData> = Vec::new();
    let drawn = rig.as_deref().filter(|_| complete);
    let shapes = match drawn {
        Some(rig) => {
            frames = resolve_frame(rig, anim.as_deref(), frame, offset);

            let bounds = (show.show_world || combat.is_some()).then(|| {
                let stale = state.reach.as_ref().is_none_or(|held| held.rig != rig_key || held.anim != anim_key || held.offset != offset);

                if stale {
                    let anims: Vec<&Animation> = anim.as_deref().into_iter().collect();
                    let bounds = rig.calculate_bounds(&anims, show.bounds_cull as f32 / CULL_SCALE, None, offset);

                    state.reach = Some(Reach { rig: rig_key, anim: anim_key, offset, bounds });
                }

                state.reach.as_ref().and_then(|held| held.bounds)
            });
            let pose = scene::Pose { rig, anim: anim.as_deref(), frame, offset, frames: &frames };

            let mut shapes = Vec::new();

            if let Some(combatant) = combat.as_ref() {
                let start = state.loop_start.unwrap_or(0.0);
                let end = state.loop_end.or_else(|| state.loop_bound().map(|bound| bound.max(0) as f32));
                let period = end.map(|end| (end - start).floor() as i32 + 1);
                let stance = collision::Pose { rig, anim: anim.as_deref(), frame, offset, zoom: view.zoom, crest: bounds.flatten().map(|reach| reach.min_y) };
                let glow = |_: usize, landed: i32| match combatant.attacking {
                    true => collision::glow(look.fade, period, played, landed),
                    false => 0.0,
                };

                shapes.extend(scene::combat(collision::posed(&combatant.profile, look, &stance, |x, y| view.to_screen(x, y), glow)));
            }

            shapes.extend(scene::shapes(&pose, bounds.flatten(), &view, &show));
            shapes.extend(camera);
            shapes
        }
        None => camera,
    };

    let sheet = drawn.and_then(|rig| rig.sheet.image_data.as_deref());
    let drawn_rig = drawn.is_some();

    let hardware = host::hardware_drawn();
    let scene = hardware.then(|| {
        let held = state.gpu_sheet.take().filter(|(kept, _)| rig.as_ref().is_some_and(|rig| Arc::ptr_eq(kept, rig)));
        let pixels = held.map(|(_, pixels)| pixels).or_else(|| sheet.map(|image| Arc::from(image.as_raw().as_slice())));
        let handle = sheet.zip(pixels.as_ref()).map(|(image, pixels)| (image.width(), image.height(), Arc::clone(pixels)));

        state.gpu_sheet = rig.clone().zip(pixels);

        scene::gpu_scene((width, height), clear, handle, &frames, &view, &shapes)
    });

    let scene = match scene {
        Some(scene) if host::gpu_available() => {
            host::submit_scene(scene.into());
            drop(state);
            ui.window().request_redraw();

            if controls_dirty {
                sync_controls(ui, context);
            }

            return;
        }
        other => other,
    };

    #[cfg(target_os = "ios")]
    let drawn = scene.and_then(host::render_offscreen);
    #[cfg(not(target_os = "ios"))]
    let drawn: Option<Vec<u8>> = scene.and(None);

    let buffer = drawn.unwrap_or_else(|| {
        let mut buffer = std::mem::take(&mut state.pixels);

        buffer.clear();
        buffer.resize(width as usize * height as usize * 4, 0);
        scene::clear(&mut buffer, clear);

        if drawn_rig {
            scene::paint(&mut buffer, width, height, sheet, &frames, &view, &shapes);
        } else {
            scene::paint_shapes(&mut buffer, width, height, &view, &shapes);
        }

        buffer
    });
    let image = Image::from_rgba8_premultiplied(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&buffer, width, height));

    state.pixels = buffer;
    drop(state);
    ui.global::<Viewer>().set_frame(image);

    if controls_dirty {
        sync_controls(ui, context);
    }
}
