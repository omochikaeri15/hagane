use std::collections::HashMap;

use ab_glyph::FontRef;
use codex::files::img022;
use image::{Rgba, RgbaImage};

use crate::common::formats::SpriteSheet;
use crate::systems::combat::CustomIcon;

use super::board::*;
use super::draw::{custom_icons, get_icon_image};
use super::render::PageFonts;
use super::{TalentCard, TalentsData};

const COLOR_WHITE: Rgba<u8> = Rgba([255, 255, 255, 255]);
const COLOR_BLACK: Rgba<u8> = Rgba([0, 0, 0, 255]);
const COLOR_NORMAL: Rgba<u8> = Rgba([181, 140, 20, 255]);
const COLOR_ULTRA: Rgba<u8> = Rgba([120, 20, 20, 255]);
const BOX_ALPHA: f32 = 0.4;
const IDLE_ALPHA: f32 = 0.45;

const MIN_WIDTH: f32 = 442.0;
const CANVAS_ROWS: f32 = 1200.0;
const CARD_GAP: f32 = 8.0;
const CARD_PAD: f32 = 6.0;
const CARD_RADIUS: f32 = 5.0;
const ICON_SIZE: f32 = 30.0;
const PLATE_HEIGHT: f32 = 26.0;
const TEXT_SIZE: f32 = 13.0;
const CELL_GAP: f32 = 8.0;
const ROW_GAP: f32 = 4.0;
const BOX_PAD: f32 = 4.0;
const BOX_RADIUS: f32 = 4.0;
const HEAD_HEIGHT: f32 = ICON_SIZE + BOX_PAD * 2.0;
const LABEL_GAP: f32 = 8.0;
const LEVEL_GAP: f32 = 6.0;
const LEVEL_WIDTH: f32 = 28.0;
const LEVEL_PAD_X: f32 = 7.0;
const LEVEL_PAD_Y: f32 = 3.0;
const COIN_SIZE: f32 = 18.0;
const COIN_GAP: f32 = 4.0;
const TOTAL_COIN: f32 = 24.0;
const TOTAL_SIZE: f32 = 20.0;
const TOTAL_GAP: f32 = 6.0;
const TOTAL_ROOM: f32 = 16.0;

const LEVEL: &str = "LEVEL";
const COST: &str = "NP COST";
const SPLIT: &str = ": ";

struct Card<'a> {
    held: &'a TalentCard,
    icon: RgbaImage,
    plate: Option<RgbaImage>,
    tint: Rgba<u8>,
}

#[derive(Clone, Copy)]
struct Columns {
    level: f32,
    cost: f32,
}

struct Coins {
    small: Option<RgbaImage>,
    large: Option<RgbaImage>,
}

fn logical(picture: &RgbaImage) -> f32 {
    picture.width() as f32 / RENDER_SCALE as f32
}

fn level_field(kind: &Type<'_, '_>, level: u8) -> f32 {
    (kind.width(Weight::Bold, TEXT_SIZE, &level.to_string()) + LEVEL_PAD_X * 2.0).max(LEVEL_WIDTH)
}

fn limit(max_level: u8) -> String {
    format!("/ {max_level}")
}

fn level_box(kind: &Type<'_, '_>, held: &TalentCard) -> f32 {
    BOX_PAD * 2.0 + kind.width(Weight::Bold, TEXT_SIZE, LEVEL) + LEVEL_GAP + level_field(kind, held.level) + LEVEL_GAP + kind.width(Weight::Bold, TEXT_SIZE, &limit(held.max_level))
}

fn cost_text(coin: Option<&RgbaImage>, cost: i32) -> String {
    if coin.is_some() { cost.to_string() } else { format!("{COST}{SPLIT}{cost}") }
}

fn cost_box(kind: &Type<'_, '_>, coin: Option<&RgbaImage>, cost: i32) -> f32 {
    BOX_PAD * 2.0 + coin.map_or(0.0, |coin| logical(coin) + COIN_GAP) + kind.width(Weight::Bold, TEXT_SIZE, &cost_text(coin, cost))
}

fn name_width(kind: &Type<'_, '_>, card: &Card<'_>) -> f32 {
    card.plate.as_ref().map_or_else(|| kind.width(Weight::Bold, TEXT_SIZE, &card.held.item.text), logical)
}

fn row_width(kind: &Type<'_, '_>, line: &str) -> f32 {
    let inner = match line.split_once(SPLIT) {
        Some((label, reading)) => kind.width(Weight::Regular, TEXT_SIZE, label) + LABEL_GAP + kind.width(Weight::Bold, TEXT_SIZE, reading),
        None => kind.width(Weight::Bold, TEXT_SIZE, line),
    };

    inner + BOX_PAD * 2.0
}

fn card_width(kind: &Type<'_, '_>, card: &Card<'_>, columns: Columns) -> f32 {
    let head = ICON_SIZE + CELL_GAP + name_width(kind, card) + CELL_GAP * 2.0 + columns.level + CELL_GAP + columns.cost;
    let rows = card.held.lines.iter().map(|line| row_width(kind, line)).fold(0.0, f32::max);
    let told = kind.widest(Weight::Regular, TEXT_SIZE, card.held.description.lines()) + BOX_PAD * 2.0;

    head.max(rows).max(told) + CARD_PAD * 2.0
}

fn centered(top: f32, tall: f32, size: f32) -> f32 {
    top + (tall - Type::line(size)) / 2.0
}

fn card(board: &mut Board, kind: &Type<'_, '_>, held: &Card<'_>, coin: Option<&RgbaImage>, columns: Columns) {
    let top = board.top();
    let left = CONTENT_PADDING;
    let wide = board.span();
    let rows = held.held.lines.len() as f32;
    let row_tall = Type::line(TEXT_SIZE) + BOX_PAD * 2.0;
    let told = held.held.description.lines().count() as f32;
    let told_tall = if told > 0.0 { told * Type::line(TEXT_SIZE) + BOX_PAD * 2.0 + ROW_GAP } else { 0.0 };
    let tall = CARD_PAD * 2.0 + HEAD_HEIGHT + told_tall + rows * (row_tall + ROW_GAP);
    let dark = mixed(COLOR_BLACK, BOX_ALPHA, held.tint);
    let head_top = top + CARD_PAD;
    let text_top = centered(head_top, HEAD_HEIGHT, TEXT_SIZE);

    board.fill((left, top, wide, tall), CARD_RADIUS, held.tint);
    board.stamp(&held.icon, (left + CARD_PAD, head_top, ICON_SIZE, HEAD_HEIGHT));

    let name_left = left + CARD_PAD + ICON_SIZE + CELL_GAP;

    match &held.plate {
        Some(plate) => board.stamp(plate, (name_left, head_top, logical(plate), HEAD_HEIGHT)),
        None => board.write(kind, (name_left, text_top), Weight::Bold, TEXT_SIZE, COLOR_WHITE, &held.held.item.text),
    }

    let cost_left = left + wide - CARD_PAD - columns.cost;
    let mut cost_at = cost_left + BOX_PAD + (columns.cost - cost_box(kind, coin, held.held.cost)) / 2.0;

    board.fill((cost_left, head_top, columns.cost, HEAD_HEIGHT), BOX_RADIUS, dark);

    if let Some(coin) = coin {
        board.stamp(coin, (cost_at, head_top, logical(coin), HEAD_HEIGHT));
        cost_at += logical(coin) + COIN_GAP;
    }

    board.write(kind, (cost_at, text_top), Weight::Bold, TEXT_SIZE, COLOR_WHITE, &cost_text(coin, held.held.cost));

    let level_left = cost_left - CELL_GAP - columns.level;
    let label_left = level_left + BOX_PAD + (columns.level - level_box(kind, held.held)) / 2.0;
    let field_wide = level_field(kind, held.held.level);
    let field_tall = Type::line(TEXT_SIZE) + LEVEL_PAD_Y * 2.0;
    let field_left = label_left + kind.width(Weight::Bold, TEXT_SIZE, LEVEL) + LEVEL_GAP;
    let number = held.held.level.to_string();

    board.fill((level_left, head_top, columns.level, HEAD_HEIGHT), BOX_RADIUS, dark);
    board.write(kind, (label_left, text_top), Weight::Bold, TEXT_SIZE, COLOR_WHITE, LEVEL);
    board.outline((field_left, head_top + (HEAD_HEIGHT - field_tall) / 2.0, field_wide, field_tall), RADIUS_SM, BORDER_WIDTH, COLOR_INPUT, COLOR_INPUT_BORDER);
    board.write(kind, (field_left + (field_wide - kind.width(Weight::Bold, TEXT_SIZE, &number)) / 2.0, text_top), Weight::Bold, TEXT_SIZE, COLOR_TEXT, &number);
    board.write(kind, (field_left + field_wide + LEVEL_GAP, text_top), Weight::Bold, TEXT_SIZE, COLOR_WHITE, &limit(held.held.max_level));

    let mut row_top = head_top + HEAD_HEIGHT + ROW_GAP;

    if told > 0.0 {
        board.fill((left + CARD_PAD, row_top, wide - CARD_PAD * 2.0, told_tall - ROW_GAP), BOX_RADIUS, dark);

        for (index, line) in held.held.description.lines().enumerate() {
            board.write(kind, (left + CARD_PAD + BOX_PAD, row_top + BOX_PAD + index as f32 * Type::line(TEXT_SIZE)), Weight::Regular, TEXT_SIZE, COLOR_WHITE, line);
        }

        row_top += told_tall;
    }

    for line in &held.held.lines {
        let text_top = row_top + BOX_PAD;
        let text_left = left + CARD_PAD + BOX_PAD;

        board.fill((left + CARD_PAD, row_top, row_width(kind, line), row_tall), BOX_RADIUS, dark);

        match line.split_once(SPLIT) {
            Some((label, reading)) => {
                board.write(kind, (text_left, text_top), Weight::Regular, TEXT_SIZE, COLOR_WHITE, label);
                board.write(kind, (text_left + kind.width(Weight::Regular, TEXT_SIZE, label) + LABEL_GAP, text_top), Weight::Bold, TEXT_SIZE, COLOR_WHITE, reading);
            }
            None => board.write(kind, (text_left, text_top), Weight::Bold, TEXT_SIZE, COLOR_WHITE, line),
        }

        row_top += row_tall + ROW_GAP;
    }

    board.advance(tall);
}

fn coins(sheets: &[SpriteSheet]) -> Coins {
    let found = sheets.iter().find_map(|sheet| sheet.crop(img022::ICON_NP_COST));
    let sized = |tall: f32| found.as_ref().map(|coin| fitted(RgbaImage::clone(coin), tall));

    Coins { small: sized(COIN_SIZE), large: sized(TOTAL_COIN) }
}

fn tint(held: &TalentCard) -> Rgba<u8> {
    let base = if held.ultra { COLOR_ULTRA } else { COLOR_NORMAL };

    if held.level == 0 { mixed(base, IDLE_ALPHA, COLOR_BACKGROUND) } else { base }
}

pub fn build_talents_image(
    priority: &[String],
    layers: &[SpriteSheet],
    coin_layers: &[SpriteSheet],
    data: &TalentsData,
    fonts: &PageFonts<'_>,
) -> Result<RgbaImage, String> {
    let faces = Faces::load(&fonts.text)?;
    let bold = FontRef::try_from_slice(fonts.bold).map_err(|err| format!("Failed to load the bold font: {err}"))?;
    let font = faces.named(priority, &data.head.name);
    let kind = Type { text: font, spare: &faces.jp, bold: &bold };
    let head = Head::load(data.head.icon.as_ref());
    let custom: HashMap<CustomIcon, RgbaImage> = custom_icons();
    let coins = coins(coin_layers);

    let cards: Vec<Card<'_>> = data
        .cards
        .iter()
        .map(|held| Card {
            held,
            icon: get_icon_image(&held.item, layers, &custom, px(ICON_SIZE) as u32),
            plate: held.plate.as_ref().and_then(picture).map(|found| fitted(found, PLATE_HEIGHT)),
            tint: tint(held),
        })
        .collect();

    let total = data.cost.to_string();
    let total_wide = coins.large.as_ref().map_or(0.0, |coin| logical(coin) + TOTAL_GAP) + kind.width(Weight::Bold, TOTAL_SIZE, &total);
    let columns = Columns {
        level: cards.iter().map(|held| level_box(&kind, held.held)).fold(0.0, f32::max),
        cost: cards.iter().map(|held| cost_box(&kind, coins.small.as_ref(), held.held.cost)).fold(0.0, f32::max),
    };
    let widest = cards.iter().map(|held| card_width(&kind, held, columns)).fold(0.0, f32::max);
    let width = (widest + CONTENT_PADDING * 2.0).max(head.width() + TOTAL_ROOM + total_wide).max(MIN_WIDTH);
    let title = Title { name: &data.head.name, id: &data.head.id_str, top: data.head.top.as_ref().map(|(label, value)| (label.as_str(), value.as_str())) };
    let mut board = Board::open(width, CANVAS_ROWS, &head, font, &title);

    let total_top = CONTENT_PADDING + HEADER_HEIGHT - TOTAL_COIN;
    let mut total_left = width - CONTENT_PADDING - total_wide;

    if let Some(coin) = &coins.large {
        board.stamp(coin, (total_left, total_top, logical(coin), TOTAL_COIN));
        total_left += logical(coin) + TOTAL_GAP;
    }

    board.write(&kind, (total_left, centered(total_top, TOTAL_COIN, TOTAL_SIZE)), Weight::Bold, TOTAL_SIZE, COLOR_WHITE, &total);
    board.section(&kind, None);

    for held in &cards {
        card(&mut board, &kind, held, coins.small.as_ref(), columns);
        board.advance(CARD_GAP);
    }

    board.advance(-CARD_GAP);

    Ok(board.close())
}
