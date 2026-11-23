use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use tracing::{error, warn};

use foundry::systems::animation::export::{EncoderMessage, EncoderStatus, ExportConfig, ExportFormat, leader};

use super::render::{Format, Progress};

pub(crate) const JOB_LIMIT: Duration = Duration::from_secs(180);
pub(crate) const FILE_NAME: &str = "name";
pub(crate) const TOO_BIG: &str = "big";
const IN_FLIGHT: usize = 2;

pub(crate) enum Ended {
    Finished,
    Oversized,
    Late,
}

pub(crate) struct Feed<'a> {
    pub(crate) config: ExportConfig,
    pub(crate) frames: i32,
    pub(crate) delay: u32,
    pub(crate) limit: u64,
    pub(crate) time: Duration,
    pub(crate) progress: &'a Progress,
}

pub(crate) fn container(format: Format) -> ExportFormat {
    match format {
        Format::Avif => ExportFormat::Avif,
        Format::WebP => ExportFormat::WebP,
        Format::Gif => ExportFormat::Gif,
    }
}

pub(crate) fn run(feed: Feed<'_>, mut draw: impl FnMut(i32) -> Vec<u8>) -> Ended {
    let Feed { config, frames, delay, limit, time, progress } = feed;
    let (width, height) = (config.width, config.height);
    let (sender, receiver) = mpsc::sync_channel(IN_FLIGHT);
    let (finished, watch) = mpsc::channel::<()>();
    let abort = AtomicBool::new(false);
    let oversized = AtomicBool::new(false);

    progress.total.store(u32::try_from(frames).unwrap_or(0), Ordering::Relaxed);

    thread::scope(|scope| {
        let stop = &abort;

        scope.spawn(move || {
            if watch.recv_timeout(time).is_err_and(|reason| reason == mpsc::RecvTimeoutError::Timeout) {
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
                        abort.store(true, Ordering::Relaxed);
                    }
                    EncoderStatus::Size(_) | EncoderStatus::Finished => {}
                },
                &abort,
            )
        });

        for at in 0..frames {
            if abort.load(Ordering::Relaxed) {
                break;
            }

            if sender.send(EncoderMessage::Frame(draw(at), width, height, delay)).is_err() {
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
        return Ended::Oversized;
    }

    if abort.load(Ordering::Relaxed) {
        return Ended::Late;
    }

    Ended::Finished
}
