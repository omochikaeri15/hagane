use std::sync::Arc;
use std::time::Instant;

use codex::graphics::backdrop::{Backdrop, Camera, Screen};
use tracing::{info, warn};

use foundry::domains::utilities::background::{self, Catalog, DEFAULT_SEED, FRAME_RATE, Overrides, STAGE_WIDTH};
use foundry::systems::animation::export::{ExportConfig, process};

use super::cache::{Cache, Key};
use super::encode::{self, Ended, Feed};
use super::library::Library;
use super::prose::Trouble;
use super::render::{Format, MAX_FRAMES, Progress};

const STAMP: &str = env!("HAGANE_BACKDROP_STAMP");
const QUALITY: u32 = 80;
const COMPRESSION: u32 = 0;
const LOOP_END: i32 = 299;
const REPLAY_CEILING: i32 = 100_000;
const FRAME_DELAY: u32 = 1000 / FRAME_RATE;
const PREFIX: &str = "bg";
const MIN_SIDE: u32 = 2;
const MAX_SIDE: u32 = 4096;
const MAX_PIXELS: u32 = 4 * 1024 * 1024;
const MAX_OFFSET: i32 = 100_000;

pub(crate) struct Options {
    pub(crate) id: u32,
    pub(crate) seed: Option<u32>,
    pub(crate) range: Option<(i32, i32)>,
    pub(crate) offset: Option<(i32, i32)>,
    pub(crate) size: Option<(u32, u32)>,
    pub(crate) format: Format,
    pub(crate) quality: Option<u32>,
    pub(crate) compression: Option<u32>,
}

pub(crate) struct Job {
    key: Key,
    id: u32,
    format: Format,
    scene: Backdrop,
    seed: u32,
    first: i32,
    last: i32,
    quality: u32,
    compression: u32,
    camera: Camera,
}

pub(crate) enum Planned {
    Ready(Vec<u8>, String),
    Encode(Box<Job>),
}

pub(crate) fn ids(library: &Library) -> Catalog {
    Catalog::scan(&library.vault.vfs)
}

fn size(camera: Camera) -> (u32, u32) {
    let even = |length: f32| ((length.round().max(2.0) as u32) / 2 * 2).max(2);

    (even(camera.width), even(camera.height))
}

fn framed(options: &Options) -> Result<Camera, Trouble> {
    let usual = Camera::default();
    let (width, height) = options.size.map_or((usual.width, usual.height), |(width, height)| ((width / 2 * 2) as f32, (height / 2 * 2) as f32));
    let asked = options.size.unwrap_or((MIN_SIDE, MIN_SIDE));

    if asked.0 < MIN_SIDE || asked.1 < MIN_SIDE || asked.0 > MAX_SIDE || asked.1 > MAX_SIDE || asked.0.saturating_mul(asked.1) > MAX_PIXELS {
        return Err(Trouble::BadSize { side: MAX_SIDE, megapixels: MAX_PIXELS / (1024 * 1024) });
    }

    let (right, up) = options.offset.map_or((0, 0), |(right, up)| (right.clamp(-MAX_OFFSET, MAX_OFFSET), up.clamp(-MAX_OFFSET, MAX_OFFSET)));
    let center = [usual.x + right as f32, usual.height / 2.0 - up as f32];

    Ok(Camera::framing([center[0] - width / 2.0, center[1] - height / 2.0], [width, height]))
}

pub(crate) fn plan(library: &Library, cache: &Cache, options: &Options) -> Result<Planned, Trouble> {
    let vfs = &library.vault.vfs;

    if !ids(library).ids.contains(&options.id) {
        return Err(Trouble::NoBackground);
    }

    let loaded = background::load(vfs, options.id, &Overrides::default())?;
    let mut recipe = cache.recipe(STAMP);

    recipe.add(&options.id.to_le_bytes());

    for name in &loaded.files {
        let path = vfs.find(name.as_str()).ok_or_else(|| format!("{name} went missing while it was being read"))?;
        let bytes = vfs.source(&path).read().map_err(|err| format!("{name} could not be read: {err}"))?;

        recipe.add(name.as_bytes());
        recipe.add(&bytes);
    }

    let seed = options.seed.unwrap_or(DEFAULT_SEED);
    let scene = Backdrop::new(loaded.assets, Screen::default(), STAGE_WIDTH, seed);

    if !scene.animated() {
        if options.seed.is_some() {
            return Err(Trouble::Still(options.id, "--seed"));
        }

        if options.range.is_some() {
            return Err(Trouble::Still(options.id, "--range"));
        }
    }

    let (first, last) = if scene.animated() { options.range.unwrap_or((0, LOOP_END)) } else { (0, 0) };

    if first < 0 || last < first || last > REPLAY_CEILING {
        return Err(Trouble::BadFrames(REPLAY_CEILING));
    }

    let frames = last - first + 1;

    if frames > MAX_FRAMES {
        return Err(Trouble::TooMany { frames, limit: MAX_FRAMES });
    }

    let camera = framed(options)?;
    let (width, height) = size(camera);
    let quality = options.quality.unwrap_or(QUALITY);
    let compression = options.compression.unwrap_or(COMPRESSION);

    recipe.add(options.format.extension().as_bytes());

    for number in [seed, first as u32, last as u32, quality, compression, FRAME_RATE, STAGE_WIDTH as u32, width, height] {
        recipe.add(&number.to_le_bytes());
    }

    for number in [camera.x, camera.y, camera.zoom] {
        recipe.add(&number.to_le_bytes());
    }

    let key = recipe.finish();

    if cache.get(&key, encode::TOO_BIG).is_some() {
        return Err(Trouble::TooBig);
    }

    if let Some(bytes) = cache.get(&key, options.format.extension())
        && let Some(name) = cache.get(&key, encode::FILE_NAME).and_then(|name| String::from_utf8(name).ok())
    {
        return Ok(Planned::Ready(bytes, name));
    }

    Ok(Planned::Encode(Box::new(Job { key, id: options.id, format: options.format, scene, seed, first, last, quality, compression, camera })))
}

impl Job {
    pub(crate) fn run(mut self, cache: &Cache, progress: &Progress, limit: u64) -> Result<(Vec<u8>, String), Trouble> {
        let started = Instant::now();
        let (width, height) = size(self.camera);
        let frames = self.last - self.first + 1;
        let stem = format!("{PREFIX}{}", background::export_stem(self.id));
        let name = process::file_name(&stem, false, self.first, self.last, &encode::container(self.format));
        let scratch = cache.scratch(&self.key, self.format.extension());
        let config = ExportConfig {
            width,
            height,
            format: encode::container(self.format),
            quality_percent: self.quality,
            compression_percent: self.compression,
            fps: FRAME_RATE,
            start_frame: self.first,
            end_frame: self.last,
            output_path: scratch.clone(),
            base_name: stem,
            background: false,
        };
        let camera = Camera { width: width as f32, height: height as f32, ..self.camera };

        self.scene.restart(self.seed);

        for _ in 0..self.first {
            self.scene.tick();
        }

        let scene = &mut self.scene;
        let feed = Feed { config, frames, delay: FRAME_DELAY, limit, time: encode::JOB_LIMIT.saturating_sub(started.elapsed()), progress };
        let ended = encode::run(feed, |at| {
            if at > 0 {
                scene.tick();
            }

            let quads = scene.render(camera);
            let mut pixels = vec![0; width as usize * height as usize * 4];

            background::paint(&mut pixels, width, height, &quads, |surface| scene.image(surface).map(Arc::clone));
            pixels.chunks_exact(width as usize * 4).rev().flatten().copied().collect()
        });

        match ended {
            Ended::Oversized => {
                info!(id = self.id, format = self.format.extension(), frames, seconds = started.elapsed().as_secs(), "A background passed the size limit and was stopped");

                if let Err(err) = cache.keep(&self.key, encode::TOO_BIG, encode::TOO_BIG.as_bytes()) {
                    warn!(id = self.id, "The size refusal could not be cached: {}", err);
                }

                return Err(Trouble::TooBig);
            }
            Ended::Late => {
                warn!(id = self.id, frames, seconds = started.elapsed().as_secs(), "A background hit the time limit");
                return Err(Trouble::TooLong);
            }
            Ended::Finished => {}
        }

        if !scratch.exists() {
            return Err(Trouble::Other("the encoder did not produce a file".to_owned()));
        }

        info!(id = self.id, format = self.format.extension(), width, height, frames, ms = started.elapsed().as_millis(), "Background exported");

        if let Err(err) = cache.keep(&self.key, encode::FILE_NAME, name.as_bytes()) {
            warn!(id = self.id, "The export's file name could not be cached: {}", err);
        }

        let bytes = cache.admit(&self.key, self.format.extension(), &scratch).map_err(|err| format!("the export could not be stored: {err}"))?;

        Ok((bytes, name))
    }
}
