use std::ops::Range;
#[cfg(feature = "disk")]
use std::path::{Path, PathBuf};

use ab_glyph::{Font, FontRef, PxScale};
use image::imageops::{self, FilterType};
use image::{Rgba, RgbaImage};
use imageproc::drawing::{draw_filled_circle_mut, draw_filled_rect_mut, draw_text_mut, text_size};
use imageproc::rect::Rect;

use crate::Source;
#[cfg(feature = "disk")]
use crate::common::assets;
use crate::common::gfx::{autocrop, open_image};

use super::draw::{CellShape, draw_bordered_rect_mut, draw_centered_text, line_offset, wrap_text};
use super::{StageCrown, StageDropTable, StageGround, StageMaterials, StageNote, StagePlate, StageSheet};

const COLOR_BACKGROUND: Rgba<u8> = Rgba([33, 33, 33, 255]);
const COLOR_TEXT: Rgba<u8> = Rgba([230, 230, 230, 255]);
const COLOR_WHITE: Rgba<u8> = Rgba([255, 255, 255, 255]);
const COLOR_RULE: Rgba<u8> = Rgba([71, 71, 71, 255]);
const COLOR_CELL_HEADER: Rgba<u8> = Rgba([15, 15, 15, 255]);
const COLOR_CELL_VALUE: Rgba<u8> = Rgba([58, 58, 58, 255]);
const COLOR_CELL_BORDER: Rgba<u8> = Rgba([42, 42, 42, 255]);
const COLOR_TABLE_HEADER: Rgba<u8> = Rgba([18, 18, 18, 255]);
const COLOR_TABLE_ROW: Rgba<u8> = Rgba([26, 26, 26, 255]);
const COLOR_FRAME: Rgba<u8> = Rgba([31, 106, 165, 255]);
const COLOR_CROWN: Rgba<u8> = Rgba([71, 71, 71, 255]);
const COLOR_CROWN_BORDER: Rgba<u8> = Rgba([122, 122, 122, 255]);
const COLOR_CROWN_OFF: Rgba<u8> = Rgba([38, 38, 38, 255]);

const EMPTY_ALPHA: f32 = 0.35;
const OFF_ALPHA: f32 = 0.4;

const RENDER_SCALE: f32 = 2.0;
const CONTENT_WIDTH: f32 = 720.0;
const CONTENT_PADDING: f32 = 8.0;
const BLOCK_GAP: f32 = 20.0;
const HEAD_GAP: f32 = 8.0;
const LINE_HEIGHT: f32 = 1.3;
const TEXT_SIZE: f32 = 16.0;
const RADIUS: f32 = 4.0;
const RULE: f32 = 1.0;

const MAP_BANNER: f32 = 50.0;
const STAGE_BANNER: f32 = 35.0;
const BANNER_GAP: f32 = 12.0;
const FALLBACK_NAME: f32 = 32.0;

const CROWN_WIDTH: f32 = 50.0;
const CROWN_HEIGHT: f32 = 30.0;
const CROWN_GAP: f32 = 5.0;
const CROWN_TEXT: f32 = 14.0;
const CROWN_GLYPH: &str = "\u{1F732}";

const SECTION_TITLE: f32 = 18.0;
const SECTION_GAP: f32 = 6.0;
const NOTE_TITLE: f32 = 14.0;
const NOTE_GAP: f32 = 10.0;
const NOTE_LINE_GAP: f32 = 2.0;

const CELL_WIDTH: f32 = 82.0;
const CELL_HEIGHT: f32 = 28.0;
const CELL_GAP: f32 = 4.0;
const CELL_TEXT: f32 = 13.0;
const CELL_BORDER: f32 = 2.0;

const DROP_WIDTH: f32 = 345.0;
const DROP_GAP: f32 = 15.0;
const ICON: f32 = 32.0;
const ROW_PAD_Y: f32 = 4.0;
const MATERIAL_PAD_X: f32 = 6.0;
const MATERIAL_GAP: f32 = 8.0;
const MATERIAL_TEXT: f32 = 12.0;
const TREASURE_PAD_X: f32 = 8.0;
const TREASURE_GAP: f32 = 8.0;
const TREASURE_HEAD: f32 = 13.0;

const GROUND_PAD_X: f32 = 6.0;
const GROUND_GAP: f32 = 6.0;
const GROUND_TEXT: f32 = 12.0;
const FALLBACK_TEXT: f32 = 11.0;

const BOUND_BASE: f32 = 600.0;
const BOUND_CHUNK: f32 = 120.0;
const BOUND_WRAP: usize = 40;

const FRAME_THICKNESS: f32 = 7.0;
const FRAME_PADDING: f32 = 4.0;
const FRAME_RADIUS: f32 = 6.0;

pub struct StageFonts<'a> {
    pub regular: &'a [u8],
    pub bold: &'a [u8],
    pub symbols: &'a [u8],
    pub fallbacks: Vec<&'a [u8]>,
}

#[cfg(feature = "disk")]
impl StageFonts<'static> {
    pub fn embedded() -> Self {
        Self { regular: assets::FONT_JP, bold: assets::FONT_JP_BOLD, symbols: assets::FONT_SYMBOLS, fallbacks: vec![assets::FONT_KR, assets::FONT_TC, assets::FONT_TH] }
    }
}

#[derive(Clone, Copy)]
enum Seat {
    Middle,
    Floor,
}

#[derive(Clone, Copy, PartialEq)]
enum Weight {
    Regular,
    Bold,
    Symbols,
}

struct Faces<'a> {
    regular: FontRef<'a>,
    bold: FontRef<'a>,
    symbols: FontRef<'a>,
    fallbacks: Vec<FontRef<'a>>,
}

struct Page<'a> {
    image: RgbaImage,
    faces: Faces<'a>,
}

fn px(value: f32) -> i32 {
    (value * RENDER_SCALE).round() as i32
}

fn faded(color: Rgba<u8>, alpha: f32, over: Rgba<u8>) -> Rgba<u8> {
    let mixed = |top: u8, under: u8| (f32::from(top) * alpha + f32::from(under) * (1.0 - alpha)).round() as u8;

    Rgba([mixed(color[0], over[0]), mixed(color[1], over[1]), mixed(color[2], over[2]), 255])
}

fn covers(font: &FontRef<'_>, text: &str) -> bool {
    text.chars().all(|glyph| glyph.is_ascii() || glyph.is_whitespace() || font.glyph_id(glyph).0 != 0)
}

fn picture(source: &Source) -> Option<RgbaImage> {
    let cropped = autocrop(open_image(source)?.to_rgba8());

    (cropped.width() > 1 || cropped.height() > 1).then_some(cropped)
}

fn fitted(picture: RgbaImage, width: Option<u32>, height: u32) -> RgbaImage {
    let ratio = (height as f32 / picture.height().max(1) as f32).min(width.map_or(f32::MAX, |width| width as f32 / picture.width().max(1) as f32));
    let size = (((picture.width() as f32 * ratio).round() as u32).max(1), ((picture.height() as f32 * ratio).round() as u32).max(1));

    if size == picture.dimensions() { picture } else { imageops::resize(&picture, size.0, size.1, FilterType::Lanczos3) }
}

fn top_rounded(image: &mut RgbaImage, rect: Rect, radius: i32, color: Rgba<u8>) {
    let (width, height) = (rect.width() as i32, rect.height() as i32);
    let radius = radius.min(width / 2).min(height).max(0);

    if radius == 0 {
        draw_filled_rect_mut(image, rect, color);
        return;
    }

    if height > radius {
        draw_filled_rect_mut(image, Rect::at(rect.left(), rect.top() + radius).of_size(width as u32, (height - radius) as u32), color);
    }

    if width > radius * 2 {
        draw_filled_rect_mut(image, Rect::at(rect.left() + radius, rect.top()).of_size((width - radius * 2) as u32, radius as u32), color);
    }

    draw_filled_circle_mut(image, (rect.left() + radius, rect.top() + radius), radius, color);
    draw_filled_circle_mut(image, (rect.left() + width - 1 - radius, rect.top() + radius), radius, color);
}

fn portions(widths: &[u16], total: f32, gap: f32) -> Vec<(f32, f32)> {
    let share: f32 = widths.iter().map(|width| f32::from(*width)).sum::<f32>().max(1.0);
    let usable = total - gap * widths.len().saturating_sub(1) as f32;
    let mut at = 0.0;

    widths
        .iter()
        .map(|width| {
            let wide = usable * f32::from(*width) / share;
            let placed = (at, wide);

            at += wide + gap;
            placed
        })
        .collect()
}

impl<'a> Page<'a> {
    fn face(&self, weight: Weight, text: &str) -> &FontRef<'a> {
        let chosen = match weight {
            Weight::Regular => &self.faces.regular,
            Weight::Bold => &self.faces.bold,
            Weight::Symbols => return &self.faces.symbols,
        };

        if covers(chosen, text) { chosen } else { self.faces.fallbacks.iter().find(|font| covers(font, text)).unwrap_or(chosen) }
    }

    fn scale(font: &FontRef<'_>, size: f32) -> PxScale {
        PxScale::from(size * RENDER_SCALE * font.height_unscaled() / font.units_per_em().unwrap_or(font.height_unscaled()))
    }

    fn width(&self, weight: Weight, size: f32, text: &str) -> f32 {
        let font = self.face(weight, text);

        text_size(Self::scale(font, size), font, text).0 as f32 / RENDER_SCALE
    }

    fn line(size: f32) -> f32 {
        size * LINE_HEIGHT
    }

    fn write(&mut self, at: (f32, f32), weight: Weight, size: f32, color: Rgba<u8>, text: &str) {
        let font = self.face(weight, text).clone();
        let scale = Self::scale(&font, size);
        let offset = line_offset(&font, scale, px(Self::line(size)));

        draw_text_mut(&mut self.image, color, px(at.0), px(at.1) + offset, scale, &font, text);
    }

    fn label(&mut self, frame: (f32, f32, f32, f32), weight: Weight, size: f32, color: Rgba<u8>, text: &str) {
        let lines: Vec<&str> = text.split('\n').collect();
        let block = Self::line(size) * lines.len() as f32;
        let mut top = frame.1 + (frame.3 - block) / 2.0;

        for line in lines {
            let left = frame.0 + (frame.2 - self.width(weight, size, line)) / 2.0;

            self.write((left.max(frame.0), top), weight, size, color, line);
            top += Self::line(size);
        }
    }

    fn fill(&mut self, frame: (f32, f32, f32, f32), color: Rgba<u8>) {
        let rect = Rect::at(px(frame.0), px(frame.1)).of_size((px(frame.0 + frame.2) - px(frame.0)).max(1) as u32, (px(frame.1 + frame.3) - px(frame.1)).max(1) as u32);

        draw_filled_rect_mut(&mut self.image, rect, color);
    }

    fn head(&mut self, frame: (f32, f32, f32, f32)) {
        let rect = Rect::at(px(frame.0), px(frame.1)).of_size((px(frame.0 + frame.2) - px(frame.0)).max(1) as u32, (px(frame.1 + frame.3) - px(frame.1)).max(1) as u32);

        top_rounded(&mut self.image, rect, px(RADIUS), COLOR_TABLE_HEADER);
    }

    fn stripe(index: usize) -> Rgba<u8> {
        if index.is_multiple_of(2) { COLOR_TABLE_ROW } else { COLOR_BACKGROUND }
    }

    fn stamp(&mut self, picture: &RgbaImage, frame: (f32, f32, f32, f32), seat: Seat) {
        let spare = px(frame.3) - picture.height() as i32;
        let left = px(frame.0) + (px(frame.2) - picture.width() as i32) / 2;
        let top = px(frame.1)
            + match seat {
                Seat::Middle => spare / 2,
                Seat::Floor => spare,
            };

        imageops::overlay(&mut self.image, picture, i64::from(left), i64::from(top));
    }

    fn icon(&mut self, source: Option<&Source>, fallback: &str, frame: (f32, f32, f32, f32), seat: Seat) {
        match source.and_then(picture) {
            Some(found) => {
                let side = px(ICON) as u32;
                let inner = (frame.0, frame.1 + (frame.3 - ICON) / 2.0, frame.2, ICON);

                self.stamp(&fitted(found, Some(side), side), inner, seat);
            }
            None => self.label(frame, Weight::Regular, FALLBACK_TEXT, COLOR_TEXT, fallback),
        }
    }

    fn rule(&mut self, left: f32, top: f32, width: f32) {
        self.fill((left, top, width, RULE), COLOR_RULE);
    }

    fn banners(&mut self, map: &StagePlate, stage: &StagePlate, left: f32, top: f32) -> f32 {
        let drawn = |plate: &StagePlate, height: f32| plate.image.as_ref().and_then(picture).map(|found| fitted(found, None, px(height) as u32));
        let plates = [(map, drawn(map, MAP_BANNER)), (stage, drawn(stage, STAGE_BANNER))];
        let tall = |plate: &Option<RgbaImage>| plate.as_ref().map_or(Self::line(FALLBACK_NAME), |found| found.height() as f32 / RENDER_SCALE);
        let height = plates.iter().map(|(_, found)| tall(found)).fold(0.0, f32::max);
        let mut at = left;

        for (plate, found) in &plates {
            let own = tall(found);

            match found {
                Some(found) => {
                    let wide = found.width() as f32 / RENDER_SCALE;

                    self.stamp(found, (at, top + height - own, wide, own), Seat::Middle);
                    at += wide + BANNER_GAP;
                }
                None => {
                    self.write((at, top + height - own), Weight::Bold, FALLBACK_NAME, COLOR_TEXT, &plate.name);
                    at += self.width(Weight::Bold, FALLBACK_NAME, &plate.name) + BANNER_GAP;
                }
            }
        }

        top + height
    }

    fn crowns(&mut self, crowns: &[StageCrown], left: f32, top: f32) -> f32 {
        if crowns.is_empty() {
            return top;
        }

        for (index, crown) in crowns.iter().enumerate() {
            let at = left + index as f32 * (CROWN_WIDTH + CROWN_GAP);
            let rect = Rect::at(px(at), px(top)).of_size(px(CROWN_WIDTH) as u32, px(CROWN_HEIGHT) as u32);
            let (fill, edge, border, ink) = match (crown.enabled, crown.selected) {
                (false, _) => (COLOR_CROWN_OFF, COLOR_CROWN_BORDER, 1.0, faded(COLOR_TEXT, OFF_ALPHA, COLOR_CROWN_OFF)),
                (true, true) => (COLOR_FRAME, COLOR_WHITE, 2.0, COLOR_WHITE),
                (true, false) => (COLOR_CROWN, COLOR_CROWN_BORDER, 1.0, COLOR_TEXT),
            };
            let number = (index + 1).to_string();
            let wide = self.width(Weight::Regular, CROWN_TEXT, &number);
            let whole = wide + self.width(Weight::Symbols, CROWN_TEXT, CROWN_GLYPH);
            let start = at + (CROWN_WIDTH - whole) / 2.0;
            let line = top + (CROWN_HEIGHT - Self::line(CROWN_TEXT)) / 2.0;

            draw_bordered_rect_mut(&mut self.image, CellShape::new(rect, px(RADIUS), px(border)), fill, edge);
            self.write((start, line), Weight::Regular, CROWN_TEXT, ink, &number);
            self.write((start + wide, line), Weight::Symbols, CROWN_TEXT, ink, CROWN_GLYPH);
        }

        top + CROWN_HEIGHT
    }

    fn section(&mut self, title: &str, left: f32, top: f32, width: f32) -> f32 {
        self.write((left, top), Weight::Bold, SECTION_TITLE, COLOR_TEXT, title);

        let ruled = top + Self::line(SECTION_TITLE) + SECTION_GAP;

        self.rule(left, ruled, width);
        ruled + RULE + SECTION_GAP
    }

    fn cells(&mut self, labels: &[String], heading: bool, left: f32, top: f32) {
        for (index, label) in labels.iter().enumerate() {
            let at = left + index as f32 * (CELL_WIDTH + CELL_GAP);
            let cell = CellShape::new(Rect::at(px(at), px(top)).of_size(px(CELL_WIDTH) as u32, px(CELL_HEIGHT) as u32), px(RADIUS), px(CELL_BORDER));
            let font = self.face(Weight::Regular, label).clone();

            draw_bordered_rect_mut(&mut self.image, cell, if heading { COLOR_CELL_HEADER } else { COLOR_CELL_VALUE }, COLOR_CELL_BORDER);
            draw_centered_text(&mut self.image, COLOR_TEXT, cell, Self::scale(&font, CELL_TEXT), &font, label);
        }
    }

    fn information(&mut self, sheet: &StageSheet, left: f32, top: f32) -> f32 {
        let mut at = self.section("Information", left, top, CONTENT_WIDTH);

        for (labels, heading) in [(&sheet.headers_1, true), (&sheet.values_1, false), (&sheet.headers_2, true), (&sheet.values_2, false)] {
            self.cells(labels, heading, left, at);
            at += CELL_HEIGHT + CELL_GAP;
        }

        at - CELL_GAP
    }

    fn materials(&mut self, held: &StageMaterials, left: f32, top: f32) -> f32 {
        let mut at = self.section(&held.heading, left, top, DROP_WIDTH);
        let column = (DROP_WIDTH - MATERIAL_PAD_X * 2.0 - MATERIAL_GAP * 3.0) / 4.0;
        let slot = |index: usize| left + MATERIAL_PAD_X + index as f32 * (column + MATERIAL_GAP);
        let rows = [Self::line(MATERIAL_TEXT), ICON, Self::line(TEXT_SIZE)].map(|inner| inner + ROW_PAD_Y * 2.0);

        for (count, chunk) in held.chunks.iter().enumerate() {
            if count > 0 {
                at += MATERIAL_GAP;
            }

            self.head((left, at, DROP_WIDTH, rows[0]));
            self.fill((left, at + rows[0], DROP_WIDTH, rows[1]), Self::stripe(0));
            self.fill((left, at + rows[0] + rows[1], DROP_WIDTH, rows[2]), Self::stripe(1));

            for (index, material) in chunk.iter().enumerate() {
                let ink = if material.empty { faded(COLOR_TEXT, EMPTY_ALPHA, Self::stripe(1)) } else { COLOR_TEXT };

                self.label((slot(index), at, column, rows[0]), Weight::Regular, MATERIAL_TEXT, COLOR_WHITE, &material.name);
                self.icon(material.icon.as_ref(), &material.name, (slot(index), at + rows[0], column, rows[1]), Seat::Middle);
                self.label((slot(index), at + rows[0] + rows[1], column, rows[2]), Weight::Regular, TEXT_SIZE, ink, &material.odds);
            }

            at += rows.iter().sum::<f32>();
        }

        at
    }

    fn table(&mut self, held: &StageDropTable, left: f32, top: f32) -> f32 {
        let mut at = self.section(&held.title, left, top, DROP_WIDTH);
        let columns = portions(&[1, 1, 1], DROP_WIDTH - TREASURE_PAD_X * 2.0, TREASURE_GAP);
        let place = |index: usize, top: f32, height: f32| (left + TREASURE_PAD_X + columns[index].0, top, columns[index].1, height);
        let heading = Self::line(TREASURE_HEAD) + ROW_PAD_Y * 2.0;
        let row = ICON + ROW_PAD_Y * 2.0;

        self.head((left, at, DROP_WIDTH, heading));

        for (index, label) in [held.first_header.as_str(), "Item", "Amount"].into_iter().enumerate() {
            self.label(place(index, at, heading), Weight::Regular, TREASURE_HEAD, COLOR_WHITE, label);
        }

        at += heading;

        for (index, drop) in held.rows.iter().enumerate() {
            self.fill((left, at, DROP_WIDTH, row), Self::stripe(index));
            self.label(place(0, at, row), Weight::Regular, TEXT_SIZE, COLOR_TEXT, &drop.left);
            self.icon(drop.icon.as_ref(), &drop.name, place(1, at, row), Seat::Middle);
            self.label(place(2, at, row), Weight::Regular, TEXT_SIZE, COLOR_TEXT, &drop.amount);
            at += row;
        }

        at
    }

    fn drops(&mut self, sheet: &StageSheet, left: f32, top: f32) -> f32 {
        let beside = sheet.materials.as_ref().map(|held| self.materials(held, left, top));
        let start = if beside.is_some() { left + DROP_WIDTH + DROP_GAP } else { left };
        let mut at = top;

        for (index, held) in sheet.tables.iter().enumerate() {
            if index > 0 {
                at += DROP_GAP;
            }

            at = self.table(held, start, at);
        }

        at.max(beside.unwrap_or(top))
    }

    fn notes(&mut self, notes: &[StageNote], left: f32, top: f32) -> f32 {
        let mut at = top;

        for note in notes {
            self.write((left, at), Weight::Bold, NOTE_TITLE, COLOR_TEXT, &note.title);
            at += Self::line(NOTE_TITLE) + SECTION_GAP;

            for (index, line) in note.lines.iter().enumerate() {
                let font = self.face(Weight::Regular, line).clone();

                if index > 0 {
                    at += NOTE_LINE_GAP;
                }

                for wrapped in wrap_text(line, &font, Self::scale(&font, TEXT_SIZE), CONTENT_WIDTH * RENDER_SCALE) {
                    self.write((left, at), Weight::Regular, TEXT_SIZE, COLOR_TEXT, &wrapped);
                    at += Self::line(TEXT_SIZE);
                }
            }

            at += NOTE_GAP;
        }

        at
    }

    fn ground(&mut self, sheet: &StageSheet, shown: &Range<usize>, noted: bool, left: f32, top: f32) -> f32 {
        let StageGround { headers, spawns } = &sheet.ground;
        let mut at = self.section("Battleground", left, top, CONTENT_WIDTH);

        if noted {
            at = self.notes(&sheet.notes, left, at);
        }

        if spawns.is_empty() {
            self.write((left, at), Weight::Regular, TEXT_SIZE, COLOR_TEXT, "No enemies defined for this stage.");
            return at + Self::line(TEXT_SIZE);
        }

        let widths: Vec<u16> = headers.iter().map(|(_, width)| *width).collect();
        let columns = portions(&widths, CONTENT_WIDTH - GROUND_PAD_X * 2.0, GROUND_GAP);
        let place = |index: usize, top: f32, height: f32| (left + GROUND_PAD_X + columns[index].0, top, columns[index].1, height);
        let lines = headers.iter().map(|(label, _)| label.split('\n').count()).max().unwrap_or(1) as f32;
        let heading = Self::line(GROUND_TEXT) * lines + ROW_PAD_Y * 2.0;
        let row = ICON + ROW_PAD_Y * 2.0;

        self.head((left, at, CONTENT_WIDTH, heading));

        for (index, (label, _)) in headers.iter().enumerate() {
            self.label(place(index, at, heading), Weight::Regular, GROUND_TEXT, COLOR_WHITE, label);
        }

        at += heading;

        for (index, spawn) in spawns.iter().enumerate().skip(shown.start).take(shown.len()) {
            self.fill((left, at, CONTENT_WIDTH, row), Self::stripe(index));
            self.icon(spawn.icon.as_ref(), &spawn.fallback, place(0, at, row), Seat::Floor);

            for (cell, text) in spawn.cells.iter().enumerate().take(columns.len().saturating_sub(1)) {
                self.label(place(cell + 1, at, row), Weight::Regular, GROUND_TEXT, COLOR_TEXT, text);
            }

            at += row;
        }

        at
    }
}

fn bound(sheet: &StageSheet) -> f32 {
    let rows = sheet.ground.spawns.len() + sheet.tables.iter().map(|held| held.rows.len() + 2).sum::<usize>();
    let chunks = sheet.materials.as_ref().map_or(0, |held| held.chunks.len() + 1);
    let lines: usize = sheet.notes.iter().map(|note| note.lines.iter().map(|line| line.len() / BOUND_WRAP + 2).sum::<usize>() + 3).sum();

    BOUND_BASE + rows as f32 * (ICON + ROW_PAD_Y * 2.0) + chunks as f32 * BOUND_CHUNK + lines as f32 * TEXT_SIZE * LINE_HEIGHT
}

#[derive(Clone)]
enum Part {
    Whole,
    Head,
    Ground(Range<usize>, bool),
}

fn faces<'a>(fonts: &StageFonts<'a>) -> Result<Faces<'a>, String> {
    let face = |bytes, name: &str| FontRef::try_from_slice(bytes).map_err(|err| format!("Failed to load the {name} font: {err}"));

    Ok(Faces {
        regular: face(fonts.regular, "regular")?,
        bold: face(fonts.bold, "bold")?,
        symbols: face(fonts.symbols, "symbol")?,
        fallbacks: fonts.fallbacks.iter().filter_map(|bytes| FontRef::try_from_slice(bytes).ok()).collect(),
    })
}

fn drawn(sheet: &StageSheet, faces: Faces<'_>, part: &Part) -> RgbaImage {
    let width = px(CONTENT_WIDTH + CONTENT_PADDING * 2.0) as u32;
    let mut page = Page { image: RgbaImage::from_pixel(width, px(bound(sheet)) as u32, COLOR_BACKGROUND), faces };
    let left = CONTENT_PADDING;
    let mut at = page.banners(&sheet.map, &sheet.stage, left, CONTENT_PADDING) + HEAD_GAP;
    let everything = 0..sheet.ground.spawns.len();

    page.rule(left, at, CONTENT_WIDTH);
    at += RULE + HEAD_GAP;
    at = page.crowns(&sheet.crowns, left, at) + HEAD_GAP;

    if !matches!(part, Part::Ground(..)) {
        at = page.information(sheet, left, at) + BLOCK_GAP;

        if sheet.materials.is_some() || !sheet.tables.is_empty() {
            at = page.drops(sheet, left, at) + BLOCK_GAP;
        }
    }

    at = match part {
        Part::Whole => page.ground(sheet, &everything, true, left, at) + CONTENT_PADDING,
        Part::Ground(shown, noted) => page.ground(sheet, shown, *noted, left, at) + CONTENT_PADDING,
        Part::Head => at - BLOCK_GAP + CONTENT_PADDING,
    };

    let height = (px(at) as u32).min(page.image.height());
    let content = imageops::crop_imm(&page.image, 0, 0, width, height).to_image();
    let thick = px(FRAME_THICKNESS);
    let margin = thick + px(FRAME_PADDING);
    let outer = (width + margin as u32 * 2, height + margin as u32 * 2);
    let mut framed = RgbaImage::new(outer.0, outer.1);
    let shape = CellShape::new(Rect::at(0, 0).of_size(outer.0, outer.1), px(FRAME_RADIUS) + thick, thick);

    draw_bordered_rect_mut(&mut framed, shape, COLOR_BACKGROUND, COLOR_FRAME);
    imageops::overlay(&mut framed, &content, i64::from(margin), i64::from(margin));

    framed
}

pub fn build_stage_image(sheet: &StageSheet, fonts: &StageFonts<'_>) -> Result<RgbaImage, String> {
    Ok(drawn(sheet, faces(fonts)?, &Part::Whole))
}

fn chunks(rows: usize, first: usize, later: usize) -> Vec<Range<usize>> {
    let mut held = Vec::new();
    let mut at = 0;

    while at < rows {
        let room = if held.is_empty() { first } else { later }.max(1);
        let end = (at + room).min(rows);

        held.push(at..end);
        at = end;
    }

    held
}

pub fn build_stage_pages(sheet: &StageSheet, fonts: &StageFonts<'_>, limit: u32) -> Result<Vec<RgbaImage>, String> {
    let whole = drawn(sheet, faces(fonts)?, &Part::Whole);

    if whole.height() <= limit || sheet.ground.spawns.is_empty() {
        return Ok(vec![whole]);
    }

    let row = px(ICON + ROW_PAD_Y * 2.0).max(1) as u32;
    let bare = |noted: bool| -> Result<u32, String> { Ok(drawn(sheet, faces(fonts)?, &Part::Ground(0..0, noted)).height()) };
    let room = |taken: u32| (limit.saturating_sub(taken) / row) as usize;
    let mut pages = vec![drawn(sheet, faces(fonts)?, &Part::Head)];

    for (index, shown) in chunks(sheet.ground.spawns.len(), room(bare(true)?), room(bare(false)?)).into_iter().enumerate() {
        pages.push(drawn(sheet, faces(fonts)?, &Part::Ground(shown, index == 0)));
    }

    Ok(pages)
}

#[cfg(feature = "disk")]
pub fn stage_file_name(sheet: &StageSheet, page: Option<usize>) -> String {
    let part = page.map_or_else(String::new, |page| format!(".{}", page + 1));

    format!("{}.{}crown{part}.stage.png", sheet.id, u32::from(sheet.crown) + 1)
}

#[cfg(feature = "disk")]
pub fn save_stage_to_disk(image: &RgbaImage, sheet: &StageSheet) -> Result<PathBuf, String> {
    let folder = Path::new("exports");

    std::fs::create_dir_all(folder).map_err(|err| err.to_string())?;

    let target = folder.join(stage_file_name(sheet, None));

    image.save(&target).map_err(|err| err.to_string())?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Columns share the width by their portions, with the gaps taken out first, the way the app's rows do.
    #[test]
    fn portions_split_the_width_after_the_gaps() {
        let placed = portions(&[1, 1, 2], 110.0, 5.0);

        assert_eq!(placed, [(0.0, 25.0), (30.0, 25.0), (60.0, 50.0)]);
        assert_eq!(faded(Rgba([200, 200, 200, 255]), 0.5, Rgba([100, 100, 100, 255])), Rgba([150, 150, 150, 255]));
    }

    // A long enemy list is cut so the first page, which also carries the notes, may hold fewer rows than the rest, and no row is lost or repeated.
    #[test]
    fn rows_are_cut_into_pages_in_order() {
        assert_eq!(chunks(10, 3, 4), [0..3, 3..7, 7..10]);
        assert_eq!(chunks(2, 5, 5).first(), Some(&(0..2)));
        assert_eq!(chunks(3, 0, 0), [0..1, 1..2, 2..3], "a page always takes at least one row");
        assert!(chunks(0, 3, 3).is_empty());
    }
}
