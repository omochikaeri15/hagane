use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use codex::graphics::animate::resolve_frame;
use codex::graphics::rig::{Animation, Rig};
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

use foundry::Source;
use foundry::common::job::ProgressCounter;
use foundry::domains::settings::{AnimSettings, CombatLook};
use foundry::systems::animation::export::process::{self, calculate_export_time};
use foundry::systems::animation::export::{BACKGROUND, BoundsOutcome, EncoderMessage, EncoderStatus, ExportConfig, ExportFormat, ExportMode, FrameTiming, ShowcaseLengths, find_bounds, leader};
use foundry::systems::animation::collision::{self, Combatant, Mark};
use foundry::systems::animation::{Loop, Motion, MotionSet, Placement, Role, loop_frame, name_prefix, overlay, paint, playback_frames};

use super::cache::{Cache, Key};
use super::grammar::{Looping, Overlays};
use super::library::Library;
use super::prose::Trouble;

const STAMP: &str = env!("HAGANE_ANIMATION_STAMP");
const QUALITY: u32 = 80;
const COMPRESSION: u32 = 0;
pub(crate) const LOOP_LIMIT: Duration = Duration::from_secs(5);
const FPS: u32 = 30;
const IN_FLIGHT: usize = 2;
const CLEAR: [u8; 4] = [0, 0, 0, 0];
const SHORT_LOOP: i32 = 2;
const SHOWCASE_ROLES: [Role; 4] = [Role::Walk, Role::Idle, Role::Attack, Role::Knockback];
const JOB_LIMIT: Duration = Duration::from_secs(180);
const FILE_NAME: &str = "name";
const TOO_BIG: &str = "big";
const OVERLAY_MARGIN: f32 = 2.0;
pub(crate) const MAX_FRAMES: i32 = 8400;
const NO_CULL: i32 = 0;
const STILL: i32 = 1;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Clip {
    Walk,
    Idle,
    Attack,
    Knockback,
    Showcase,
    Burrow,
    Surface,
    Dig,
    Spirit,
    Model,
}

impl Clip {
    pub(crate) fn motion(self) -> &'static str {
        match self {
            Clip::Walk => "Walk",
            Clip::Idle => "Idle",
            Clip::Attack => "Attack",
            Clip::Knockback => "Knockback",
            Clip::Showcase => "Showcase",
            Clip::Burrow => "Burrow",
            Clip::Surface => "Surface",
            Clip::Dig => "Dig",
            Clip::Spirit => "Spirit",
            Clip::Model => "Model",
        }
    }

    pub(crate) fn slug(self) -> &'static str {
        match self {
            Clip::Walk => "walk",
            Clip::Idle => "idle",
            Clip::Attack => "attack",
            Clip::Knockback => "knockback",
            Clip::Showcase => "showcase",
            Clip::Burrow => "burrow",
            Clip::Surface => "surface",
            Clip::Dig => "dig",
            Clip::Spirit => "spirit",
            Clip::Model => "model",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub(crate) enum Format {
    #[default]
    Avif,
    WebP,
    Gif,
}

impl Format {
    fn export(self) -> ExportFormat {
        match self {
            Format::Avif => ExportFormat::Avif,
            Format::WebP => ExportFormat::WebP,
            Format::Gif => ExportFormat::Gif,
        }
    }

    pub(crate) fn extension(self) -> &'static str {
        match self {
            Format::Avif => "avif",
            Format::WebP => "webp",
            Format::Gif => "gif",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Format::Avif => "AVIF",
            Format::WebP => "WebP",
            Format::Gif => "GIF",
        }
    }
}

pub(crate) struct Options {
    pub(crate) clip: Clip,
    pub(crate) format: Format,
    pub(crate) range: Option<(i32, i32)>,
    pub(crate) cull: Option<i32>,
    pub(crate) overlays: Overlays,
    pub(crate) looping: Option<Looping>,
    pub(crate) background: bool,
    pub(crate) quality: Option<u32>,
    pub(crate) compression: Option<u32>,
}

#[derive(Default)]
pub(crate) struct Progress {
    pub(crate) searched: AtomicU32,
    pub(crate) window: AtomicU32,
    pub(crate) drawn: AtomicU32,
    pub(crate) done: AtomicU32,
    pub(crate) total: AtomicU32,
}

struct Track {
    role: Option<Role>,
    anim: Option<Animation>,
    looping: Loop,
    length: i32,
    combat: Option<Combatant>,
}

pub(crate) struct Job {
    key: Key,
    format: Format,
    clip: Clip,
    name: String,
    prefix: String,
    rig: Rig,
    tracks: Vec<Track>,
    range: Option<(i32, i32)>,
    looping: Option<Looping>,
    overlays: Overlays,
    background: bool,
    quality: u32,
    compression: u32,
    tolerance: f32,
    show: AnimSettings,
    look: CombatLook,
}

pub(crate) enum Planned {
    Ready(Vec<u8>, String),
    Encode(Box<Job>),
}

#[derive(Clone, Copy)]
enum Span {
    Whole(i32),
    Range(i32, i32),
    Loop(i32, i32),
    Showcase(ShowcaseLengths),
}

impl Span {
    fn frames(self) -> i32 {
        match self {
            Span::Whole(length) => length,
            Span::Range(from, to) | Span::Loop(from, to) => from.abs_diff(to) as i32 + 1,
            Span::Showcase(lengths) => lengths.walk + lengths.idle + lengths.attack + lengths.kb,
        }
    }
}

fn chosen(set: &MotionSet, clip: Clip) -> Vec<&Motion> {
    match clip {
        Clip::Showcase => SHOWCASE_ROLES.iter().filter_map(|role| set.motions.iter().find(|motion| motion.role == Some(*role) && motion.file.is_some())).collect(),
        Clip::Model => set.motions.iter().find(|motion| motion.file.is_none()).into_iter().collect(),
        _ => set.motions.iter().find(|motion| motion.file.is_some() && motion.name.as_deref().is_some_and(|name| name.eq_ignore_ascii_case(clip.motion()))).into_iter().collect(),
    }
}

fn read(source: &Source) -> Result<Arc<[u8]>, String> {
    source.read().map_err(|err| format!("{} could not be read: {err}", source.path.display()))
}

fn too_many(frames: i32) -> Result<(), Trouble> {
    if frames > MAX_FRAMES {
        return Err(Trouble::TooMany { frames, limit: MAX_FRAMES });
    }

    Ok(())
}

pub(crate) fn plan(library: &Library, cache: &Cache, set: &MotionSet, options: &Options) -> Result<Option<Planned>, Trouble> {
    let motions = chosen(set, options.clip);
    let Some(first) = motions.first() else {
        return Ok(None);
    };
    let defaults = &library.settings.animation;
    let mut recipe = cache.recipe(STAMP);
    let mut tracks = Vec::with_capacity(motions.len());

    recipe.add(set.name.as_bytes());
    recipe.add(&serde_json::to_vec(defaults).map_err(|err| format!("the animation settings could not be read: {err}"))?);
    recipe.add(format!("{:?}", defaults.combat_look()).as_bytes());

    for motion in &motions {
        let anim = match motion.file.as_ref() {
            Some(file) => {
                let bytes = read(file)?;

                recipe.add(&bytes);
                Some(Animation::parse(&*bytes).map_err(|err| format!("{} could not be parsed: {err:?}", file.path.display()))?)
            }
            None => None,
        };
        let length = anim.as_ref().map_or(STILL, |anim| match (options.clip, motion.role) {
            (Clip::Showcase, Some(Role::Walk)) if playback_frames(anim) <= SHORT_LOOP => 0,
            (Clip::Showcase, Some(Role::Walk)) => defaults.default_showcase_walk,
            (Clip::Showcase, Some(Role::Idle)) if playback_frames(anim) <= SHORT_LOOP => 0,
            (Clip::Showcase, Some(Role::Idle)) => defaults.default_showcase_idle,
            (Clip::Showcase, Some(Role::Knockback)) => defaults.default_showcase_kb,
            _ => anim.declared_frames(),
        });

        recipe.add(format!("{:?}", (motion.role, motion.looping, &motion.combat, motion.slug())).as_bytes());
        tracks.push(Track { role: motion.role, anim, looping: motion.looping, length, combat: motion.combat.clone() });
    }

    let (sheet, cuts, model) = (read(&first.rig.png)?, read(&first.rig.cut)?, read(&first.rig.model)?);

    for part in [&sheet, &cuts, &model] {
        recipe.add(part);
    }

    let tolerance = if options.clip == Clip::Spirit { NO_CULL } else { options.cull.unwrap_or(defaults.bounds_cull) };
    let range = options.range.unwrap_or((i32::MIN, i32::MIN));
    let looping = options.looping.map_or([-1, -1, -1], |held| [held.minimum, held.maximum.unwrap_or(-1), held.difference]);
    let overlays = options.overlays;

    recipe.add(options.clip.slug().as_bytes());
    recipe.add(options.format.extension().as_bytes());
    recipe.add(&[u8::from(overlays.combat), u8::from(overlays.rig), u8::from(overlays.world), u8::from(overlays.origin), u8::from(options.background)]);

    let quality = options.quality.unwrap_or(QUALITY);
    let compression = options.compression.unwrap_or(COMPRESSION);

    for number in [quality as i32, compression as i32, FPS as i32, tolerance, range.0, range.1, looping[0], looping[1], looping[2], defaults.default_showcase_walk, defaults.default_showcase_idle, defaults.default_showcase_kb] {
        recipe.add(&number.to_le_bytes());
    }

    let key = recipe.finish();

    if cache.get(&key, TOO_BIG).is_some() {
        return Err(Trouble::TooBig);
    }

    if let Some(bytes) = cache.get(&key, options.format.extension())
        && let Some(name) = cache.get(&key, FILE_NAME).and_then(|name| String::from_utf8(name).ok())
    {
        return Ok(Some(Planned::Ready(bytes, name)));
    }

    let rig = Rig::parse(&*sheet, &*cuts, &*model).map_err(|err| format!("the rig could not be parsed: {err:?}"))?;
    let mut show = defaults.clone();

    show.show_rig = overlays.rig;
    show.show_world = overlays.world;
    show.show_origin = overlays.origin;
    show.show_combat = overlays.combat;

    let job = Job {
        key,
        format: options.format,
        clip: options.clip,
        name: set.name.clone(),
        prefix: name_prefix(&set.name, &first.slug()),
        rig,
        tracks,
        range: options.range,
        looping: options.looping,
        overlays,
        background: options.background,
        quality,
        compression,
        tolerance: tolerance as f32 / 100.0,
        look: defaults.combat_look(),
        show,
    };

    if job.looping.is_none() {
        too_many(job.planned_frames())?;
    }

    Ok(Some(Planned::Encode(Box::new(job))))
}

fn reach(marks: &[Mark]) -> Option<(f32, f32, f32)> {
    marks
        .iter()
        .map(|mark| match mark {
            Mark::Line { from, to, .. } => (from.0.min(to.0), from.0.max(to.0), from.1.max(to.1)),
            Mark::Box { corner, size, .. } => (corner.0.min(corner.0 + size.0), corner.0.max(corner.0 + size.0), corner.1.max(corner.1 + size.1)),
        })
        .reduce(|held, next| (held.0.min(next.0), held.1.max(next.1), held.2.max(next.2)))
}

impl Job {
    fn lengths(&self) -> ShowcaseLengths {
        let length = |role: Role| self.tracks.iter().find(|track| track.role == Some(role)).map_or(0, |track| track.length);

        ShowcaseLengths { walk: length(Role::Walk), idle: length(Role::Idle), attack: length(Role::Attack), kb: length(Role::Knockback) }
    }

    fn planned_frames(&self) -> i32 {
        match (self.clip, self.range) {
            (Clip::Showcase, _) => Span::Showcase(self.lengths()).frames(),
            (_, Some((from, to))) => Span::Range(from, to).frames(),
            _ => self.tracks.first().map_or(0, |track| track.length),
        }
    }

    fn span(&self, first: &Track, progress: &Progress) -> Result<Span, Trouble> {
        if let Some(looping) = self.looping {
            let Some(anim) = first.anim.as_ref() else {
                return Err(Trouble::NoLoop);
            };
            let deadline = Instant::now() + LOOP_LIMIT;
            let (from, to) = self.range.unwrap_or((0, i32::MAX));
            let to = looping.maximum.map_or(to, |maximum| to.min(maximum));
            let start = from.max(0);

            if to < i32::MAX {
                progress.window.store(u32::try_from(to - start + 1).unwrap_or(0), Ordering::Relaxed);
            }

            let searching = |frame: usize| {
                let searched = i64::try_from(frame).unwrap_or(i64::MAX) - i64::from(start) + 1;

                progress.searched.store(u32::try_from(searched.max(0)).unwrap_or(u32::MAX), Ordering::Relaxed);
                Instant::now() < deadline
            };
            let cycle = self.rig.calculate_cycle_between(anim, looping.difference as f32, Some(looping.minimum), from, to, searching).ok_or(Trouble::NoLoop)?;

            return Ok(Span::Loop(cycle.start, cycle.end));
        }

        Ok(match (self.clip, self.range) {
            (Clip::Showcase, _) => Span::Showcase(self.lengths()),
            (_, Some((from, to))) => Span::Range(from, to),
            _ => Span::Whole(first.length),
        })
    }

    fn track(&self, role: Role) -> Option<&Track> {
        self.tracks.iter().find(|track| track.role == Some(role))
    }

    fn fighter<'a>(&self, track: Option<&'a Track>) -> Option<&'a Combatant> {
        track.and_then(|track| track.combat.as_ref()).filter(|_| self.overlays.combat)
    }

    fn file_name(&self, span: Span) -> String {
        let (from, to) = match span {
            Span::Whole(length) => (0, length - 1),
            Span::Range(from, to) | Span::Loop(from, to) => (from, to),
            Span::Showcase(_) => (0, span.frames() - 1),
        };

        process::file_name(&self.prefix, matches!(span, Span::Showcase(_)), from, to, &self.format.export())
    }

    pub(crate) fn run(self, cache: &Cache, progress: &Progress, limit: u64) -> Result<(Vec<u8>, String), Trouble> {
        let started = Instant::now();
        let offset = Placement::default().offset(self.rig.model.alignment.len());
        let Some(first) = self.tracks.first() else {
            return Err(Trouble::Other("the animation has no frames".to_owned()));
        };
        let span = self.span(first, progress)?;
        let frames = span.frames();

        too_many(frames)?;

        if frames <= 0 {
            return Err(Trouble::Other("the animation has no frames".to_owned()));
        }

        progress.total.store(u32::try_from(frames).unwrap_or(0), Ordering::Relaxed);

        let abort = AtomicBool::new(false);
        let swept: Vec<(&Animation, Option<i32>)> = self
            .tracks
            .iter()
            .filter(|track| track.length > 0)
            .filter_map(|track| {
                let anim = track.anim.as_ref()?;
                let last = match span {
                    Span::Showcase(_) => track.length,
                    Span::Range(..) => playback_frames(anim) - 1,
                    Span::Loop(_, end) => end,
                    Span::Whole(length) => length - 1,
                };

                Some((anim, Some(last.max(0))))
            })
            .collect();
        let sprite = match find_bounds::search(&self.rig, &swept, self.tolerance, offset, &ProgressCounter::default(), &abort) {
            BoundsOutcome::Found(bounds) => bounds,
            BoundsOutcome::Empty => return Err(Trouble::NoBounds),
            BoundsOutcome::Aborted => return Err(Trouble::Other("the bounds search stopped".to_owned())),
        };
        let lengths = self.lengths();
        let (mode, frame_start, frame_end) = match span {
            Span::Showcase(_) => (ExportMode::Showcase, 0, frames - 1),
            Span::Loop(start, end) => (ExportMode::Loop, start, end),
            Span::Range(..) | Span::Whole(_) => (ExportMode::Manual, 0, frames - 1),
        };
        let timing = FrameTiming { mode, frame_start, frame_end, loop_supported: first.looping.native(first.anim.as_ref()) };
        let seat = |at: i32| -> (Option<&Track>, f32) {
            match span {
                Span::Showcase(_) => {
                    let (role, local) = lengths.segment(at);
                    let track = self.track(role);
                    let anim = track.and_then(|track| track.anim.as_ref());
                    let local = if anim.is_some_and(|anim| anim.declared_frames() > 1) { local } else { 0.0 };

                    (track, calculate_export_time(&timing, anim, local, at))
                }
                Span::Range(from, to) => {
                    let raw = (from + if to >= from { at } else { -at }) as f32;
                    let time = first.anim.as_ref().map_or(0.0, |anim| {
                        let inside = raw >= 0.0 && raw < playback_frames(anim) as f32;

                        if inside && first.looping.native(Some(anim)) { raw } else { loop_frame(anim, raw) }
                    });

                    (Some(first), time)
                }
                Span::Loop(..) | Span::Whole(_) => (Some(first), calculate_export_time(&timing, first.anim.as_ref(), 0.0, at)),
            }
        };
        let drawn = (sprite.min_x, sprite.max_x, sprite.max_y);
        let (left, right, bottom) = if self.overlays.combat {
            (0..frames)
                .filter_map(|at| {
                    let (track, time) = seat(at);
                    let combatant = self.fighter(track)?;
                    let stance = collision::Pose { rig: &self.rig, anim: track.and_then(|track| track.anim.as_ref()), frame: time.floor() as i32, offset, zoom: 1.0, crest: Some(sprite.min_y) };

                    reach(&collision::posed(&combatant.profile, self.look, &stance, |x, y| (x, y), |_, _| 0.0))
                })
                .fold(drawn, |held, (left, right, low)| (held.0.min(left - OVERLAY_MARGIN), held.1.max(right + OVERLAY_MARGIN), held.2.max(low + OVERLAY_MARGIN)))
        } else {
            drawn
        };
        let width = (right - left).round() as u32;
        let height = (bottom - sprite.min_y).round() as u32;

        if width == 0 || height == 0 {
            return Err(Trouble::Other("the animation has no visible area".to_owned()));
        }

        let scratch: PathBuf = cache.scratch(&self.key, self.format.extension());
        let config = ExportConfig {
            width,
            height,
            format: self.format.export(),
            quality_percent: self.quality,
            compression_percent: self.compression,
            fps: FPS,
            start_frame: 0,
            end_frame: frames - 1,
            output_path: scratch.clone(),
            base_name: self.name.clone(),
            background: self.background,
        };
        let view_proj = paint::export_view_proj(width as f32, height as f32, left, sprite.min_y, 1.0);
        let view = overlay::View { size: (width as f32, height as f32), pan: (-left - width as f32 / 2.0, -sprite.min_y - height as f32 / 2.0), zoom: 1.0 };
        let period = (!matches!(span, Span::Showcase(_))).then_some(frames);
        let backdrop = if self.background { BACKGROUND } else { CLEAR };
        let (sender, receiver) = mpsc::sync_channel(IN_FLIGHT);
        let (finished, watch) = mpsc::channel::<()>();
        let encoder_abort = AtomicBool::new(false);
        let oversized = AtomicBool::new(false);
        let deadline = JOB_LIMIT.saturating_sub(started.elapsed());

        thread::scope(|scope| {
            let stop = &encoder_abort;

            scope.spawn(move || {
                if watch.recv_timeout(deadline).is_err_and(|reason| reason == mpsc::RecvTimeoutError::Timeout) {
                    stop.store(true, Ordering::Relaxed);
                }
            });

            let encoder = scope.spawn(|| {
                leader::run(
                    config,
                    receiver,
                    |status| match status {
                        EncoderStatus::Progress(done) => {
                            progress.done.fetch_max(done, Ordering::Relaxed);
                        }
                        EncoderStatus::Size(bytes) if bytes > limit => {
                            oversized.store(true, Ordering::Relaxed);
                            encoder_abort.store(true, Ordering::Relaxed);
                        }
                        EncoderStatus::Size(_) | EncoderStatus::Finished => {}
                    },
                    &encoder_abort,
                )
            });

            for at in 0..frames {
                if encoder_abort.load(Ordering::Relaxed) {
                    break;
                }

                let (track, time) = seat(at);
                let anim = track.and_then(|track| track.anim.as_ref());
                let frame = time.floor() as i32;
                let mut parts = resolve_frame(&self.rig, anim, frame, offset);

                paint::flatten_glow(&mut parts);

                let pixels = if self.overlays.any() {
                    let mut shapes = Vec::new();

                    if let Some(combatant) = self.fighter(track) {
                        let stance = collision::Pose { rig: &self.rig, anim, frame, offset, zoom: 1.0, crest: Some(sprite.min_y) };
                        let glow = |_: usize, landed: i32| if combatant.attacking { collision::glow(self.look.fade, period, time, landed) } else { 0.0 };

                        shapes.extend(overlay::combat(collision::posed(&combatant.profile, self.look, &stance, |x, y| view.to_screen(x, y), glow)));
                    }

                    let pose = overlay::Pose { rig: &self.rig, anim, frame, offset, frames: &parts };

                    shapes.extend(overlay::shapes(&pose, Some(sprite), &view, &self.show));

                    let mut canvas = backdrop.repeat(width as usize * height as usize);

                    overlay::paint(&mut canvas, width, height, self.rig.sheet.image_data.as_deref(), &parts, &view, &shapes);
                    canvas.chunks_exact(width as usize * 4).rev().flatten().copied().collect()
                } else {
                    paint::render_parts(self.rig.sheet.image_data.as_deref(), &parts, &view_proj, backdrop, width, height)
                };

                if sender.send(EncoderMessage::Frame(pixels, width, height, 1000 / FPS)).is_err() {
                    warn!("The encoder closed before every frame was drawn");
                    break;
                }

                progress.drawn.fetch_add(1, Ordering::Relaxed);
            }

            if sender.send(EncoderMessage::Finish).is_err() {
                warn!("The encoder closed before the export could finish");
            }

            drop(sender);

            if encoder.join().is_err() {
                error!("The encoder thread panicked");
            }

            drop(finished);
        });

        if oversized.load(Ordering::Relaxed) {
            info!(set = self.name, clip = self.clip.slug(), format = self.format.extension(), frames, seconds = started.elapsed().as_secs(), "An animation passed the size limit and was stopped");

            if let Err(err) = cache.keep(&self.key, TOO_BIG, TOO_BIG.as_bytes()) {
                warn!(set = self.name, "The size refusal could not be cached: {}", err);
            }

            return Err(Trouble::TooBig);
        }

        if encoder_abort.load(Ordering::Relaxed) {
            warn!(set = self.name, clip = self.clip.slug(), frames, seconds = started.elapsed().as_secs(), "An animation hit the time limit");
            return Err(Trouble::TooLong);
        }

        if !scratch.exists() {
            return Err(Trouble::Other("the encoder did not produce a file".to_owned()));
        }

        info!(set = self.name, clip = self.clip.slug(), format = self.format.extension(), width, height, frames, ms = started.elapsed().as_millis(), "Animation exported");

        let name = self.file_name(span);

        if let Err(err) = cache.keep(&self.key, FILE_NAME, name.as_bytes()) {
            warn!(set = self.name, "The export's file name could not be cached: {}", err);
        }

        let bytes = cache.admit(&self.key, self.format.extension(), &scratch).map_err(|err| format!("the export could not be stored: {err}"))?;

        Ok((bytes, name))
    }
}
