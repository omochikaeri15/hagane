use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use tracing::{debug, error, info, warn};

use crate::common::process;
use crate::systems::animation::export::{EncoderMessage, EncoderStatus, ExportConfig};
use crate::systems::animation::export::encoding::prepare_image;

use super::apng::Apng;
use super::get_cjxl_path;

const POLL_INTERVAL: Duration = Duration::from_millis(50);
const SLOWEST_EFFORT: u32 = 9;

pub(crate) fn encode(
    config: ExportConfig,
    receiver: mpsc::Receiver<EncoderMessage>,
    emit: &(dyn Fn(EncoderStatus) + Sync),
    temp_path: &Path,
    abort_signal: &AtomicBool
) -> bool {
    let Some(cjxl_path) = get_cjxl_path() else {
        error!("cjxl binary not found. Aborting encode.");
        return false;
    };

    let source_path = temp_path.with_extension("apng");
    let is_success = write_source(&config, receiver, emit, &source_path, abort_signal)
        && run(&cjxl_path, &config, &source_path, temp_path, abort_signal);

    if source_path.exists()
        && let Err(err) = fs::remove_file(&source_path) {
        error!("Failed to remove the cjxl source APNG: {}", err);
    }

    is_success
}

fn write_source(
    config: &ExportConfig,
    receiver: mpsc::Receiver<EncoderMessage>,
    emit: &(dyn Fn(EncoderStatus) + Sync),
    path: &Path,
    abort_signal: &AtomicBool
) -> bool {
    let mut source: Option<Apng> = None;
    let mut frames_processed = 0;

    debug!("Writing the cjxl source APNG to {:?}", path);
    while let Ok(message) = receiver.recv() {
        if abort_signal.load(Ordering::Relaxed) {
            warn!("Abort signal received. Halting the cjxl source stream.");
            return false;
        }

        match message {
            EncoderMessage::Frame(raw_pixels, width, height, delay_ms) => {
                if source.is_none() {
                    let Ok(created) = Apng::create(path, width, height)
                        .inspect_err(|err| error!("Failed to create the cjxl source APNG: {}", err)) else {
                        return false;
                    };
                    source = Some(created);
                }

                let Some(writer) = source.as_mut() else {
                    return false;
                };

                emit(EncoderStatus::Progress(frames_processed));

                let pixels = prepare_image(raw_pixels, width, height, config.background).into_raw();
                if let Err(err) = writer.push(pixels, delay_ms) {
                    error!("Failed to write frame {} into the cjxl source APNG: {}", frames_processed, err);
                    return false;
                }
                frames_processed += 1;
            },
            EncoderMessage::Finish => {
                debug!("Received Finish signal. Total frames written for cjxl: {}", frames_processed);
                let Some(writer) = source else {
                    error!("No frames reached the cjxl encoder.");
                    return false;
                };

                return writer.finish()
                    .inspect_err(|err| error!("Failed to finish the cjxl source APNG: {}", err))
                    .is_ok();
            }
        }
    }

    error!("Frame stream closed before the cjxl source APNG was finished.");
    false
}

fn run(cjxl_path: &Path, config: &ExportConfig, source: &Path, output: &Path, abort_signal: &AtomicBool) -> bool {
    let quality = config.quality_percent.min(100);
    let effort = 1 + (config.compression_percent.min(100) * (SLOWEST_EFFORT - 1) + 50) / 100;
    info!("Encoding JXL through cjxl at quality {} and effort {}", quality, effort);

    let Ok(mut child) = process::command(cjxl_path)
        .arg(source)
        .arg(output)
        .args(["-q", &quality.to_string(), "-e", &effort.to_string(), "--quiet"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .inspect_err(|err| error!("Failed to spawn cjxl: {}", err)) else {
        return false;
    };

    loop {
        if abort_signal.load(Ordering::Relaxed) {
            warn!("Abort signal received during cjxl execution.");
            stop(&mut child);
            return false;
        }

        match child.try_wait() {
            Ok(Some(status)) if status.success() => {
                info!("cjxl encode completed successfully.");
                return true;
            },
            Ok(Some(status)) => {
                let mut message = String::new();
                if let Some(mut stderr) = child.stderr.take() {
                    let _ = stderr.read_to_string(&mut message);
                }
                error!("cjxl exited with {}: {}", status, message.trim());
                return false;
            },
            Ok(None) => thread::sleep(POLL_INTERVAL),
            Err(err) => {
                error!("Failed to poll cjxl: {}", err);
                stop(&mut child);
                return false;
            }
        }
    }
}

fn stop(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}
