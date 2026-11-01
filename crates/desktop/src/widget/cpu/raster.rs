mod lanes;

use std::ops::Range;

use rayon::prelude::*;

const BAND_ROWS: usize = 16;
const STRAIGHTEN_PIXELS: usize = 16_384;
const SAFE_TEXELS: f32 = 4_194_304.0;
const LUMA: [f32; 3] = [0.299, 0.587, 0.114];
const TEXEL_SCALE: f32 = 1.0 / 255.0;
const OPAQUE_TEXEL: [f32; 4] = [1.0; 4];
const SUBTEXEL: u32 = 256;
const FILTERED_SCALE: f32 = 1.0 / (255.0 * SUBTEXEL as f32 * SUBTEXEL as f32);
const MULTIPLY: u8 = 2;
const ATTRIBUTES: usize = 10;
const UV: usize = 0;
const COLOR: usize = 2;
const TINT: usize = 6;

const UNITS: [f32; 256] = {
    let mut units = [0.0; 256];
    let mut value = 0;

    while value < 256 {
        units[value] = value as f32 * TEXEL_SCALE;
        value += 1;
    }

    units
};

#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub(crate) struct Vertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
    pub color: [f32; 4],
    pub tint: [f32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Alpha {
    Blended,
    Carried,
}

#[derive(Clone, Copy)]
pub(crate) struct Texture<'a> {
    pub width: u32,
    pub height: u32,
    pub pixels: &'a [u8],
}

pub(crate) struct Batch<'a> {
    pub texture: Option<Texture<'a>>,
    pub blend: u8,
    pub alpha: Alpha,
    pub range: Range<usize>,
}

struct Plane {
    dx: f32,
    dy: f32,
    origin: f32,
}

impl Plane {
    fn at(&self, x: f32, y: f32) -> f32 {
        self.origin + self.dx * x + self.dy * y
    }

    fn flat(&self) -> bool {
        self.dx == 0.0 && self.dy == 0.0
    }
}

struct Edge {
    a: f32,
    b: f32,
    c: f32,
    inclusive: bool,
}

impl Edge {
    fn new(from: [f32; 2], to: [f32; 2]) -> Self {
        let a = from[1] - to[1];
        let b = to[0] - from[0];

        Self { a, b, c: from[0] * to[1] - from[1] * to[0], inclusive: a > 0.0 || (a == 0.0 && b < 0.0) }
    }

    fn at(&self, x: f32, y: f32) -> f32 {
        self.a * x + self.b * y + self.c
    }

    fn covers(&self, value: f32) -> bool {
        value > 0.0 || (value == 0.0 && self.inclusive)
    }

    fn clip(&self, y: f32, span: (f32, f32)) -> Option<(f32, f32)> {
        let rest = self.b * y + self.c;
        let (low, high) = span;

        match self.a.partial_cmp(&0.0) {
            Some(std::cmp::Ordering::Greater) => Some((low.max(-rest / self.a), high)),
            Some(std::cmp::Ordering::Less) => Some((low, high.min(-rest / self.a))),
            _ => self.covers(rest).then_some(span),
        }
    }
}

#[derive(Clone, Copy)]
struct Shading {
    mode: u8,
    carried: bool,
    tinted: bool,
    color: Option<[f32; 4]>,
}

enum Paint {
    Fill([u8; 4]),
    Lookup(Box<[[u8; 256]; 4]>),
    Shaded(Shading),
}

struct Triangle<'a> {
    edges: [Edge; 3],
    left: usize,
    right: usize,
    top: usize,
    bottom: usize,
    attributes: [Plane; ATTRIBUTES],
    texture: Option<Texture<'a>>,
    paint: Paint,
}

fn plane(points: &[[f32; 2]; 3], values: [f32; 3], area: f32) -> Plane {
    let [p0, p1, p2] = *points;
    let dx = ((values[1] - values[0]) * (p2[1] - p0[1]) - (values[2] - values[0]) * (p1[1] - p0[1])) / area;
    let dy = ((values[2] - values[0]) * (p1[0] - p0[0]) - (values[1] - values[0]) * (p2[0] - p0[0])) / area;

    Plane { dx, dy, origin: values[0] - dx * p0[0] - dy * p0[1] }
}

fn attributes(vertex: &Vertex) -> [f32; ATTRIBUTES] {
    let [u, v] = vertex.uv;
    let [red, green, blue, alpha] = vertex.color;
    let [tint_red, tint_green, tint_blue, tint_alpha] = vertex.tint;

    [u, v, red, green, blue, alpha, tint_red, tint_green, tint_blue, tint_alpha]
}

fn floor(value: f32) -> f32 {
    let truncated = value as i64 as f32;

    if truncated > value { truncated - 1.0 } else { truncated }
}

fn constant(attributes: &[Plane]) -> Option<[f32; 4]> {
    let [red, green, blue, alpha] = attributes else {
        return None;
    };

    [red, green, blue, alpha].iter().all(|plane| plane.flat()).then(|| [red, green, blue, alpha].map(|plane| plane.at(0.5, 0.5)))
}

fn lookup(source: [f32; 4], mode: u8, carried: bool) -> [[u8; 256]; 4] {
    let mut table = [[0u8; 256]; 4];

    for (value, unit) in UNITS.iter().enumerate() {
        for channel in 0..3 {
            table[channel][value] = unorm(mixed(mode, source[channel], *unit, source[channel], source[3]));
        }

        table[3][value] = if carried { value as u8 } else { unorm(mixed(mode, source[3], *unit, source[3], source[3])) };
    }

    table
}

fn painted(texture: Option<Texture<'_>>, attributes: &[Plane; ATTRIBUTES], tinted: bool, blend: u8, alpha: Alpha) -> Option<Paint> {
    let mode = if blend <= 3 { blend } else { 0 };
    let carried = alpha == Alpha::Carried && (1..=3).contains(&mode);
    let color = constant(&attributes[COLOR..TINT]);
    let shading = Shading { mode, carried, tinted, color };

    if texture.is_some() {
        return Some(Paint::Shaded(shading));
    }

    let tint = if tinted { constant(&attributes[TINT..]) } else { Some([0.0; 4]) };
    let (Some(color), Some(tint)) = (color, tint) else {
        return Some(Paint::Shaded(shading));
    };

    let shaded = if tinted { wash(OPAQUE_TEXEL, tint) } else { OPAQUE_TEXEL };
    let source: [f32; 4] = std::array::from_fn(|channel| shaded[channel] * color[channel]);
    let table = lookup(source, mode, carried);

    if table.iter().all(|channel| channel.iter().enumerate().all(|(value, out)| usize::from(*out) == value)) {
        return None;
    }

    if table.iter().all(|channel| channel.iter().all(|out| *out == channel[0])) {
        return Some(Paint::Fill(table.map(|channel| channel[0])));
    }

    Some(Paint::Lookup(Box::new(table)))
}

impl<'a> Triangle<'a> {
    fn new(corners: &[Vertex], batch: &Batch<'a>, width: usize, height: usize) -> Option<Self> {
        let [v0, v1, v2] = corners else {
            return None;
        };
        let to_pixels = |vertex: &Vertex| [(vertex.position[0] + 1.0) * 0.5 * width as f32, (1.0 - vertex.position[1]) * 0.5 * height as f32];
        let p0 = to_pixels(v0);
        let (mut p1, mut p2) = (to_pixels(v1), to_pixels(v2));
        let a0 = v0;
        let (mut a1, mut a2) = (v1, v2);
        let mut area = (p1[0] - p0[0]) * (p2[1] - p0[1]) - (p1[1] - p0[1]) * (p2[0] - p0[0]);

        if area == 0.0 || !area.is_finite() {
            return None;
        }

        if area < 0.0 {
            std::mem::swap(&mut p1, &mut p2);
            std::mem::swap(&mut a1, &mut a2);
            area = -area;
        }

        let points = [p0, p1, p2];
        let xs = [p0[0], p1[0], p2[0]];
        let ys = [p0[1], p1[1], p2[1]];
        let low_x = xs.iter().copied().fold(f32::INFINITY, f32::min).floor().max(0.0);
        let high_x = xs.iter().copied().fold(f32::NEG_INFINITY, f32::max).ceil().min(width as f32);
        let low_y = ys.iter().copied().fold(f32::INFINITY, f32::min).floor().max(0.0);
        let high_y = ys.iter().copied().fold(f32::NEG_INFINITY, f32::max).ceil().min(height as f32);

        if low_x >= high_x || low_y >= high_y {
            return None;
        }

        let tinted = [a0, a1, a2].iter().any(|vertex| vertex.tint[3] != 0.0);
        let attributes: [Plane; ATTRIBUTES] = {
            let corners = [attributes(a0), attributes(a1), attributes(a2)];

            std::array::from_fn(|index| plane(&points, [corners[0][index], corners[1][index], corners[2][index]], area))
        };
        let paint = painted(batch.texture, &attributes, tinted, batch.blend, batch.alpha)?;

        Some(Self {
            edges: [Edge::new(p1, p2), Edge::new(p2, p0), Edge::new(p0, p1)],
            left: low_x as usize,
            right: high_x as usize,
            top: low_y as usize,
            bottom: high_y as usize,
            attributes,
            texture: batch.texture,
            paint,
        })
    }

    fn covers(&self, column: usize, y: f32) -> bool {
        let x = column as f32 + 0.5;

        self.edges.iter().all(|edge| edge.covers(edge.at(x, y)))
    }

    fn span(&self, y: f32) -> Option<(usize, usize, usize)> {
        let (low, high) = self.edges.iter().try_fold((f32::NEG_INFINITY, f32::INFINITY), |span, edge| edge.clip(y, span))?;

        if low > high {
            return None;
        }

        let start = ((low - 0.5).ceil() - 1.0).max(self.left as f32) as usize;
        let end = ((high - 0.5).floor() + 2.0).min(self.right as f32).max(0.0) as usize;
        let first = (start..end).find(|column| self.covers(*column, y))?;
        let last = (first..end).rev().find(|column| self.covers(*column, y)).unwrap_or(first) + 1;

        Some((start, first, last))
    }

    fn fill(&self, band: &mut [u8], first_row: usize, width: usize) {
        let rows = band.len() / (width * 4);
        let top = self.top.max(first_row);
        let bottom = self.bottom.min(first_row + rows);

        for row in top..bottom {
            let y = row as f32 + 0.5;
            let Some((start, first, last)) = self.span(y) else {
                continue;
            };
            let line = &mut band[(row - first_row) * width * 4..(row - first_row + 1) * width * 4];
            let Some(covered) = line.get_mut(first * 4..last * 4) else {
                continue;
            };

            match &self.paint {
                Paint::Fill(bytes) => {
                    for pixel in covered.as_chunks_mut::<4>().0 {
                        *pixel = *bytes;
                    }
                }
                Paint::Lookup(table) => {
                    for pixel in covered.as_chunks_mut::<4>().0 {
                        *pixel = [
                            table[0][usize::from(pixel[0])],
                            table[1][usize::from(pixel[1])],
                            table[2][usize::from(pixel[2])],
                            table[3][usize::from(pixel[3])],
                        ];
                    }
                }
                Paint::Shaded(shading) => {
                    let origin = start as f32 + 0.5;
                    let mut values = [0.0; ATTRIBUTES];

                    for (value, plane) in values.iter_mut().zip(&self.attributes) {
                        *value = plane.at(origin, y);
                    }

                    for _ in start..first {
                        for (value, plane) in values.iter_mut().zip(&self.attributes) {
                            *value += plane.dx;
                        }
                    }

                    let pixels = covered.as_chunks_mut::<4>().0;

                    match (shading.mode, self.texture) {
                        (0, Some(texture)) => self.shade::<0, true>(pixels, values, Some(texture), *shading),
                        (1, Some(texture)) => self.shade::<1, true>(pixels, values, Some(texture), *shading),
                        (2, Some(texture)) => self.shade::<2, true>(pixels, values, Some(texture), *shading),
                        (_, Some(texture)) => self.shade::<3, true>(pixels, values, Some(texture), *shading),
                        (0, None) => self.shade::<0, false>(pixels, values, None, *shading),
                        (1, None) => self.shade::<1, false>(pixels, values, None, *shading),
                        (2, None) => self.shade::<2, false>(pixels, values, None, *shading),
                        (_, None) => self.shade::<3, false>(pixels, values, None, *shading),
                    }
                }
            }
        }
    }

    #[inline(always)]
    fn shade<const MODE: u8, const TEXTURED: bool>(&self, pixels: &mut [[u8; 4]], mut values: [f32; ATTRIBUTES], texture: Option<Texture<'_>>, shading: Shading) {
        let mut steps = [0.0; ATTRIBUTES];

        for (step, plane) in steps.iter_mut().zip(&self.attributes) {
            *step = plane.dx;
        }

        let white = shading.color == Some([1.0; 4]);
        let color = shading.color.map(lanes::gather);
        let texture = texture.filter(|texture| TEXTURED && texture.width != 0 && texture.height != 0);
        let fixed = texture.filter(|_| self.attributes[UV + 1].dx == 0.0).map(|texture| texture.rows(values[UV + 1]));

        if let (Some(texture), Some(rows), Some(_), false) = (texture, fixed, shading.color, shading.tinted) {
            sweep::<MODE>(pixels, values[UV], steps[UV], texture, rows, color.filter(|_| !white), shading.carried);

            return;
        }

        let mut held: Option<Square> = None;

        for pixel in pixels {
            let current = values;

            if TEXTURED {
                values[UV] += steps[UV];
                values[UV + 1] += steps[UV + 1];
            }

            if shading.color.is_none() {
                for channel in COLOR..TINT {
                    values[channel] += steps[channel];
                }
            }

            if shading.tinted {
                for channel in TINT..ATTRIBUTES {
                    values[channel] += steps[channel];
                }
            }

            let fetched = texture.and_then(|texture| {
                let rows = fixed.unwrap_or_else(|| texture.rows(current[UV + 1]));
                let (column, across) = texture.locate(current[UV]);

                if held.as_ref().is_none_or(|square| (square.column, square.upper, square.lower) != (column, rows.upper, rows.lower)) {
                    let words = texture.corners(rows, column);

                    held = Some(Square { column, upper: rows.upper, lower: rows.lower, words, corners: lanes::corners(words) });
                }

                held.as_ref().filter(|square| square.words.iter().any(|word| *word != 0)).map(|square| (square.words, square.corners, across, rows.down))
            });

            if TEXTURED && MODE != MULTIPLY && fetched.is_none() {
                continue;
            }

            if shading.tinted {
                let texel = match fetched {
                    Some((words, _, across, down)) => filtered(words, across, down),
                    None if TEXTURED => [0.0; 4],
                    None => OPAQUE_TEXEL,
                };

                if MODE != MULTIPLY && texel == [0.0; 4] {
                    continue;
                }

                let shaded = wash(texel, [current[TINT], current[TINT + 1], current[TINT + 2], current[TINT + 3]]);
                let tone = match shading.color {
                    _ if white => [1.0; 4],
                    Some(color) => color,
                    None => [current[COLOR], current[COLOR + 1], current[COLOR + 2], current[COLOR + 3]],
                };
                let source = if white { shaded } else { [shaded[0] * tone[0], shaded[1] * tone[1], shaded[2] * tone[2], shaded[3] * tone[3]] };

                blend::<MODE>(pixel, source, shading.carried);

                continue;
            }

            let texel = match fetched {
                Some((_, corners, across, down)) => lanes::filtered(corners, across, down),
                None if TEXTURED => lanes::splat(0.0),
                None => lanes::splat(1.0),
            };

            if MODE != MULTIPLY && lanes::empty(texel) {
                continue;
            }

            let source = match color {
                _ if white => texel,
                Some(color) => lanes::mul(texel, color),
                None => lanes::mul(texel, lanes::gather([current[COLOR], current[COLOR + 1], current[COLOR + 2], current[COLOR + 3]])),
            };

            lanes::blend::<MODE>(pixel, source, shading.carried);
        }
    }
}

#[inline(always)]
fn sweep<const MODE: u8>(pixels: &mut [[u8; 4]], u: f32, step: f32, texture: Texture<'_>, rows: Rows, color: Option<lanes::Texel>, carried: bool) {
    let reach = (u.abs() + (pixels.len() as f32 + 1.0) * step.abs() * 1.01 + 1.0) * texture.width as f32;

    if reach < SAFE_TEXELS {
        sweep_span::<MODE, true>(pixels, u, step, texture, rows, color, carried);
    } else {
        sweep_span::<MODE, false>(pixels, u, step, texture, rows, color, carried);
    }
}

#[inline(always)]
fn sweep_span<const MODE: u8, const BOUNDED: bool>(pixels: &mut [[u8; 4]], mut u: f32, step: f32, texture: Texture<'_>, rows: Rows, color: Option<lanes::Texel>, carried: bool) {
    let width = texture.width as f32;
    let last = i64::from(texture.width) - 1;
    let texels = texture.pixels.as_chunks::<4>().0;
    let upper = texels.get(rows.upper..).unwrap_or_default();
    let lower = texels.get(rows.lower..).unwrap_or_default();
    let word = |line: &[[u8; 4]], index: usize| line.get(index).map_or(0, |pixel| u32::from_le_bytes(*pixel));
    let mut seen = None;
    let mut clear = true;
    let mut corners = lanes::corners([0; 4]);

    for pixel in pixels {
        let x = u * width - 0.5;

        u += step;

        let (left, column) = if BOUNDED {
            let truncated = lanes::truncate(x);
            let whole = truncated as f32;

            if whole > x { (whole - 1.0, i64::from(truncated - 1)) } else { (whole, i64::from(truncated)) }
        } else {
            let left = floor(x);

            (left, left as i64)
        };

        if seen != Some(column) {
            let (near, far) = (column.clamp(0, last) as usize, column.wrapping_add(1).clamp(0, last) as usize);
            let words = [word(upper, near), word(upper, far), word(lower, near), word(lower, far)];

            seen = Some(column);
            clear = words == [0; 4];
            corners = lanes::corners(words);
        }

        if clear && MODE != MULTIPLY {
            continue;
        }

        let texel = if clear {
            lanes::splat(0.0)
        } else {
            let fraction = (x - left) * SUBTEXEL as f32 + 0.5;
            let across = if BOUNDED { (lanes::truncate(fraction) as u32).min(SUBTEXEL) } else { (fraction as u32).min(SUBTEXEL) };

            lanes::filtered(corners, across, rows.down)
        };

        if MODE != MULTIPLY && lanes::empty(texel) {
            continue;
        }

        let source = color.map_or(texel, |color| lanes::mul(texel, color));

        lanes::blend::<MODE>(pixel, source, carried);
    }
}

struct Square {
    column: i64,
    upper: usize,
    lower: usize,
    words: [u32; 4],
    corners: lanes::Corners,
}

#[derive(Clone, Copy)]
struct Rows {
    down: u32,
    upper: usize,
    lower: usize,
}

impl Texture<'_> {
    #[inline(always)]
    fn word(self, index: usize) -> u32 {
        self.pixels.as_chunks::<4>().0.get(index).map_or(0, |pixel| u32::from_le_bytes(*pixel))
    }

    #[inline(always)]
    fn rows(self, v: f32) -> Rows {
        let y = v * self.height as f32 - 0.5;
        let top = floor(y);
        let down = (((y - top) * SUBTEXEL as f32 + 0.5) as u32).min(SUBTEXEL);
        let row = top as i64;
        let last = i64::from(self.height) - 1;
        let width = self.width as usize;

        Rows { down, upper: row.clamp(0, last) as usize * width, lower: row.wrapping_add(1).clamp(0, last) as usize * width }
    }

    #[inline(always)]
    fn locate(self, u: f32) -> (i64, u32) {
        let x = u * self.width as f32 - 0.5;
        let left = floor(x);

        (left as i64, (((x - left) * SUBTEXEL as f32 + 0.5) as u32).min(SUBTEXEL))
    }

    #[inline(always)]
    fn corners(self, rows: Rows, column: i64) -> [u32; 4] {
        let last = i64::from(self.width) - 1;
        let (near, far) = (column.clamp(0, last) as usize, column.wrapping_add(1).clamp(0, last) as usize);

        [self.word(rows.upper + near), self.word(rows.upper + far), self.word(rows.lower + near), self.word(rows.lower + far)]
    }
}

fn filtered(words: [u32; 4], across: u32, down: u32) -> [f32; 4] {
    let [first, second, third, fourth] = words;
    let channel = |shift: u32| {
        let byte = |word: u32| (word >> shift) & 0xff;
        let upper = byte(first) * (SUBTEXEL - across) + byte(second) * across;
        let lower = byte(third) * (SUBTEXEL - across) + byte(fourth) * across;

        (upper * (SUBTEXEL - down) + lower * down) as f32 * FILTERED_SCALE
    };

    [channel(0), channel(8), channel(16), channel(24)]
}

fn wash(texel: [f32; 4], tint: [f32; 4]) -> [f32; 4] {
    let lit = texel[0] * LUMA[0] + texel[1] * LUMA[1] + texel[2] * LUMA[2];
    let mix = |base: f32, target: f32| base * (1.0 - tint[3]) + target * lit * tint[3];

    [mix(texel[0], tint[0]), mix(texel[1], tint[1]), mix(texel[2], tint[2]), texel[3]]
}

#[inline(always)]
fn mixed(mode: u8, src: f32, dst: f32, coverage: f32, alpha: f32) -> f32 {
    match mode {
        1 => src + dst,
        2 => src * dst,
        3 => src + dst * (1.0 - coverage),
        _ => src + dst * (1.0 - alpha),
    }
}

#[inline(always)]
fn blend<const MODE: u8>(pixel: &mut [u8; 4], source: [f32; 4], carried: bool) {
    if MODE == 0 && source[3] == 1.0 {
        *pixel = [unorm(source[0]), unorm(source[1]), unorm(source[2]), 0xff];

        return;
    }

    let target = [UNITS[usize::from(pixel[0])], UNITS[usize::from(pixel[1])], UNITS[usize::from(pixel[2])], UNITS[usize::from(pixel[3])]];

    for channel in 0..3 {
        pixel[channel] = unorm(mixed(MODE, source[channel], target[channel], source[channel], source[3]));
    }

    if !carried {
        pixel[3] = unorm(mixed(MODE, source[3], target[3], source[3], source[3]));
    }
}

fn unorm(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

pub(crate) fn draw(pixels: &mut [u8], width: u32, height: u32, vertices: &[Vertex], batches: &[Batch<'_>]) {
    let (width, height) = (width as usize, height as usize);

    if width == 0 || height == 0 || pixels.len() < width * height * 4 {
        return;
    }

    let triangles: Vec<Triangle<'_>> = batches
        .iter()
        .flat_map(|batch| {
            vertices
                .get(batch.range.clone())
                .unwrap_or_default()
                .as_chunks::<3>()
                .0
                .iter()
                .filter_map(move |corners| Triangle::new(corners, batch, width, height))
        })
        .collect();

    pixels[..width * height * 4].par_chunks_mut(width * 4 * BAND_ROWS).enumerate().for_each(|(band, rows)| {
        let first_row = band * BAND_ROWS;

        for triangle in triangles.iter().filter(|triangle| triangle.bottom > first_row && triangle.top < first_row + BAND_ROWS) {
            triangle.fill(rows, first_row, width);
        }
    });
}

pub(crate) fn straighten(pixels: &mut [u8]) {
    pixels.par_chunks_mut(STRAIGHTEN_PIXELS * 4).for_each(|chunk| {
        for pixel in chunk.as_chunks_mut::<4>().0 {
            let alpha = u32::from(pixel[3]);

            if alpha == 0 || alpha == 0xff {
                continue;
            }

            for channel in &mut pixel[..3] {
                *channel = ((u32::from(*channel) * 0xff + alpha / 2) / alpha).min(0xff) as u8;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{blend, draw, filtered, lanes, Alpha, Batch, Texture, Vertex};

    fn agree<const MODE: u8>(next: &mut impl FnMut() -> u32) {
        let opaque = if next().is_multiple_of(4) { 0xff00_0000 } else { 0 };
        let words = [next() | opaque, next() | opaque, next() | opaque, next() | opaque];
        let (across, down) = (next() % 257, next() % 257);
        let opacity = if opaque == 0 { next() % 1001 } else { 1000 };
        let tone = [next() % 1300, next() % 1300, next() % 1300, opacity].map(|value| value as f32 / 1000.0);
        let texel = filtered(words, across, down);
        let source = [texel[0] * tone[0], texel[1] * tone[1], texel[2] * tone[2], texel[3] * tone[3]];
        let wide = lanes::mul(lanes::filtered(lanes::corners(words), across, down), lanes::gather(tone));
        let carried = MODE != 0 && next().is_multiple_of(2);
        let start = next().to_le_bytes();
        let (mut narrow_pixel, mut wide_pixel) = (start, start);

        assert_eq!(lanes::empty(lanes::filtered(lanes::corners(words), across, down)), texel == [0.0; 4]);

        blend::<MODE>(&mut narrow_pixel, source, carried);
        lanes::blend::<MODE>(&mut wide_pixel, wide, carried);

        assert_eq!(wide_pixel, narrow_pixel, "mode {MODE}, words {words:08x?}, across {across}, down {down}, tone {tone:?}, carried {carried}");
    }

    #[test]
    fn the_wide_kernel_lands_on_the_same_bytes_as_the_scalar_one() {
        // Sprites go through the 4-wide path and tinted ones through the scalar path; a single byte apart would show.
        let mut state = 0x2545_f491u32;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state
        };

        for _ in 0..50_000 {
            agree::<0>(&mut next);
            agree::<1>(&mut next);
            agree::<2>(&mut next);
            agree::<3>(&mut next);
        }
    }

    fn quad(color: [f32; 4]) -> Vec<Vertex> {
        let corner = |x: f32, y: f32| Vertex { position: [x, y], uv: [(x + 1.0) / 2.0, (1.0 - y) / 2.0], color, tint: [0.0; 4] };

        vec![corner(-1.0, 1.0), corner(-1.0, -1.0), corner(1.0, -1.0), corner(-1.0, 1.0), corner(1.0, -1.0), corner(1.0, 1.0)]
    }

    #[test]
    fn a_quad_split_into_two_triangles_covers_every_pixel_once() {
        // The diagonal runs straight through pixel centers; drawing it twice would show up as a bright seam.
        let mut pixels = vec![0u8; 8 * 8 * 4];

        draw(&mut pixels, 8, 8, &quad([0.25; 4]), &[Batch { texture: None, blend: 1, alpha: Alpha::Blended, range: 0..6 }]);

        assert!(pixels.as_chunks::<4>().0.iter().all(|pixel| *pixel == [64, 64, 64, 64]));
    }

    #[test]
    fn a_texture_drawn_at_its_own_size_comes_back_unchanged() {
        let texels: Vec<u8> = (0..16).map(|value| value * 16).collect();
        let mut pixels = vec![0u8; 2 * 2 * 4];
        let texture = Texture { width: 2, height: 2, pixels: &texels };

        draw(&mut pixels, 2, 2, &quad([1.0; 4]), &[Batch { texture: Some(texture), blend: 1, alpha: Alpha::Blended, range: 0..6 }]);

        assert_eq!(pixels, texels);
    }
}
