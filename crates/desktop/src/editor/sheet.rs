use std::fs;
use std::path::{Path, PathBuf};

use iced::alignment::Vertical;
use iced::widget::{button, column, container, responsive, row, scrollable, space, text, text_input, Column, Id};
use iced::{Element, Length, Padding, Size, Task};
use tracing::warn;

use codex::common;
use foundry::common::preview::{self, Stamp};
use foundry::domains::mods;
use foundry::domains::settings::EditorMode;
use foundry::Vfs;

pub(crate) use foundry::systems::editor::sheet::*;

use crate::app::theme;
use crate::common::feedback::{Slot, CONFIRM_LABEL, FAILURE_LABEL};
use crate::common::row_window::{self, RowWindow};
use crate::widget::{popup, smooth_scroll};

use super::{Mark, Reader};

const SIZE: Size = Size::new(420.0, 520.0);
const POPUP: popup::Spec = popup::Spec::new(popup::Kind::Sheet, SIZE);

const BODY_PADDING: f32 = 12.0;
const GAP: f32 = 8.0;
const LABEL_SIZE: f32 = 12.0;
const INPUT_SIZE: f32 = 13.0;
const INPUT_PADDING: f32 = 4.0;
const INPUT_WIDTH: f32 = 110.0;
const BUTTON_SIZE: f32 = 13.0;
const BUTTON_PADDING: f32 = 8.0;
const SCROLL_GAP: f32 = 12.0;
const FRAME_GAP: f32 = 3.0;
const FRAME_INSET: f32 = theme::CARD_BORDER_WIDTH + FRAME_GAP;

const SYNC_LABEL: &str = "Sync With \"game\"";
const SEARCH_HINT: &str = "Search Field...";
const EMPTY_LABEL: &str = "No fields match";

#[derive(Clone, PartialEq)]
pub(crate) struct Plan {
    sheet: Sheet,
    label: String,
    file: String,
    game: PathBuf,
    target_mod: String,
    values: EditorMode,
}

pub(super) fn plan(sheet: Sheet, label: String, file: String, game: PathBuf, target_mod: String, values: EditorMode) -> Plan {
    Plan { sheet, label, file, game, target_mod, values }
}

impl Plan {
    fn held(&self, vfs: &Vfs) -> Option<PathBuf> {
        mods::find(vfs, &self.target_mod, Path::new(&self.file))
    }

    fn source(&self, vfs: &Vfs) -> PathBuf {
        self.held(vfs).unwrap_or_else(|| self.game.clone())
    }
}

struct Draft {
    plan: Plan,
    read_from: PathBuf,
    stamp: Stamp,
    lines: Vec<String>,
    fields: Vec<Field>,
    inputs: Vec<String>,
    labels: Vec<String>,
    failed: bool,
}

impl Draft {
    fn load(plan: Plan, vfs: &Vfs) -> Option<Draft> {
        let read_from = plan.source(vfs);

        let bytes = fs::read(&read_from)
            .inspect_err(|err| warn!(path = %read_from.display(), "Sheet editor could not read the file: {}", err))
            .ok()?;

        let stamp = preview::stamp(&read_from)?;
        let lines: Vec<String> = common::scrub(&bytes).lines().map(str::to_owned).collect();
        let (fields, inputs) = plan.sheet.fields(&lines);
        let labels = labels(&plan, &fields);

        Some(Draft { plan, read_from, stamp, lines, fields, inputs, labels, failed: false })
    }

    fn drifted(&self) -> bool {
        preview::stamp(&self.read_from) != Some(self.stamp)
    }

    fn bounds(&self, index: usize) -> Option<(i32, i32)> {
        let field = self.fields.get(index)?;

        (self.plan.values == EditorMode::Resolved).then(|| self.plan.sheet.limits(&field.key)).flatten()
    }

    fn edit(&mut self, index: usize, typed: &str) -> bool {
        let textual = self.plan.sheet.textual();

        if !textual && !typable(typed) {
            return false;
        }

        let bounds = self.bounds(index);

        let Some(slot) = self.inputs.get_mut(index) else {
            return false;
        };

        let value = match textual {
            true => typed.to_owned(),
            false => {
                let Ok(number) = typed.parse::<i32>() else {
                    *slot = typed.to_owned();

                    return false;
                };

                let number = bounds.map_or(number, |(least, most)| number.clamp(least, most));

                number.to_string()
            }
        };

        slot.clone_from(&value);

        let Some(field) = self.fields.get(index) else {
            return false;
        };

        let line = format!("{}{value}{}", field.head, field.tail);

        let Some(held) = self.lines.get_mut(field.line) else {
            return false;
        };

        if *held == line {
            return false;
        }

        *held = line;

        true
    }

    fn restore(&mut self, vfs: &Vfs) -> bool {
        let Ok(bytes) = fs::read(&self.plan.game) else {
            warn!(path = %self.plan.game.display(), "Sheet editor could not read the vanilla file");

            return false;
        };

        self.lines = common::scrub(&bytes).lines().map(str::to_owned).collect();

        let (fields, inputs) = self.plan.sheet.fields(&self.lines);

        self.labels = labels(&self.plan, &fields);
        self.fields = fields;
        self.inputs = inputs;

        self.persist(vfs)
    }

    fn destination(&self, vfs: &Vfs) -> Option<(PathBuf, Stamp)> {
        if self.read_from != self.plan.game {
            return Some((self.read_from.clone(), self.stamp));
        }

        let path = mods::ensure_as(vfs, &self.plan.target_mod, &self.plan.game, &self.plan.file)
            .inspect_err(|err| warn!(source = %self.plan.game.display(), "Sheet editor could not stage the file: {}", err))
            .ok()?;

        if let Err(err) = vfs.create((self.plan.target_mod.as_str(), path.as_path())) {
            warn!(path = %path.display(), "Sheet editor staged a file the index would not take: {}", err);
        }

        preview::stamp(&path).map(|stamp| (path, stamp))
    }

    fn persist(&mut self, vfs: &Vfs) -> bool {
        let Some((path, stamp)) = self.destination(vfs) else {
            self.failed = true;

            return false;
        };

        let mut body = self.lines.join("\n");
        body.push('\n');

        match preview::save(&path, body.as_bytes(), stamp) {
            Ok(fresh) => {
                self.read_from = path;
                self.stamp = fresh;
                self.failed = false;

                true
            }
            Err(err) => {
                warn!(path = %path.display(), "Sheet editor could not write the file: {}", err);
                self.failed = true;

                false
            }
        }
    }
}

fn labels(plan: &Plan, fields: &[Field]) -> Vec<String> {
    fields
        .iter()
        .map(|field| match plan.values {
            EditorMode::Raw => field.key.clone(),
            EditorMode::Resolved => plan.sheet.resolved(&field.key),
        })
        .collect()
}

fn typable(typed: &str) -> bool {
    let digits = typed.strip_prefix('-').unwrap_or(typed);

    digits.chars().all(|glyph| glyph.is_ascii_digit())
}

fn list_id() -> Id {
    Id::new("sheet-list")
}

#[derive(Debug, Clone)]
pub enum Message {
    Popup(popup::Message),
    Searched(String),
    Changed(usize, String),
    Scrolled(f32),
    Sync,
    ConfirmExpired,
    FailureExpired,
}

#[derive(Default)]
pub(super) struct State {
    draft: Option<Draft>,
    frame: popup::State,
    query: String,
    shown: Vec<usize>,
    offset: f32,
    confirm: Slot<()>,
    failed: Slot<()>,
}

pub(super) fn kind() -> popup::Kind {
    POPUP.kind()
}

impl State {
    pub(super) fn begin(&mut self, plan: Plan, nudge: usize, vfs: &Vfs) {
        self.frame = popup::cascaded(nudge);
        self.query.clear();
        self.offset = 0.0;
        self.confirm.clear();
        self.failed.clear();
        self.draft = Draft::load(plan, vfs);
        self.refilter();
    }

    pub(super) fn drafting(&self) -> bool {
        self.draft.is_some()
    }

    pub(super) fn raised(&self) -> u64 {
        self.frame.raised()
    }

    pub(super) fn drifted(&self) -> bool {
        self.draft.as_ref().is_some_and(Draft::drifted)
    }

    fn refilter(&mut self) {
        let needle = self.query.trim().to_lowercase();

        self.shown = self.draft.as_ref().map_or_else(Vec::new, |draft| {
            (0..draft.labels.len())
                .filter(|index| {
                    needle.is_empty()
                        || draft.labels.get(*index).is_some_and(|label| label.to_lowercase().contains(&needle))
                        || (draft.plan.sheet.textual()
                            && draft.inputs.get(*index).is_some_and(|input| input.to_lowercase().contains(&needle)))
                })
                .collect()
        });
    }

    pub(super) fn sync(&mut self, plan: Option<Plan>, vfs: &Vfs) {
        let Some(current) = self.draft.as_ref() else {
            return;
        };

        let Some(plan) = plan else {
            self.draft = None;

            return;
        };

        let moved = current.plan.held(vfs).is_some() != (current.read_from != current.plan.game);

        if current.plan != plan || current.drifted() || moved {
            self.draft = Draft::load(plan, vfs);
            self.refilter();
        }
    }

    pub(super) fn update(&mut self, message: Message, vfs: &Vfs) -> Task<Message> {
        match message {
            Message::Popup(msg) => {
                if self.frame.update(msg, POPUP) {
                    self.draft = None;
                    self.confirm.clear();
                    self.failed.clear();
                }

                Task::none()
            }
            Message::Searched(query) => {
                self.query = query;
                self.offset = 0.0;
                self.refilter();

                iced::widget::operation::scroll_to(list_id(), scrollable::AbsoluteOffset { x: 0.0, y: 0.0 })
            }
            Message::Changed(index, typed) => {
                let Some(draft) = self.draft.as_mut() else {
                    return Task::none();
                };

                if draft.edit(index, &typed) && !draft.persist(vfs) {
                    return self.failed.set((), Message::FailureExpired);
                }

                Task::none()
            }
            Message::Scrolled(offset) => {
                self.offset = offset;

                Task::none()
            }
            Message::Sync => {
                if !self.confirm.take(&()) {
                    return self.confirm.set((), Message::ConfirmExpired);
                }

                let restored = self.draft.as_mut().is_some_and(|draft| draft.restore(vfs));

                self.refilter();

                match restored {
                    true => Task::none(),
                    false => self.failed.set((), Message::FailureExpired),
                }
            }
            Message::ConfirmExpired => {
                self.confirm.expire();

                Task::none()
            }
            Message::FailureExpired => {
                self.failed.expire();

                Task::none()
            }
        }
    }

    pub(super) fn view(&self, window: Size) -> Option<Element<'_, Message>> {
        let draft = self.draft.as_ref()?;

        Some(self.frame.view(&draft.plan.label, POPUP, window, Message::Popup, move || self.body(draft), None))
    }

    fn body<'a>(&'a self, draft: &'a Draft) -> Element<'a, Message> {
        let search = text_input(SEARCH_HINT, &self.query)
            .on_input(Message::Searched)
            .padding(INPUT_PADDING + 2.0)
            .size(INPUT_SIZE)
            .width(Length::Fill)
            .style(theme::rounded_input);

        let listing: Element<'_, Message> = match self.shown.is_empty() {
            true => container(theme::centered_text(EMPTY_LABEL).size(LABEL_SIZE).style(text::danger))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into(),
            false => responsive(move |size: Size| self.rows(draft, size)).into(),
        };

        let framed = container(listing)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(FRAME_INSET)
            .style(theme::card_container_outlined);

        let label = match (self.failed.is_set() || draft.failed, self.confirm.is_set()) {
            (true, _) => FAILURE_LABEL,
            (false, true) => CONFIRM_LABEL,
            (false, false) => SYNC_LABEL,
        };

        let sync = button(theme::button_label(label).size(BUTTON_SIZE))
            .width(Length::Fill)
            .padding(BUTTON_PADDING)
            .style(theme::danger_button)
            .on_press(Message::Sync);

        column![search, framed, sync]
            .spacing(GAP)
            .padding(BODY_PADDING)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn rows<'a>(&'a self, draft: &'a Draft, size: Size) -> Element<'a, Message> {
        let pitch = draft.plan.sheet.pitch();

        let RowWindow { range, pad_before, pad_after } =
            row_window::compute_with(self.shown.len(), size.height, self.offset, pitch, 0.0);

        let mut list = Column::with_capacity(range.len() + 2).width(Length::Fill);

        if pad_before > 0.0 {
            list = list.push(space().height(Length::Fixed(pad_before)));
        }

        for at in range {
            let Some(&index) = self.shown.get(at) else {
                continue;
            };

            list = list.push(line(draft, index, pitch));
        }

        if pad_after > 0.0 {
            list = list.push(space().height(Length::Fixed(pad_after)));
        }

        let area = scrollable(container(list).padding(Padding::default().right(SCROLL_GAP)))
            .id(list_id())
            .on_scroll(|viewport| Message::Scrolled(viewport.absolute_offset().y))
            .width(Length::Fill)
            .height(Length::Fill);

        smooth_scroll(area).into()
    }
}

fn line(draft: &Draft, index: usize, pitch: f32) -> Element<'_, Message> {
    let label = draft.labels.get(index).map_or("", String::as_str);
    let input = draft.inputs.get(index).map_or("", String::as_str);

    let field = text_input("", input)
        .on_input(move |typed| Message::Changed(index, typed))
        .padding(INPUT_PADDING)
        .size(INPUT_SIZE)
        .style(theme::rounded_input);

    let body: Element<'_, Message> = match draft.plan.sheet.textual() {
        true => column![text(label).size(LABEL_SIZE), field.width(Length::Fill)].spacing(2).into(),
        false => row![text(label).size(LABEL_SIZE).width(Length::Fill), field.width(Length::Fixed(INPUT_WIDTH))]
            .spacing(GAP)
            .align_y(Vertical::Center)
            .into(),
    };

    container(body).height(Length::Fixed(pitch)).align_y(Vertical::Center).into()
}

pub(super) fn mark(plan: &Plan, vfs: &Vfs, reader: &mut Reader) -> Mark {
    let Some(held) = plan.held(vfs) else {
        return Mark::Same;
    };

    Mark::of(reader.text(&held), reader.text(&plan.game))
}

pub(super) fn reset(plan: Plan, vfs: &Vfs) -> bool {
    Draft::load(plan, vfs).is_some_and(|mut draft| draft.restore(vfs))
}

#[cfg(test)]
mod tests {
    use super::Sheet;

    #[test]
    fn a_value_is_rewritten_without_touching_the_rest_of_its_line() {
        let (held, value) = Sheet::Param.field(3, "battle_zoom_y\t520 // camera").expect("a key and a value");

        assert_eq!(held.key, "battle_zoom_y");
        assert_eq!(value, "520");
        assert_eq!(format!("{}{}{}", held.head, 640, held.tail), "battle_zoom_y\t640 // camera");

        // a key with no value is not a field at all
        assert!(Sheet::Param.field(0, "lonely_key").is_none());
    }

    #[test]
    fn mission_text_splits_at_its_pipe() {
        let (held, value) = Sheet::MissionText.field(2, "2|Raise your User Rank by %d!<br>(more)").expect("an id and a text");

        assert_eq!((held.key.as_str(), value.as_str()), ("2", "Raise your User Rank by %d!<br>(more)"));
        assert_eq!(format!("{}{}{}", held.head, value, held.tail), "2|Raise your User Rank by %d!<br>(more)");

        // the Japanese file puts a comma where the others put a pipe
        let (held, value) = Sheet::MissionText.field(0, "0,テスト,です").expect("a comma split");
        assert_eq!((held.key.as_str(), value.as_str()), ("0", "テスト,です"));
    }

    #[test]
    fn combo_text_keeps_its_trailing_separator() {
        let (held, value) = Sheet::ComboBands.field(0, " (Sm)|").expect("a band name");

        assert_eq!((held.key.as_str(), value.as_str()), ("0", " (Sm)"));
        assert_eq!(format!("{}{}{}", held.head, "【小】", held.tail), "【小】|");
        assert!(Sheet::ComboEffects.field(3, "no separator").is_none());
    }

    #[test]
    fn a_medal_line_edits_its_name_and_description_apart() {
        let line = "Builder\tCollect all Gold Treasures";
        let (name, value) = Sheet::MedalNames.field(4, line).expect("a name");
        let (description, rest) = Sheet::MedalDescriptions.field(4, line).expect("a description");

        assert_eq!((value.as_str(), rest.as_str()), ("Builder", "Collect all Gold Treasures"));
        assert_eq!(format!("{}{}{}", name.head, "Maker", name.tail), "Maker\tCollect all Gold Treasures");
        assert_eq!(format!("{}{}{}", description.head, "Get them", description.tail), "Builder\tGet them");
    }

    #[test]
    fn a_treasure_set_line_edits_one_cell_in_place() {
        let line = "Energy Drink|Activated!|Worker Cat Efficiency increased!|";
        let (field, value) = Sheet::TreasureEffects("Treasure3_0.csv").field(0, line).expect("the effect cell");

        assert_eq!(value, "Worker Cat Efficiency increased!");
        assert_eq!(format!("{}{}{}", field.head, "Faster workers!", field.tail), "Energy Drink|Activated!|Faster workers!|");

        let (field, value) = Sheet::TreasureSets("Treasure3_0.csv").field(0, "九州 ,戦闘中に,発動中！,").expect("a comma line");
        assert_eq!(value, "九州 ");
        assert_eq!(field.tail, ",戦闘中に,発動中！,");
    }

    #[test]
    fn a_weight_line_splits_at_its_comma() {
        let (held, value) = Sheet::Weights.field(3, "3,30").expect("a tier and a weight");

        assert_eq!((held.key.as_str(), value.as_str()), ("3", "30"));
        assert_eq!(format!("{}{}{}", held.head, 45, held.tail), "3,45");
    }

    #[test]
    fn text_keeps_everything_after_its_first_tab() {
        let (held, value) = Sheet::Localizable.field(0, "help_url\thttps://example.com // not a comment").expect("a field");

        assert_eq!(value, "https://example.com // not a comment");
        assert_eq!(format!("{}{}{}", held.head, value, held.tail), "help_url\thttps://example.com // not a comment");
    }
}
