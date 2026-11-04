use std::any::Any;
use std::cell::{Cell, RefCell};
use std::io::Cursor;
use std::num::NonZero;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use rodio::buffer::SamplesBuffer;
#[cfg(target_os = "android")]
use rodio::cpal::BufferSize;
use rodio::{Decoder, DeviceSinkBuilder, DeviceSinkError, MixerDeviceSink, Player, Source};
use tracing::{debug, info, warn};

use emulator::engine::{AudioHost, AudioSource, SoundData};

use super::assets::{FileIndex, SharedLedger};

const BUFFER_VOICES: i32 = 24;
const STREAM_VOICES: i32 = 2;
const BUFFER_PRIORITY: i32 = 0;
const STREAM_PRIORITY: i32 = -1;
const FILES_DIR: &[u8] = b"files";
const QUIETEST: f32 = 0.001;
const CAF_HEADER: usize = 8;
const CHUNK_HEADER: usize = 12;
const DESCRIPTION: &[u8; 4] = b"desc";
const SAMPLES: &[u8; 4] = b"data";
const LITTLE_ENDIAN: u32 = 2;
const EDIT_COUNT: usize = 4;
#[cfg(target_os = "android")]
const STEREO: NonZero<u16> = NonZero::<u16>::MIN.saturating_add(1);
#[cfg(target_os = "android")]
const CLIP_RATE: NonZero<u32> = NonZero::<u32>::MIN.saturating_add(44_099);
#[cfg(target_os = "android")]
const BUFFERS_PER_SECOND: u32 = 20;

#[derive(Clone)]
struct Effect {
    channels: u16,
    rate: u32,
    samples: Arc<[f32]>,
}

struct EffectData {
    name: String,
    effect: Effect,
}

impl SoundData for EffectData {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Clone)]
enum Clip {
    Effect { name: String, effect: Effect },
    Track { name: String, bytes: Arc<[u8]> },
}

trait Playing {
    fn set_level(&self, level: f32);
    fn stop(&self);
    fn busy(&self) -> bool;
    fn hold(&self, held: bool);
}

#[cfg(target_os = "android")]
fn open_output() -> Result<MixerDeviceSink, DeviceSinkError> {
    DeviceSinkBuilder::from_default_device()?
        .with_sample_rate(CLIP_RATE)
        .with_channels(STEREO)
        .with_buffer_size(BufferSize::Fixed(CLIP_RATE.get() / BUFFERS_PER_SECOND))
        .open_sink_or_fallback()
        .or_else(|error| {
            warn!("emu: the audio output refused {CLIP_RATE} Hz: {error}");

            DeviceSinkBuilder::open_default_sink()
        })
}

#[cfg(not(target_os = "android"))]
fn open_output() -> Result<MixerDeviceSink, DeviceSinkError> {
    DeviceSinkBuilder::open_default_sink()
}

pub(super) struct Speaker {
    output: Option<MixerDeviceSink>,
    voices: RefCell<Vec<Weak<Voice>>>,
}

impl Speaker {
    pub(super) fn open() -> Rc<Self> {
        let output = open_output().inspect_err(|error| warn!("emu: no audio output: {error}")).ok();

        if let Some(output) = &output {
            info!(config = ?output.config(), "emu: audio output opened");
        }

        Rc::new(Self { output, voices: RefCell::new(Vec::new()) })
    }

    pub(super) fn hold(&self, held: bool) {
        for voice in self.voices.borrow().iter().filter_map(Weak::upgrade) {
            if let Some(playing) = voice.playing.borrow().as_ref() {
                playing.hold(held);
            }
        }
    }

    pub(super) fn hush(&self) {
        for voice in self.voices.borrow().iter().filter_map(Weak::upgrade) {
            voice.stop();
        }
    }

    fn start(&self, clip: &Clip, looped: bool, level: f32) -> Option<Box<dyn Playing>> {
        let mixer = self.output.as_ref()?.mixer();
        let player = Player::connect_new(mixer);

        player.set_volume(level);

        match clip {
            Clip::Effect { name, effect } => {
                let (Some(channels), Some(rate)) = (NonZero::new(effect.channels), NonZero::new(effect.rate)) else {
                    warn!("emu: {name} has no channels or no sample rate");

                    return None;
                };
                let source = SamplesBuffer::new(channels, rate, effect.samples.to_vec());

                if looped {
                    player.append(source.repeat_infinite());
                } else {
                    player.append(source);
                }
            }
            Clip::Track { name, bytes } => {
                let cursor = Cursor::new(Arc::clone(bytes));
                let decoded = if looped { Decoder::new_looped(cursor).map(|source| player.append(source)) } else { Decoder::new(cursor).map(|source| player.append(source)) };

                if let Err(error) = decoded {
                    warn!("emu: {name} could not be decoded: {error}");

                    return None;
                }
            }
        }

        Some(Box::new(player))
    }
}

impl Playing for Player {
    fn set_level(&self, level: f32) {
        self.set_volume(level);
    }

    fn stop(&self) {
        Player::stop(self);
    }

    fn busy(&self) -> bool {
        !self.empty()
    }

    fn hold(&self, held: bool) {
        if held {
            self.pause();
        } else {
            self.play();
        }
    }
}

struct Voice {
    clip: Clip,
    stream: bool,
    looping: Cell<bool>,
    level: Cell<f32>,
    playing: RefCell<Option<Box<dyn Playing>>>,
    speaker: Rc<Speaker>,
    claims: Rc<Cell<i32>>,
}

impl AudioSource for Voice {
    fn is_playing(&self) -> bool {
        self.playing.borrow().as_ref().is_some_and(|playing| playing.busy())
    }

    fn is_stopped(&self) -> bool {
        !self.is_playing()
    }

    fn play(&self) {
        if self.stream && self.is_playing() {
            return;
        }

        self.stop();
        *self.playing.borrow_mut() = self.speaker.start(&self.clip, self.looping.get(), self.level.get());
    }

    fn set_loop(&self, looping: bool) {
        self.looping.set(looping);
    }

    fn set_volume(&self, volume: f32) {
        let level = if volume > QUIETEST { volume.min(1.0) } else { 0.0 };

        self.level.set(level);

        if let Some(playing) = self.playing.borrow().as_ref() {
            playing.set_level(level);
        }
    }

    fn stop(&self) {
        if let Some(playing) = self.playing.borrow_mut().take() {
            playing.stop();
        }
    }

    fn priority(&self) -> i32 {
        if self.stream { STREAM_PRIORITY } else { BUFFER_PRIORITY }
    }
}

impl Drop for Voice {
    fn drop(&mut self) {
        self.stop();
        self.claims.set(self.claims.get() - 1);
    }
}

pub(super) struct Pool {
    files: Rc<RefCell<FileIndex>>,
    ledger: SharedLedger,
    speaker: Rc<Speaker>,
    buffers: Rc<Cell<i32>>,
    streams: Rc<Cell<i32>>,
}

impl Pool {
    pub(super) fn new(files: Rc<RefCell<FileIndex>>, ledger: SharedLedger, speaker: Rc<Speaker>) -> Self {
        Self { files, ledger, speaker, buffers: Rc::new(Cell::new(0)), streams: Rc::new(Cell::new(0)) }
    }

    fn name(path: &[u8]) -> String {
        let base = path.rsplit(|byte| *byte == b'/').next().unwrap_or(path);

        String::from_utf8_lossy(base).into_owned()
    }

    fn read(&self, path: &[u8]) -> Option<(String, Vec<u8>)> {
        let name = Self::name(path);
        let source = self.files.borrow().get(name.as_str()).cloned()?;

        if !source.in_memory() {
            self.ledger.borrow_mut().note(&name, &source.path);
        }

        let bytes = source.read().map(|bytes| bytes.to_vec()).inspect_err(|error| warn!("emu: {name} could not be read: {error}")).ok()?;

        Some((name, bytes))
    }

    fn claim(&mut self, clip: Clip, stream: bool) -> Option<Rc<dyn AudioSource>> {
        let (claims, limit) = if stream { (&self.streams, STREAM_VOICES) } else { (&self.buffers, BUFFER_VOICES) };

        if claims.get() >= limit {
            return None;
        }

        claims.set(claims.get() + 1);

        let voice = Rc::new(Voice {
            clip,
            stream,
            looping: Cell::new(false),
            level: Cell::new(1.0),
            playing: RefCell::new(None),
            speaker: Rc::clone(&self.speaker),
            claims: Rc::clone(claims),
        });

        let mut voices = self.speaker.voices.borrow_mut();

        voices.retain(|held| held.strong_count() > 0);
        voices.push(Rc::downgrade(&voice));
        drop(voices);

        Some(voice)
    }
}

impl AudioHost for Pool {
    fn resume_all(&mut self) {
        self.speaker.hold(false);
    }

    fn active_sources(&mut self) -> i32 {
        self.speaker.voices.borrow().iter().filter_map(Weak::upgrade).filter(|voice| voice.is_playing()).count() as i32
    }

    fn source_capacity(&mut self) -> i32 {
        BUFFER_VOICES + STREAM_VOICES
    }

    fn new_buffer_source(&mut self, data: &Rc<dyn SoundData>) -> Option<Rc<dyn AudioSource>> {
        let data = data.as_any().downcast_ref::<EffectData>()?;
        let clip = Clip::Effect { name: data.name.clone(), effect: data.effect.clone() };

        self.claim(clip, false)
    }

    fn new_stream_source(&mut self, path: &[u8], _source_type: i32) -> Option<Rc<dyn AudioSource>> {
        let (name, bytes) = self.read(path)?;

        self.claim(Clip::Track { name, bytes: Arc::from(bytes) }, true)
    }

    fn file_exists(&mut self, path: &[u8]) -> bool {
        self.files.borrow().contains_key(Self::name(path).as_str())
    }

    fn files_dir(&mut self) -> Vec<u8> {
        FILES_DIR.to_vec()
    }

    fn remove_file(&mut self, path: &[u8]) -> bool {
        debug!("emu: the game asked to delete {}, which is left in place", String::from_utf8_lossy(path));

        false
    }

    fn load_sound_data(&mut self, path: &[u8]) -> Option<Rc<dyn SoundData>> {
        let (name, bytes) = self.read(path)?;
        let Some(effect) = parse_caf(&bytes) else {
            warn!("emu: {name} is not a readable caf");

            return None;
        };

        Some(Rc::new(EffectData { name, effect }))
    }
}

fn parse_caf(bytes: &[u8]) -> Option<Effect> {
    let mut at = CAF_HEADER;
    let mut format: Option<(u32, u32, u32)> = None;

    while let Some(header) = bytes.get(at..at + CHUNK_HEADER) {
        let kind = header.get(..4)?;
        let size = i64::from_be_bytes(header.get(4..)?.try_into().ok()?);
        let body = at + CHUNK_HEADER;

        if kind == DESCRIPTION {
            let rate = f64::from_be_bytes(bytes.get(body..body + 8)?.try_into().ok()?);
            let flags = u32::from_be_bytes(bytes.get(body + 12..body + 16)?.try_into().ok()?);
            let channels = u32::from_be_bytes(bytes.get(body + 24..body + 28)?.try_into().ok()?);

            format = Some((rate as u32, flags, channels));
        }

        if kind == SAMPLES {
            let (rate, flags, channels) = format?;
            let end = if size < 0 { bytes.len() } else { (body + size as usize).min(bytes.len()) };
            let pcm = bytes.get(body + EDIT_COUNT..end)?;
            let samples: Vec<f32> = pcm
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&word| {
                    let sample = if flags & LITTLE_ENDIAN != 0 { i16::from_le_bytes(word) } else { i16::from_be_bytes(word) };

                    f32::from(sample) / f32::from(i16::MAX)
                })
                .collect();

            return Some(Effect { channels: channels as u16, rate, samples: Arc::from(samples) });
        }

        at = body.checked_add(usize::try_from(size).ok()?)?;
    }

    None
}
