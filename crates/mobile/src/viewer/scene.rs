use std::ops::Range;

use image::RgbaImage;

use codex::graphics::animate::FrameData;
use codex::graphics::rig::{Animation, BoundingBox, Rig};
use codex::graphics::tools::{joint, part};
use foundry::domains::settings::AnimSettings;
use foundry::systems::animation::multiply_mat3;
use foundry::systems::raster::{self, Alpha, Batch, Texture, Vertex};

use super::Region;

const WORLD_COLOR: [f32; 4] = [0.0, 1.0, 0.0, 0.5];
const ORIGIN_MARK_COLOR: [f32; 4] = [0.0, 1.0, 0.0, 1.0];
const WORLD_WIDTH: f32 = 1.5;
const ORIGIN_MARK_RADIUS: f32 = 5.0;
const PART_COLOR: [f32; 4] = [1.0, 0.35, 0.35, 0.32];
const PART_ORIGIN_COLOR: [f32; 4] = [0.3, 0.9, 1.0, 0.82];
const PART_WIDTH: f32 = 1.5;
const PART_ORIGIN_RADIUS: f32 = 2.4;
const DIM: [f32; 4] = [0.0, 0.0, 0.0, 125.0 / 255.0];
const SELECTION_COLOR: [f32; 4] = [1.0, 1.0, 0.0, 1.0];
const REGION_WIDTH: f32 = 1.0;
const CIRCLE_SEGMENTS: usize = 16;
const NO_TINT: [f32; 4] = [0.0; 4];
const VERTS_PER_PART: usize = 4;

type Quad = [(f32, f32); 4];

#[derive(Clone, Copy, Debug)]
pub enum Shape {
    Line { from: (f32, f32), to: (f32, f32), color: [f32; 4], width: f32 },
    Circle { center: (f32, f32), radius: f32, color: [f32; 4] },
    Rect { min: (f32, f32), max: (f32, f32), fill: [f32; 4], stroke: Option<([f32; 4], f32)> },
}

#[derive(Clone, Copy)]
pub struct View {
    pub size: (f32, f32),
    pub pan: (f32, f32),
    pub zoom: f32,
}

pub struct Pose<'a> {
    pub rig: &'a Rig,
    pub anim: Option<&'a Animation>,
    pub frame: i32,
    pub offset: Option<usize>,
    pub frames: &'a [FrameData],
}

impl View {
    pub fn to_screen(self, x: f32, y: f32) -> (f32, f32) {
        (self.size.0 / 2.0 + (x + self.pan.0) * self.zoom, self.size.1 / 2.0 + (y + self.pan.1) * self.zoom)
    }

    fn view_proj(&self) -> [f32; 9] {
        let (width, height) = self.size;
        let projection = [2.0 / width, 0.0, 0.0, 0.0, -2.0 / height, 0.0, -1.0, 1.0, 1.0];
        let camera = [self.zoom, 0.0, 0.0, 0.0, self.zoom, 0.0, width / 2.0 + self.pan.0 * self.zoom, height / 2.0 + self.pan.1 * self.zoom, 1.0];

        multiply_mat3(&projection, &camera)
    }
}

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

pub fn shapes(pose: &Pose<'_>, reach: Option<BoundingBox>, view: &View, show: &AnimSettings) -> Vec<Shape> {
    let mut shapes = Vec::new();
    let ground = view.to_screen(0.0, 0.0);

    if let Some(reach) = show.show_world.then_some(reach).flatten() {
        let left = view.to_screen(reach.min_x, 0.0).0;
        let right = view.to_screen(reach.max_x, 0.0).0;
        let top = view.to_screen(0.0, reach.min_y).1;

        shapes.push(Shape::Line { from: (left, ground.1), to: (right, ground.1), color: WORLD_COLOR, width: WORLD_WIDTH });

        if top < ground.1 {
            shapes.push(Shape::Line { from: ground, to: (ground.0, top), color: WORLD_COLOR, width: WORLD_WIDTH });
        }
    }

    if show.show_origin {
        shapes.push(Shape::Circle { center: ground, radius: ORIGIN_MARK_RADIUS, color: ORIGIN_MARK_COLOR });
    }

    if show.show_rig {
        outlines(pose, view, &mut shapes);
    }

    shapes
}

fn outlines(pose: &Pose<'_>, view: &View, shapes: &mut Vec<Shape>) {
    let joints = joint::resolve(pose.rig, pose.anim, pose.frame, pose.offset);
    let placed: Vec<(Option<usize>, Quad)> = match part::resolve(pose.rig, pose.anim, pose.frame, pose.offset) {
        Ok(mapped) => mapped.into_iter().map(|entry| (Some(entry.part), corners(&entry.frame, view))).collect(),
        Err(_) => pose.frames.iter().map(|frame| (None, corners(frame, view))).collect(),
    };

    for (index, quad) in placed {
        let origin = index.and_then(|part| joints.get(part)).and_then(|joint| joint.origin).map_or_else(|| centroid(&quad), |(x, y)| view.to_screen(x, y));
        let [top_left, bottom_left, top_right, bottom_right] = quad;

        for (from, to) in [(top_left, top_right), (top_right, bottom_right), (bottom_right, bottom_left), (bottom_left, top_left)] {
            shapes.push(Shape::Line { from, to, color: PART_COLOR, width: PART_WIDTH });
        }

        shapes.push(Shape::Circle { center: origin, radius: PART_ORIGIN_RADIUS, color: PART_ORIGIN_COLOR });
    }
}

fn corners(frame: &FrameData, view: &View) -> Quad {
    std::array::from_fn(|at| view.to_screen(frame.vertices[at * 2], frame.vertices[at * 2 + 1]))
}

fn centroid(quad: &Quad) -> (f32, f32) {
    let sum = quad.iter().fold((0.0, 0.0), |held, at| (held.0 + at.0, held.1 + at.1));

    (sum.0 / 4.0, sum.1 / 4.0)
}

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

fn ndc(view: &View, point: (f32, f32)) -> [f32; 2] {
    [point.0 / view.size.0 * 2.0 - 1.0, 1.0 - point.1 / view.size.1 * 2.0]
}

fn premultiplied(color: [f32; 4]) -> [f32; 4] {
    [color[0] * color[3], color[1] * color[3], color[2] * color[3], color[3]]
}

fn solid(view: &View, point: (f32, f32), color: [f32; 4]) -> Vertex {
    Vertex { position: ndc(view, point), uv: [0.0, 0.0], color: premultiplied(color), tint: NO_TINT }
}

fn push_quad(vertices: &mut Vec<Vertex>, view: &View, corners: [(f32, f32); 4], color: [f32; 4]) {
    let [a, b, c, d] = corners;

    for point in [a, b, c, a, c, d] {
        vertices.push(solid(view, point, color));
    }
}

fn push_line(vertices: &mut Vec<Vertex>, view: &View, from: (f32, f32), to: (f32, f32), color: [f32; 4], width: f32) {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let length = dx.hypot(dy);

    if length <= 0.0 {
        return;
    }

    let (nx, ny) = (-dy / length * width / 2.0, dx / length * width / 2.0);

    push_quad(vertices, view, [(from.0 + nx, from.1 + ny), (to.0 + nx, to.1 + ny), (to.0 - nx, to.1 - ny), (from.0 - nx, from.1 - ny)], color);
}

fn push_circle(vertices: &mut Vec<Vertex>, view: &View, center: (f32, f32), radius: f32, color: [f32; 4]) {
    let point = |segment: usize| {
        let angle = segment as f32 / CIRCLE_SEGMENTS as f32 * std::f32::consts::TAU;

        (center.0 + angle.cos() * radius, center.1 + angle.sin() * radius)
    };

    for segment in 0..CIRCLE_SEGMENTS {
        vertices.push(solid(view, center, color));
        vertices.push(solid(view, point(segment), color));
        vertices.push(solid(view, point(segment + 1), color));
    }
}

pub fn shape_vertices(view: &View, shapes: &[Shape], vertices: &mut Vec<Vertex>) -> Range<usize> {
    let start = vertices.len();

    for shape in shapes {
        match *shape {
            Shape::Line { from, to, color, width } => push_line(vertices, view, from, to, color, width),
            Shape::Circle { center, radius, color } => push_circle(vertices, view, center, radius, color),
            Shape::Rect { min, max, fill, stroke } => {
                if fill[3] > 0.0 {
                    push_quad(vertices, view, [min, (max.0, min.1), max, (min.0, max.1)], fill);
                }

                if let Some((color, width)) = stroke {
                    let corners = [min, (max.0, min.1), max, (min.0, max.1)];

                    for at in 0..4 {
                        push_line(vertices, view, corners[at], corners[(at + 1) % 4], color, width);
                    }
                }
            }
        }
    }

    start..vertices.len()
}

pub fn paint(pixels: &mut [u8], width: u32, height: u32, sheet: Option<&RgbaImage>, frames: &[FrameData], view: &View, shapes: &[Shape]) {
    let parts: Vec<(FrameData, [f32; 4])> = frames.iter().map(|frame| (frame.clone(), NO_TINT)).collect();
    let mut vertices = Vec::with_capacity(parts.len() * 6 + shapes.len() * 6);
    let part_batches = part_vertices(&parts, &view.view_proj(), &mut vertices);
    let overlay = shape_vertices(view, shapes, &mut vertices);
    let texture = sheet.map(|image| Texture { width: image.width(), height: image.height(), pixels: image.as_raw() });
    let mut batches: Vec<Batch<'_>> = Vec::with_capacity(part_batches.len() + 1);

    if texture.is_some() {
        batches.extend(part_batches.into_iter().map(|(blend, range)| Batch { texture, blend, alpha: Alpha::Carried, range }));
    }

    if !overlay.is_empty() {
        batches.push(Batch { texture: None, blend: 0, alpha: Alpha::Blended, range: overlay });
    }

    raster::draw(pixels, width, height, &vertices, &batches);
}

pub fn paint_shapes(pixels: &mut [u8], width: u32, height: u32, view: &View, shapes: &[Shape]) {
    let mut vertices = Vec::with_capacity(shapes.len() * 6);
    let overlay = shape_vertices(view, shapes, &mut vertices);

    if overlay.is_empty() {
        return;
    }

    raster::draw(pixels, width, height, &vertices, &[Batch { texture: None, blend: 0, alpha: Alpha::Blended, range: overlay }]);
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
