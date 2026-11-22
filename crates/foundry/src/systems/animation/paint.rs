use std::ops::Range;

use codex::graphics::animate::FrameData;
use image::RgbaImage;

use crate::systems::raster::{self, Alpha, Batch, Texture, Vertex};

use super::multiply_mat3;

const NO_TINT: [f32; 4] = [0.0; 4];
const VERTS_PER_PART: usize = 4;

fn blend_variant(glow: u8) -> u8 {
    match glow {
        1 => 1,
        2 => 2,
        3 => 3,
        _ => 0,
    }
}

pub fn part_vertices(parts: &[(FrameData, [f32; 4])], view_proj: &[f32; 9], vertices: &mut Vec<Vertex>) -> Vec<(u8, Range<usize>)> {
    let mut batches: Vec<(u8, Range<usize>)> = Vec::new();

    for (part, tint) in parts {
        let mvp = multiply_mat3(view_proj, &part.final_matrix);
        let corner = |at: usize| {
            let x = part.vertices[2 * at];
            let y = part.vertices[2 * at + 1];

            Vertex {
                position: [mvp[0] * x + mvp[3] * y + mvp[6], mvp[1] * x + mvp[4] * y + mvp[7]],
                uv: [part.uvs[2 * at], part.uvs[2 * at + 1]],
                color: [part.opacity; 4],
                tint: *tint,
            }
        };
        let quad: [Vertex; VERTS_PER_PART] = std::array::from_fn(corner);
        let start = vertices.len();

        vertices.extend(FrameData::INDICES.iter().map(|index| quad[usize::from(*index)]));

        let variant = blend_variant(part.glow);

        match batches.last_mut() {
            Some((held, range)) if *held == variant => range.end = vertices.len(),
            _ => batches.push((variant, start..vertices.len())),
        }
    }

    batches
}

pub fn flatten_glow(parts: &mut [FrameData]) {
    for part in parts {
        part.glow = match part.glow {
            1 | 3 => 1,
            2 => 0,
            other => other,
        };
    }
}

pub fn export_view_proj(width: f32, height: f32, region_x: f32, region_y: f32, zoom: f32) -> [f32; 9] {
    let projection = [2.0 / width, 0.0, 0.0, 0.0, -2.0 / height, 0.0, -1.0, 1.0, 1.0];
    let pan_x = -region_x - width / (2.0 * zoom);
    let pan_y = -region_y - height / (2.0 * zoom);
    let view = [zoom, 0.0, 0.0, 0.0, zoom, 0.0, width / 2.0 + pan_x * zoom, height / 2.0 + pan_y * zoom, 1.0];

    multiply_mat3(&projection, &view)
}

pub fn render_parts(sheet: Option<&RgbaImage>, frames: &[FrameData], view_proj: &[f32; 9], background: [u8; 4], width: u32, height: u32) -> Vec<u8> {
    let mut pixels = background.repeat(width as usize * height as usize);

    if let Some(image) = sheet.filter(|_| !frames.is_empty()) {
        let parts: Vec<(FrameData, [f32; 4])> = frames.iter().map(|frame| (frame.clone(), NO_TINT)).collect();
        let mut vertices = Vec::with_capacity(parts.len() * 6);
        let part_batches = part_vertices(&parts, view_proj, &mut vertices);
        let texture = Some(Texture { width: image.width(), height: image.height(), pixels: image.as_raw() });
        let batches: Vec<Batch<'_>> = part_batches.into_iter().map(|(blend, range)| Batch { texture, blend, alpha: Alpha::Carried, range }).collect();

        raster::draw(&mut pixels, width, height, &vertices, &batches);
    }

    pixels.chunks_exact(width as usize * 4).rev().flatten().copied().collect()
}
