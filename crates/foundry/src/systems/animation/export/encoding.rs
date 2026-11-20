use std::fs;
use std::io::{BufWriter, Cursor, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

use image::RgbaImage;
use tracing::{error, info, warn};

use crate::systems::animation::alpha;

use super::frames::Intake;
use super::{apng, avif, gif, jxl, webp, EncoderMessage, EncoderStatus, ExportConfig, ExportFormat};

pub(crate) fn encode_native(
    config: ExportConfig,
    receiver: mpsc::Receiver<EncoderMessage>,
    emit: &(dyn Fn(EncoderStatus) + Sync),
    temp_path: &PathBuf,
    abort_signal: &AtomicBool
) -> bool {
    match config.format {
        ExportFormat::Gif => return gif::export(Intake { config: &config, receiver, emit, abort: abort_signal }, temp_path),
        ExportFormat::Png => return apng::export(Intake { config: &config, receiver, emit, abort: abort_signal }, temp_path),
        ExportFormat::Avif => return avif::export(Intake { config: &config, receiver, emit, abort: abort_signal }, temp_path),
        ExportFormat::Jxl => return jxl::export(Intake { config: &config, receiver, emit, abort: abort_signal }, temp_path),
        ExportFormat::WebP => return webp::export(Intake { config: &config, receiver, emit, abort: abort_signal }, temp_path),
        _ => {}
    }

    let mut frames_processed = 0;
    let mut is_success = false;

    match config.format {
        ExportFormat::Zip => {
            let mut frame_index = 0;
            let step_direction = if config.start_frame <= config.end_frame { 1 } else { -1 };

            let Ok(file) = fs::File::create(temp_path) else {
                error!("Failed to create temporary Zip file at {:?}", temp_path);
                return false;
            };

            let mut zip_writer = zip::ZipWriter::new(BufWriter::new(file));

            let compression_method = if config.compression_percent == 0 {
                zip::CompressionMethod::Stored
            } else {
                zip::CompressionMethod::Deflated
            };

            let zip_options = zip::write::SimpleFileOptions::default().compression_method(compression_method);

            while let Ok(message) = receiver.recv() {
                if abort_signal.load(Ordering::Relaxed) {
                    info!("Zip exporting aborted midway.");
                    return false;
                }

                match message {
                    EncoderMessage::Frame(raw_pixels, width, height, _) => {
                        let image_data = prepare_image(raw_pixels, width, height, config.background);
                        let current_frame = config.start_frame + (frame_index * step_direction);
                        let entry_name = format!("{}.{}f.png", config.base_name, current_frame);

                        if let Err(e) = zip_writer.start_file(entry_name, zip_options) {
                            error!("Failed to start Zip file entry: {}", e);
                            break;
                        }

                        let mut memory_buffer = Cursor::new(Vec::new());
                        if let Err(e) = image_data.write_to(&mut memory_buffer, image::ImageFormat::Png) {
                            error!("Failed compiling PNG frame to buffer: {}", e);
                            break;
                        }

                        if let Err(e) = zip_writer.write_all(memory_buffer.get_ref()) {
                            error!("Failed to write PNG buffer to Zip entry: {}", e);
                            break;
                        }

                        frame_index += 1;
                        frames_processed += 1;

                        emit(EncoderStatus::Progress(frames_processed));
                    },
                    EncoderMessage::Finish => {
                        is_success = true;
                        break;
                    },
                }
            }
            if let Err(e) = zip_writer.finish() {
                error!("Failed finalizing the Zip archive structure: {}", e);
                is_success = false;
            }
        },
        _ => {
            warn!("Native export method unsupported for requested format.");
        }
    }
    is_success
}

pub(crate) fn prepare_image(mut pixel_buffer: Vec<u8>, width: u32, height: u32, is_opaque_background: bool) -> RgbaImage {
    if is_opaque_background {
        for chunk in pixel_buffer.as_chunks_mut::<4>().0 {
            chunk[3] = 255;
        }
    } else {
        alpha::straighten(&mut pixel_buffer);
    }

    let image_buffer = match RgbaImage::from_raw(width, height, pixel_buffer) {
        Some(buffer) => buffer,
        None => {
            error!("Failed converting raw frame array back to structured RgbaImage. Recovering with empty image.");
            return RgbaImage::new(width, height);
        }
    };

    image::imageops::flip_vertical(&image_buffer)
}
