use std::num::{NonZeroU8, NonZeroU32};

use codex::files::Caf;
use codex::files::sound::{self, EFFECT, MUSIC};
use tracing::warn;
use vorbis_enc::{BitrateManagementStrategy, EncoderBuilder};

use super::cache::Cache;
use super::library::Library;
use super::prose::Trouble;

const STAMP: &str = env!("HAGANE_AUDIO_STAMP");
const QUALITY: f32 = 0.5;
const FULL_SCALE: f32 = 32768.0;

pub(crate) struct Track {
    pub(crate) name: String,
    pub(crate) bytes: Vec<u8>,
    pub(crate) cached: bool,
}

fn held(library: &Library, id: u32, extension: &str) -> Result<Option<Vec<u8>>, Trouble> {
    let vfs = &library.vault.vfs;

    for name in sound::names(id, extension) {
        if let Some(path) = vfs.find(name.as_str()) {
            return vfs.source(&path).read().map(|bytes| Some(bytes.to_vec())).map_err(|err| format!("{name} could not be read: {err}").into());
        }
    }

    Ok(None)
}

pub(crate) fn encoded(effect: &[u8]) -> Result<Vec<u8>, String> {
    let caf = Caf::parse(effect).map_err(|err| err.to_string())?;
    let samples = caf.samples().ok_or("its audio is not 16-bit PCM")?;
    let channels = u8::try_from(caf.channels_per_frame).ok().and_then(NonZeroU8::new).ok_or("it has no channels")?;
    let rate = NonZeroU32::new(caf.sample_rate as u32).ok_or("it has no sample rate")?;
    let planes: Vec<Vec<f32>> = (0..usize::from(channels.get())).map(|channel| samples.iter().skip(channel).step_by(usize::from(channels.get())).map(|sample| f32::from(*sample) / FULL_SCALE).collect()).collect();
    let mut encoder = EncoderBuilder::new(rate, channels, Vec::new())
        .bitrate_management_strategy(BitrateManagementStrategy::QualityVbr { target_quality: QUALITY })
        .build()
        .map_err(|err| err.to_string())?;

    encoder.encode_audio_block(&planes).map_err(|err| err.to_string())?;
    encoder.finish().map_err(|err| err.to_string())
}

pub(crate) fn track(library: &Library, cache: &Cache, id: u32) -> Result<Option<Track>, Trouble> {
    let name = format!("{id:03}.{MUSIC}");

    if let Some(bytes) = held(library, id, MUSIC)? {
        return Ok(Some(Track { name, bytes, cached: true }));
    }

    let Some(effect) = held(library, id, EFFECT)? else {
        return Ok(None);
    };
    let mut recipe = cache.recipe(STAMP);

    recipe.add(&effect);

    let key = recipe.finish();

    if let Some(bytes) = cache.get(&key, MUSIC) {
        return Ok(Some(Track { name, bytes, cached: true }));
    }

    let bytes = encoded(&effect).map_err(|reason| format!("{id:03}.{EFFECT} could not be converted: {reason}"))?;

    if let Err(err) = cache.keep(&key, MUSIC, &bytes) {
        warn!(id, "A converted sound could not be cached: {}", err);
    }

    Ok(Some(Track { name, bytes, cached: false }))
}
