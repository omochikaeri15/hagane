use ab_glyph::{Font, FontRef, PxScale};
use image::imageops;
use image::{Rgba, RgbaImage};
use imageproc::drawing::{draw_filled_rect_mut, draw_text_mut, text_size};
use imageproc::rect::Rect;

use crate::Source;
use crate::common::gfx::{autocrop, open_image};
use crate::systems::combat::Mark;
use crate::systems::statblock::OrbFace;

use super::draw::*;
use super::render::Fonts;

pub(super) const COLOR_BACKGROUND: Rgba<u8> = Rgba([33, 33, 33, 255]);
pub(super) const COLOR_TEXT: Rgba<u8> = Rgba([230, 230, 230, 255]);
pub(super) const COLOR_TEXT_WEAK: Rgba<u8> = Rgba([112, 112, 112, 255]);
pub(super) const COLOR_SUPERSCRIPT: Rgba<u8> = Rgba([171, 171, 171, 255]);
pub(super) const COLOR_RULE: Rgba<u8> = Rgba([71, 71, 71, 255]);
pub(super) const COLOR_INPUT: Rgba<u8> = Rgba([17, 17, 17, 255]);
pub(super) const COLOR_INPUT_BORDER: Rgba<u8> = Rgba([71, 71, 71, 255]);
const COLOR_FRAME: Rgba<u8> = Rgba([31, 106, 165, 255]);

pub(super) const RADIUS_SM: f32 = 4.0;
pub(super) const RADIUS_MD: f32 = 6.0;
pub(super) const RADIUS_LG: f32 = 8.0;

pub(super) const RENDER_SCALE: i32 = 2;
pub(super) const TEXT_SCALE: f32 = 1.5;
pub(super) const CONTENT_PADDING: f32 = 8.0;
pub(super) const SECTION_GAP: f32 = 8.0;
pub(super) const RULE_THICKNESS: f32 = 1.0;
pub(super) const BORDER_WIDTH: f32 = 1.0;

const ICON_BOX_WIDTH: f32 = 110.0;
const ICON_BOX_HEIGHT: f32 = 83.0;
const HEADER_GAP_X: f32 = 12.0;
pub(super) const HEADER_HEIGHT: f32 = ICON_BOX_HEIGHT;

const NAME_WRAP_WIDTH: f32 = 145.0;
const NAME_MAX_FONT_SIZE: f32 = 22.0 * TEXT_SCALE;
const NAME_MIN_FONT_SIZE: f32 = 8.0 * TEXT_SCALE;
const NAME_LINE_GAP: f32 = -4.0;
const NAME_TO_ID_GAP: f32 = -1.5;
const NAME_FONT_STEP: f32 = 0.5;
const NAME_MAX_LINES: usize = 2;

const INFO_TEXT_SIZE: f32 = 11.0 * TEXT_SCALE;
const LEVEL_ROW_GAP: f32 = 6.0;
const LEVEL_INPUT_MIN_WIDTH: f32 = 28.0;
const LEVEL_INPUT_PADDING_X: f32 = 7.0;
const LEVEL_INPUT_PADDING_Y: f32 = 2.0;

const LINE_HEIGHT: f32 = 1.3;
const HEADING_SIZE: f32 = 18.0;
const HEADING_GAP: f32 = 6.0;
const SECTION_BREAK: f32 = 14.0;
const RAISED_SHADE: f32 = 0.75;
const GROWTH: u32 = 2;

const ORB_SIZE: f32 = 34.0;
const ORB_GAP: f32 = 4.0;
const DARKENED: f32 = 0.4;

const FRAME_THICKNESS: f32 = 7.0;
const FRAME_PADDING: f32 = 4.0;
const FRAME_RADIUS: f32 = RADIUS_MD;

pub(super) fn px(value: f32) -> i32 {
    (value * RENDER_SCALE as f32).round() as i32
}

pub(super) fn covers(font: &FontRef<'_>, text: &str) -> bool {
    text.chars().all(|glyph| glyph.is_ascii() || glyph.is_whitespace() || font.glyph_id(glyph).0 != 0)
}

pub(super) struct Faces<'a> {
    pub(super) jp: FontRef<'a>,
    kr: FontRef<'a>,
    tc: FontRef<'a>,
    th: FontRef<'a>,
}

impl<'a> Faces<'a> {
    pub(super) fn load(fonts: &Fonts<'a>) -> Result<Self, String> {
        let face = |bytes: &'a [u8], name: &str| FontRef::try_from_slice(bytes).map_err(|err| format!("Failed to load {name} font: {err}"));

        Ok(Self { jp: face(fonts.jp, "JP")?, kr: face(fonts.kr, "KR")?, tc: face(fonts.tc, "TC")?, th: face(fonts.th, "TH")? })
    }

    fn spoken(&self, lang: &str) -> &FontRef<'a> {
        match lang {
            "ko" | "kr" => &self.kr,
            "tw" | "zh" | "zh-tw" => &self.tc,
            "th" => &self.th,
            _ => &self.jp,
        }
    }

    pub(super) fn named(&self, priority: &[String], name: &str) -> &FontRef<'a> {
        priority
            .iter()
            .map(|lang| self.spoken(lang))
            .find(|candidate| covers(candidate, name))
            .unwrap_or_else(|| self.spoken(priority.first().map_or("en", String::as_str)))
    }
}

pub(super) struct Title<'a> {
    pub(super) name: &'a str,
    pub(super) id: &'a str,
    pub(super) top: Option<(&'a str, &'a str)>,
}

pub(super) struct Head {
    icon: Option<RgbaImage>,
    column: i32,
}

impl Head {
    pub(super) fn load(icon: Option<&Source>) -> Self {
        let icon_box_width = px(ICON_BOX_WIDTH);
        let icon_box_height = px(ICON_BOX_HEIGHT);

        let icon = icon.and_then(open_image).map(|icon_img| {
            let rgba = autocrop(icon_img.to_rgba8());
            let (source_w, source_h) = (rgba.width() as f32, rgba.height() as f32);

            let scale = (icon_box_width as f32 / source_w).min(icon_box_height as f32 / source_h);
            let target_w = ((source_w * scale).round() as u32).max(1);
            let target_h = ((source_h * scale).round() as u32).max(1);

            if rgba.width() == target_w && rgba.height() == target_h {
                return rgba;
            }

            image::imageops::resize(&rgba, target_w, target_h, image::imageops::FilterType::Lanczos3)
        });
        let column = icon.as_ref().map_or(icon_box_width, |icon| icon_box_width.max(icon.width() as i32));

        Self { icon, column }
    }

    pub(super) fn width(&self) -> f32 {
        (CONTENT_PADDING * 2.0) + HEADER_GAP_X + NAME_WRAP_WIDTH + (self.column as f32 / RENDER_SCALE as f32)
    }

    pub(super) fn draw(&self, target_image: &mut RgbaImage, font: &FontRef<'_>, title: &Title<'_>) {
        let scale_f = RENDER_SCALE as f32;
        let padding = px(CONTENT_PADDING);
        let header_height = px(ICON_BOX_HEIGHT);
        let cell_radius = px(RADIUS_SM);

        let info_scale = PxScale::from(INFO_TEXT_SIZE * scale_f);
        let info_line_height = px(INFO_TEXT_SIZE);
        let input_padding_x = px(LEVEL_INPUT_PADDING_X);
        let input_height = info_line_height + px(LEVEL_INPUT_PADDING_Y) * 2;
        let level_row_height = if title.top.is_some() { input_height.max(info_line_height) } else { 0 };

        let name_to_id_gap = px(NAME_TO_ID_GAP);
        let name_box_height = (header_height - name_to_id_gap - info_line_height - level_row_height)
            .max(px(NAME_MIN_FONT_SIZE));

        let mut name_size = NAME_MAX_FONT_SIZE;
        let mut name_lines = wrap_text(title.name, font, PxScale::from(name_size * scale_f), NAME_WRAP_WIDTH * scale_f);
        let name_overflows = |lines: usize, size: f32| -> bool {
            lines > NAME_MAX_LINES || lines as i32 * px(size + NAME_LINE_GAP) > name_box_height
        };

        while name_overflows(name_lines.len(), name_size) && name_size > NAME_MIN_FONT_SIZE {
            name_size -= NAME_FONT_STEP;
            name_lines = wrap_text(title.name, font, PxScale::from(name_size * scale_f), NAME_WRAP_WIDTH * scale_f);
        }

        let name_scale = PxScale::from(name_size * scale_f);
        let name_line_height = px(name_size + NAME_LINE_GAP);
        let name_block_height = name_lines.len() as i32 * name_line_height;

        let header_top = padding;

        if let Some(icon) = &self.icon {
            let x_offset = padding as i64 + ((self.column - icon.width() as i32) / 2) as i64;
            let seated = (header_height - icon.height() as i32).max(0);

            image::imageops::overlay(target_image, icon, x_offset, (header_top + seated) as i64);
        }

        let text_start_x = padding + self.column + px(HEADER_GAP_X);
        let info_top = header_top;

        let mut name_y = info_top + (name_box_height - name_block_height).max(0) / 2;
        let name_offset = line_offset(font, name_scale, name_line_height);
        for line in &name_lines {
            draw_text_mut(target_image, COLOR_TEXT, text_start_x, name_y + name_offset, name_scale, font, line);
            name_y += name_line_height;
        }

        let info_offset = line_offset(font, info_scale, info_line_height);
        let id_top = info_top + name_box_height + name_to_id_gap;
        draw_text_mut(
            target_image, COLOR_TEXT_WEAK, text_start_x, id_top + info_offset,
            info_scale, font, &format!("ID: {}", title.id),
        );

        let Some((top_label, top_value)) = title.top else {
            return;
        };

        let level_top = id_top + info_line_height;
        draw_text_mut(
            target_image, COLOR_TEXT, text_start_x,
            level_top + (level_row_height - info_line_height) / 2 + info_offset,
            info_scale, font, top_label,
        );

        let (prefix_width, _) = text_size(info_scale, font, top_label);
        let (value_width, _) = text_size(info_scale, font, top_value);
        let input_width = (value_width as i32 + input_padding_x * 2).max(px(LEVEL_INPUT_MIN_WIDTH));
        let input_x = text_start_x + prefix_width as i32 + px(LEVEL_ROW_GAP);
        let input_y = level_top + (level_row_height - input_height) / 2;
        let input_rect = Rect::at(input_x, input_y).of_size(input_width as u32, input_height as u32);

        let input_cell = CellShape::new(input_rect, cell_radius, px(BORDER_WIDTH));
        draw_bordered_rect_mut(target_image, input_cell, COLOR_INPUT, COLOR_INPUT_BORDER);
        draw_centered_text(target_image, COLOR_TEXT, input_cell, info_scale, font, top_value);
    }
}

pub(super) fn tag_width(font: &FontRef<'_>, text: &str) -> f32 {
    if text.is_empty() {
        return 0.0;
    }

    let scale = PxScale::from(INFO_TEXT_SIZE * RENDER_SCALE as f32);
    let widest = text.lines().map(|line| text_size(scale, font, line).0).max().unwrap_or(0);

    widest as f32 / RENDER_SCALE as f32 + LEVEL_INPUT_PADDING_X * 2.0 + HEADER_GAP_X
}

pub(super) fn framed(content: &RgbaImage) -> RgbaImage {
    let border_thick = px(FRAME_THICKNESS);
    let margin = border_thick + px(FRAME_PADDING);

    let final_width_with_pad = content.width() + (margin * 2) as u32;
    let final_height_with_pad = content.height() + (margin * 2) as u32;
    let mut final_background_layer = RgbaImage::new(final_width_with_pad, final_height_with_pad);

    let border_radius = px(FRAME_RADIUS);
    let outer_rect = Rect::at(0, 0).of_size(final_width_with_pad, final_height_with_pad);

    draw_bordered_rect_mut(
        &mut final_background_layer,
        CellShape::new(outer_rect, border_radius + border_thick, border_thick),
        COLOR_BACKGROUND, COLOR_FRAME,
    );

    image::imageops::overlay(&mut final_background_layer, content, margin as i64, margin as i64);

    final_background_layer
}

pub(super) fn mixed(top: Rgba<u8>, alpha: f32, under: Rgba<u8>) -> Rgba<u8> {
    let channel = |over: u8, below: u8| (f32::from(over) * alpha + f32::from(below) * (1.0 - alpha)).round() as u8;

    Rgba([channel(top[0], under[0]), channel(top[1], under[1]), channel(top[2], under[2]), 255])
}

pub(super) fn picture(source: &Source) -> Option<RgbaImage> {
    let cropped = autocrop(open_image(source)?.to_rgba8());

    (cropped.width() > 1 || cropped.height() > 1).then_some(cropped)
}

fn scaled(picture: RgbaImage, ratio: f32) -> RgbaImage {
    let size = (((picture.width() as f32 * ratio).round() as u32).max(1), ((picture.height() as f32 * ratio).round() as u32).max(1));

    if size == picture.dimensions() { picture } else { safe_resize(picture, size.0, size.1) }
}

pub(super) fn fitted(picture: RgbaImage, height: f32) -> RgbaImage {
    let ratio = px(height).max(1) as f32 / picture.height().max(1) as f32;

    scaled(picture, ratio)
}

pub(super) fn boxed(picture: RgbaImage, canvas: f32, side: f32) -> RgbaImage {
    let longest = picture.width().max(picture.height()).max(1) as f32;
    let ratio = (canvas / longest).min(1.0) * px(side) as f32 / canvas;

    scaled(picture, ratio)
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Weight {
    Regular,
    Bold,
}

#[derive(Clone, Copy)]
pub(super) struct Type<'a, 'f> {
    pub(super) text: &'a FontRef<'f>,
    pub(super) spare: &'a FontRef<'f>,
    pub(super) bold: &'a FontRef<'f>,
}

impl<'a, 'f> Type<'a, 'f> {
    fn face(&self, weight: Weight, line: &str) -> &'a FontRef<'f> {
        let plain = if covers(self.text, line) { self.text } else { self.spare };

        if weight == Weight::Bold && covers(self.bold, line) { self.bold } else { plain }
    }

    fn style(size: f32, color: Rgba<u8>) -> SuperscriptStyle {
        SuperscriptStyle::new(size * TEXT_SCALE, RENDER_SCALE as f32, mixed(color, RAISED_SHADE, Rgba([0, 0, 0, 255])))
    }

    pub(super) fn line(size: f32) -> f32 {
        size * LINE_HEIGHT
    }

    pub(super) fn width(&self, weight: Weight, size: f32, line: &str) -> f32 {
        (Self::style(size, COLOR_TEXT).measure(self.face(weight, line), line) as f32 / RENDER_SCALE as f32).ceil()
    }

    pub(super) fn widest<'t>(&self, weight: Weight, size: f32, lines: impl IntoIterator<Item = &'t str>) -> f32 {
        lines.into_iter().map(|line| self.width(weight, size, line)).fold(0.0, f32::max)
    }
}

pub(super) struct Board {
    pub(super) image: RgbaImage,
    pub(super) at: i32,
    sections: usize,
}

impl Board {
    pub(super) fn open(width: f32, rows: f32, head: &Head, font: &FontRef<'_>, title: &Title<'_>) -> Self {
        let wide = (width.ceil() as i32 * RENDER_SCALE) as u32;
        let mut image = RgbaImage::from_pixel(wide, px(rows) as u32, COLOR_BACKGROUND);
        let padding = px(CONTENT_PADDING);
        let rule = padding + px(HEADER_HEIGHT) + px(SECTION_GAP);

        head.draw(&mut image, font, title);
        draw_filled_rect_mut(&mut image, Rect::at(padding, rule).of_size(wide - padding as u32 * 2, px(RULE_THICKNESS) as u32), COLOR_RULE);

        Self { image, at: rule + px(RULE_THICKNESS) + px(SECTION_GAP), sections: 0 }
    }

    pub(super) fn tag(&mut self, font: &FontRef<'_>, text: &str) -> i32 {
        if text.is_empty() {
            return 0;
        }

        let scale = PxScale::from(INFO_TEXT_SIZE * RENDER_SCALE as f32);
        let padding = px(CONTENT_PADDING);
        let line = px(INFO_TEXT_SIZE);
        let lines: Vec<&str> = text.lines().collect();
        let widest = lines.iter().map(|held| text_size(scale, font, held).0).max().unwrap_or(0);
        let wide = widest as i32 + px(LEVEL_INPUT_PADDING_X) * 2;
        let tall = line * lines.len() as i32 + px(LEVEL_INPUT_PADDING_Y) * 2;
        let left = self.image.width() as i32 - padding - wide;
        let cell = CellShape::new(Rect::at(left, padding).of_size(wide.max(1) as u32, tall.max(1) as u32), px(RADIUS_SM), px(BORDER_WIDTH));

        draw_bordered_rect_mut(&mut self.image, cell, COLOR_INPUT, COLOR_INPUT_BORDER);

        for (row, held) in lines.iter().enumerate() {
            let top = padding + px(LEVEL_INPUT_PADDING_Y) + line * row as i32;
            let band = CellShape::new(Rect::at(left, top).of_size(wide.max(1) as u32, line.max(1) as u32), 0, 0);

            draw_centered_text(&mut self.image, COLOR_TEXT, band, scale, font, held);
        }

        wide
    }

    pub(super) fn orbs(&mut self, faces: &[OrbFace], room: i32) {
        let Some(count) = i32::try_from(faces.len()).ok().filter(|count| *count > 0) else {
            return;
        };
        let natural = px(ORB_SIZE) * count + px(ORB_GAP) * (count - 1);
        let room = if room > 0 { room.min(natural) } else { natural };
        let size = px(ORB_SIZE).min(room);
        let step = if count > 1 { (room - size) / (count - 1) } else { 0 };
        let padding = px(CONTENT_PADDING);
        let top = padding + px(HEADER_HEIGHT) - size;
        let left = self.image.width() as i32 - padding - room;

        for (at, face) in faces.iter().enumerate() {
            let mut picture = safe_resize(face.image.clone(), size.max(1) as u32, size.max(1) as u32);

            if face.dimmed {
                for pixel in picture.pixels_mut() {
                    for channel in &mut pixel.0[..3] {
                        *channel = (f32::from(*channel) * DARKENED) as u8;
                    }
                }
            }

            imageops::overlay(&mut self.image, &picture, i64::from(left + step * at as i32), i64::from(top));
        }

        let Some(colors) = mark_colors(Mark::Hit) else {
            return;
        };
        let lit: Vec<i32> = faces.iter().enumerate().filter(|(_, face)| face.lit).map(|(at, _)| step * at as i32).collect();
        let edge = size.max(1) as u32;

        if count > 1 && lit.len() == faces.len() {
            imageops::overlay(&mut self.image, &ring(room.max(1) as u32, edge, px(RADIUS_SM), px(MARK_WIDTH), colors), i64::from(left), i64::from(top));
            return;
        }

        for shift in lit {
            imageops::overlay(&mut self.image, &ring(edge, edge, px(RADIUS_SM), px(MARK_WIDTH), colors), i64::from(left + shift), i64::from(top));
        }
    }

    pub(super) fn close(self) -> RgbaImage {
        let height = ((self.at + px(CONTENT_PADDING)).max(1) as u32).min(self.image.height());

        framed(&imageops::crop_imm(&self.image, 0, 0, self.image.width(), height).to_image())
    }

    pub(super) fn width(&self) -> f32 {
        self.image.width() as f32 / RENDER_SCALE as f32
    }

    pub(super) fn span(&self) -> f32 {
        self.width() - CONTENT_PADDING * 2.0
    }

    pub(super) fn top(&self) -> f32 {
        self.at as f32 / RENDER_SCALE as f32
    }

    pub(super) fn advance(&mut self, by: f32) {
        self.at += px(by);
    }

    fn reach(&mut self, bottom: i32) {
        let needed = bottom.max(0) as u32;

        if needed <= self.image.height() {
            return;
        }

        let mut grown = RgbaImage::from_pixel(self.image.width(), needed.max(self.image.height() * GROWTH), COLOR_BACKGROUND);

        imageops::replace(&mut grown, &self.image, 0, 0);
        self.image = grown;
    }

    pub(super) fn section(&mut self, kind: &Type<'_, '_>, title: Option<&str>) {
        if self.sections > 0 {
            self.advance(SECTION_BREAK);
        }

        self.sections += 1;

        let Some(title) = title else {
            return;
        };

        self.write(kind, (CONTENT_PADDING, self.top()), Weight::Regular, HEADING_SIZE, COLOR_TEXT, title);
        self.advance(Type::line(HEADING_SIZE) + HEADING_GAP);
    }

    pub(super) fn heading_width(kind: &Type<'_, '_>, title: &str) -> f32 {
        kind.width(Weight::Regular, HEADING_SIZE, title)
    }

    pub(super) fn write(&mut self, kind: &Type<'_, '_>, at: (f32, f32), weight: Weight, size: f32, color: Rgba<u8>, line: &str) {
        let font = kind.face(weight, line);
        let style = Type::style(size, color);
        let tall = px(Type::line(size));

        self.reach(px(at.1) + tall);
        style.draw(&mut self.image, color, px(at.0), px(at.1) + line_offset(font, style.base, tall), font, line);
    }

    pub(super) fn paragraph<'t>(&mut self, kind: &Type<'_, '_>, size: f32, lines: impl IntoIterator<Item = &'t str>) {
        for line in lines {
            self.write(kind, (CONTENT_PADDING, self.top()), Weight::Regular, size, COLOR_TEXT, line);
            self.advance(Type::line(size));
        }
    }

    fn rect(frame: (f32, f32, f32, f32)) -> Rect {
        Rect::at(px(frame.0), px(frame.1)).of_size((px(frame.0 + frame.2) - px(frame.0)).max(1) as u32, (px(frame.1 + frame.3) - px(frame.1)).max(1) as u32)
    }

    pub(super) fn fill(&mut self, frame: (f32, f32, f32, f32), radius: f32, color: Rgba<u8>) {
        self.reach(px(frame.1 + frame.3));
        draw_rounded_rect_mut(&mut self.image, Self::rect(frame), px(radius), color);
    }

    pub(super) fn outline(&mut self, frame: (f32, f32, f32, f32), radius: f32, border: f32, fill: Rgba<u8>, edge: Rgba<u8>) {
        self.reach(px(frame.1 + frame.3));
        draw_bordered_rect_mut(&mut self.image, CellShape::new(Self::rect(frame), px(radius), px(border)), fill, edge);
    }

    pub(super) fn shade(&mut self, frame: (f32, f32, f32, f32), radius: f32, color: Rgba<u8>) {
        let rect = Self::rect(frame);
        let mut layer = RgbaImage::new(rect.width(), rect.height());

        draw_rounded_rect_mut(&mut layer, Rect::at(0, 0).of_size(rect.width(), rect.height()), px(radius), color);
        self.reach(px(frame.1 + frame.3));
        imageops::overlay(&mut self.image, &layer, i64::from(rect.left()), i64::from(rect.top()));
    }

    pub(super) fn stamp(&mut self, picture: &RgbaImage, frame: (f32, f32, f32, f32)) {
        let left = px(frame.0) + (px(frame.2) - picture.width() as i32) / 2;
        let top = px(frame.1) + (px(frame.3) - picture.height() as i32) / 2;

        self.reach(px(frame.1 + frame.3));
        imageops::overlay(&mut self.image, picture, i64::from(left), i64::from(top));
    }
}
