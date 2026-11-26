use std::env;
use std::path::{Path, PathBuf};

use codex::graphics::rig::Animation;
use tracing::info;

use crate::systems::animation::{loop_frame, Loop};

use super::{ExportConfig, ExportFormat, ExportMode, ExportRequest, FrameTiming};

fn extension(format: &ExportFormat) -> &'static str {
    match format {
        ExportFormat::Gif => "gif",
        ExportFormat::WebP => "webp",
        ExportFormat::Avif => "avif",
        ExportFormat::Jxl => "jxl",
        ExportFormat::Png => "png",
        ExportFormat::Mp4 => "mp4",
        ExportFormat::Mkv => "mkv",
        ExportFormat::Webm => "webm",
        ExportFormat::Zip => "zip",
    }
}

fn stem(name_prefix: &str, is_showcase: bool, start_frame: i32, end_frame: i32) -> (String, String) {
    let range_part = if start_frame == end_frame {
        format!("{}f", start_frame)
    } else {
        format!("{}f~{}f", start_frame, end_frame)
    };

    let prefix_display = if is_showcase {
        name_prefix.split('.').next().map_or_else(|| "unit.showcase".to_string(), |first| format!("{}.showcase", first))
    } else {
        name_prefix.to_owned()
    };

    let base = if prefix_display.is_empty() { "animation".to_string() } else { prefix_display };
    let full = format!("{}.{}", base, range_part);

    (base, full)
}

pub fn file_name(name_prefix: &str, is_showcase: bool, start_frame: i32, end_frame: i32, format: &ExportFormat) -> String {
    format!("{}.{}", stem(name_prefix, is_showcase, start_frame, end_frame).1, extension(format))
}

pub fn build_config(request: &ExportRequest) -> ExportConfig {
    let is_showcase = request.timing.mode == ExportMode::Showcase;

    let (start_frame, end_frame) = if is_showcase {
        let total_frames = request.showcase.walk + request.showcase.idle + request.showcase.attack + request.showcase.kb;
        (0, if total_frames > 0 { total_frames - 1 } else { 0 })
    } else {
        (request.timing.frame_start, request.timing.frame_end)
    };

    let (base_name, file_name) = match &request.file_name {
        None => stem(&request.name_prefix, is_showcase, start_frame, end_frame),
        Some(name) => {
            let path_object = Path::new(name);
            let base = path_object.file_stem().unwrap_or(path_object.as_os_str()).to_string_lossy().to_string();
            (base, name.clone())
        }
    };

    let mut output_path = env::current_dir().unwrap_or_else(|_| PathBuf::from("../../../../.."));
    output_path.push("exports");

    let mut final_file_name = file_name;

    let suffix = format!(".{}", extension(&request.format));

    if !final_file_name.to_lowercase().ends_with(&suffix) {
        final_file_name = format!("{}{}", final_file_name, suffix);
    }

    output_path.push(&final_file_name);
    info!("Starting export process targeted to: {:?}", output_path);

    ExportConfig {
        width: request.width,
        height: request.height,
        format: request.format.clone(),
        quality_percent: request.quality_percent,
        compression_percent: request.compression_percent,
        fps: request.fps,
        start_frame,
        end_frame,
        output_path,
        base_name,
        background: request.background,
    }
}

pub fn calculate_export_time(
    timing: &FrameTiming,
    animation_option: Option<&Animation>,
    looping: Loop,
    current_time: f32,
    current_progress: i32,
) -> f32 {
    let Some(animation) = animation_option else {
        return 0.0;
    };

    let is_showcase = timing.mode == ExportMode::Showcase;

    let raw_frame = if is_showcase {
        current_time
    } else {
        let start = timing.frame_start;
        let step = if timing.frame_start < timing.frame_end { 1 } else { -1 };
        (start + (current_progress * step)) as f32
    };

    if !is_showcase || looping == Loop::Knockback {
        return looping.exported(animation, raw_frame);
    }

    if animation.modifications.iter().any(|modification| modification.loop_count == -1) {
        raw_frame
    } else {
        loop_frame(animation, raw_frame)
    }
}