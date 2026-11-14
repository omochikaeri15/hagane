mod values;

use std::cell::RefCell;
use std::sync::LazyLock;

use iced::alignment::{Horizontal, Vertical};
use iced::widget::image::Handle;
use iced::widget::{button, column, container, image as iced_image, scrollable, text, tooltip, Column, Row};
use iced::{Element, Font, Length, Theme};
use codex::combat::{get_ability, Faction, Identity};
use rustc_hash::FxHashMap;

use foundry::common::gfx::autocrop;
use foundry::systems::combat::NameBook;
use foundry::domains::settings::EditorMode;
use foundry::{Vault, Vfs};

pub(crate) use foundry::systems::editor::figures::talents::*;

use crate::app::theme;
use crate::common::glyphs::Ruler;
use crate::widget::smooth_scroll;

use super::schema::{TALENT_HEAD, TALENT_SLOTS, TALENT_STRIDE};
use super::{cards, Draft, Frame, Message, GLYPH_RATIO};
use values::Value;

struct Heading {
    label: &'static str,
    content: f32,
}

static FLOOR: LazyLock<[f32; HEADINGS.len()]> = LazyLock::new(|| {
    HEADINGS.map(|heading| heading.content.max(wrapped(heading.label)))
});

fn widths(ability: f32) -> [f32; HEADINGS.len()] {
    let mut widths = *FLOOR;
    widths[ABILITY_COLUMN] = widths[ABILITY_COLUMN].max(ability);

    widths
}

fn table_width(widths: &[f32]) -> f32 {
    widths.iter().sum::<f32>() + COLUMN_GAP * widths.len().saturating_sub(1) as f32
}

fn wrapped(label: &str) -> f32 {
    let words: Vec<usize> = label.split_whitespace().map(|word| word.chars().count()).collect();

    let widest = (1..=words.len().min(HEADER_LINES))
        .map(|lines| split(&words, lines))
        .min()
        .unwrap_or_default();

    widest as f32 * LABEL_SIZE * GLYPH_RATIO
}

const COLUMN_GAP: f32 = 6.0;

const ROW_GAP: f32 = 2.0;

const SCROLLBAR_WIDTH: f32 = 6.0;

const SCROLLBAR_SPACING: f32 = 2.0;

const LABEL_SLACK: f32 = 1.0;

const ROW_PADDING: f32 = 4.0;

const HEADINGS: [Heading; 6 + VALUES * 2] = [
    Heading { label: "Ability", content: 100.0 },
    Heading { label: "Image", content: 52.0 },
    Heading { label: "Text ID", content: 48.0 },
    Heading { label: "Cost", content: 48.0 },
    Heading { label: "Ultra", content: 48.0 },
    Heading { label: "Total Levels", content: 48.0 },
    Heading { label: "Min Val 1", content: 40.0 },
    Heading { label: "Max Val 1", content: 40.0 },
    Heading { label: "Min Val 2", content: 40.0 },
    Heading { label: "Max Val 2", content: 40.0 },
    Heading { label: "Min Val 3", content: 40.0 },
    Heading { label: "Max Val 3", content: 40.0 },
    Heading { label: "Min Val 4", content: 40.0 },
    Heading { label: "Max Val 4", content: 40.0 },
];

#[cfg(test)]
const FRAME_BORDER: f32 = 6.0;

const LABEL_SIZE: f32 = 12.0;

const NOTICE_SIZE: f32 = 11.0;

const PREVIEW_HEIGHT: f32 = 15.0;

const TRAIT_WIDTH: f32 = 74.0;

const TRAIT_SIZE: f32 = 11.0;

const TRAIT_INSET: f32 = 3.0;

const HINT_PADDING: f32 = 6.0;

const TILE_WIDTH: f32 = 92.0;

#[cfg(test)]
fn columns() -> f32 {
    table_width(&*FLOOR) + FRAME_BORDER
}

pub(super) fn view<'a>(draft: &'a Draft, frame: Frame<'a>) -> Element<'a, Message> {
    let Frame { width, query, armed, .. } = frame;

    if draft.values() == EditorMode::Resolved {
        return resolved(draft, &frame);
    }

    raw(draft, width, query, armed)
}

fn raw<'a>(draft: &'a Draft, width: f32, query: &'a str, armed: bool) -> Element<'a, Message> {
    let filtered = query.trim().to_lowercase();

    let shown: Vec<usize> = (0..draft.len())
        .filter(|index| {
            filtered.is_empty() || draft.schema().label(*index).to_lowercase().contains(&filtered)
        })
        .collect();

    let spare = spare(draft);

    cards::shell(
        Some(cards::search(query, width, SEARCH_PLACEHOLDER)),
        cards::grid(draft, width, &shown, dim_from(spare, &shown)),
        cards::footer(vec![cards::sync(armed)]),
    )
}

fn resolved<'a>(draft: &'a Draft, frame: &Frame<'a>) -> Element<'a, Message> {
    let Frame { width, armed, names, vault, glossary, picker, hunt, .. } = *frame;
    let vfs = &vault.vfs;

    if let Some(index) = picker {
        let (listing, placeholder) = match offset(index) {
            TEXT_ID => (wording(vault, index, hunt), WORDING_HUNT),
            _ => (chooser(names, vfs, index, hunt), IMAGE_HUNT),
        };

        let top = column![back(), cards::hunt(hunt, width, placeholder)];

        return cards::shell(Some(top.into()), listing, cards::footer(vec![cards::sync(armed)]));
    }

    let filled = filled(draft);
    let widths = widths(names.ability_width(glossary));
    let mut rows = Column::new().spacing(ROW_GAP);

    for slot in 0..TALENT_SLOTS.min(filled + 1) {
        rows = rows.push(striped(talent_row(draft, slot, names, vfs, glossary, &widths), slot));
    }

    let body = container(sideways(column![headings(&widths), rows].into()))
        .height(Length::Fill)
        .center_x(Length::Fill);

    cards::shell(None, body.into(), cards::footer(vec![traits(draft, glossary), cards::sync(armed)]))
}

fn sideways<'a>(table: Element<'a, Message>) -> Element<'a, Message> {
    let rail = scrollable::Scrollbar::new().width(SCROLLBAR_WIDTH).spacing(SCROLLBAR_SPACING);
    let area = scrollable(table).direction(scrollable::Direction::Horizontal(rail)).width(Length::Shrink);

    smooth_scroll(area).into()
}

fn scrolled<'a>(rows: Column<'a, Message>) -> Element<'a, Message> {
    let area = scrollable(rows).width(Length::Fill).height(Length::Fill);

    smooth_scroll(area).into()
}

fn chooser<'a>(names: &Names, vfs: &Vfs, index: usize, hunt: &str) -> Element<'a, Message> {
    let wanted = hunt.trim();

    let mut rows = Column::new().spacing(ROW_GAP);
    let mut line = Row::new().spacing(ROW_GAP).align_y(Vertical::Center);
    let mut used = 0;

    for choice in names.choices(vfs).into_iter().filter(|choice| choice.matches(wanted)) {
        if used == CHOOSER_COLUMNS {
            rows = rows.push(centered(line.into()));
            line = Row::new().spacing(ROW_GAP).align_y(Vertical::Center);
            used = 0;
        }

        line = line.push(names.tile(vfs, choice, index));
        used += 1;
    }

    scrolled(rows.push(centered(line.into())))
}

fn striped<'a>(content: Element<'a, Message>, index: usize) -> Element<'a, Message> {
    container(content)
        .padding(ROW_PADDING)
        .style(move |theme: &Theme| theme::zebra_table_row(theme, index))
        .into()
}

fn headings<'a>(widths: &[f32; HEADINGS.len()]) -> Element<'a, Message> {
    let mut line = Row::new().spacing(COLUMN_GAP).align_y(Vertical::Center);

    for (heading, width) in HEADINGS.iter().zip(*widths) {
        line = line.push(
            container(
                text(heading.label).size(LABEL_SIZE).align_x(Horizontal::Center).width(Length::Fill),
            )
            .width(Length::Fixed(width)),
        );
    }

    container(line.width(Length::Fixed(table_width(widths))))
        .padding(ROW_PADDING)
        .style(theme::zebra_table_header)
        .into()
}

fn centered<'a>(content: Element<'a, Message>) -> Element<'a, Message> {
    container(content).width(Length::Fill).center_x(Length::Fill).into()
}

fn talent_row<'a>(
    draft: &'a Draft,
    slot: usize,
    names: &Names,
    vfs: &Vfs,
    glossary: &'a NameBook,
    widths: &[f32; HEADINGS.len()],
) -> Element<'a, Message> {
    let base = TALENT_HEAD + slot * TALENT_STRIDE;
    let ability = draft.reads_at(base + ABILITY).unwrap_or_default();
    let named = draft.reads_at(base + NAME_ID).unwrap_or(-1);

    let skills = cards::options(catalog(glossary), Skill::of(ability, glossary), move |pick: Skill| {
        Message::Picked(base + ABILITY, pick.id)
    });
    let skills = disclaimed(skills, ability, glossary);

    let picture = names.control(vfs, image_id(named, ability), named, base + NAME_ID);

    let mut cells = vec![
        skills,
        picture,
        drills(draft.reads_at(base + TEXT_ID).unwrap_or_default().to_string(), base + TEXT_ID),
        cards::number(draft, base + COST_ID),
        ultra(draft.reads_at(base + LIMIT).unwrap_or_default(), base + LIMIT),
        cards::number(draft, base + MAX_LEVEL),
    ];

    for pair in 0..VALUES {
        let low = base + MIN_1 + pair * 2;
        let meaning = values::value(ability, pair);

        cells.push(explained(cards::number(draft, low), meaning));
        cells.push(explained(cards::number(draft, low + 1), meaning));
    }

    let mut line = Row::new().spacing(COLUMN_GAP).align_y(Vertical::Center);

    for (cell, width) in cells.into_iter().zip(*widths) {
        line = line.push(container(cell).width(Length::Fixed(width)));
    }

    line.width(Length::Fixed(table_width(widths))).into()
}

fn ultra<'a>(current: i32, index: usize) -> Element<'a, Message> {
    let set = current == ULTRA;

    let style: theme::ButtonStyleFn =
        if set { theme::success_button } else { theme::neutral_button };

    let label = theme::centered_text(if set { "Yes" } else { "No" })
        .size(TRAIT_SIZE)
        .width(Length::Fill)
        .wrapping(text::Wrapping::None);

    button(label)
        .width(Length::Fill)
        .padding(TRAIT_INSET)
        .style(style)
        .on_press(Message::Picked(index, i32::from(!set)))
        .into()
}

fn disclaimed<'a>(field: Element<'a, Message>, ability: i32, glossary: &NameBook) -> Element<'a, Message> {
    let blocker = get_ability(Identity::WaveBlock).and_then(|definition| definition.talent_id);
    let immune = get_ability(Identity::ImmuneWave).and_then(|definition| definition.talent_id);

    let (Some(blocker), Some(immune)) = (blocker, immune) else {
        return field;
    };

    if ability != i32::from(blocker) {
        return field;
    }

    let blocker_ability = glossary.ability(Identity::WaveBlock, Faction::Cat);
    let immune_ability = glossary.ability(Identity::ImmuneWave, Faction::Cat);
    let mislabel = glossary.talent(blocker).unwrap_or(blocker_ability);
    let genuine = glossary.talent(immune).unwrap_or(immune_ability);
    let notice = format!(
        "Disclaimer: PONOS assigns the name \"{mislabel}\" to what the community knows as the {blocker_ability} ability\nIf you want the genuine {immune_ability} ability, please select \"{genuine}\" instead"
    );

    let bubble = container(theme::centered_text(notice).size(TRAIT_SIZE))
        .padding(HINT_PADDING)
        .style(container::bordered_box);

    tooltip(field, bubble, tooltip::Position::Top).into()
}

fn explained<'a>(field: Element<'a, Message>, meaning: Option<Value>) -> Element<'a, Message> {
    let Some(value) = meaning else {
        return field;
    };

    let bubble = container(text(value.label()).size(TRAIT_SIZE))
        .padding(HINT_PADDING)
        .style(container::bordered_box);

    tooltip(field, bubble, tooltip::Position::Top).into()
}

fn drills<'a>(label: String, index: usize) -> Element<'a, Message> {
    button(theme::centered_text(label).size(TRAIT_SIZE).width(Length::Fill).wrapping(text::Wrapping::None))
        .width(Length::Fill)
        .padding(TRAIT_INSET)
        .style(theme::neutral_button)
        .on_press(Message::Picker(Some(index)))
        .into()
}

fn wording<'a>(vault: &Vault, index: usize, hunt: &str) -> Element<'a, Message> {
    let texts = vault.vds.cats.descriptions(&vault.vfs);
    let wanted = hunt.trim().to_lowercase();
    let numeric = !wanted.is_empty() && wanted.chars().all(|glyph| glyph.is_ascii_digit());

    let mut rows = Column::new().spacing(ROW_GAP);

    for (id, body) in texts.iter().enumerate() {
        if body.trim().is_empty() {
            continue;
        }

        let kept = match (wanted.is_empty(), numeric) {
            (true, _) => true,
            (false, true) => id.to_string().contains(&wanted),
            (false, false) => body.to_lowercase().contains(&wanted),
        };

        if !kept {
            continue;
        }

        let label = text(format!("{id}  {}", body.replace('\n', " "))).size(TRAIT_SIZE);

        rows = rows.push(
            button(label)
                .width(Length::Fill)
                .padding(TRAIT_INSET)
                .style(theme::neutral_button)
                .on_press(Message::Picked(index, id as i32)),
        );
    }

    scrolled(rows)
}

fn back<'a>() -> Element<'a, Message> {
    container(
        button(theme::centered_text("Back").size(TRAIT_SIZE))
            .width(Length::Fixed(TRAIT_WIDTH))
            .padding(TRAIT_INSET)
            .style(theme::neutral_button)
            .on_press(Message::Picker(None)),
    )
    .width(Length::Fill)
    .center_x(Length::Fill)
    .padding(ROW_PADDING)
    .into()
}

fn traits<'a>(draft: &'a Draft, glossary: &'a NameBook) -> Element<'a, Message> {
    let mask = draft.reads_at(TYPE_ID).unwrap_or_default();
    let live = arming(draft);

    let mut stack = Column::new().spacing(ROW_GAP).align_x(Horizontal::Center);
    let mut buttons = Row::new().spacing(ROW_GAP).align_y(Vertical::Center);
    let mut used = 0;

    for (bit, identity) in TRAITS.into_iter().enumerate() {
        if used == TRAITS.len() / 2 {
            stack = stack.push(buttons);
            buttons = Row::new().spacing(ROW_GAP).align_y(Vertical::Center);
            used = 0;
        }

        buttons = buttons.push(trait_button(bit, glossary.trait_label(identity), mask, live));
        used += 1;
    }

    let title = if live {
        text(TYPE_ID_TITLE).size(NOTICE_SIZE)
    } else {
        text(IDLE_TYPE_ID).size(NOTICE_SIZE).style(text::secondary)
    };

    column![title, stack.push(buttons)].spacing(ROW_GAP).align_x(Horizontal::Center).into()
}

fn trait_button<'a>(bit: usize, name: &'a str, mask: i32, live: bool) -> Element<'a, Message> {
    let set = mask & (1 << bit) != 0;

    let style: theme::ButtonStyleFn = match (live, set) {
        (false, _) => theme::inert_button,
        (true, true) => theme::success_button,
        (true, false) => theme::neutral_button,
    };

    let label =
        theme::centered_text(name).size(TRAIT_SIZE).width(Length::Fill).wrapping(text::Wrapping::None);

    let control = button(label).width(Length::Fixed(TRAIT_WIDTH)).padding(TRAIT_INSET).style(style);

    if live {
        return control.on_press(Message::Picked(TYPE_ID, mask ^ (1 << bit))).into();
    }

    control.into()
}

#[derive(Default)]
pub(super) struct Names {
    listed: RefCell<Option<Vec<Named>>>,
    images: RefCell<FxHashMap<i32, Option<Handle>>>,
    ability: RefCell<Option<f32>>,
}

impl Names {
    pub(super) fn forget(&self) {
        self.listed.borrow_mut().take();
        self.images.borrow_mut().clear();
        self.ability.borrow_mut().take();
    }

    fn ability_width(&self, glossary: &NameBook) -> f32 {
        if let Some(held) = *self.ability.borrow() {
            return held;
        }

        let mut ruler = Ruler::new(Font::DEFAULT, cards::INPUT_SIZE);
        let widest = catalog(glossary)
            .iter()
            .map(|skill| ruler.width(&skill.to_string()))
            .fold(0.0, f32::max);
        let width = cards::combo_room(widest.ceil() + LABEL_SLACK);

        *self.ability.borrow_mut() = Some(width);

        width
    }

    fn choices(&self, vfs: &Vfs) -> Vec<Named> {
        if let Some(cached) = self.listed.borrow().as_ref() {
            return cached.clone();
        }

        let mut listed = vec![Named(-1)];
        listed.extend(available(vfs).into_iter().map(Named));

        *self.listed.borrow_mut() = Some(listed.clone());

        listed
    }

    fn control<'a>(&self, vfs: &Vfs, drawn: i32, current: i32, index: usize) -> Element<'a, Message> {
        button(self.face(vfs, drawn, Named(current)))
            .width(Length::Fill)
            .padding(TRAIT_INSET)
            .style(theme::neutral_button)
            .on_press(Message::Picker(Some(index)))
            .into()
    }

    fn tile<'a>(&self, vfs: &Vfs, choice: Named, index: usize) -> Element<'a, Message> {
        button(self.face(vfs, choice.0, choice))
            .width(Length::Fixed(TILE_WIDTH))
            .padding(TRAIT_INSET)
            .style(theme::neutral_button)
            .on_press(Message::Picked(index, choice.0))
            .into()
    }

    fn face<'a>(&self, vfs: &Vfs, drawn: i32, fallback: Named) -> Element<'a, Message> {
        let Some(handle) = self.handle(vfs, drawn) else {
            return theme::centered_text(fallback.to_string())
                .size(TRAIT_SIZE)
                .width(Length::Fill)
                .wrapping(text::Wrapping::None)
                .into();
        };

        container(iced_image(handle).height(Length::Fixed(PREVIEW_HEIGHT)))
            .width(Length::Fill)
            .center_x(Length::Fill)
            .into()
    }

    fn handle(&self, vfs: &Vfs, id: i32) -> Option<Handle> {
        if let Some(cached) = self.images.borrow().get(&id) {
            return cached.clone();
        }

        let loaded = load(vfs, id);
        self.images.borrow_mut().insert(id, loaded.clone());

        loaded
    }
}

fn load(vfs: &Vfs, id: i32) -> Option<Handle> {
    if id <= 0 {
        return None;
    }

    let path = vfs.find(&format!("Skill_name_{id:03}.png"))?;
    let decoded = image::open(&path).ok()?;
    let cropped = autocrop(decoded.to_rgba8());

    Some(Handle::from_rgba(cropped.width(), cropped.height(), cropped.into_raw()))
}

#[cfg(test)]
mod tests {
    use super::{TALENT_HEAD, TALENT_STRIDE};
    use crate::editor::figures::schema::{self, Subject};
    use crate::editor::figures::TALENT_SIZE;

    const SURGE_ATTACK: i32 = 56;

    fn surging() -> Vec<i32> {
        let mut cells = vec![0; TALENT_HEAD + TALENT_STRIDE];
        cells[TALENT_HEAD] = SURGE_ATTACK;

        cells
    }

    fn pair_at(pair: usize) -> usize {
        TALENT_HEAD + super::MIN_1 + pair * 2
    }

    #[test]
    fn a_pair_the_engine_reads_whole_mirrors_between_its_halves() {
        let cells = surging();

        for pair in 1..super::VALUES {
            let least = pair_at(pair);

            assert_eq!(super::mirror(least, &cells), Some(least + 1), "pair {pair} minimum");
            assert_eq!(super::mirror(least + 1, &cells), Some(least), "pair {pair} maximum");
        }
    }

    #[test]
    fn an_interpolated_pair_keeps_its_halves_apart() {
        let cells = surging();

        assert_eq!(
            super::mirror(pair_at(0), &cells),
            None,
            "a surge chance climbs with the talent level, so its two halves differ",
        );
    }

    #[test]
    fn talents_publish_no_columns() {
        assert!(
            schema::of(Subject::Talents).order().is_empty(),
            "talents: codex now publishes named columns, so the generated labels must become a table"
        );
    }

    #[test]
    fn every_talent_ability_resolves_to_a_name() {
        let listed = &*super::CATALOG;

        assert!(listed.len() > 1, "codex publishes no talent abilities, so the picker would be empty");

        for skill in &listed[1..] {
            assert!(
                skill.name.is_some(),
                "talent ability {} resolves to no name, so the picker falls back to its number",
                skill.id,
            );
        }
    }

    #[test]
    fn the_popup_minimum_fits_every_column() {
        assert!(
            super::columns() <= TALENT_SIZE.width,
            "the talents popup minimum is narrower than its own columns, so the table would clip"
        );
    }
}
