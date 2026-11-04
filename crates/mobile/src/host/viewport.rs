use std::ops::Range;
use std::sync::Arc;

use tracing::debug;

use foundry::systems::raster::{self, Alpha, Batch, Texture, Vertex};

use super::assets::SheetCache;
use super::gpu::{Scene, SceneRun, SceneSheet, SceneSlot};
use super::sink::Frame;
use super::text::label_key;

const BAR_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
const UNTINTED: [f32; 4] = [0.0; 4];
const PLAY_HEIGHT: f32 = 720.0;
const CLEARED: [u8; 4] = [0, 0, 0, 0];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Canvas {
    pub surfaced: bool,
    pub scale: f32,
    pub padded: bool,
    pub aspect: f32,
}

impl Canvas {
    fn inset(self, width: f32, height: f32) -> Option<(f32, f32)> {
        (self.surfaced && self.padded && self.scale < 1.0).then(|| ((width * (1.0 - self.scale) / 2.0).round(), (height * (1.0 - self.scale) / 2.0).round()))
    }

    pub fn unpad(self, x: f32, y: f32, width: f32, height: f32) -> (f32, f32) {
        self.inset(width, height).map_or((x, y), |(left, top)| ((x - left) / self.scale, (y - top) / self.scale))
    }
}

struct Fit {
    scale: f32,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

fn fit(width: f32, height: f32, design_width: f32, aspect: f32) -> Fit {
    let wide = if design_width > 0.0 { width / design_width } else { 1.0 };

    if aspect <= 0.0 || design_width <= 0.0 {
        return Fit { scale: wide, x: 0.0, y: 0.0, width, height };
    }

    let scale = wide.min(height / (design_width * aspect));
    let fitted_width = design_width * scale;
    let fitted_height = design_width * aspect * scale;

    Fit { scale, x: (width - fitted_width) / 2.0, y: (height - fitted_height) / 2.0, width: fitted_width, height: fitted_height }
}

struct Run {
    sheet: Option<Box<str>>,
    blend: u8,
    range: Range<usize>,
}

fn compose(frame: &Frame, sheets: &SheetCache, canvas: Canvas, width: f32, height: f32) -> (Vec<Vertex>, Vec<Run>) {
    let fit = fit(width, height, frame.design_width, canvas.aspect);
    let scale = fit.scale;
    let mut vertices: Vec<Vertex> = Vec::with_capacity(frame.quads.len() * 6 + 36);
    let mut runs: Vec<Run> = Vec::new();
    let surface = |cx: f32, cy: f32| Vertex { position: [cx / width * 2.0 - 1.0, 1.0 - cy / height * 2.0], uv: [0.0, 0.0], color: BAR_COLOR, tint: UNTINTED };
    let mut absent: Vec<&str> = Vec::new();
    let resolved: Vec<Option<(&str, (u32, u32))>> = frame.sheets.iter().map(|name| sheets.get(name).map(|sheet| (name.as_ref(), (sheet.width, sheet.height)))).collect();

    for quad in &frame.quads {
        let label = quad.label.map(label_key);
        let sheet: Option<(&str, (u32, u32))> = match (quad.sheet, label.as_ref()) {
            (Some(index), _) => resolved.get(usize::from(index)).copied().flatten(),
            (None, Some(key)) => sheets.get(key).map(|sheet| (key.as_ref(), (sheet.width, sheet.height))),
            (None, None) => None,
        };

        if label.is_some() && sheet.is_none() {
            continue;
        }

        if let Some(name) = frame.sheet(quad)
            && sheet.is_none()
            && !absent.contains(&name)
        {
            absent.push(name);
        }

        let key = sheet.map(|(name, _)| name);
        let uv = sheet.as_ref().map_or([[0.0f32; 2]; 4], |(_, (sheet_width, sheet_height))| {
            let sheet_width = (*sheet_width).max(1) as f32;
            let sheet_height = (*sheet_height).max(1) as f32;
            let left = quad.source[0] / sheet_width;
            let top = quad.source[1] / sheet_height;
            let right = (quad.source[0] + quad.source[2]) / sheet_width;
            let bottom = (quad.source[1] + quad.source[3]) / sheet_height;

            [[left, top], [left, bottom], [right, bottom], [right, top]]
        });
        let clip = |point: [f32; 2]| [(point[0] * scale + fit.x) / width * 2.0 - 1.0, 1.0 - (point[1] * scale + fit.y) / height * 2.0];
        let corner = |slot: usize| Vertex { position: clip(quad.corners[slot]), uv: uv[slot], color: quad.colors[slot], tint: UNTINTED };
        let start = vertices.len();

        vertices.extend([corner(0), corner(1), corner(2), corner(0), corner(2), corner(3)]);

        let end = vertices.len();

        match runs.last_mut() {
            Some(last) if last.sheet.as_deref() == key && last.blend == quad.blend => last.range.end = end,
            _ => runs.push(Run { sheet: key.map(Box::from), blend: quad.blend, range: start..end }),
        }
    }

    let letterbox = frame.letterbox.max(0.0);
    let pad = if canvas.surfaced { letterbox * scale } else { 0.0 };
    let floor = fit.y + (frame.design_height - letterbox).min(letterbox + PLAY_HEIGHT) * scale;
    let mut bars = vec![[0.0, fit.y, width, pad], [0.0, floor, width, if pad > 0.0 { height - floor } else { 0.0 }]];

    if canvas.aspect > 0.0 {
        bars.extend([[0.0, 0.0, width, fit.y], [0.0, fit.y + fit.height, width, height - fit.y - fit.height], [0.0, fit.y, fit.x, fit.height], [fit.x + fit.width, fit.y, width - fit.x - fit.width, fit.height]]);
    }

    let start = vertices.len();

    for [x, y, bar_width, bar_height] in bars.into_iter().filter(|bar| bar[2] > 0.0 && bar[3] > 0.0) {
        vertices.extend([surface(x, y), surface(x, y + bar_height), surface(x + bar_width, y + bar_height), surface(x, y), surface(x + bar_width, y + bar_height), surface(x + bar_width, y)]);
    }

    let end = vertices.len();

    if end > start {
        runs.push(Run { sheet: None, blend: 0, range: start..end });
    }

    if !absent.is_empty() {
        debug!("emu: {} sheets drawn but never decoded: {absent:?}", absent.len());
    }

    (vertices, runs)
}

pub(super) fn scene(frame: &Frame, sheets: &SheetCache, canvas: Canvas, width: f32, height: f32, pixel_width: u32, pixel_height: u32) -> Scene {
    let (vertices, runs) = compose(frame, sheets, canvas, width, height);
    let mut held: Vec<(&str, SceneSheet)> = Vec::new();
    let runs = runs
        .iter()
        .map(|run| {
            let sheet = run.sheet.as_deref().and_then(|name| {
                held.iter().position(|(seen, _)| *seen == name).or_else(|| {
                    let sheet = sheets.get(name)?;

                    held.push((name, SceneSheet { width: sheet.width, height: sheet.height, pixels: Arc::clone(&sheet.pixels) }));

                    Some(held.len() - 1)
                })
            });

            SceneRun { sheet, blend: run.blend, range: run.range.clone() }
        })
        .collect();
    let sheets = held.into_iter().map(|(_, sheet)| sheet).collect();

    Scene { vertices, runs, sheets, width: pixel_width, height: pixel_height, slot: SceneSlot::Battle, clear: [0.0; 4], canvas }
}

pub(super) fn paint(frame: &Frame, sheets: &SheetCache, canvas: Canvas, width: f32, height: f32, pixel_width: u32, pixel_height: u32) -> Vec<u8> {
    #[cfg(target_os = "ios")]
    if let Some(pixels) = super::offscreen::render(&scene(frame, sheets, canvas, width, height, pixel_width, pixel_height)) {
        return pixels;
    }

    let mut pixels = CLEARED.repeat(pixel_width as usize * pixel_height as usize);
    let (vertices, runs) = compose(frame, sheets, canvas, width, height);

    let batches: Vec<Batch<'_>> = runs
        .iter()
        .map(|run| Batch {
            texture: run.sheet.as_deref().and_then(|name| sheets.get(name)).map(|sheet| Texture { width: sheet.width, height: sheet.height, pixels: &sheet.pixels }),
            blend: run.blend,
            alpha: Alpha::Carried,
            range: run.range.clone(),
        })
        .collect();

    raster::draw(&mut pixels, pixel_width, pixel_height, &vertices, &batches);

    pixels
}
