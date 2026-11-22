use std::ops::Range;
use std::sync::Arc;

use codex::graphics::animate::FrameData;
use foundry::systems::animation::paint::part_vertices;
use foundry::systems::raster::{self, Alpha, Batch};

use crate::host::ViewerScene;

use super::Region;

pub use foundry::systems::animation::overlay::{Pose, Shape, View, combat, paint, shape_vertices, shapes};

const DIM: [f32; 4] = [0.0, 0.0, 0.0, 125.0 / 255.0];
const SELECTION_COLOR: [f32; 4] = [1.0, 1.0, 0.0, 1.0];
const REGION_WIDTH: f32 = 1.0;
const NO_TINT: [f32; 4] = [0.0; 4];

pub fn clear(pixels: &mut [u8], color: [u8; 4]) {
    for pixel in pixels.as_chunks_mut::<4>().0 {
        *pixel = color;
    }
}

fn dim_outside(size: (f32, f32), min: (f32, f32), max: (f32, f32), shapes: &mut Vec<Shape>) {
    let left = min.0.clamp(0.0, size.0);
    let top = min.1.clamp(0.0, size.1);
    let right = max.0.clamp(0.0, size.0);
    let bottom = max.1.clamp(0.0, size.1);
    let band = |min: (f32, f32), max: (f32, f32)| Shape::Rect { min, max, fill: DIM, stroke: None };

    if top > 0.0 {
        shapes.push(band((0.0, 0.0), (size.0, top)));
    }

    if bottom < size.1 {
        shapes.push(band((0.0, bottom), (size.0, size.1)));
    }

    if left > 0.0 {
        shapes.push(band((0.0, top), (left, bottom)));
    }

    if right < size.0 {
        shapes.push(band((right, top), (size.0, bottom)));
    }
}

pub fn camera(view: &View, selecting: bool, region: Option<Region>) -> Vec<Shape> {
    let mut shapes = Vec::new();

    if selecting {
        return shapes;
    }

    if let Some(region) = region {
        let min = view.to_screen(region.x, region.y);
        let max = view.to_screen(region.x + region.w, region.y + region.h);

        dim_outside(view.size, min, max, &mut shapes);
        shapes.push(Shape::Rect { min, max, fill: [0.0; 4], stroke: Some((SELECTION_COLOR, REGION_WIDTH)) });
    }

    shapes
}

pub fn gpu_scene(size: (u32, u32), clear: [u8; 4], sheet: Option<(u32, u32, Arc<[u8]>)>, frames: &[FrameData], view: &View, shapes: &[Shape]) -> ViewerScene {
    let parts: Vec<(FrameData, [f32; 4])> = frames.iter().map(|frame| (frame.clone(), NO_TINT)).collect();
    let mut vertices = Vec::with_capacity(parts.len() * 6 + shapes.len() * 6);
    let part_batches = part_vertices(&parts, &view.view_proj(), &mut vertices);
    let overlay = shape_vertices(view, shapes, &mut vertices);
    let mut runs: Vec<(Option<usize>, u8, Range<usize>)> = Vec::with_capacity(part_batches.len() + 1);

    if sheet.is_some() {
        runs.extend(part_batches.into_iter().map(|(blend, range)| (Some(0), blend, range)));
    }

    if !overlay.is_empty() {
        runs.push((None, 0, overlay));
    }

    ViewerScene { vertices, runs, sheet, width: size.0, height: size.1, clear }
}

pub fn paint_shapes(pixels: &mut [u8], width: u32, height: u32, view: &View, shapes: &[Shape]) {
    let mut vertices = Vec::with_capacity(shapes.len() * 6);
    let overlay = shape_vertices(view, shapes, &mut vertices);

    if overlay.is_empty() {
        return;
    }

    raster::draw(pixels, width, height, &vertices, &[Batch { texture: None, blend: 0, alpha: Alpha::Blended, range: overlay }]);
}
