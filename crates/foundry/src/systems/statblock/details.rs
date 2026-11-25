use ab_glyph::FontRef;
use image::{Rgba, RgbaImage};

use super::board::*;
use super::render::PageFonts;
use super::{ComboCard, DetailsData, EvolveData};

const COLOR_WHITE: Rgba<u8> = Rgba([255, 255, 255, 255]);
const COLOR_CARD: Rgba<u8> = Rgba([7, 7, 7, 255]);
const COLOR_CARD_EDGE: Rgba<u8> = Rgba([122, 122, 122, 255]);
const COLOR_EMPTY: Rgba<u8> = Rgba([45, 45, 45, 255]);
const COLOR_BADGE: Rgba<u8> = Rgba([0, 0, 0, 161]);

const MIN_WIDTH: f32 = 442.0;
const CANVAS_ROWS: f32 = 1200.0;
const BODY_SIZE: f32 = 14.0;
const BLOCK_GAP: f32 = 8.0;

const ITEM_SIZE: f32 = 64.0;
const ITEM_CANVAS: f32 = 128.0;
const ITEM_GAP: f32 = 5.0;
const AMOUNT_SIZE: f32 = 12.0;
const BADGE_PAD_X: f32 = 4.0;
const BADGE_PAD_Y: f32 = 1.0;
const COST_ICON: f32 = 28.0;
const COST_SIZE: f32 = 16.0;
const COST_GAP: f32 = 5.0;

const SEATS: usize = 5;
const SEAT_SIZE: f32 = 58.0;
const SEAT_CANVAS: f32 = 68.0;
const SEAT_GAP: f32 = 4.0;
const SEAT_LABEL: f32 = 10.0;
const COMBO_NAME: f32 = 15.0;
const COMBO_EFFECT: f32 = 12.0;
const COMBO_NOTE: f32 = 10.0;
const COMBO_LINE_GAP: f32 = 2.0;
const COMBO_GAP: f32 = 6.0;
const COMBO_PAD_X: f32 = 12.0;
const COMBO_PAD_Y: f32 = 6.0;
const COMBO_BODY_GAP: f32 = 14.0;
const COMBO_EDGE: f32 = 1.5;

const DESCRIPTION: &str = "Description";
const EVOLVE: &str = "Evolve";
const COMBOS: &str = "Cat Combo";

struct Evolved {
    items: Vec<Option<RgbaImage>>,
    cost: Option<RgbaImage>,
}

struct Combo<'a> {
    card: &'a ComboCard,
    seats: Vec<Option<RgbaImage>>,
}

impl Combo<'_> {
    fn lines(&self) -> impl Iterator<Item = (&str, Weight, f32)> {
        let effect = Some(self.card.effect.as_str()).filter(|effect| !effect.trim().is_empty());

        std::iter::once((self.card.name.as_str(), Weight::Bold, COMBO_NAME))
            .chain(effect.map(|effect| (effect, Weight::Regular, COMBO_EFFECT)))
            .chain(self.card.note.as_deref().map(|note| (note, Weight::Regular, COMBO_NOTE)))
    }

    fn text_width(&self, kind: &Type<'_, '_>) -> f32 {
        self.lines().map(|(line, weight, size)| kind.width(weight, size, line)).fold(0.0, f32::max)
    }

    fn text_height(&self) -> f32 {
        self.lines().map(|(_, _, size)| Type::line(size) + COMBO_LINE_GAP).sum::<f32>() - COMBO_LINE_GAP
    }
}

fn seats_width() -> f32 {
    SEATS as f32 * (SEAT_SIZE + SEAT_GAP) - SEAT_GAP
}

fn items_width(count: usize) -> f32 {
    (count as f32 * (ITEM_SIZE + ITEM_GAP) - ITEM_GAP).max(0.0)
}

fn amount(value: i32) -> String {
    format!("\u{00d7}{value}")
}

fn badge(board: &mut Board, kind: &Type<'_, '_>, text: &str, size: f32, right: f32, bottom: f32) {
    let wide = kind.width(Weight::Regular, size, text) + BADGE_PAD_X * 2.0;
    let tall = Type::line(size) + BADGE_PAD_Y * 2.0;
    let frame = (right - wide, bottom - tall, wide, tall);

    board.shade(frame, RADIUS_SM, COLOR_BADGE);
    board.write(kind, (frame.0 + BADGE_PAD_X, frame.1 + BADGE_PAD_Y), Weight::Regular, size, COLOR_WHITE, text);
}

fn evolve(board: &mut Board, kind: &Type<'_, '_>, held: &EvolveData, drawn: &Evolved) {
    board.section(kind, Some(EVOLVE));

    if !held.lines.is_empty() {
        board.paragraph(kind, BODY_SIZE, held.lines.iter().map(String::as_str));
        board.advance(BLOCK_GAP);
    }

    if !held.items.is_empty() {
        let top = board.top();

        for (index, (item, icon)) in held.items.iter().zip(&drawn.items).enumerate() {
            let frame = (CONTENT_PADDING + index as f32 * (ITEM_SIZE + ITEM_GAP), top, ITEM_SIZE, ITEM_SIZE);

            match icon {
                Some(icon) => board.stamp(icon, frame),
                None => {
                    board.fill(frame, RADIUS_SM, COLOR_EMPTY);
                    board.write(kind, (frame.0 + BADGE_PAD_X, frame.1 + BADGE_PAD_X), Weight::Regular, AMOUNT_SIZE, COLOR_TEXT, &item.fallback);
                }
            }

            badge(board, kind, &amount(item.amount), AMOUNT_SIZE, frame.0 + ITEM_SIZE, top + ITEM_SIZE);
        }

        board.advance(ITEM_SIZE + BLOCK_GAP);
    }

    if let Some(cost) = &held.cost {
        let top = board.top();
        let icon_wide = drawn.cost.as_ref().map_or(0.0, |icon| icon.width() as f32 / RENDER_SCALE as f32 + COST_GAP);

        if let Some(icon) = &drawn.cost {
            board.stamp(icon, (CONTENT_PADDING, top, icon_wide - COST_GAP, COST_ICON));
        }

        board.write(kind, (CONTENT_PADDING + icon_wide, top + (COST_ICON - Type::line(COST_SIZE)) / 2.0), Weight::Bold, COST_SIZE, COLOR_WHITE, &cost.amount.to_string());
        board.advance(COST_ICON + BLOCK_GAP);
    }

    board.advance(-BLOCK_GAP);
}

fn combo(board: &mut Board, kind: &Type<'_, '_>, held: &Combo<'_>) {
    let top = board.top();
    let tall = SEAT_SIZE + COMBO_PAD_Y * 2.0;
    let wide = board.span();
    let seats_left = CONTENT_PADDING + wide - COMBO_PAD_X - seats_width();
    let text_room = seats_left - COMBO_BODY_GAP - CONTENT_PADDING - COMBO_PAD_X;
    let mut line_top = top + (tall - held.text_height()) / 2.0;

    board.outline((CONTENT_PADDING, top, wide, tall), RADIUS_MD, COMBO_EDGE, COLOR_CARD, COLOR_CARD_EDGE);

    for (line, weight, size) in held.lines() {
        let left = CONTENT_PADDING + COMBO_PAD_X + (text_room - kind.width(weight, size, line)) / 2.0;

        board.write(kind, (left, line_top), weight, size, COLOR_TEXT, line);
        line_top += Type::line(size) + COMBO_LINE_GAP;
    }

    for (index, (seat, icon)) in held.card.seats.iter().zip(&held.seats).enumerate() {
        let frame = (seats_left + index as f32 * (SEAT_SIZE + SEAT_GAP), top + COMBO_PAD_Y, SEAT_SIZE, SEAT_SIZE);

        match icon {
            Some(icon) => board.stamp(icon, frame),
            None => board.fill(frame, RADIUS_SM, COLOR_EMPTY),
        }

        if let Some(label) = &seat.label {
            let wide = kind.width(Weight::Regular, SEAT_LABEL, label) + BADGE_PAD_X * 2.0;
            let tall = Type::line(SEAT_LABEL) + BADGE_PAD_Y * 2.0;

            badge(board, kind, label, SEAT_LABEL, frame.0 + (SEAT_SIZE + wide) / 2.0, frame.1 + (SEAT_SIZE + tall) / 2.0);
        }
    }

    board.advance(tall);
}

pub fn build_details_image(priority: &[String], data: &DetailsData, fonts: &PageFonts<'_>) -> Result<RgbaImage, String> {
    let faces = Faces::load(&fonts.text)?;
    let bold = FontRef::try_from_slice(fonts.bold).map_err(|err| format!("Failed to load the bold font: {err}"))?;
    let font = faces.named(priority, &data.head.name);
    let kind = Type { text: font, spare: &faces.jp, bold: &bold };
    let head = Head::load(data.head.icon.as_ref());

    let seated = |source: &crate::Source, canvas: f32, side: f32| picture(source).map(|found| boxed(found, canvas, side));
    let evolved = data.evolve.as_ref().map(|held| Evolved {
        items: held.items.iter().map(|item| item.icon.as_ref().and_then(|icon| seated(icon, ITEM_CANVAS, ITEM_SIZE))).collect(),
        cost: held.cost.as_ref().and_then(|cost| cost.icon.as_ref()).and_then(picture).map(|found| fitted(found, COST_ICON)),
    });
    let combos: Vec<Combo<'_>> = data
        .combos
        .iter()
        .map(|card| Combo { card, seats: card.seats.iter().map(|seat| seat.icon.as_ref().and_then(|icon| seated(icon, SEAT_CANVAS, SEAT_SIZE))).collect() })
        .collect();

    let lines = |held: &[String]| kind.widest(Weight::Regular, BODY_SIZE, held.iter().map(String::as_str));
    let mut inner = lines(&data.description).max(Board::heading_width(&kind, DESCRIPTION));

    if let Some(held) = &data.evolve {
        inner = inner.max(lines(&held.lines)).max(items_width(held.items.len()));
    }

    if !combos.is_empty() {
        let text = combos.iter().map(|held| held.text_width(&kind)).fold(0.0, f32::max);

        inner = inner.max(COMBO_PAD_X * 2.0 + text + COMBO_BODY_GAP + seats_width());
    }

    let width = (inner + CONTENT_PADDING * 2.0).max(head.width()).max(MIN_WIDTH);
    let title = Title { name: &data.head.name, id: &data.head.id_str, top: data.head.top.as_ref().map(|(label, value)| (label.as_str(), value.as_str())) };
    let mut board = Board::open(width, CANVAS_ROWS, &head, font, &title);

    board.section(&kind, Some(DESCRIPTION));
    board.paragraph(&kind, BODY_SIZE, data.description.iter().map(String::as_str));

    if let (Some(held), Some(drawn)) = (&data.evolve, &evolved) {
        evolve(&mut board, &kind, held, drawn);
    }

    if !combos.is_empty() {
        board.section(&kind, Some(COMBOS));

        for held in &combos {
            combo(&mut board, &kind, held);
            board.advance(COMBO_GAP);
        }

        board.advance(-COMBO_GAP);
    }

    Ok(board.close())
}
