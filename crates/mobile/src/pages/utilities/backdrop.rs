use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use image::RgbaImage;
use slint::{ComponentHandle, Image, Rgba8Pixel, SharedPixelBuffer, Timer, TimerMode};
use tracing::{error, info, warn};

use codex::chapter::stage::Background;
use codex::graphics::backdrop::{Backdrop as Scene, Camera, Layers, Quad, Screen, Surface};
use foundry::domains::utilities::background::{self as builder, Catalog, Names, Overrides, DEFAULT_SEED, STAGE_WIDTH};
use foundry::systems::animation::export::{EncoderMessage, EncoderStatus, ExportFormat, ExportMode, ExportRequest, FrameTiming, ShowcaseLengths, leader, process};
use foundry::systems::raster::{self, Alpha, Batch, Texture, Vertex};

use crate::memory::Remembered;
use crate::pages::Context;
use crate::{AppWindow, Backdrop, Tone, Utilities, bridge, delivery, picker};

use super::Shared;

const ZOOM_MIN: f32 = 0.1;
const ZOOM_MAX: f32 = 8.0;
const ZOOM_RATE_PER_PIXEL: f32 = 0.0016;
const WHEEL_LINE_PIXELS: f32 = 0.6;
const ZOOM_SCROLL_STRENGTH: f32 = 2.5;
const HORIZON_SHARE: f32 = 0x208 as f32 / 0x280 as f32;
const DEFAULT_LOOP_END: i32 = 299;
const REPLAY_CEILING: i32 = 100_000;
const SEED_CEILING: u64 = i32::MAX as u64;
const TICK_MS: u64 = 33;
const HOLD_DELAY_MS: f64 = 200.0;
const HOLD_FRAMES_PER_SEC: f32 = 30.0;
const MAX_SCALE: f32 = 2.0;
const MAX_SIDE: u32 = 4096;
const FRAME_DELAY_MS: u32 = 33;
const FEEDBACK_MS: u64 = 2000;
const NO_BACKGROUNDS_HINT: &str = "No backgrounds within mount";
const VANILLA_HINT: &str = "Vanilla background loaded, clear the ID field to use your own files";
const RANGES_PER_LINE: usize = 6;
const FORMATS: [ExportFormat; 3] = [ExportFormat::Gif, ExportFormat::WebP, ExportFormat::Zip];
const EXPORT_IDLE: &str = "Export";
const EXPORT_BUSY: &str = "Exporting...";
const EXPORT_DONE: &str = "Exported!";
const EXPORT_FAILED: &str = "Failed!";

type Stamp = (i32, i32, i32, i32, (u32, u32), u32);

struct Loaded {
    scene: Scene,
    layers: Layers,
    seed: u32,
    animated: bool,
    id: u32,
}

pub struct State {
    overrides: Overrides,
    names: Option<Names>,
    id_input: String,
    seed_input: String,
    catalog: Catalog,
    hint: String,
    scanned: bool,
    scanning: bool,
    generation: u64,
    loading: Option<u64>,
    notice: Option<String>,
    watched: BTreeSet<String>,
    loaded: Option<Loaded>,
    center: f32,
    lift: f32,
    zoom: f32,
    viewport: (f32, f32),
    scale: f32,
    fitted: bool,
    frame: f32,
    playing: bool,
    loop_start: Option<f32>,
    loop_end: Option<f32>,
    range_inputs: (String, String),
    hold: Option<(i8, f64)>,
    last_time: Option<Instant>,
    last_stamp: Option<Stamp>,
    pixels: Vec<u8>,
    active: bool,
    timer: Timer,
    format: ExportFormat,
    exporting: bool,
    feedback: Option<bool>,
    feedback_timer: Timer,
    encoder: Option<bridge::Ticket>,
}

impl State {
    pub fn new(remembered: &Remembered) -> Self {
        let screen = Screen::default();

        Self {
            overrides: Overrides::default(),
            names: None,
            id_input: String::new(),
            seed_input: String::new(),
            catalog: Catalog::default(),
            hint: String::new(),
            scanned: false,
            scanning: false,
            generation: 0,
            loading: None,
            notice: None,
            watched: BTreeSet::new(),
            loaded: None,
            center: screen.width as f32 / 2.0,
            lift: 0.0,
            zoom: 1.0,
            viewport: (screen.width as f32, screen.height as f32),
            scale: 1.0,
            fitted: false,
            frame: 0.0,
            playing: true,
            loop_start: None,
            loop_end: None,
            range_inputs: (String::new(), String::new()),
            hold: None,
            last_time: None,
            last_stamp: None,
            pixels: Vec::new(),
            active: false,
            timer: Timer::default(),
            format: FORMATS.get(usize::from(remembered.background_format)).cloned().unwrap_or(ExportFormat::Gif),
            exporting: false,
            feedback: None,
            feedback_timer: Timer::default(),
            encoder: None,
        }
    }

    fn seed(&self) -> u32 {
        self.seed_input.parse::<u32>().ok().filter(|seed| *seed > 0).unwrap_or(DEFAULT_SEED)
    }

    fn id(&self) -> Option<u32> {
        self.id_input.parse::<u32>().ok()
    }

    fn vanilla(&self) -> bool {
        !self.id_input.is_empty() && self.overrides.csv.is_none()
    }

    fn next_generation(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.generation
    }

    fn camera(&self) -> Camera {
        Camera { x: self.center, y: self.lift, zoom: self.zoom, width: self.viewport.0, height: self.viewport.1 }
    }

    fn orient(&mut self) {
        self.center = self.viewport.0 / 2.0;
        self.lift = 0.0;
        self.zoom = 1.0;
    }

    fn pan(&mut self, dx: f32, dy: f32) {
        self.center -= dx / self.zoom.max(f32::EPSILON);
        self.lift += dy;
    }

    fn zoom_at(&mut self, factor: f32, at: (f32, f32)) {
        let screen = Screen::default();
        let before = self.camera();
        let zoom = (self.zoom * factor).clamp(ZOOM_MIN, ZOOM_MAX);
        let horizon = screen.height as f32 * HORIZON_SHARE;
        let scene_x = (at.0 - before.width / 2.0) / before.zoom.max(f32::EPSILON) + self.center;
        let scene_y = (at.1 - before.horizon()) / before.zoom.max(f32::EPSILON) + horizon;

        self.zoom = zoom;
        self.center = scene_x - (at.0 - before.width / 2.0) / zoom;
        self.lift = at.1 - horizon - zoom * (scene_y - horizon);
    }

    fn resized(&mut self, width: f32, height: f32) {
        if width < 1.0 || height < 1.0 {
            return;
        }

        self.viewport = (width, height);

        if !self.fitted {
            self.fitted = true;
            self.lift = ((height - Screen::default().height as f32) / 2.0).round();
        }
    }

    fn animated(&self) -> bool {
        self.loaded.as_ref().is_some_and(|loaded| loaded.animated)
    }

    fn bounds(&self) -> (f32, f32) {
        if !self.animated() {
            return (0.0, 0.0);
        }

        let start = self.loop_start.unwrap_or(0.0).max(0.0);
        let end = self.loop_end.unwrap_or(DEFAULT_LOOP_END as f32).max(start);

        (start, end)
    }

    fn step(&mut self, direction: f32) {
        let (min, max) = self.bounds();
        let stepped = self.frame + direction;

        self.frame = if stepped < min {
            max
        } else if stepped > max {
            min
        } else {
            stepped
        };
    }

    fn sync_frame(&mut self, force: bool) {
        let Some(loaded) = self.loaded.as_mut() else {
            return;
        };
        let target = (self.frame.floor().max(0.0) as i32).min(REPLAY_CEILING);
        let held = loaded.scene.frame();

        if held == target && !force {
            return;
        }

        if force || target < held {
            loaded.scene.restart(loaded.seed);
        }

        while loaded.scene.frame() < target {
            loaded.scene.tick();
        }

        loaded.layers = loaded.scene.layers();
    }

    fn pixel_size(&self) -> (u32, u32) {
        let scale = self.scale.min(MAX_SCALE);

        (((self.viewport.0 * scale).round() as u32).clamp(1, MAX_SIDE), ((self.viewport.1 * scale).round() as u32).clamp(1, MAX_SIDE))
    }
}

fn wrapped_ranges(catalog: &Catalog) -> String {
    let ranges = catalog.ranges();
    let spans: Vec<&str> = ranges.split(", ").collect();

    spans.chunks(RANGES_PER_LINE).map(|line| line.join(", ")).collect::<Vec<String>>().join("\n")
}

fn paint(pixels: &mut [u8], width: u32, height: u32, quads: &[Quad], image_of: impl Fn(Surface) -> Option<Arc<RgbaImage>>) {
    let mut vertices: Vec<Vertex> = Vec::with_capacity(quads.len() * 6);
    let mut runs: Vec<(Option<usize>, u8, std::ops::Range<usize>)> = Vec::new();
    let mut textures: Vec<Arc<RgbaImage>> = Vec::new();

    for quad in quads {
        let texture = quad.surface.and_then(&image_of).map(|image| match textures.iter().position(|held| Arc::ptr_eq(held, &image)) {
            Some(index) => index,
            None => {
                textures.push(image);
                textures.len() - 1
            }
        });

        if quad.surface.is_some() && texture.is_none() {
            continue;
        }

        let uv = texture.map_or([[0.0f32; 2]; 4], |index| {
            let image = &textures[index];
            let (image_width, image_height) = (image.width().max(1) as f32, image.height().max(1) as f32);
            let left = quad.source[0] / image_width;
            let top = quad.source[1] / image_height;
            let right = (quad.source[0] + quad.source[2]) / image_width;
            let bottom = (quad.source[1] + quad.source[3]) / image_height;

            [[left, top], [left, bottom], [right, bottom], [right, top]]
        });
        let corner = |slot: usize| Vertex {
            position: [quad.corners[slot][0] / width as f32 * 2.0 - 1.0, 1.0 - quad.corners[slot][1] / height as f32 * 2.0],
            uv: uv[slot],
            color: quad.colors[slot],
            tint: [0.0; 4],
        };
        let start = vertices.len();

        vertices.extend([corner(0), corner(1), corner(2), corner(0), corner(2), corner(3)]);

        let end = vertices.len();

        match runs.last_mut() {
            Some((held_texture, held_blend, range)) if *held_texture == texture && *held_blend == quad.blend => range.end = end,
            _ => runs.push((texture, quad.blend, start..end)),
        }
    }

    let batches: Vec<Batch<'_>> = runs
        .into_iter()
        .map(|(texture, blend, range)| Batch {
            texture: texture.and_then(|index| textures.get(index)).map(|image| Texture { width: image.width(), height: image.height(), pixels: image.as_raw() }),
            blend,
            alpha: Alpha::Blended,
            range,
        })
        .collect();

    raster::draw(pixels, width, height, &vertices, &batches);
}

fn render(scene: &Scene, layers: &Layers, camera: Camera, width: u32, height: u32, pixels: &mut Vec<u8>) {
    let camera = Camera { width: width as f32, height: height as f32, ..camera.rescaled(camera.zoom * (width as f32 / camera.width.max(1.0))) };
    let quads = scene.compose(layers, camera);

    pixels.clear();
    pixels.resize(width as usize * height as usize * 4, 0);
    paint(pixels, width, height, &quads, |surface| scene.image(surface).map(Arc::clone));
}

pub fn wire(window: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let utilities = window.global::<Utilities>();
    let backdrop = window.global::<Backdrop>();
    let on = |state: &Shared, context: &Rc<Context>, window: &AppWindow| (Rc::clone(state), Rc::clone(context), window.as_weak());

    {
        let held = Rc::clone(state);
        let ctx = Rc::clone(context);
        let ticket = bridge::subscribe::<EncoderStatus>(move |ui, status| {
            if matches!(status, EncoderStatus::Finished) {
                finished(ui, &held, &ctx, true);
            }
        });

        state.borrow_mut().backdrop.encoder = Some(ticket);
    }

    let (held, ctx, weak) = on(state, context, window);
    utilities.on_pick_csv(move || {
        if let Some(ui) = weak.upgrade() {
            let landing = Rc::clone(&held);

            picker::open(
                &ui,
                &ctx,
                "Select a Background Table",
                &["csv"],
                Box::new(move |ui, ctx, path| {
                    {
                        let mut state = landing.borrow_mut();

                        state.backdrop.overrides.csv = Some(path);
                        state.backdrop.scanned = false;
                    }

                    scan(ui, &landing, ctx);
                    load(ui, &landing, ctx);
                }),
            );
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    utilities.on_id_edited(move |value| {
        let Some(ui) = weak.upgrade() else {
            return;
        };

        if value.chars().all(|c| c.is_ascii_digit()) {
            held.borrow_mut().backdrop.id_input = value.to_string();
            load(&ui, &held, &ctx);
        } else {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    utilities.on_seed_edited(move |value| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let valid = value.chars().all(|c| c.is_ascii_digit()) && (value.is_empty() || value.parse::<u64>().is_ok_and(|seed| (1..=SEED_CEILING).contains(&seed)));

        if valid {
            let mut state = held.borrow_mut();

            state.backdrop.seed_input = value.to_string();

            let seed = state.backdrop.seed();

            if let Some(loaded) = state.backdrop.loaded.as_mut()
                && loaded.seed != seed
            {
                loaded.seed = seed;
                drop(state);
                held.borrow_mut().backdrop.sync_frame(true);
            }
        }

        refresh(&ui, &held, &ctx);
    });

    let (held, ctx, _) = on(state, context, window);
    backdrop.on_resized(move |width, height| {
        held.borrow_mut().backdrop.resized(width, height);
        let _ = &ctx;
    });

    let (held, _, _) = on(state, context, window);
    backdrop.on_pan(move |dx, dy| held.borrow_mut().backdrop.pan(dx, dy));

    let (held, _, _) = on(state, context, window);
    backdrop.on_orient(move || held.borrow_mut().backdrop.orient());

    let (held, _, _) = on(state, context, window);
    backdrop.on_pinch(move |factor, x, y| {
        if factor > 0.0 {
            held.borrow_mut().backdrop.zoom_at(factor, (x, y));
        }
    });

    let (held, _, _) = on(state, context, window);
    backdrop.on_zoom_wheel(move |delta, x, y| {
        let factor = (delta * WHEEL_LINE_PIXELS * ZOOM_SCROLL_STRENGTH * ZOOM_RATE_PER_PIXEL).exp();

        held.borrow_mut().backdrop.zoom_at(factor, (x, y));
    });

    let (held, ctx, weak) = on(state, context, window);
    backdrop.on_toggle_play(move || {
        {
            let mut state = held.borrow_mut();

            state.backdrop.playing = !state.backdrop.playing;
            state.backdrop.hold = None;
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, _, _) = on(state, context, window);
    backdrop.on_hold(move |direction| held.borrow_mut().backdrop.hold = Some((direction.clamp(-1, 1) as i8, 0.0)));

    let (held, ctx, weak) = on(state, context, window);
    backdrop.on_release(move || {
        {
            let mut state = held.borrow_mut();

            if let Some((direction, held_ms)) = state.backdrop.hold.take()
                && held_ms <= HOLD_DELAY_MS
            {
                state.backdrop.step(f32::from(direction));
            }
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    backdrop.on_set_frame(move |value| {
        {
            let mut state = held.borrow_mut();
            let trimmed = value.trim();
            let target = if trimmed.is_empty() { Some(0.0) } else { trimmed.parse::<f32>().ok() };

            if let Some(target) = target {
                let (min, max) = state.backdrop.bounds();

                state.backdrop.frame = if state.backdrop.animated() { target.clamp(min, max) } else { 0.0 };
            }
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    backdrop.on_set_range_start(move |value| {
        {
            let mut state = held.borrow_mut();

            state.backdrop.loop_start = value.parse::<f32>().ok().map(|start| start.max(0.0));
            state.backdrop.range_inputs.0 = value.to_string();
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    backdrop.on_set_range_end(move |value| {
        {
            let mut state = held.borrow_mut();

            state.backdrop.loop_end = value.parse::<f32>().ok().map(|end| end.max(0.0));
            state.backdrop.range_inputs.1 = value.to_string();
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, _) = on(state, context, window);
    backdrop.on_format_set(move |index| {
        let format = FORMATS.get(index.max(0) as usize).cloned().unwrap_or(ExportFormat::Gif);

        held.borrow_mut().backdrop.format = format;
        ctx.memory.update(|memory| memory.background_format = index.clamp(0, 2) as u8);
    });

    let (held, ctx, weak) = on(state, context, window);
    backdrop.on_export_pressed(move || {
        if let Some(ui) = weak.upgrade() {
            export(&ui, &held, &ctx);
        }
    });
}

pub fn show(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    {
        let mut held = state.borrow_mut();

        if !held.backdrop.active {
            held.backdrop.active = true;
            held.backdrop.last_time = None;

            let weak = ui.as_weak();
            let landing: Weak<std::cell::RefCell<super::State>> = Rc::downgrade(state);
            let ticking: Weak<Context> = Rc::downgrade(context);

            held.backdrop.timer.start(TimerMode::Repeated, Duration::from_millis(TICK_MS), move || {
                if let (Some(ui), Some(state), Some(context)) = (weak.upgrade(), landing.upgrade(), ticking.upgrade()) {
                    tick(&ui, &state, &context);
                }
            });
        }
    }

    let scanned = state.borrow().backdrop.scanned || state.borrow().backdrop.scanning;

    if !scanned {
        scan(ui, state, context);
    }

    refresh(ui, state, context);
}

pub fn hide(state: &Shared) {
    let mut held = state.borrow_mut();

    if held.backdrop.active {
        held.backdrop.active = false;
        held.backdrop.timer.stop();
    }
}

pub fn invalidate(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    state.borrow_mut().backdrop.scanned = false;

    if state.borrow().backdrop.active {
        scan(ui, state, context);
        load(ui, state, context);
    }
}

fn scan(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let generation = {
        let mut held = state.borrow_mut();

        held.backdrop.scanning = true;
        held.backdrop.next_generation()
    };
    let vault = context.engine.vault();
    let csv = state.borrow().backdrop.overrides.csv.clone();
    let landing = Rc::clone(state);
    let ctx = Rc::clone(context);

    bridge::offload(
        ui,
        "background_scan",
        move || match csv {
            Some(path) => std::fs::read(&path)
                .inspect_err(|err| warn!(path = %path.display(), "Background table could not be read: {}", err))
                .ok()
                .and_then(|bytes| Background::parse(bytes, None).ok())
                .map(|table| Catalog::from_table(&table))
                .unwrap_or_default(),
            None => Catalog::scan(&vault.vfs),
        },
        move |ui, catalog| {
            {
                let mut held = landing.borrow_mut();

                if held.backdrop.generation != generation {
                    return;
                }

                held.backdrop.scanning = false;
                held.backdrop.scanned = true;
                held.backdrop.hint = wrapped_ranges(&catalog);
                held.backdrop.catalog = catalog;
            }

            refresh(ui, &landing, &ctx);
        },
    );
}

fn load(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let (id, overrides, generation) = {
        let mut held = state.borrow_mut();
        let Some(id) = held.backdrop.id() else {
            held.backdrop.loading = None;
            held.backdrop.names = None;
            held.backdrop.notice = None;
            held.backdrop.loaded = None;
            held.backdrop.last_stamp = None;
            drop(held);
            refresh(ui, state, context);
            return;
        };
        let generation = held.backdrop.next_generation();
        let overrides = if held.backdrop.vanilla() { Overrides::default() } else { held.backdrop.overrides.clone() };

        held.backdrop.loading = Some(generation);
        (id, overrides, generation)
    };
    let vault = context.engine.vault();
    let landing = Rc::clone(state);
    let ctx = Rc::clone(context);

    bridge::offload(
        ui,
        "background_load",
        move || builder::load(&vault.vfs, id, &overrides),
        move |ui, outcome| {
            {
                let mut held = landing.borrow_mut();

                if held.backdrop.loading != Some(generation) {
                    return;
                }

                held.backdrop.loading = None;

                match outcome {
                    Ok(loaded) => {
                        let seed = held.backdrop.seed();
                        let mut scene = Scene::new(loaded.assets, Screen::default(), STAGE_WIDTH, seed);
                        let animated = scene.animated();
                        let layers = scene.layers();

                        held.backdrop.names = Some(loaded.names);
                        held.backdrop.watched = loaded.watched;
                        held.backdrop.notice = None;
                        held.backdrop.loaded = Some(Loaded { scene, layers, seed, animated, id: loaded.id });
                        held.backdrop.frame = 0.0;
                        held.backdrop.loop_start = None;
                        held.backdrop.loop_end = None;
                        held.backdrop.range_inputs = (String::new(), String::new());
                        held.backdrop.last_stamp = None;
                    }
                    Err(problem) => {
                        held.backdrop.names = None;
                        held.backdrop.notice = Some(problem);
                        held.backdrop.loaded = None;
                        held.backdrop.last_stamp = None;
                    }
                }
            }

            refresh(ui, &landing, &ctx);
        },
    );
}

fn tick(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let now = Instant::now();
    let mut held = state.borrow_mut();
    let backdrop = &mut held.backdrop;
    let delta = backdrop.last_time.map_or(TICK_MS as f64, |last| (now.duration_since(last).as_secs_f64() * 1000.0).clamp(0.0, 100.0));

    backdrop.last_time = Some(now);
    backdrop.scale = ui.window().scale_factor();

    let animated = backdrop.animated();
    let mut controls_dirty = false;

    if backdrop.playing && animated {
        let before = backdrop.frame.floor();

        backdrop.frame += (delta / TICK_MS as f64) as f32;

        let (start, end) = backdrop.bounds();

        if backdrop.frame > end {
            backdrop.frame = start;
        }

        controls_dirty = before != backdrop.frame.floor();
    } else if let Some((direction, held_ms)) = backdrop.hold {
        let total = held_ms + delta;

        backdrop.hold = Some((direction, total));

        if total > HOLD_DELAY_MS {
            backdrop.step(f32::from(direction) * HOLD_FRAMES_PER_SEC * (delta / 1000.0) as f32);
            controls_dirty = true;
        }
    }

    backdrop.sync_frame(false);

    let pixels = backdrop.pixel_size();
    let frame = backdrop.frame.floor() as i32;
    let seed = backdrop.seed();
    let stamp = ((backdrop.center * 100.0) as i32, (backdrop.lift * 100.0) as i32, (backdrop.zoom * 1000.0) as i32, frame, pixels, seed);

    if backdrop.last_stamp == Some(stamp) {
        drop(held);

        if controls_dirty {
            refresh(ui, state, context);
        }

        return;
    }

    backdrop.last_stamp = Some(stamp);

    let Some(loaded) = backdrop.loaded.as_ref() else {
        return;
    };
    let camera = backdrop.camera();
    let (width, height) = pixels;
    let mut buffer = std::mem::take(&mut backdrop.pixels);

    render(&loaded.scene, &loaded.layers, camera, width, height, &mut buffer);

    let image = Image::from_rgba8_premultiplied(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&buffer, width, height));

    backdrop.pixels = buffer;
    drop(held);
    ui.global::<Backdrop>().set_frame(image);

    if controls_dirty {
        refresh(ui, state, context);
    }
}

fn export(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let plan = {
        let held = state.borrow();
        let backdrop = &held.backdrop;

        if backdrop.exporting {
            return;
        }

        let Some(loaded) = backdrop.loaded.as_ref() else {
            return;
        };
        let (width, height) = backdrop.pixel_size();
        let even = |value: u32| (value.max(2) / 2 * 2).max(2);
        let camera = backdrop.camera();

        (loaded.scene.clone(), loaded.seed, loaded.id, backdrop.playing && backdrop.animated(), backdrop.bounds(), backdrop.frame.floor() as i32, camera, even(width), even(height), backdrop.format.clone(), backdrop.encoder)
    };
    let (scene, seed, id, animated, (start, end), current, camera, width, height, format, ticket) = plan;
    let Some(ticket) = ticket else {
        return;
    };

    state.borrow_mut().backdrop.exporting = true;
    refresh(ui, state, context);

    let stem = builder::export_stem(id);

    if !animated {
        let landing = Rc::clone(state);
        let ctx = Rc::clone(context);

        bridge::offload(
            ui,
            "background_shot",
            move || {
                let mut scene = scene;

                seek(&mut scene, seed, current);

                let layers = scene.layers();
                let mut pixels = Vec::new();

                render(&scene, &layers, camera, width, height, &mut pixels);

                let image = RgbaImage::from_raw(width, height, pixels).ok_or_else(|| "the frame had the wrong size".to_owned())?;
                let directory = Path::new("exports");

                std::fs::create_dir_all(directory).map_err(|err| err.to_string())?;

                let target = directory.join(format!("bg{stem}.{current}f.png"));

                image.save(&target).map_err(|err| err.to_string())?;

                Ok::<PathBuf, String>(target)
            },
            move |ui, outcome| {
                match &outcome {
                    Ok(path) => info!(path = %path.display(), "Background exported"),
                    Err(reason) => error!("The background export failed: {reason}"),
                }

                finished(ui, &landing, &ctx, outcome.is_ok());
            },
        );

        return;
    }

    let (first, last) = (start.floor() as i32, end.floor() as i32);
    let request = ExportRequest {
        timing: FrameTiming { mode: ExportMode::Manual, frame_start: first, frame_end: last, loop_supported: true },
        showcase: ShowcaseLengths { walk: 0, idle: 0, attack: 0, kb: 0 },
        file_name: None,
        name_prefix: format!("bg{stem}"),
        format,
        quality_percent: 80,
        compression_percent: 30,
        fps: 30,
        width,
        height,
        background: true,
    };
    let config = process::build_config(&request);
    let (tx, rx) = mpsc::sync_channel::<EncoderMessage>(8);
    let abort = Arc::new(AtomicBool::new(false));
    let encoder_abort = Arc::clone(&abort);
    let weak = ui.as_weak();
    let encoder = thread::Builder::new().name("background_encoder".to_owned()).spawn(move || {
        leader::run(config, rx, |status| bridge::post(&weak, ticket, status), &encoder_abort);
    });

    if let Err(err) = encoder {
        error!("The background encoder thread could not be started: {err}");
        finished(ui, state, context, false);
        return;
    }

    let renderer = thread::Builder::new().name("background_render".to_owned()).spawn(move || {
        let mut scene = scene;
        let (low, high) = (first.min(last), first.max(last));

        seek(&mut scene, seed, low);

        let mut pixels = Vec::new();

        for frame in low..=high {
            let layers = scene.layers();

            render(&scene, &layers, camera, width, height, &mut pixels);

            let flipped: Vec<u8> = pixels.chunks_exact(width as usize * 4).rev().flatten().copied().collect();

            if tx.send(EncoderMessage::Frame(flipped, width, height, FRAME_DELAY_MS)).is_err() {
                warn!("The background encoder stopped accepting frames");
                return;
            }

            if frame < high {
                scene.tick();
            }
        }

        if tx.send(EncoderMessage::Finish).is_err() {
            warn!("The background encoder closed before the export could finish");
        }
    });

    if let Err(err) = renderer {
        error!("The background render thread could not be started: {err}");
        abort.store(true, Ordering::Relaxed);
    }
}

fn seek(scene: &mut Scene, seed: u32, frame: i32) {
    scene.restart(seed);

    for _ in 0..frame.max(0) {
        scene.tick();
    }
}

fn finished(ui: &AppWindow, state: &Shared, context: &Rc<Context>, ok: bool) {
    {
        let mut held = state.borrow_mut();

        held.backdrop.exporting = false;
        held.backdrop.feedback = Some(ok);

        let weak = ui.as_weak();
        let landing = Rc::clone(state);
        let ctx = Rc::clone(context);

        held.backdrop.feedback_timer.start(TimerMode::SingleShot, Duration::from_millis(FEEDBACK_MS), move || {
            landing.borrow_mut().backdrop.feedback = None;

            if let Some(ui) = weak.upgrade() {
                refresh(&ui, &landing, &ctx);
            }
        });
    }

    if ok {
        delivery::announce(ui, context);
    }

    refresh(ui, state, context);
}

pub fn refresh(ui: &AppWindow, state: &Shared, _context: &Rc<Context>) {
    let utilities = ui.global::<Utilities>();
    let backdrop = ui.global::<Backdrop>();
    let held = state.borrow();
    let bg = &held.backdrop;
    let vanilla = bg.vanilla();
    let names = bg.names.as_ref();
    let face = |picked: Option<&Path>, shown: Option<&str>, idle: &str| {
        if vanilla {
            return (shown.map_or_else(|| idle.to_owned(), |name| picker::label(Path::new(name))), Tone::Neutral);
        }

        picked.map_or((idle.to_owned(), Tone::Neutral), |path| (picker::label(path), Tone::Success))
    };
    let (png_label, png_tone) = face(bg.overrides.png.as_deref(), names.map(|names| names.png.as_str()), "Add PNG");
    let (cut_label, cut_tone) = face(bg.overrides.imgcut.as_deref(), names.map(|names| names.imgcut.as_str()), "Add IMGCUT");
    let (csv_label, csv_tone) = face(bg.overrides.csv.as_deref(), names.map(|names| names.csv.as_str()), "Add BG CSV");
    let available = !bg.catalog.ids.is_empty();
    let hint = if vanilla { VANILLA_HINT.to_owned() } else if available { bg.hint.clone() } else { NO_BACKGROUNDS_HINT.to_owned() };
    let (export_label, export_tone) = match (bg.exporting, bg.feedback) {
        (true, _) => (EXPORT_BUSY, Tone::StatusWarning),
        (false, Some(true)) => (EXPORT_DONE, Tone::StatusSuccess),
        (false, Some(false)) => (EXPORT_FAILED, Tone::StatusDanger),
        (false, None) => (EXPORT_IDLE, Tone::Primary),
    };

    utilities.set_bg_png_label(png_label.into());
    utilities.set_bg_png_tone(png_tone);
    utilities.set_bg_cut_label(cut_label.into());
    utilities.set_bg_cut_tone(cut_tone);
    utilities.set_bg_csv_label(csv_label.into());
    utilities.set_bg_csv_tone(csv_tone);
    utilities.set_bg_files_enabled(!vanilla);
    utilities.set_bg_id(bg.id_input.as_str().into());
    utilities.set_bg_id_enabled(available);
    utilities.set_bg_hint(hint.into());
    utilities.set_bg_seed(bg.seed_input.as_str().into());
    utilities.set_bg_notice(bg.notice.clone().unwrap_or_default().into());
    backdrop.set_loaded(bg.loaded.is_some());
    backdrop.set_playing(bg.playing);
    backdrop.set_animated(bg.animated());
    backdrop.set_frame_text((bg.frame.floor() as i32).to_string().into());
    backdrop.set_range_start(bg.range_inputs.0.as_str().into());
    backdrop.set_range_end(bg.range_inputs.1.as_str().into());
    backdrop.set_format(FORMATS.iter().position(|format| *format == bg.format).unwrap_or(0) as i32);
    backdrop.set_export_label(export_label.into());
    backdrop.set_export_tone(export_tone);
    backdrop.set_exporting(bg.exporting);
}

pub fn pick_png(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let landing = Rc::clone(state);

    picker::open(
        ui,
        context,
        "Select a PNG",
        &["png"],
        Box::new(move |ui, ctx, path| {
            landing.borrow_mut().backdrop.overrides.png = Some(path);
            load(ui, &landing, ctx);
        }),
    );
}

pub fn pick_cut(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let landing = Rc::clone(state);

    picker::open(
        ui,
        context,
        "Select an IMGCUT",
        &["imgcut"],
        Box::new(move |ui, ctx, path| {
            landing.borrow_mut().backdrop.overrides.imgcut = Some(path);
            load(ui, &landing, ctx);
        }),
    );
}
