use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use slint::{ComponentHandle, ModelRc, SharedString, Timer, TimerMode, VecModel};
use tracing::{error, warn};

use codex::graphics::animate::resolve_frame;
use codex::graphics::rig::{Animation, Rig};
use foundry::Source;
use foundry::common::job::ProgressCounter;
use foundry::systems::animation::export::process::calculate_export_time;
use foundry::systems::animation::export::{BoundsOutcome, EncoderMessage, EncoderStatus, ExportFormat, ExportMode, ExportRequest, FrameTiming, LoopStatus, ShowcaseLengths, find_bounds, find_loop, leader, process};
use foundry::systems::animation::{Role, playback_frames};

use crate::pages::Context;
use crate::{AppWindow, Export, Tone, bridge, delivery};

use super::{Region, scene};

const NOTHING_LOADED: &str = "Load an animation first";
const NO_CAMERA: &str = "Set a camera first";
const RIG_LOADING: &str = "The rig is still loading, try again in a moment";
const DEFAULT_CULL: i32 = 100;
const PERCENT_MIN: i32 = 0;
const PERCENT_MAX: i32 = 100;
const SHOWN_SECS: f32 = 3.0;
const POLL_MS: u64 = 100;
const FORMATS: [ExportFormat; 3] = [ExportFormat::Gif, ExportFormat::WebP, ExportFormat::Zip];
const GRAY: [u8; 4] = [80, 80, 80, 255];
const CLEAR: [u8; 4] = [0, 0, 0, 0];

enum Outcome {
    Completed,
    Terminated,
}

enum Phase {
    Running,
    Aborting,
    Done(Outcome, Option<Instant>),
}

struct Job {
    phase: Phase,
    abort: Arc<AtomicBool>,
    rendered: Arc<AtomicI32>,
    encoded: i32,
    total: i32,
}

enum Found {
    Done,
    Terminated,
    Error(String),
}

enum SearchPhase {
    Running,
    Aborting,
    Done(Found, Option<Instant>),
}

struct Search {
    phase: SearchPhase,
    abort: Arc<AtomicBool>,
    progress: Arc<ProgressCounter>,
    frames: usize,
    started: Instant,
}

pub struct Form {
    frame_start: i32,
    frame_end: i32,
    max_frame: i32,
    frame_start_str: String,
    frame_end_str: String,
    mode: ExportMode,
    loop_supported: bool,
    showcasable: bool,
    loop_tolerance: i32,
    loop_tolerance_str: String,
    loop_min: i32,
    loop_min_str: String,
    loop_max: Option<i32>,
    loop_max_str: String,
    cull_percent: i32,
    cull_percent_str: String,
    showcase_walk_str: String,
    showcase_idle_str: String,
    showcase_attack_str: String,
    showcase_kb_str: String,
    showcase_walk_len: i32,
    showcase_idle_len: i32,
    detected_attack_len: i32,
    showcase_attack_len: i32,
    showcase_kb_len: i32,
    detected_walk_len: i32,
    detected_idle_len: i32,
    fps: i32,
    zoom: f32,
    region: [f32; 4],
    region_str: [String; 4],
    file_name: String,
    name_prefix: String,
    format: ExportFormat,
    quality_percent: i32,
    quality_percent_str: String,
    compression_percent: i32,
    compression_percent_str: String,
    background: bool,
    user_bg_preference: bool,
    job: Option<Job>,
    loop_job: Option<Search>,
    bounds_job: Option<Search>,
    synced_key: Option<(String, Option<usize>)>,
    scanned_showcase: Option<String>,
    poll: Timer,
    tickets: Option<Tickets>,
}

#[derive(Clone, Copy)]
struct Tickets {
    encoder: bridge::Ticket,
    search: bridge::Ticket,
    bounds: bridge::Ticket,
}

impl Default for Form {
    fn default() -> Self {
        Self {
            frame_start: 0,
            frame_end: 0,
            max_frame: 100,
            frame_start_str: String::new(),
            frame_end_str: String::new(),
            mode: ExportMode::Manual,
            loop_supported: false,
            showcasable: false,
            loop_tolerance: 30,
            loop_tolerance_str: String::new(),
            loop_min: 15,
            loop_min_str: String::new(),
            loop_max: None,
            loop_max_str: String::new(),
            cull_percent: DEFAULT_CULL,
            cull_percent_str: String::new(),
            showcase_walk_str: String::new(),
            showcase_idle_str: String::new(),
            showcase_attack_str: String::new(),
            showcase_kb_str: String::new(),
            showcase_walk_len: 115,
            showcase_idle_len: 115,
            detected_attack_len: 0,
            showcase_attack_len: 0,
            showcase_kb_len: 60,
            detected_walk_len: 115,
            detected_idle_len: 115,
            fps: 30,
            zoom: 1.0,
            region: [0.0; 4],
            region_str: Default::default(),
            file_name: String::new(),
            name_prefix: String::new(),
            format: ExportFormat::Gif,
            quality_percent: 80,
            quality_percent_str: String::new(),
            compression_percent: 30,
            compression_percent_str: String::new(),
            background: false,
            user_bg_preference: false,
            job: None,
            loop_job: None,
            bounds_job: None,
            synced_key: None,
            scanned_showcase: None,
            poll: Timer::default(),
            tickets: None,
        }
    }
}

fn is_forced_opaque(format: &ExportFormat) -> bool {
    *format == ExportFormat::Gif
}

fn percentage(input: &str, empty: i32) -> Option<(i32, String)> {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return Some((empty, String::new()));
    }

    let parsed = trimmed.parse::<i32>().ok()?;

    (PERCENT_MIN..=PERCENT_MAX).contains(&parsed).then(|| (parsed, trimmed.to_owned()))
}

fn is_typable_number(value: &str, allow_negative: bool, allow_decimal: bool) -> bool {
    let mut seen_dot = false;

    value.chars().enumerate().all(|(index, glyph)| match glyph {
        '0'..='9' => true,
        '-' => allow_negative && index == 0,
        '.' if allow_decimal && !seen_dot => {
            seen_dot = true;
            true
        }
        _ => false,
    })
}

fn prefix(set_name: &str, slug: &str) -> String {
    let mut parts = set_name.split('_');
    let mut clean = parts.next().unwrap_or("").to_owned();

    if let Some(form) = parts.next()
        && !clean.is_empty()
        && clean.chars().all(|c| c.is_ascii_digit())
    {
        let number = match form.chars().next() {
            Some('f') => 1,
            Some('c') => 2,
            Some('s') => 3,
            Some('u') => 4,
            _ => 0,
        };

        if number > 0 {
            clean = format!("{clean}-{number}");
        }
    }

    format!("{clean}.{slug}")
}

fn format_index(format: &ExportFormat) -> usize {
    FORMATS.iter().position(|held| held == format).unwrap_or(0)
}

impl Form {
    fn with_settings(settings: &foundry::domains::settings::Settings, memory: &crate::memory::Remembered, previous: &Form) -> Self {
        let format = FORMATS.get(usize::from(memory.export_format)).cloned().unwrap_or(ExportFormat::Gif);

        Self {
            cull_percent: settings.animation.bounds_cull,
            showcase_walk_len: settings.animation.default_showcase_walk,
            showcase_idle_len: settings.animation.default_showcase_idle,
            showcase_kb_len: settings.animation.default_showcase_kb,
            detected_walk_len: settings.animation.default_showcase_walk,
            detected_idle_len: settings.animation.default_showcase_idle,
            mode: previous.mode.clone(),
            background: is_forced_opaque(&format) || memory.export_background,
            user_bg_preference: memory.export_background,
            quality_percent: memory.export_quality.unwrap_or(80),
            quality_percent_str: memory.export_quality.map_or_else(String::new, |value| value.to_string()),
            compression_percent: memory.export_compression.unwrap_or(30),
            compression_percent_str: memory.export_compression.map_or_else(String::new, |value| value.to_string()),
            format,
            job: previous.job.take_running(),
            loop_job: None,
            bounds_job: None,
            tickets: previous.tickets,
            synced_key: previous.synced_key.clone(),
            ..Self::default()
        }
    }

    fn to_request(&self) -> ExportRequest {
        ExportRequest {
            timing: FrameTiming { mode: self.mode.clone(), frame_start: self.frame_start, frame_end: self.frame_end, loop_supported: self.loop_supported },
            showcase: ShowcaseLengths { walk: self.showcase_walk_len, idle: self.showcase_idle_len, attack: self.showcase_attack_len, kb: self.showcase_kb_len },
            file_name: (!self.file_name.trim().is_empty()).then(|| self.file_name.clone()),
            name_prefix: self.name_prefix.clone(),
            format: self.format.clone(),
            quality_percent: self.quality_percent as u32,
            compression_percent: self.compression_percent as u32,
            fps: self.fps as u32,
            width: self.region[2].round() as u32,
            height: self.region[3].round() as u32,
            background: self.background,
        }
    }

    fn set_region(&mut self, region: Region) {
        self.region = [region.x, region.y, region.w, region.h];
        self.region_str = [region.x.to_string(), region.y.to_string(), region.w.to_string(), region.h.to_string()];
        self.zoom = 1.0;
    }

    fn clear_region(&mut self) {
        self.region = [0.0; 4];
        self.region_str = Default::default();
        self.zoom = 1.0;
    }

    pub fn camera_region(&self, open: bool) -> Option<Region> {
        (open && self.region[2] > 0.1 && self.region[3] > 0.1).then(|| Region { x: self.region[0], y: self.region[1], w: self.region[2], h: self.region[3] })
    }

    fn job_active(&self) -> bool {
        matches!(self.job.as_ref().map(|job| &job.phase), Some(Phase::Running | Phase::Aborting))
    }

    fn search_active(search: Option<&Search>) -> bool {
        matches!(search.map(|job| &job.phase), Some(SearchPhase::Running | SearchPhase::Aborting))
    }

    fn busy(&self) -> bool {
        self.job_active() || Self::search_active(self.loop_job.as_ref()) || Self::search_active(self.bounds_job.as_ref())
    }

    fn length_of(&mut self, role: Role, frames: i32) {
        match role {
            Role::Walk => self.showcase_walk_len = frames,
            Role::Idle => self.showcase_idle_len = frames,
            Role::Attack => self.showcase_attack_len = frames,
            Role::Knockback => self.showcase_kb_len = frames,
        }
    }

    fn tick(&mut self) -> bool {
        if let Some(job) = self.job.as_mut() {
            job.encoded = job.encoded.max(0);

            if let Phase::Done(_, shown) = &mut job.phase
                && shown.is_none()
            {
                *shown = Some(Instant::now());
            }
        }

        if self.job.as_ref().is_some_and(|job| matches!(&job.phase, Phase::Done(_, Some(at)) if at.elapsed().as_secs_f32() > SHOWN_SECS)) {
            self.job = None;
        }

        for slot in [&mut self.loop_job, &mut self.bounds_job] {
            if let Some(search) = slot.as_mut() {
                search.frames = search.frames.max(search.progress.current());

                if let SearchPhase::Done(_, shown) = &mut search.phase
                    && shown.is_none()
                {
                    *shown = Some(Instant::now());
                }
            }

            if slot.as_ref().is_some_and(|search| matches!(&search.phase, SearchPhase::Done(_, Some(at)) if at.elapsed().as_secs_f32() > SHOWN_SECS)) {
                *slot = None;
            }
        }

        self.busy() || self.job.is_some() || self.loop_job.is_some() || self.bounds_job.is_some()
    }
}

trait TakeRunning {
    fn take_running(&self) -> Option<Job>;
}

impl TakeRunning for Option<Job> {
    fn take_running(&self) -> Option<Job> {
        None
    }
}

fn form<'a>(context: &'a Rc<Context>) -> std::cell::RefMut<'a, super::State> {
    context.viewer.borrow_mut()
}

pub fn wire(window: &AppWindow, context: &Rc<Context>) {
    let export = window.global::<Export>();
    let on = |context: &Rc<Context>, window: &AppWindow| (Rc::clone(context), window.as_weak());

    {
        let (weak_context, encoder_context, bounds_context) = (Rc::downgrade(context), Rc::downgrade(context), Rc::downgrade(context));
        let encoder = bridge::subscribe::<((String, Option<usize>), EncoderStatus)>(move |ui, (key, status)| {
            if let Some(context) = encoder_context.upgrade() {
                encoder_status(ui, &context, key, status);
            }
        });
        let search = bridge::subscribe::<((String, Option<usize>), LoopStatus)>(move |ui, (key, status)| {
            if let Some(context) = weak_context.upgrade() {
                loop_status(ui, &context, key, status);
            }
        });
        let bounds = bridge::subscribe::<((String, Option<usize>), BoundsOutcome)>(move |ui, (key, outcome)| {
            if let Some(context) = bounds_context.upgrade() {
                bounds_outcome(ui, &context, key, outcome);
            }
        });

        form(context).export.tickets = Some(Tickets { encoder, search, bounds });
    }

    let (ctx, weak) = on(context, window);
    export.on_set_mode(move |index| {
        let Some(ui) = weak.upgrade() else {
            return;
        };

        {
            let mut state = form(&ctx);
            let mode = match index {
                1 => ExportMode::Loop,
                2 if state.export.showcasable => ExportMode::Showcase,
                _ => ExportMode::Manual,
            };

            if mode == ExportMode::Showcase {
                state.export.showcase_walk_str.clear();
                state.export.showcase_idle_str.clear();
                state.export.showcase_attack_str.clear();
                state.export.showcase_kb_str.clear();
            }

            state.export.mode = mode;
        }

        scan_showcase(&ctx);
        refresh(&ui, &ctx);
    });

    let (ctx, weak) = on(context, window);
    export.on_set_format(move |index| {
        let format = FORMATS.get(index.max(0) as usize).cloned().unwrap_or(ExportFormat::Gif);

        {
            let mut state = form(&ctx);

            state.export.format = format.clone();
            state.export.background = is_forced_opaque(&format) || state.export.user_bg_preference;
        }

        ctx.memory.update(|memory| memory.export_format = format_index(&format) as u8);

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &ctx);
        }
    });

    let (ctx, weak) = on(context, window);
    export.on_set_name(move |value| {
        form(&ctx).export.file_name = value.to_string();

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &ctx);
        }
    });

    let (ctx, weak) = on(context, window);
    export.on_set_start(move |value| {
        {
            let mut state = form(&ctx);

            state.export.frame_start_str = value.to_string();

            if value.trim().is_empty() {
                state.export.frame_start = 0;
            } else if let Ok(parsed) = value.trim().parse::<i32>() {
                state.export.frame_start = parsed;
            }
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &ctx);
        }
    });

    let (ctx, weak) = on(context, window);
    export.on_set_end(move |value| {
        {
            let mut state = form(&ctx);

            state.export.frame_end_str = value.to_string();

            if value.trim().is_empty() {
                state.export.frame_end = state.export.max_frame;
            } else if let Ok(parsed) = value.trim().parse::<i32>() {
                state.export.frame_end = parsed;
            }
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &ctx);
        }
    });

    let (ctx, weak) = on(context, window);
    export.on_set_tolerance(move |value| {
        if is_typable_number(&value, false, false) {
            let mut state = form(&ctx);

            state.export.loop_tolerance_str = value.to_string();

            if let Ok(parsed) = value.parse::<i32>() {
                state.export.loop_tolerance = parsed;
            }
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &ctx);
        }
    });

    let (ctx, weak) = on(context, window);
    export.on_set_loop_min(move |value| {
        {
            let mut state = form(&ctx);

            state.export.loop_min_str = value.to_string();

            if let Ok(parsed) = value.parse::<i32>() {
                state.export.loop_min = parsed;
            }
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &ctx);
        }
    });

    let (ctx, weak) = on(context, window);
    export.on_set_loop_max(move |value| {
        {
            let mut state = form(&ctx);

            state.export.loop_max = value.parse::<i32>().ok();
            state.export.loop_max_str = value.to_string();
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &ctx);
        }
    });

    let (ctx, weak) = on(context, window);
    export.on_set_cull(move |value| {
        if let Some((percent, text)) = percentage(&value, DEFAULT_CULL) {
            {
                let mut state = form(&ctx);

                state.export.cull_percent = percent;
                state.export.cull_percent_str = text;
            }

            ctx.state.apply(|settings| settings.animation.bounds_cull = percent);
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &ctx);
        }
    });

    for (role, which) in [(Role::Walk, 0), (Role::Idle, 1), (Role::Attack, 2), (Role::Knockback, 3)] {
        let (ctx, weak) = on(context, window);
        let set = move |value: SharedString| {
            {
                let mut state = form(&ctx);
                let text = value.to_string();
                let fallback = match role {
                    Role::Walk => state.export.detected_walk_len,
                    Role::Idle => state.export.detected_idle_len,
                    Role::Attack => state.export.detected_attack_len,
                    Role::Knockback => ctx.state.settings().animation.default_showcase_kb,
                };
                let parsed = if text.trim().is_empty() { Some(fallback) } else { text.parse::<i32>().ok() };

                match role {
                    Role::Walk => state.export.showcase_walk_str = text,
                    Role::Idle => state.export.showcase_idle_str = text,
                    Role::Attack => state.export.showcase_attack_str = text,
                    Role::Knockback => state.export.showcase_kb_str = text,
                }

                if let Some(parsed) = parsed {
                    state.export.length_of(role, parsed);
                }
            }

            if let Some(ui) = weak.upgrade() {
                refresh(&ui, &ctx);
            }
        };

        match which {
            0 => export.on_set_walk(set),
            1 => export.on_set_idle(set),
            2 => export.on_set_attack(set),
            _ => export.on_set_kb(set),
        }
    }

    let (ctx, weak) = on(context, window);
    export.on_set_region(move |which, value| {
        let which = which.clamp(0, 3) as usize;

        if is_typable_number(&value, which < 2, true) {
            let mut state = form(&ctx);

            state.export.region_str[which] = value.to_string();

            if value.trim().is_empty() {
                state.export.region[which] = 0.0;
            } else if let Ok(parsed) = value.parse::<f32>() {
                state.export.region[which] = parsed;
            }
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &ctx);
        }
    });

    let (ctx, weak) = on(context, window);
    export.on_set_quality(move |value| {
        if let Some((percent, text)) = percentage(&value, PERCENT_MAX) {
            {
                let mut state = form(&ctx);

                state.export.quality_percent = percent;
                state.export.quality_percent_str = text.clone();
            }

            ctx.memory.update(|memory| memory.export_quality = (!text.is_empty()).then_some(percent));
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &ctx);
        }
    });

    let (ctx, weak) = on(context, window);
    export.on_set_compression(move |value| {
        if let Some((percent, text)) = percentage(&value, PERCENT_MIN) {
            {
                let mut state = form(&ctx);

                state.export.compression_percent = percent;
                state.export.compression_percent_str = text.clone();
            }

            ctx.memory.update(|memory| memory.export_compression = (!text.is_empty()).then_some(percent));
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &ctx);
        }
    });

    let (ctx, weak) = on(context, window);
    export.on_toggle_background(move |enabled| {
        {
            let mut state = form(&ctx);

            state.export.background = enabled;
            state.export.user_bg_preference = enabled;
        }

        ctx.memory.update(|memory| memory.export_background = enabled);

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &ctx);
        }
    });

    let (ctx, weak) = on(context, window);
    export.on_begin(move || {
        if let Some(ui) = weak.upgrade() {
            begin(&ui, &ctx);
        }
    });

    let (ctx, weak) = on(context, window);
    export.on_find_loop(move || {
        if let Some(ui) = weak.upgrade() {
            find_loop(&ui, &ctx);
        }
    });

    let (ctx, weak) = on(context, window);
    export.on_use_bounds(move || {
        if let Some(ui) = weak.upgrade() {
            use_bounds(&ui, &ctx);
        }
    });

    let (ctx, weak) = on(context, window);
    export.on_set_camera(move || {
        if let Some(ui) = weak.upgrade() {
            ui.global::<Export>().set_open(false);
            form(&ctx).region = None;
            super::begin_selecting(&ui, &ctx);
        }
    });

    let (ctx, weak) = on(context, window);
    export.on_closed(move || {
        form(&ctx).region = None;

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &ctx);
        }
    });
}

pub fn open(ui: &AppWindow, context: &Rc<Context>) {
    synced(ui, context);
    ui.global::<Export>().set_open(true);
    refresh(ui, context);
}

pub fn region_picked(ui: &AppWindow, context: &Rc<Context>, region: Option<Region>) {
    if let Some(region) = region {
        form(context).export.set_region(region);
    }

    ui.global::<Export>().set_open(true);
    refresh(ui, context);
}

fn current_key(state: &super::State) -> Option<(String, Option<usize>)> {
    state.set.as_ref().map(|set| (set.name.clone(), state.selected()))
}

pub fn synced(ui: &AppWindow, context: &Rc<Context>) {
    {
        let mut state = form(context);
        let Some(key) = current_key(&state) else {
            return;
        };

        if state.export.synced_key.as_ref() != Some(&key) {
            let unit_changed = state.export.synced_key.as_ref().is_none_or(|(name, _)| *name != key.0);

            state.export.synced_key = Some(key);

            if unit_changed {
                let memory = context.memory.get();
                let settings = context.state.snapshot();

                state.export = Form::with_settings(&settings, &memory, &state.export);
            }

            state.export.loop_supported = state.loop_supported();
            state.export.showcasable = state.set.as_ref().is_some_and(|set| set.motions.iter().any(|motion| motion.role.is_some() && motion.file.is_some()));

            let unsupported = (state.export.mode == ExportMode::Loop && !state.export.loop_supported) || (state.export.mode == ExportMode::Showcase && !state.export.showcasable);

            if unsupported {
                state.export.mode = ExportMode::Manual;
                state.export.frame_start = 0;
                state.export.frame_end = 0;
                state.export.frame_start_str.clear();
                state.export.frame_end_str.clear();
            }

            if state.export.mode != ExportMode::Showcase {
                let max = state.anim().map_or(0, |anim| anim.declared_frames() - 1);

                state.export.max_frame = max;
                state.export.frame_start = 0;
                state.export.frame_end = max;
                state.export.frame_start_str.clear();
                state.export.frame_end_str.clear();
            }

            let slug = state.motion().map_or_else(|| "anim".to_owned(), |motion| motion.slug());
            let set_name = state.set.as_ref().map(|set| set.name.clone()).unwrap_or_default();

            state.export.name_prefix = prefix(&set_name, &slug);
        }
    }

    scan_showcase(context);

    if ui.global::<Export>().get_open() {
        refresh(ui, context);
    }
}

fn role_paths(state: &super::State) -> Vec<(Role, Source)> {
    state.set.as_ref().map(|set| set.motions.iter().filter_map(|motion| Some((motion.role?, motion.file.clone()?))).collect()).unwrap_or_default()
}

fn scan_showcase(context: &Rc<Context>) {
    let mut state = form(context);

    if state.export.mode != ExportMode::Showcase {
        return;
    }

    let Some(name) = state.set.as_ref().map(|set| set.name.clone()) else {
        return;
    };

    if state.export.scanned_showcase.as_deref() == Some(name.as_str()) {
        return;
    }

    state.export.scanned_showcase = Some(name);

    let paths = role_paths(&state);
    let parse = |role: Role| -> Option<Animation> {
        let (_, source) = paths.iter().find(|(known, _)| *known == role)?;
        let bytes = source.read().ok()?;

        Animation::parse(&bytes).ok()
    };
    let settings = context.state.snapshot();

    for role in [Role::Walk, Role::Idle, Role::Attack, Role::Knockback] {
        if !paths.iter().any(|(known, _)| *known == role) {
            state.export.length_of(role, 0);
        }
    }

    if let Some(attack) = parse(Role::Attack) {
        let total = attack.declared_frames();

        state.export.detected_attack_len = total;

        if state.export.showcase_attack_str.is_empty() {
            state.export.showcase_attack_len = total;
        }
    }

    if let Some(walk) = parse(Role::Walk) {
        let length = if playback_frames(&walk) <= 2 { 0 } else { settings.animation.default_showcase_walk };

        state.export.detected_walk_len = length;

        if state.export.showcase_walk_str.is_empty() || state.export.showcase_walk_len == settings.animation.default_showcase_walk {
            state.export.showcase_walk_len = length;
        }
    }

    if let Some(idle) = parse(Role::Idle) {
        let length = if playback_frames(&idle) <= 2 { 0 } else { settings.animation.default_showcase_idle };

        state.export.detected_idle_len = length;

        if state.export.showcase_idle_str.is_empty() || state.export.showcase_idle_len == settings.animation.default_showcase_idle {
            state.export.showcase_idle_len = length;
        }
    }
}

fn search_face(search: Option<&Search>, idle: &str, abort: &str, terminated: &str, locked: bool) -> (SharedString, Tone, bool) {
    match search.map(|job| &job.phase) {
        Some(SearchPhase::Running | SearchPhase::Aborting) => (abort.into(), Tone::Danger, true),
        Some(SearchPhase::Done(Found::Terminated, _)) => (terminated.into(), Tone::Danger, !locked),
        _ => (idle.into(), Tone::Neutral, !locked),
    }
}

pub fn refresh(ui: &AppWindow, context: &Rc<Context>) {
    let export = ui.global::<Export>();
    let open = export.get_open();
    let mut state = form(context);
    let region = state.export.camera_region(open);

    state.region = region;
    state.last_stamp = None;

    let f = &state.export;
    let locked = f.job_active();
    let loop_active = Form::search_active(f.loop_job.as_ref());
    let bounds_active = Form::search_active(f.bounds_job.as_ref());
    let modes: Vec<SharedString> = if f.showcasable { vec!["Manual".into(), "Loop".into(), "Showcase".into()] } else { vec!["Manual".into(), "Loop".into()] };
    let mode_index = match f.mode {
        ExportMode::Manual => 0,
        ExportMode::Loop => 1,
        ExportMode::Showcase => 2,
    };
    let (display_start, display_end) = if f.mode == ExportMode::Showcase {
        let total = f.showcase_walk_len + f.showcase_idle_len + f.showcase_attack_len + f.showcase_kb_len;

        (0, if total > 0 { total - 1 } else { 0 })
    } else {
        (f.frame_start, f.frame_end)
    };
    let range_part = if display_start == display_end { format!("{display_start}f") } else { format!("{display_start}f~{display_end}f") };
    let prefix_display = if f.mode == ExportMode::Showcase {
        f.name_prefix.split('.').next().map_or_else(|| "unit.showcase".to_owned(), |first| format!("{first}.showcase"))
    } else {
        f.name_prefix.clone()
    };
    let name_hint = if prefix_display.is_empty() { "animation".to_owned() } else { format!("{prefix_display}.{range_part}") };
    let (loop_label, loop_tone, loop_enabled) = search_face(f.loop_job.as_ref(), "Find Loop", "Abort Loop", "Loop Terminated!", locked);
    let (bounds_label, bounds_tone, bounds_enabled) = search_face(f.bounds_job.as_ref(), "Use Bounds", "Abort Bounds", "Terminated!", locked);
    let (ratio, status) = if let Some(job) = f.bounds_job.as_ref().filter(|job| matches!(job.phase, SearchPhase::Running | SearchPhase::Aborting)) {
        (job.progress.fraction(), format!("Searching | {} frames", job.frames))
    } else if let Some(job) = f.loop_job.as_ref().filter(|job| matches!(job.phase, SearchPhase::Running | SearchPhase::Aborting)) {
        (job.started.elapsed().as_secs_f32() % 1.0, format!("Searching | {} frames", job.frames))
    } else if let Some(job) = f.job.as_ref() {
        let rendered = job.rendered.load(Ordering::Relaxed);
        let ratio_of = |value: i32| (value as f32 / job.total.max(1) as f32).min(1.0);

        match &job.phase {
            Phase::Running if rendered < job.total => {
                let ratio = ratio_of(rendered);

                (ratio, format!("Rendering | {rendered}f/{}f ({}%)", job.total, (ratio * 100.0) as i32))
            }
            Phase::Running => {
                let ratio = ratio_of(job.encoded);

                (ratio, format!("Encoding | {}f/{}f ({}%)", job.encoded, job.total, (ratio * 100.0) as i32))
            }
            Phase::Aborting => (1.0, "Aborting...".to_owned()),
            Phase::Done(Outcome::Completed, _) => (1.0, "Done".to_owned()),
            Phase::Done(Outcome::Terminated, _) => (1.0, "Ready".to_owned()),
        }
    } else if let Some(SearchPhase::Done(result, _)) = f.loop_job.as_ref().map(|job| &job.phase) {
        (
            1.0,
            match result {
                Found::Done => "Done".to_owned(),
                Found::Terminated => "Loop Terminated!".to_owned(),
                Found::Error(message) => message.clone(),
            },
        )
    } else if let Some(SearchPhase::Done(result, _)) = f.bounds_job.as_ref().map(|job| &job.phase) {
        (
            1.0,
            match result {
                Found::Done => "Done".to_owned(),
                Found::Terminated => "Bounds Terminated!".to_owned(),
                Found::Error(message) => message.clone(),
            },
        )
    } else {
        (1.0, "Ready".to_owned())
    };
    let valid = f.region[2] > 0.1 && f.region[3] > 0.1;
    let (export_label, export_tone, export_enabled) = match f.job.as_ref().map(|job| &job.phase) {
        Some(Phase::Running) => ("Abort Export", Tone::Danger, true),
        Some(Phase::Aborting) => ("Aborting...", Tone::Danger, false),
        Some(Phase::Done(Outcome::Terminated, _)) => ("Export Terminated!", Tone::Danger, valid && !loop_active && !bounds_active),
        _ if valid => ("Begin Export", Tone::Primary, !loop_active && !bounds_active),
        _ => ("No Camera Set", Tone::Neutral, false),
    };

    export.set_modes(ModelRc::new(VecModel::from(modes)));
    export.set_mode(mode_index);
    export.set_format(format_index(&f.format) as i32);
    export.set_end_hint(f.max_frame.to_string().into());
    export.set_frame_start(f.frame_start_str.as_str().into());
    export.set_frame_end(f.frame_end_str.as_str().into());
    export.set_tolerance(f.loop_tolerance_str.as_str().into());
    export.set_loop_min(f.loop_min_str.as_str().into());
    export.set_loop_max(f.loop_max_str.as_str().into());
    export.set_loop_label(loop_label);
    export.set_loop_tone(loop_tone);
    export.set_loop_enabled(loop_enabled);
    export.set_walk(f.showcase_walk_str.as_str().into());
    export.set_idle(f.showcase_idle_str.as_str().into());
    export.set_attack(f.showcase_attack_str.as_str().into());
    export.set_kb(f.showcase_kb_str.as_str().into());
    export.set_walk_hint(f.detected_walk_len.to_string().into());
    export.set_idle_hint(f.detected_idle_len.to_string().into());
    export.set_attack_hint(f.detected_attack_len.to_string().into());
    export.set_kb_hint(context.state.settings().animation.default_showcase_kb.to_string().into());
    export.set_cull(f.cull_percent_str.as_str().into());
    export.set_cull_hint(DEFAULT_CULL.to_string().into());
    export.set_camera_enabled(!locked && !bounds_active);
    export.set_bounds_label(bounds_label);
    export.set_bounds_tone(bounds_tone);
    export.set_bounds_enabled(bounds_enabled);
    export.set_region_x(f.region_str[0].as_str().into());
    export.set_region_y(f.region_str[1].as_str().into());
    export.set_region_w(f.region_str[2].as_str().into());
    export.set_region_h(f.region_str[3].as_str().into());
    export.set_name(f.file_name.as_str().into());
    export.set_name_hint(name_hint.into());
    export.set_quality(f.quality_percent_str.as_str().into());
    export.set_compression(f.compression_percent_str.as_str().into());
    export.set_background(f.background);
    export.set_background_enabled(!is_forced_opaque(&f.format));
    export.set_status(status.into());
    export.set_ratio(ratio);
    export.set_export_label(export_label.into());
    export.set_export_tone(export_tone);
    export.set_export_enabled(export_enabled);

    let polling = f.job.is_some() || f.loop_job.is_some() || f.bounds_job.is_some();

    if polling && !state.export.poll.running() {
        let weak = ui.as_weak();
        let ticking = Rc::downgrade(context);

        state.export.poll.start(TimerMode::Repeated, Duration::from_millis(POLL_MS), move || {
            if let (Some(ui), Some(context)) = (weak.upgrade(), ticking.upgrade()) {
                let keep = form(&context).export.tick();

                if !keep {
                    form(&context).export.poll.stop();
                }

                refresh(&ui, &context);
            }
        });
    }
}

struct RenderJob {
    rig: Arc<Rig>,
    animation: Option<Arc<Animation>>,
    roles: Vec<(Role, Source)>,
    offset: Option<usize>,
    timing: FrameTiming,
    lengths: ShowcaseLengths,
    region: (f32, f32),
    zoom: f32,
    width: u32,
    height: u32,
    fps: i32,
    background: bool,
    tx: mpsc::Sender<EncoderMessage>,
    abort: Arc<AtomicBool>,
    progress: Arc<AtomicI32>,
}

struct ShowcaseMotions {
    walk: Option<Animation>,
    idle: Option<Animation>,
    attack: Option<Animation>,
    kb: Option<Animation>,
}

impl ShowcaseMotions {
    fn load(roles: &[(Role, Source)]) -> Self {
        let parse = |role: Role| -> Option<Animation> {
            let (_, source) = roles.iter().find(|(known, _)| *known == role)?;
            let bytes = source.read().ok()?;

            Animation::parse(&bytes).ok()
        };

        Self { walk: parse(Role::Walk), idle: parse(Role::Idle), attack: parse(Role::Attack), kb: parse(Role::Knockback) }
    }

    fn get(&self, role: Role) -> Option<&Animation> {
        match role {
            Role::Walk => self.walk.as_ref(),
            Role::Idle => self.idle.as_ref(),
            Role::Attack => self.attack.as_ref(),
            Role::Knockback => self.kb.as_ref(),
        }
    }
}

fn showcase_segment(lengths: ShowcaseLengths, progress: i32) -> (Role, f32) {
    let (walk, idle, attack) = (lengths.walk, lengths.idle, lengths.attack);

    if progress < walk {
        (Role::Walk, (progress % walk.max(1)) as f32)
    } else if progress < walk + idle {
        (Role::Idle, ((progress - walk) % idle.max(1)) as f32)
    } else if progress < walk + idle + attack {
        (Role::Attack, (progress - (walk + idle)) as f32)
    } else {
        (Role::Knockback, ((progress - (walk + idle + attack)) % lengths.kb.max(1)) as f32)
    }
}

fn remap_glow(parts: &mut [codex::graphics::animate::FrameData]) {
    for part in parts {
        match part.glow {
            1 | 3 => part.glow = 1,
            2 => part.glow = 0,
            _ => {}
        }
    }
}

fn render(job: RenderJob) {
    let motions = (job.timing.mode == ExportMode::Showcase).then(|| ShowcaseMotions::load(&job.roles));
    let frame_count = (job.timing.frame_end - job.timing.frame_start).abs() + 1;
    let delay_ms = (1000.0 / job.fps.max(1) as f32) as u32;
    let background = if job.background { GRAY } else { CLEAR };
    let view_proj = scene::export_view_proj(job.width as f32, job.height as f32, job.region.0, job.region.1, job.zoom);
    let seat = |progress: i32| {
        let (animation, local_time) = match &motions {
            Some(motions) => {
                let (role, time) = showcase_segment(job.lengths, progress);
                let animation = motions.get(role);
                let time = match animation {
                    Some(anim) if anim.declared_frames() > 1 => time,
                    _ => 0.0,
                };

                (animation, time)
            }
            None => (job.animation.as_deref(), 0.0),
        };

        (animation, calculate_export_time(&job.timing, animation, local_time, progress))
    };

    for progress in 0..frame_count {
        if job.abort.load(Ordering::Relaxed) {
            warn!("The export render loop was aborted at frame {progress}/{frame_count}");
            return;
        }

        let (animation, frame_time) = seat(progress);
        let mut parts = resolve_frame(&job.rig, animation, frame_time.floor() as i32, job.offset);

        remap_glow(&mut parts);

        let pixels = scene::render_parts(job.rig.sheet.image_data.as_deref(), &parts, &view_proj, background, job.width, job.height);

        if job.tx.send(EncoderMessage::Frame(pixels, job.width, job.height, delay_ms)).is_err() {
            warn!("The encoder closed mid-export, the render loop stops");
            return;
        }

        job.progress.store(progress + 1, Ordering::Relaxed);
    }

    if job.tx.send(EncoderMessage::Finish).is_err() {
        warn!("The encoder closed before the export could finish");
    }
}

fn begin(ui: &AppWindow, context: &Rc<Context>) {
    let weak = ui.as_weak();
    let spawned = {
        let mut state = form(context);
        let Some(key) = state.export.synced_key.clone() else {
            drop(state);
            context.toast(ui, NOTHING_LOADED, true);
            return;
        };
        let Some(tickets) = state.export.tickets else {
            return;
        };

        if let Some(job) = state.export.job.as_mut() {
            match job.phase {
                Phase::Running => {
                    job.abort.store(true, Ordering::Relaxed);
                    job.phase = Phase::Aborting;
                    drop(state);
                    refresh(ui, context);
                    return;
                }
                Phase::Aborting => return,
                Phase::Done(..) => {}
            }
        }

        let Some(rig) = state.rig() else {
            drop(state);
            context.toast(ui, RIG_LOADING, true);
            return;
        };

        if state.export.region[2] <= 0.1 || state.export.region[3] <= 0.1 {
            drop(state);
            context.toast(ui, NO_CAMERA, true);
            return;
        }

        state.export.region[2] = state.export.region[2].round();
        state.export.region[3] = state.export.region[3].round();

        let request = state.export.to_request();
        let config = process::build_config(&request);
        let total = (config.end_frame - config.start_frame).abs() + 1;
        let timing = FrameTiming { frame_start: config.start_frame, frame_end: config.end_frame, ..request.timing.clone() };
        let abort = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(AtomicI32::new(0));
        let (frame_tx, frame_rx) = mpsc::channel();
        let encoder_abort = Arc::clone(&abort);
        let encoder_key = key.clone();
        let encoder_weak = weak.clone();
        let leader = thread::Builder::new().name("export_encoder".to_owned()).spawn(move || {
            leader::run(config, frame_rx, |status| bridge::post(&encoder_weak, tickets.encoder, (encoder_key.clone(), status)), &encoder_abort);
        });

        if let Err(err) = leader {
            error!("The export encoder thread could not be started: {err}");
            return;
        }

        let job = RenderJob {
            rig,
            animation: state.anim(),
            roles: role_paths(&state),
            offset: state.offset(),
            timing,
            lengths: request.showcase,
            region: (state.export.region[0], state.export.region[1]),
            zoom: state.export.zoom,
            width: request.width.max(1),
            height: request.height.max(1),
            fps: state.export.fps,
            background: state.export.background,
            tx: frame_tx,
            abort: Arc::clone(&abort),
            progress: Arc::clone(&progress),
        };

        if let Err(err) = thread::Builder::new().name("export_render".to_owned()).spawn(move || render(job)) {
            error!("The export render thread could not be started: {err}");
            abort.store(true, Ordering::Relaxed);
            return;
        }

        state.export.job = Some(Job { phase: Phase::Running, abort, rendered: progress, encoded: 0, total });
        true
    };

    if spawned {
        refresh(ui, context);
    }
}

fn encoder_status(ui: &AppWindow, context: &Rc<Context>, key: (String, Option<usize>), status: EncoderStatus) {
    {
        let mut state = form(context);

        if state.export.synced_key.as_ref() != Some(&key) && !matches!(status, EncoderStatus::Finished) {
            return;
        }

        let Some(job) = state.export.job.as_mut() else {
            return;
        };

        match status {
            EncoderStatus::Progress(progress) => job.encoded = progress as i32,
            EncoderStatus::Finished => {
                let outcome = if matches!(job.phase, Phase::Aborting) { Outcome::Terminated } else { Outcome::Completed };

                job.phase = Phase::Done(outcome, None);
            }
        }
    }

    if matches!(status, EncoderStatus::Finished) && form(context).export.job.as_ref().is_some_and(|job| matches!(job.phase, Phase::Done(Outcome::Completed, _))) {
        delivery::announce(ui, context);
    }

    refresh(ui, context);
}

fn find_loop(ui: &AppWindow, context: &Rc<Context>) {
    {
        let mut state = form(context);
        let Some(key) = state.export.synced_key.clone() else {
            drop(state);
            context.toast(ui, NOTHING_LOADED, true);
            return;
        };
        let Some(tickets) = state.export.tickets else {
            return;
        };

        if state.export.job_active() {
            return;
        }

        if let Some(search) = state.export.loop_job.as_mut() {
            match search.phase {
                SearchPhase::Running => {
                    search.abort.store(true, Ordering::Relaxed);
                    search.phase = SearchPhase::Aborting;
                    drop(state);
                    refresh(ui, context);
                    return;
                }
                SearchPhase::Aborting => return,
                SearchPhase::Done(..) => {}
            }
        }

        let (Some(rig), Some(animation)) = (state.rig(), state.anim()) else {
            return;
        };
        let tolerance = state.export.loop_tolerance as f32;
        let minimum = state.export.loop_min;
        let maximum = state.export.loop_max;
        let abort = Arc::new(AtomicBool::new(false));
        let worker_abort = Arc::clone(&abort);
        let weak = ui.as_weak();
        let spawned = thread::Builder::new().name("loop_search".to_owned()).spawn(move || {
            find_loop::search(&rig, &animation, tolerance, minimum, maximum, |status| bridge::post(&weak, tickets.search, (key.clone(), status)), &worker_abort);
        });

        if let Err(err) = spawned {
            error!("The loop search thread could not be started: {err}");
            return;
        }

        state.export.loop_job = Some(Search { phase: SearchPhase::Running, abort, progress: Arc::new(ProgressCounter::default()), frames: 0, started: Instant::now() });
    }

    refresh(ui, context);
}

fn loop_status(ui: &AppWindow, context: &Rc<Context>, key: (String, Option<usize>), status: LoopStatus) {
    {
        let mut state = form(context);
        let applies = state.export.synced_key.as_ref() == Some(&key);
        let Some(search) = state.export.loop_job.as_mut() else {
            return;
        };

        match status {
            LoopStatus::Searching(frames) => search.frames = frames,
            LoopStatus::Found(start, end) => {
                let aborting = matches!(search.phase, SearchPhase::Aborting);

                search.phase = SearchPhase::Done(if aborting { Found::Terminated } else { Found::Done }, None);

                if !aborting && applies {
                    state.export.frame_start = start;
                    state.export.frame_end = end;
                    state.export.frame_start_str = start.to_string();
                    state.export.frame_end_str = end.to_string();
                }
            }
            LoopStatus::Error(message) => {
                let aborting = matches!(search.phase, SearchPhase::Aborting);

                search.phase = SearchPhase::Done(if aborting { Found::Terminated } else { Found::Error(message) }, None);
            }
        }
    }

    refresh(ui, context);
}

fn use_bounds(ui: &AppWindow, context: &Rc<Context>) {
    {
        let mut state = form(context);
        let Some(key) = state.export.synced_key.clone() else {
            drop(state);
            context.toast(ui, NOTHING_LOADED, true);
            return;
        };
        let Some(tickets) = state.export.tickets else {
            return;
        };

        if state.export.job_active() {
            return;
        }

        if let Some(search) = state.export.bounds_job.as_mut() {
            match search.phase {
                SearchPhase::Running => {
                    search.abort.store(true, Ordering::Relaxed);
                    search.phase = SearchPhase::Aborting;
                    drop(state);
                    refresh(ui, context);
                    return;
                }
                SearchPhase::Aborting => return,
                SearchPhase::Done(..) => {}
            }
        }

        let Some(rig) = state.rig() else {
            drop(state);
            context.toast(ui, RIG_LOADING, true);
            return;
        };
        let tolerance = state.export.cull_percent as f32 / 100.0;
        let showcase = state.export.mode == ExportMode::Showcase;
        let scan_limit = state.export.frame_end.max(state.export.frame_start).max(0);
        let current = state.anim();
        let offset = state.offset();
        let roles = role_paths(&state);
        let slots = [
            (Role::Walk, state.export.showcase_walk_len),
            (Role::Idle, state.export.showcase_idle_len),
            (Role::Attack, state.export.showcase_attack_len),
            (Role::Knockback, state.export.showcase_kb_len),
        ];
        let abort = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(ProgressCounter::default());
        let (worker_abort, worker_progress) = (Arc::clone(&abort), Arc::clone(&progress));
        let weak = ui.as_weak();
        let spawned = thread::Builder::new().name("bounds_search".to_owned()).spawn(move || {
            let mut showcase_motions = Vec::new();

            if showcase {
                for (role, length) in slots {
                    if length <= 0 {
                        continue;
                    }

                    if let Some((_, source)) = roles.iter().find(|(known, _)| *known == role)
                        && let Ok(bytes) = source.read()
                        && let Ok(anim) = Animation::parse(&bytes)
                    {
                        showcase_motions.push((anim, length));
                    }
                }
            }

            let motions: Vec<(&Animation, Option<i32>)> = if showcase {
                showcase_motions.iter().map(|(anim, length)| (anim, Some(*length))).collect()
            } else {
                current.iter().map(|anim| (anim.as_ref(), Some(scan_limit))).collect()
            };
            let outcome = find_bounds::search(&rig, &motions, tolerance, offset, &worker_progress, &worker_abort);

            bridge::post(&weak, tickets.bounds, (key, outcome));
        });

        if let Err(err) = spawned {
            error!("The bounds search thread could not be started: {err}");
            return;
        }

        state.export.bounds_job = Some(Search { phase: SearchPhase::Running, abort, progress, frames: 0, started: Instant::now() });
    }

    refresh(ui, context);
}

fn bounds_outcome(ui: &AppWindow, context: &Rc<Context>, key: (String, Option<usize>), outcome: BoundsOutcome) {
    {
        let mut state = form(context);
        let applies_key = state.export.synced_key.as_ref() == Some(&key);
        let Some(search) = state.export.bounds_job.as_mut() else {
            return;
        };
        let terminated = matches!(outcome, BoundsOutcome::Aborted) || matches!(search.phase, SearchPhase::Aborting | SearchPhase::Done(Found::Terminated, _));
        let applies = !terminated && applies_key;
        let result = if terminated {
            Found::Terminated
        } else if let BoundsOutcome::Found(bounds) = outcome {
            if applies {
                state.export.set_region(Region { x: bounds.min_x, y: bounds.min_y, w: bounds.width(), h: bounds.height() });
            }

            Found::Done
        } else {
            if applies {
                state.export.clear_region();
            }

            Found::Error("Nothing to measure".to_owned())
        };

        if let Some(search) = state.export.bounds_job.as_mut() {
            search.phase = SearchPhase::Done(result, None);
        }
    }

    refresh(ui, context);
}
