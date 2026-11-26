#[cfg(feature = "disk")]
use std::fs;
#[cfg(feature = "disk")]
use std::path::{Path, PathBuf};

use ab_glyph::FontRef;
use image::{Rgba, RgbaImage};
use imageproc::rect::Rect;
use codex::combat::Identity;
use codex::files::img015;

#[cfg(feature = "disk")]
use crate::common::assets;
use crate::common::frames;
use crate::common::formats::SpriteSheet;
use crate::systems::combat::{AbilityItem, CustomIcon, Mark, TRAITS_PER_ROW};

use super::draw::*;
use super::board::*;
use super::{SpiritData, StatCell, StatblockData};

const COLOR_CELL_HEADER: Rgba<u8> = Rgba([15, 15, 15, 255]);
const COLOR_CELL_VALUE: Rgba<u8> = Rgba([58, 58, 58, 255]);
const COLOR_CELL_BORDER: Rgba<u8> = Rgba([42, 42, 42, 255]);
const COLOR_CARD: Rgba<u8> = Rgba([7, 7, 7, 255]);

const CELL_WIDTH: f32 = 82.0;
const CELL_HEIGHT: f32 = 28.0;
const CELL_GAP: f32 = 4.0;
const CELL_TEXT_SIZE: f32 = 12.5 * TEXT_SCALE;
const CELL_BORDER_WIDTH: f32 = 2.0;

const ABILITY_ICON_SIZE: f32 = 40.0;
const ABILITY_TEXT_SIZE: f32 = 13.0 * TEXT_SCALE;
const ABILITY_TEXT_GAP: f32 = 8.0;
const ABILITY_LINE_GAP: f32 = -2.0;
const ABILITY_OVERFLOW_PADDING: f32 = 4.5;
const ICON_GAP_X: f32 = 3.2;
const ICON_GAP_Y: f32 = 5.5;
const ICON_TRAIT_GAP_Y: f32 = 7.0;

const CANVAS_ROWS: f32 = 4000.0;

const CARD_PADDING: f32 = 8.0;
const SPIRIT_PADDING_X: f32 = 8.0;

pub struct Fonts<'a> {
    pub jp: &'a [u8],
    pub kr: &'a [u8],
    pub tc: &'a [u8],
    pub th: &'a [u8],
}

#[cfg(feature = "disk")]
impl Fonts<'static> {
    pub fn embedded() -> Self {
        Self { jp: assets::FONT_JP, kr: assets::FONT_KR, tc: assets::FONT_TC, th: assets::FONT_TH }
    }
}

pub struct PageFonts<'a> {
    pub text: Fonts<'a>,
    pub bold: &'a [u8],
}

#[cfg(feature = "disk")]
impl PageFonts<'static> {
    pub fn embedded() -> Self {
        Self { text: Fonts::embedded(), bold: assets::FONT_JP_BOLD }
    }
}

pub fn build_statblock_image(
    priority: &[String],
    layers: &[SpriteSheet],
    data: StatblockData,
    fonts: &Fonts<'_>,
) -> Result<RgbaImage, String> {
    let scale = RENDER_SCALE;
    let scale_f = scale as f32;

    let padding = px(CONTENT_PADDING);
    let section_gap = px(SECTION_GAP);
    let cell_width = px(CELL_WIDTH);
    let cell_height = px(CELL_HEIGHT);
    let cell_gap = px(CELL_GAP);
    let cell_radius = px(RADIUS_SM);
    let cell_border = px(CELL_BORDER_WIDTH);
    let icon_size = px(ABILITY_ICON_SIZE);
    let icon_gap_x = px(ICON_GAP_X);
    let icon_gap_y = px(ICON_GAP_Y);
    let trait_gap_y = px(ICON_TRAIT_GAP_Y);
    let text_gap_x = px(ABILITY_TEXT_GAP);
    let card_padding = px(CARD_PADDING);

    let head = Head::load(data.icon.as_ref());

    let max_cols = data.headers_1.len().max(data.headers_2.len()) as f32;
    let base_grid_width =
        (CONTENT_PADDING * 2.0) + (CELL_WIDTH * max_cols) + (CELL_GAP * (max_cols - 1.0).max(0.0));
    let header_width = head.width();

    let faces = Faces::load(fonts)?;
    let jp_font = &faces.jp;
    let font = faces.named(priority, &data.name);
    let line_font = |line: &str| -> &FontRef { if covers(font, line) { font } else { jp_font } };

    let measure_style = SuperscriptStyle::new(ABILITY_TEXT_SIZE, 1.0, COLOR_SUPERSCRIPT);
    let ability_style = SuperscriptStyle::new(ABILITY_TEXT_SIZE, scale_f, COLOR_SUPERSCRIPT);
    let cell_style = SuperscriptStyle::new(CELL_TEXT_SIZE, scale_f, COLOR_SUPERSCRIPT);

    let icon_row_width = |held: usize| -> f32 {
        if held == 0 { return 0.0; }
        (CONTENT_PADDING * 2.0) + (held as f32 * (ABILITY_ICON_SIZE + ICON_GAP_X)) - ICON_GAP_X
    };
    let check_icon_row_width = |items: &Vec<AbilityItem>| -> f32 { icon_row_width(items.len()) };

    let calc_spirit_width = |spirit: &SpiritData| -> f32 {
        let mut spirit_max = 0.0_f32;
        let start_x = CONTENT_PADDING;
        let text_x = start_x + ABILITY_ICON_SIZE + ABILITY_TEXT_GAP;

        for line in spirit.dmg_text.split('\n') {
            spirit_max = spirit_max.max(text_x + measure_style.measure(line_font(line), line) as f32);
        }

        for spirit_item in spirit.b1.iter().chain(spirit.b2.iter()) {
            for line in spirit_item.text.split('\n') {
                spirit_max = spirit_max.max(text_x + measure_style.measure(line_font(line), line) as f32);
            }
        }

        for spirit_items in [&spirit.traits, &spirit.h1, &spirit.h2, &spirit.footer] {
            if spirit_items.is_empty() { continue; }
            let icon_width = start_x + (spirit_items.len() as f32 * (ABILITY_ICON_SIZE + ICON_GAP_X)) - ICON_GAP_X;
            spirit_max = spirit_max.max(icon_width);
        }
        spirit_max
    };

    let mut list_max_width = 0.0_f32;
    for item in data.b1.iter().chain(data.b2.iter()) {
        let mut max_line_width = 0.0_f32;
        for line in item.text.split('\n') {
            max_line_width = max_line_width.max(measure_style.measure(line_font(line), line) as f32);
        }

        let mut container_width =
            CONTENT_PADDING + ABILITY_ICON_SIZE + ABILITY_TEXT_GAP + max_line_width + CONTENT_PADDING;

        if item.icon_id == Some(img015::ICON_CONJURE)
            && let Some(spirit) = &data.spirit_data {
            container_width = container_width.max(CONTENT_PADDING + calc_spirit_width(spirit) + SPIRIT_PADDING_X);
        }
        list_max_width = list_max_width.max(container_width);
    }

    let mut max_needed_width = base_grid_width;
    max_needed_width = max_needed_width.max(header_width + tag_width(font, &data.tag));
    max_needed_width = max_needed_width.max(list_max_width);
    max_needed_width = max_needed_width.max(icon_row_width(data.traits.len().min(TRAITS_PER_ROW)));
    max_needed_width = max_needed_width.max(check_icon_row_width(&data.h1));
    max_needed_width = max_needed_width.max(check_icon_row_width(&data.h2));
    max_needed_width = max_needed_width.max(check_icon_row_width(&data.footer));

    let canvas_width = (max_needed_width.ceil() as i32) * scale;
    let title = Title { name: &data.name, id: &data.id_str, top: Some((&data.top_label, &data.top_value)) };
    let mut board = Board::open(max_needed_width, CANVAS_ROWS, &head, font, &title);

    let room = board.tag(font, &data.tag);

    board.orbs(&data.orbs, room);

    let custom_assets = custom_icons();
    let mut current_y_global = board.at;
    let target_image = &mut board.image;

    let row_pitch = cell_height + cell_gap;
    let r1_hy = current_y_global;
    let r1_dy = current_y_global + row_pitch;
    let r2_hy = current_y_global + row_pitch * 2;
    let r2_dy = current_y_global + row_pitch * 3;

    let render_row = |ui_img: &mut RgbaImage, headers: &[String], row_data: &[StatCell], marks: &[Mark], h_y: i32, d_y: i32| {
        for col in 0..headers.len() {
            let current_x = padding + ((col as i32) * (cell_width + cell_gap));

            let h_cell = CellShape::new(Rect::at(current_x, h_y).of_size(cell_width as u32, cell_height as u32), cell_radius, cell_border);
            draw_bordered_rect_mut(ui_img, h_cell, COLOR_CELL_HEADER, COLOR_CELL_BORDER);
            draw_centered_text(ui_img, COLOR_TEXT, h_cell, cell_style.base, font, &headers[col]);

            let d_cell = CellShape::new(Rect::at(current_x, d_y).of_size(cell_width as u32, cell_height as u32), cell_radius, cell_border);
            draw_bordered_rect_mut(ui_img, d_cell, COLOR_CELL_VALUE, COLOR_CELL_BORDER);

            match &row_data[col] {
                StatCell::Frames(frames) => draw_centered_superscript(
                    ui_img, COLOR_TEXT, d_cell, &cell_style, font, &frame_text(*frames),
                ),
                StatCell::Text(text) => draw_centered_text(ui_img, COLOR_TEXT, d_cell, cell_style.base, font, text),
            }

            if let Some(colors) = marks.get(col).copied().and_then(mark_colors) {
                let pair = ring(cell_width as u32, (d_y + cell_height - h_y) as u32, cell_radius, px(MARK_WIDTH), colors);

                image::imageops::overlay(ui_img, &pair, current_x as i64, h_y as i64);
            }
        }
    };

    render_row(target_image, &data.headers_1, &data.data_1, &data.marks_1, r1_hy, r1_dy);
    render_row(target_image, &data.headers_2, &data.data_2, &data.marks_2, r2_hy, r2_dy);

    current_y_global += row_pitch * 3 + cell_height + section_gap;

    let ability_line_height = px(ABILITY_TEXT_SIZE + ABILITY_LINE_GAP);
    let ability_offset = line_offset(font, ability_style.base, ability_line_height);

    let draw_icon_row = |canvas_image: &mut RgbaImage, items: &Vec<AbilityItem>, start_y: i32, start_x: i32, most: usize| -> i32 {
        if items.is_empty() { return start_y; }
        let mut current_x = start_x;
        let mut current_y = start_y;
        for (index, ability_item) in items.iter().enumerate() {
            if current_x + icon_size > canvas_width - padding || (index > 0 && index % most == 0) {
                current_x = start_x;
                current_y += icon_size + icon_gap_y;
            }
            let icon_surface = get_icon_image(ability_item, layers, &custom_assets, icon_size as u32);
            image::imageops::overlay(canvas_image, &icon_surface, current_x as i64, current_y as i64);
            current_x += icon_size + icon_gap_x;
        }
        current_y + icon_size
    };

    let overflow_padding = |text: &str| -> i32 {
        let block_height = text.split('\n').count() as i32 * ability_line_height;

        if block_height > icon_size { px(ABILITY_OVERFLOW_PADDING) } else { 0 }
    };

    let row_height = |text: &str| -> i32 { icon_size + overflow_padding(text) * 2 };

    let draw_text_block = |canvas_image: &mut RgbaImage, text: &str, icon_x: i32, icon_y: i32| {
        let lines: Vec<&str> = text.split('\n').collect();
        let block_height = lines.len() as i32 * ability_line_height;
        let mut line_y = icon_y + (icon_size - block_height) / 2;

        for line in lines {
            ability_style.draw(canvas_image, COLOR_TEXT, icon_x + icon_size + text_gap_x, line_y + ability_offset, line_font(line), line);
            line_y += ability_line_height;
        }
    };

    let draw_spirit_icons = |spirit_image: &mut RgbaImage, spirit_items: &[AbilityItem], start_y: i32, start_x_absolute: i32| -> i32 {
        if spirit_items.is_empty() { return start_y; }
        let mut current_x = start_x_absolute;
        for spirit_item in spirit_items {
            let icon_surface = get_icon_image(spirit_item, layers, &custom_assets, icon_size as u32);
            image::imageops::overlay(spirit_image, &icon_surface, current_x as i64, start_y as i64);
            current_x += icon_size + icon_gap_x;
        }
        start_y + icon_size
    };

    let draw_spirit_list = |spirit_image: &mut RgbaImage, spirit_items: &[AbilityItem], start_y: i32, start_x_absolute: i32| -> i32 {
        if spirit_items.is_empty() { return start_y; }
        let mut current_y = start_y;
        for (index, spirit_item) in spirit_items.iter().enumerate() {
            let icon_surface = get_icon_image(spirit_item, layers, &custom_assets, icon_size as u32);
            let icon_y = current_y + overflow_padding(&spirit_item.text);
            image::imageops::overlay(spirit_image, &icon_surface, start_x_absolute as i64, icon_y as i64);

            draw_text_block(spirit_image, &spirit_item.text, start_x_absolute, icon_y);
            current_y += row_height(&spirit_item.text);
            if index < spirit_items.len() - 1 { current_y += icon_gap_y; }
        }
        current_y
    };

    let draw_spirit_card = |canvas_image: &mut RgbaImage, spirit: &SpiritData, card_start_y: i32| -> i32 {
        let card_inner_y = card_start_y + icon_gap_y;
        let start_x_absolute = padding + card_padding;
        let spirit_panel_width = px(calc_spirit_width(spirit) + SPIRIT_PADDING_X);

        let mut final_panel_height = card_padding;
        final_panel_height += row_height(&spirit.dmg_text) + icon_gap_y;

        let mut has_previous_section = false;
        let mut last_section_was_trait = false;

        let add_gap = |total_height: &mut i32, has_previous_element: &mut bool, current_is_trait: bool, was_last_element_trait: &mut bool| {
            if *has_previous_element { *total_height += if *was_last_element_trait { trait_gap_y } else { icon_gap_y }; }
            *has_previous_element = true;
            *was_last_element_trait = current_is_trait;
        };

        if !spirit.traits.is_empty() { final_panel_height += icon_size; has_previous_section = true; last_section_was_trait = true; }
        if !spirit.h1.is_empty() { add_gap(&mut final_panel_height, &mut has_previous_section, false, &mut last_section_was_trait); final_panel_height += icon_size; }
        if !spirit.h2.is_empty() { add_gap(&mut final_panel_height, &mut has_previous_section, false, &mut last_section_was_trait); final_panel_height += icon_size; }

        if !spirit.b1.is_empty() || !spirit.b2.is_empty() {
            add_gap(&mut final_panel_height, &mut has_previous_section, false, &mut last_section_was_trait);
            let calc_list_height = |items: &[AbilityItem]| -> i32 {
                let gaps = (items.len() as i32 - 1).max(0) * icon_gap_y;

                items.iter().map(|list_item| row_height(&list_item.text)).sum::<i32>() + gaps
            };
            if !spirit.b1.is_empty() { final_panel_height += calc_list_height(&spirit.b1); }
            if !spirit.b1.is_empty() && !spirit.b2.is_empty() { final_panel_height += icon_gap_y; }
            if !spirit.b2.is_empty() { final_panel_height += calc_list_height(&spirit.b2); }
        }

        if !spirit.footer.is_empty() { add_gap(&mut final_panel_height, &mut has_previous_section, false, &mut last_section_was_trait); final_panel_height += icon_size; }
        final_panel_height += card_padding;

        let spirit_rect = Rect::at(padding, card_inner_y).of_size(spirit_panel_width as u32, final_panel_height as u32);
        draw_bottom_rounded_rect_mut(canvas_image, spirit_rect, px(RADIUS_LG), COLOR_CARD);

        let mut current_y_offset = card_inner_y + card_padding;
        let area_item = AbilityItem { identity: Identity::AreaAttack, icon_id: Some(img015::ICON_AREA_ATTACK), border_id: None, custom_icon: CustomIcon::None, text: String::new(), mark: Mark::None, dimmed: false, switchable: false };
        let area_icon = get_icon_image(&area_item, layers, &custom_assets, icon_size as u32);
        let area_icon_y = current_y_offset + overflow_padding(&spirit.dmg_text);
        image::imageops::overlay(canvas_image, &area_icon, start_x_absolute as i64, area_icon_y as i64);

        draw_text_block(canvas_image, &spirit.dmg_text, start_x_absolute, area_icon_y);
        current_y_offset += row_height(&spirit.dmg_text) + icon_gap_y;

        has_previous_section = false;
        last_section_was_trait = false;

        if !spirit.traits.is_empty() { current_y_offset = draw_spirit_icons(canvas_image, &spirit.traits, current_y_offset, start_x_absolute); has_previous_section = true; last_section_was_trait = true; }
        if !spirit.h1.is_empty() { add_gap(&mut current_y_offset, &mut has_previous_section, false, &mut last_section_was_trait); current_y_offset = draw_spirit_icons(canvas_image, &spirit.h1, current_y_offset, start_x_absolute); }
        if !spirit.h2.is_empty() { add_gap(&mut current_y_offset, &mut has_previous_section, false, &mut last_section_was_trait); current_y_offset = draw_spirit_icons(canvas_image, &spirit.h2, current_y_offset, start_x_absolute); }

        if !spirit.b1.is_empty() || !spirit.b2.is_empty() {
            add_gap(&mut current_y_offset, &mut has_previous_section, false, &mut last_section_was_trait);
            if !spirit.b1.is_empty() { current_y_offset = draw_spirit_list(canvas_image, &spirit.b1, current_y_offset, start_x_absolute); }
            if !spirit.b1.is_empty() && !spirit.b2.is_empty() { current_y_offset += icon_gap_y; }
            if !spirit.b2.is_empty() { current_y_offset = draw_spirit_list(canvas_image, &spirit.b2, current_y_offset, start_x_absolute); }
        }

        if !spirit.footer.is_empty() { add_gap(&mut current_y_offset, &mut has_previous_section, false, &mut last_section_was_trait); draw_spirit_icons(canvas_image, &spirit.footer, current_y_offset, start_x_absolute); }

        card_inner_y + final_panel_height
    };

    let draw_list = |canvas_image: &mut RgbaImage, items: &Vec<AbilityItem>, start_y: i32| -> i32 {
        if items.is_empty() { return start_y; }
        let mut current_y = start_y;
        for (index, item) in items.iter().enumerate() {
            let icon_surface = get_icon_image(item, layers, &custom_assets, icon_size as u32);
            let icon_y = current_y + overflow_padding(&item.text);
            image::imageops::overlay(canvas_image, &icon_surface, padding as i64, icon_y as i64);

            draw_text_block(canvas_image, &item.text, padding, icon_y);
            current_y += row_height(&item.text);

            if item.icon_id == Some(img015::ICON_CONJURE)
                && let Some(spirit) = &data.spirit_data {
                current_y = draw_spirit_card(canvas_image, spirit, current_y);
            }

            if index < items.len() - 1 { current_y += icon_gap_y; }
        }
        current_y
    };

    let mut previously_drew_section = false;
    let mut last_main_section_was_trait = false;

    let draw_section_gap = |current_y: &mut i32, has_previous_element: &mut bool, current_is_trait: bool, was_last_element_trait: &mut bool| {
        if *has_previous_element { *current_y += if *was_last_element_trait { trait_gap_y } else { icon_gap_y }; }
        *has_previous_element = true;
        *was_last_element_trait = current_is_trait;
    };

    if !data.traits.is_empty() {
        current_y_global = draw_icon_row(target_image, &data.traits, current_y_global, padding, TRAITS_PER_ROW);
        previously_drew_section = true;
        last_main_section_was_trait = true;
    }
    if !data.h1.is_empty() {
        draw_section_gap(&mut current_y_global, &mut previously_drew_section, false, &mut last_main_section_was_trait);
        current_y_global = draw_icon_row(target_image, &data.h1, current_y_global, padding, usize::MAX);
    }
    if !data.h2.is_empty() {
        draw_section_gap(&mut current_y_global, &mut previously_drew_section, false, &mut last_main_section_was_trait);
        current_y_global = draw_icon_row(target_image, &data.h2, current_y_global, padding, usize::MAX);
    }
    if !data.b1.is_empty() || !data.b2.is_empty() {
        draw_section_gap(&mut current_y_global, &mut previously_drew_section, false, &mut last_main_section_was_trait);
        if !data.b1.is_empty() { current_y_global = draw_list(target_image, &data.b1, current_y_global); }
        if !data.b1.is_empty() && !data.b2.is_empty() { current_y_global += icon_gap_y; }
        if !data.b2.is_empty() { current_y_global = draw_list(target_image, &data.b2, current_y_global); }
    }
    if !data.footer.is_empty() {
        draw_section_gap(&mut current_y_global, &mut previously_drew_section, false, &mut last_main_section_was_trait);
        current_y_global = draw_icon_row(target_image, &data.footer, current_y_global, padding, usize::MAX);
    }

    board.at = current_y_global;

    Ok(board.close())
}

fn frame_text(count: i32) -> String {
    frames::label(count)
}

#[cfg(feature = "disk")]
pub fn save_page_to_disk(image: &RgbaImage, file_name: &str) -> Result<PathBuf, String> {
    let export_dir = Path::new("exports");
    fs::create_dir_all(export_dir).map_err(|err| err.to_string())?;

    let filename = export_dir.join(file_name);

    image.save(&filename).map_err(|err| err.to_string())?;
    Ok(filename)
}

#[cfg(feature = "disk")]
pub fn save_to_disk(image: &RgbaImage, is_cat: bool, id_str: &str, top_value: &str) -> Result<PathBuf, String> {
    let safe_val_str = top_value.replace(|c: char| !c.is_alphanumeric() && c != '+', "");
    let prefix = if is_cat { "Lv" } else { "Mag" };

    save_page_to_disk(image, &format!("{}.{}{}.statblock.png", id_str, prefix, safe_val_str))
}
