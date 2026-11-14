use std::cell::RefCell;
use std::fmt;
use std::path::{Path, PathBuf};

use iced::alignment::{Horizontal, Vertical};
use iced::widget::image::Handle;
use iced::widget::{button, column, container, image as iced_image, scrollable, text, Column, Row, Space};
use iced::{ContentFit, Element, Length};
use rustc_hash::FxHashMap;

use foundry::common::gfx::autocrop;
use foundry::domains::cat::combo;
use foundry::domains::cat::files;
use foundry::domains::cat::scanner::CatEntry;
use foundry::domains::settings::EditorMode;
use codex::cat::unit::ComboStrength;
use foundry::Vault;

pub(crate) use foundry::systems::editor::figures::*;

pub(crate) use foundry::systems::editor::figures::combos::*;

use crate::app::theme;
use crate::common::feedback::CONFIRM_LABEL;
use crate::common::row_window::{self, RowWindow};
use crate::widget::{hover_hint, popup, smooth_scroll};

use super::schema::{combo_form, combo_unit, COMBO_SLOTS};
use super::{cards, Draft, Frame, Message};

const NOTICE_SIZE: f32 = 11.0;

const FIELD_SIZE: f32 = 13.0;

const ROW_GAP: f32 = 6.0;

const FIELD_INSET: f32 = 2.0;

const SLOT_GAP: f32 = 4.0;

const SLOT_INSET: f32 = 2.0;

const TILE: f32 = 46.0;

const TILE_GAP: f32 = 4.0;

const TILE_PITCH: f32 = TILE + TILE_GAP;

const ADD_WIDTH: f32 = 172.0;

const CHROME: f32 = 150.0;

const SLOT_SIZE: f32 = (super::COMBO_SIZE.width
    - popup::FRAME_BORDER * 2.0
    - cards::BODY_PADDING * 2.0
    - SLOT_GAP * (COMBO_SLOTS as f32 - 1.0))
    / COMBO_SLOTS as f32;

pub(super) fn view<'a>(draft: &'a Draft, frame: Frame<'a>) -> Element<'a, Message> {
    let Frame { width, height, offset, armed, removing, cats, catalog, vault, picker, hunt, .. } =
        frame;

    if let Some(index) = picker {
        let top = column![back(), cards::hunt(hunt, width, HUNT_PLACEHOLDER)];
        let listing = chooser(catalog, cats, index, hunt, width, height, offset);

        return cards::shell(Some(top.into()), listing, cards::footer(vec![cards::sync(armed)]));
    }

    let footing = cards::footer(vec![
        Row::new()
            .spacing(ROW_GAP)
            .push(removal(draft, removing))
            .push(cards::sync(armed))
            .into(),
    ]);

    if draft.values() != EditorMode::Resolved {
        let shown: Vec<usize> = (0..draft.len()).collect();

        return cards::shell(Some(heading(draft, vault)), cards::grid(draft, width, &shown, None), footing);
    }

    let effects = Row::new()
        .spacing(ROW_GAP)
        .push(picked(draft, catalog, vault, EFFECT, NO_EFFECT, EFFECT_SPAN))
        .push(picked(draft, catalog, vault, POWER, NO_EFFECT, POWER_SPAN));

    let body = column![effects, slots(draft, cats, draft.anchor())].spacing(ROW_GAP);

    cards::shell(
        Some(heading(draft, vault)),
        container(body).padding(cards::BODY_PADDING).into(),
        footing,
    )
}

fn heading<'a>(draft: &'a Draft, vault: &Vault) -> Element<'a, Message> {
    let held = listing(draft, vault);
    let current = held
        .iter()
        .find(|option| option.raw == aimed(draft))
        .cloned()
        .unwrap_or_else(|| Labeled { raw: ABSENT, text: NEW_COMBO.to_owned() });

    let chooser = cards::options(held, current, |pick: Labeled| Message::Aimed(addressed(pick.raw)));

    cards::header(container(chooser).width(Length::Fill).center_x(Length::Fill))
}

fn listing(draft: &Draft, vault: &Vault) -> Vec<Labeled> {
    let mut held = vec![Labeled { raw: ABSENT, text: NEW_COMBO.to_owned() }];

    let Some((unit, form)) = draft.anchor() else {
        return held;
    };

    let (Ok(id), Ok(reached)) = (u32::try_from(unit), usize::try_from(form)) else {
        return held;
    };

    let rows = vault.vds.cats.combos(&vault.vfs);
    let names = vault.vds.cats.combo_names(&vault.vfs);

    held.extend(
        combo::combo_lines(vault, id, reached)
            .into_iter()
            .filter_map(|line| Some(Labeled { raw: i32::try_from(line).ok()?, text: titled(&rows, &names, line) })),
    );

    let current = aimed(draft);

    if current >= 0 && !held.iter().any(|option| option.raw == current) {
        held.push(Labeled { raw: current, text: named(draft, &names) });
    }

    held
}

fn picked<'a>(
    draft: &'a Draft,
    catalog: &Catalog,
    vault: &Vault,
    field: &'static str,
    absent: &'static str,
    span: u16,
) -> Element<'a, Message> {
    let Some(index) = draft.at(field) else {
        return Space::new().into();
    };

    let options = offerable(draft, catalog, vault, field, absent);
    let current = draft.reads_at(index).unwrap_or(ABSENT);
    let chosen = options.iter().find(|option| option.raw == current).cloned();

    let control: Element<'_, Message> = match chosen {
        Some(chosen) => {
            cards::options(options, chosen, move |pick: Labeled| Message::Picked(index, pick.raw))
        }
        None => cards::number(draft, index),
    };

    container(control).width(Length::FillPortion(span)).into()
}

fn offerable(
    draft: &Draft,
    catalog: &Catalog,
    vault: &Vault,
    field: &'static str,
    absent: &'static str,
) -> Vec<Labeled> {
    let offered = catalog.texts(vault, field, absent);

    if field != POWER {
        return offered;
    }

    let effect = draft.at(EFFECT).and_then(|index| draft.reads_at(index)).unwrap_or(ABSENT);
    let params = vault.vds.cats.combo_params(&vault.vfs);

    let Some(row) = usize::try_from(effect).ok().and_then(|index| params.get(index)) else {
        return vec![Labeled { raw: ABSENT, text: absent.to_owned() }];
    };

    offered
        .into_iter()
        .filter(|option| option.raw < 0 || row.magnitude(ComboStrength::from(option.raw)).is_some())
        .collect()
}

fn removal<'a>(draft: &'a Draft, armed: bool) -> Element<'a, Message> {
    let label = theme::centered_text(if armed { CONFIRM_LABEL } else { REMOVE_LABEL })
        .size(FIELD_SIZE)
        .wrapping(text::Wrapping::None);

    let control = button(label).width(Length::Fixed(ADD_WIDTH)).padding([FIELD_INSET + 3.0, 10.0]);

    if draft.absent() {
        return hover_hint(control.style(theme::inert_button), NOTHING_TO_REMOVE);
    }

    control.style(theme::danger_button).on_press(Message::Removed).into()
}

fn slots<'a>(draft: &'a Draft, cats: &[CatEntry], anchor: Option<(i32, i32)>) -> Element<'a, Message> {
    let mut line = Row::new().spacing(SLOT_GAP).align_y(Vertical::Center);

    for slot in 0..COMBO_SLOTS {
        line = line.push(member(draft, cats, slot, anchor));
    }

    container(line).width(Length::Fill).center_x(Length::Fill).into()
}

fn member<'a>(
    draft: &'a Draft,
    cats: &[CatEntry],
    slot: usize,
    anchor: Option<(i32, i32)>,
) -> Element<'a, Message> {
    let Some(index) = draft.at(&combo_unit(slot)) else {
        return Space::new().into();
    };

    let unit = draft.reads_at(index).unwrap_or(ABSENT);
    let form = draft.at(&combo_form(slot)).and_then(|at| draft.reads_at(at)).unwrap_or(ABSENT);
    let held = anchor == Some((unit, form));

    let face: Element<'_, Message> = match icon_of(cats, unit, form) {
        Some(path) => iced_image(Handle::from_path(path))
            .content_fit(ContentFit::Contain)
            .width(Length::Fill)
            .height(Length::Fill)
            .into(),
        None => theme::centered_text(vacant_label(unit, form))
            .size(FIELD_SIZE)
            .width(Length::Fill)
            .into(),
    };

    let style: theme::ButtonStyleFn = if held { theme::inert_button } else { theme::neutral_button };

    let control = button(face)
        .width(Length::Fixed(SLOT_SIZE))
        .height(Length::Fixed(SLOT_SIZE))
        .padding(SLOT_INSET)
        .style(style);

    if held {
        return hover_hint(control, ANCHOR_HINT);
    }

    control.on_press(Message::Picker(Some(index))).into()
}

fn back<'a>() -> Element<'a, Message> {
    container(
        button(theme::centered_text("Back").size(FIELD_SIZE))
            .width(Length::Fixed(ADD_WIDTH * 0.5))
            .padding(FIELD_INSET + 1.0)
            .style(theme::neutral_button)
            .on_press(Message::Picker(None)),
    )
    .width(Length::Fill)
    .center_x(Length::Fill)
    .padding(SLOT_GAP)
    .into()
}

fn chooser<'a>(
    catalog: &'a Catalog,
    cats: &'a [CatEntry],
    index: usize,
    hunt: &str,
    width: f32,
    height: f32,
    offset: f32,
) -> Element<'a, Message> {
    let shown = catalog.shown(cats, hunt);
    let per_row = (((cards::usable(width) + TILE_GAP) / TILE_PITCH).floor() as usize).max(1);
    let rows = shown.div_ceil(per_row);
    let viewport = (height - CHROME).max(TILE_PITCH);

    let RowWindow { range, pad_before, pad_after } =
        row_window::compute_with(rows, viewport, offset, TILE, TILE_GAP);

    let visible = catalog.window(range.start * per_row..range.end * per_row);

    let mut stack = Column::new().spacing(TILE_GAP).align_x(Horizontal::Center);

    if pad_before > 0.0 {
        stack = stack.push(Space::new().height(Length::Fixed(pad_before)));
    }

    if range.start == 0 {
        stack = stack.push(clearing(index));
    }

    for chunk in visible.chunks(per_row) {
        let mut line = Row::new().spacing(TILE_GAP).align_y(Vertical::Center);

        for entry in chunk {
            line = line.push(tile(catalog, entry, index));
        }

        stack = stack.push(line);
    }

    if pad_after > 0.0 {
        stack = stack.push(Space::new().height(Length::Fixed(pad_after)));
    }

    let area = scrollable(container(stack).width(Length::Fill).center_x(Length::Fill))
        .on_scroll(|viewport| Message::Scrolled(viewport.absolute_offset().y))
        .width(Length::Fill)
        .height(Length::Fill);

    smooth_scroll(area).into()
}

fn tile<'a>(catalog: &Catalog, entry: &Entry, index: usize) -> Element<'a, Message> {
    let face: Element<'_, Message> = match catalog.handle(&entry.path) {
        Some(handle) => iced_image(handle)
            .content_fit(ContentFit::Contain)
            .width(Length::Fill)
            .height(Length::Fill)
            .into(),
        None => theme::centered_text(format!("{:03}", entry.id)).size(NOTICE_SIZE).into(),
    };

    button(face)
        .width(Length::Fixed(TILE))
        .height(Length::Fixed(TILE))
        .padding(SLOT_INSET)
        .style(theme::neutral_button)
        .on_press(Message::Slotted(index, entry.id as i32, entry.form as i32))
        .into()
}

fn clearing<'a>(index: usize) -> Element<'a, Message> {
    let label = theme::centered_text(CLEAR_SLOT).size(NOTICE_SIZE).wrapping(text::Wrapping::None);

    container(
        button(label)
            .width(Length::Fixed(TILE * 2.0))
            .height(Length::Fixed(TILE))
            .padding(SLOT_INSET)
            .style(theme::neutral_button)
            .on_press(Message::Slotted(index, ABSENT, ABSENT)),
    )
    .width(Length::Fill)
    .center_x(Length::Fill)
    .into()
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct Labeled {
    raw: i32,
    text: String,
}

impl fmt::Display for Labeled {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.text)
    }
}

#[derive(Default)]
pub(super) struct Catalog {
    listed: RefCell<Option<(usize, Vec<Entry>)>>,
    matched: RefCell<Option<(String, Vec<usize>)>>,
    texts: RefCell<FxHashMap<&'static str, Vec<Labeled>>>,
    images: RefCell<FxHashMap<PathBuf, Option<Handle>>>,
}

impl Catalog {
    pub(super) fn forget(&self) {
        self.listed.borrow_mut().take();
        self.matched.borrow_mut().take();
        self.texts.borrow_mut().clear();
        self.images.borrow_mut().clear();
    }

    fn build(&self, cats: &[CatEntry]) {
        if self.listed.borrow().as_ref().is_some_and(|(seen, _)| *seen == cats.len()) {
            return;
        }

        let listed: Vec<Entry> = cats
            .iter()
            .flat_map(|cat| {
                (0..files::FORM_COUNT).filter_map(|form| {
                    let path = cat.deploy_icon_paths.get(form)?.clone()?;
                    let named = cat.names.get(form).cloned().flatten().unwrap_or_default();

                    Some(Entry {
                        id: cat.id,
                        form,
                        path,
                        hunted: format!("{:03} {}", cat.id, named).to_lowercase(),
                    })
                })
            })
            .collect();

        *self.listed.borrow_mut() = Some((cats.len(), listed));
        self.matched.borrow_mut().take();
    }

    fn refine(&self, hunt: &str) {
        let wanted = hunt.trim().to_lowercase();

        if self.matched.borrow().as_ref().is_some_and(|(seen, _)| *seen == wanted) {
            return;
        }

        let listed = self.listed.borrow();
        let Some((_, entries)) = listed.as_ref() else {
            return;
        };

        let kept: Vec<usize> = entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| wanted.is_empty() || entry.hunted.contains(&wanted))
            .map(|(index, _)| index)
            .collect();

        *self.matched.borrow_mut() = Some((wanted, kept));
    }

    fn shown(&self, cats: &[CatEntry], hunt: &str) -> usize {
        self.build(cats);
        self.refine(hunt);

        self.matched.borrow().as_ref().map_or(0, |(_, kept)| kept.len())
    }

    fn window(&self, range: std::ops::Range<usize>) -> Vec<Entry> {
        let matched = self.matched.borrow();
        let listed = self.listed.borrow();

        let (Some((_, kept)), Some((_, entries))) = (matched.as_ref(), listed.as_ref()) else {
            return Vec::new();
        };

        kept.get(range)
            .unwrap_or_default()
            .iter()
            .filter_map(|index| entries.get(*index).cloned())
            .collect()
    }

    fn texts(&self, vault: &Vault, field: &'static str, absent: &'static str) -> Vec<Labeled> {
        if let Some(cached) = self.texts.borrow().get(field) {
            return cached.clone();
        }

        let table = match field {
            POWER => vault.vds.cats.combo_bands(&vault.vfs),
            _ => vault.vds.cats.combo_effects(&vault.vfs),
        };

        let mut options = vec![Labeled { raw: ABSENT, text: absent.to_owned() }];

        options.extend(table.iter().enumerate().filter_map(|(line, entry)| {
            let text = entry.as_deref()?.trim();

            (!text.is_empty()).then(|| Labeled { raw: line as i32, text: text.to_owned() })
        }));

        self.texts.borrow_mut().insert(field, options.clone());

        options
    }

    fn handle(&self, path: &Path) -> Option<Handle> {
        if let Some(cached) = self.images.borrow().get(path) {
            return cached.clone();
        }

        let loaded = thumbnail(path);
        self.images.borrow_mut().insert(path.to_path_buf(), loaded.clone());

        loaded
    }
}

fn thumbnail(path: &Path) -> Option<Handle> {
    let decoded = image::open(path).ok()?;
    let cropped = autocrop(decoded.to_rgba8());

    let longest = cropped.width().max(cropped.height()).max(1);
    let width = (cropped.width() * THUMB / longest).max(1);
    let height = (cropped.height() * THUMB / longest).max(1);
    let scaled = image::imageops::thumbnail(&cropped, width, height);

    Some(Handle::from_rgba(scaled.width(), scaled.height(), scaled.into_raw()))
}

#[cfg(test)]
mod tests {
    use super::{next_key, rule};
    use crate::editor::figures::schema::{self, Subject};

    fn rows(keys: &[(i32, i32)]) -> Vec<String> {
        keys.iter().map(|(series, id)| format!("{id},{series},-1")).collect()
    }

    #[test]
    fn every_combo_column_has_a_rule() {
        for entry in schema::of(Subject::Combo).order() {
            assert!(
                rule(entry.field).is_some(),
                "combo: codex publishes {}, which no resolved rule arm names",
                entry.field,
            );
        }
    }

    #[test]
    fn a_new_combo_takes_the_next_free_id_in_the_series_it_was_opened_on() {
        let lines = rows(&[(6, 3064), (6, 3066), (1, 109)]);

        assert_eq!(next_key(&lines, ',', 6), (6, 3067), "the id counts on within series 6 alone");
        assert_eq!(next_key(&lines, ',', 1), (1, 110), "series 1 counts from its own highest");
    }

    // Vanilla never gaps a combo's members: all 263 active rows are leading-packed,
    // and no row anywhere carries a half-filled slot.
    #[test]
    fn clearing_a_middle_member_closes_the_gap_it_leaves() {
        let schema = schema::of(Subject::Combo);
        let mut cells = vec![-1; schema.known()];

        let at = |field: &str| schema.index_of(field).expect("published slot column");

        cells[at("slot_1_unit_id")] = 44;
        cells[at("slot_1_form")] = 0;
        cells[at("slot_3_unit_id")] = 90;
        cells[at("slot_3_form")] = 2;

        for (index, raw) in super::packed(schema, &cells) {
            cells[index] = raw;
        }

        assert_eq!(cells[at("slot_2_unit_id")], 90, "the third member moves up into the hole");
        assert_eq!(cells[at("slot_2_form")], 2, "and brings its form with it");
        assert_eq!(cells[at("slot_3_unit_id")], -1, "the slot it left is emptied");
        assert!(super::packed(schema, &cells).is_empty(), "a packed row rewrites nothing");
    }

    // Measured over the shipped files: every one of the 263 active combos sits inside its
    // effect's param row, and only the three effects with a sixth column ever use Activated.
    #[test]
    fn a_power_is_offered_only_where_its_effect_declares_a_magnitude() {
        use codex::cat::unit::{ComboStrength, NyancomboParam};

        let ordinary = NyancomboParam { magnitudes: vec![Some(10), Some(20), Some(30), Some(50), Some(-20)] };
        let granting = NyancomboParam {
            magnitudes: vec![Some(1), Some(1), Some(1), Some(1), Some(1), Some(1)],
        };

        assert!(
            ordinary.magnitude(ComboStrength::Grant).is_none(),
            "a five-column effect declares no Activated magnitude, so it must not offer one",
        );
        assert!(ordinary.magnitude(ComboStrength::Down).is_some(), "Down is a column every effect has");
        assert!(granting.magnitude(ComboStrength::Grant).is_some(), "a six-column effect does offer it");
    }

    #[test]
    fn a_blank_combo_opens_with_the_unit_it_was_summoned_from() {
        let schema = schema::of(Subject::Combo);
        let mut fields: Vec<String> = vec!["-1".to_owned(); schema.known()];

        super::seed(&mut fields, schema, Some((44, 2)));

        let at = |field: &str| schema.index_of(field).expect("published slot column");

        assert_eq!(fields[at("slot_1_unit_id")], "44");
        assert_eq!(fields[at("slot_1_form")], "2");
    }

    #[test]
    fn a_superseded_row_hands_the_new_combo_to_the_highest_live_series() {
        let lines = rows(&[(-1, 0), (1, 109), (6, 3066)]);

        assert_eq!(
            next_key(&lines, ',', -1),
            (6, 3067),
            "a negative series is retired, so a new combo must not join it",
        );
    }
}
