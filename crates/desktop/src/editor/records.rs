use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};

use iced::alignment::Vertical;
use iced::widget::{button, column, container, pick_list, row, scrollable, text, text_input, Column};
use iced::{Element, Length, Padding, Size, Task};
use tracing::warn;

use foundry::common::formats::json::{self, Field, Record, Value};
use foundry::common::preview::{self, Stamp};
use foundry::domains::mods;
use foundry::domains::settings::EditorMode;
use foundry::Vfs;

pub(crate) use foundry::systems::editor::records::*;

use crate::app::theme;
use crate::common::feedback::{Slot, CONFIRM_LABEL, FAILURE_LABEL};
use crate::widget::{popup, smooth_scroll};

use super::{Mark, Reader};

const SIZE: Size = Size::new(420.0, 460.0);
const POPUP: popup::Spec = popup::Spec::new(popup::Kind::Records, SIZE);

const BODY_PADDING: f32 = 12.0;
const GAP: f32 = 8.0;
const ROW_GAP: f32 = 6.0;
const LABEL_SIZE: f32 = 12.0;
const INPUT_SIZE: f32 = 13.0;
const INPUT_PADDING: f32 = 4.0;
const INPUT_WIDTH: f32 = 170.0;
const BUTTON_SIZE: f32 = 13.0;
const BUTTON_PADDING: f32 = 8.0;
const SCROLL_GAP: f32 = 12.0;
const FRAME_GAP: f32 = 3.0;
const FRAME_INSET: f32 = theme::CARD_BORDER_WIDTH + FRAME_GAP;

const SYNC_LABEL: &str = "Sync With \"game\"";
const MISSING_LABEL: &str = "This record is no longer in the file";
const LIST_JOINT: &str = ", ";
const PATH_JOINT: &str = "/";
const QUOTE: char = '"';
const HEADING_SIZE: f32 = 13.0;
const SECTION_GAP: f32 = 6.0;

#[derive(Clone, PartialEq)]
pub(crate) struct Plan {
    book: Book,
    label: String,
    file: String,
    game: PathBuf,
    target_mod: String,
    path: Vec<String>,
    values: EditorMode,
    named: Named,
}

pub(super) struct Seed {
    pub(super) book: Book,
    pub(super) label: String,
    pub(super) file: String,
    pub(super) game: PathBuf,
    pub(super) target_mod: String,
    pub(super) path: Vec<String>,
    pub(super) values: EditorMode,
    pub(super) named: Named,
}

pub(super) fn plan(seed: Seed) -> Plan {
    let Seed { book, label, file, game, target_mod, path, values, named } = seed;

    Plan { book, label, file, game, target_mod, path, values, named }
}

impl Plan {
    fn held(&self, vfs: &Vfs) -> Option<PathBuf> {
        mods::find(vfs, &self.target_mod, Path::new(&self.file))
    }

    fn source(&self, vfs: &Vfs) -> PathBuf {
        self.held(vfs).unwrap_or_else(|| self.game.clone())
    }
}

fn subtree(text: &str, path: &[String], shallow: bool) -> (Vec<Record>, Option<Range<usize>>) {
    let mut records: Vec<Record> = json::records(text)
        .into_iter()
        .filter(|record| match shallow {
            true => record.path == path,
            false => record.path.starts_with(path),
        })
        .collect();

    records.sort_by_key(|record| record.span.start);

    let span = json::objects(text).into_iter().find(|node| node.path == path).map(|node| node.span);

    (records, span)
}

fn object<'a>(text: &'a str, path: &[String]) -> Option<&'a str> {
    let (_, span) = subtree(text, path, false);

    text.get(span?)
}

fn own_fields(text: &str, path: &[String]) -> Option<Vec<(String, String)>> {
    let (records, _) = subtree(text, path, true);
    let record = records.into_iter().next()?;

    record.fields.iter().map(|field| Some((field.key.clone(), text.get(field.span.clone())?.to_owned()))).collect()
}

fn snapshot(text: &str, path: &[String], shallow: bool) -> Option<String> {
    match shallow {
        true => own_fields(text, path).map(|fields| fields.into_iter().map(|(key, value)| format!("{key}={value}")).collect::<Vec<_>>().join("\n")),
        false => object(text, path).map(str::to_owned),
    }
}

fn shown(value: &Value) -> String {
    match value {
        Value::Number(number) => number.clone(),
        Value::Text(words) => words.clone(),
        Value::Numbers(items) | Value::Texts(items) | Value::Tokens(items) => items.join(LIST_JOINT),
        Value::Flag(held) => held.to_string(),
        Value::Other => String::new(),
    }
}

struct Draft {
    plan: Plan,
    read_from: PathBuf,
    stamp: Stamp,
    text: String,
    records: Vec<Record>,
    span: Option<Range<usize>>,
    lines: Vec<(usize, usize)>,
    inputs: Vec<String>,
    failed: bool,
}

impl Draft {
    fn load(plan: Plan, vfs: &Vfs) -> Option<Draft> {
        let read_from = plan.source(vfs);

        let text = fs::read_to_string(&read_from)
            .inspect_err(|err| warn!(path = %read_from.display(), "Record editor could not read the file: {}", err))
            .ok()?;

        let stamp = preview::stamp(&read_from)?;
        let mut draft = Draft {
            plan,
            read_from,
            stamp,
            text,
            records: Vec::new(),
            span: None,
            lines: Vec::new(),
            inputs: Vec::new(),
            failed: false,
        };

        draft.index();

        Some(draft)
    }

    fn index(&mut self) {
        let (records, span) = subtree(&self.text, &self.plan.path, self.plan.book.shallow(&self.plan.path));

        self.records = records;
        self.span = span;
        self.lines.clear();
        self.inputs.clear();

        for (at, record) in self.records.iter().enumerate() {
            for (slot, field) in record.fields.iter().enumerate() {
                self.lines.push((at, slot));
                self.inputs.push(shown(&field.value));
            }
        }
    }

    fn field(&self, index: usize) -> Option<&Field> {
        let (at, slot) = *self.lines.get(index)?;

        self.records.get(at)?.fields.get(slot)
    }

    fn drifted(&self) -> bool {
        preview::stamp(&self.read_from) != Some(self.stamp)
    }

    fn rule(&self, key: &str) -> Rule {
        match self.plan.values {
            EditorMode::Raw => Rule::Plain,
            EditorMode::Resolved => self.plan.book.rule(key),
        }
    }

    fn edit(&mut self, index: usize, typed: &str) -> bool {
        let Some(field) = self.field(index).cloned() else {
            return false;
        };

        if let Some(slot) = self.inputs.get_mut(index) {
            typed.clone_into(slot);
        }

        let rendered = match &field.value {
            Value::Number(_) => {
                let digits = typed.strip_prefix('-').unwrap_or(typed);

                if digits.is_empty() || !digits.chars().all(|glyph| glyph.is_ascii_digit()) {
                    return false;
                }

                let Ok(number) = typed.parse::<i64>() else {
                    return false;
                };

                let number = match self.rule(&field.key) {
                    Rule::Floor(least) => number.max(i64::from(least)),
                    Rule::Plain | Rule::Choice(_) => number,
                };

                number.to_string()
            }
            Value::Text(_) => json::quote(typed),
            Value::Numbers(_) => {
                let parsed: Option<Vec<i64>> = typed
                    .split(',')
                    .map(str::trim)
                    .filter(|item| !item.is_empty())
                    .map(|item| item.parse::<i64>().ok())
                    .collect();

                match (parsed, typed.contains(QUOTE)) {
                    (Some(items), _) => json::numbers(&self.text, &field.span, &items),
                    (None, true) => match json::tokenize(typed) {
                        Some(items) => json::tokens(&self.text, &field.span, &items),
                        None => return false,
                    },
                    (None, false) => return false,
                }
            }
            Value::Texts(_) if typed.contains(QUOTE) => match json::tokenize(typed) {
                Some(items) => json::tokens(&self.text, &field.span, &items),
                None => return false,
            },
            Value::Texts(_) => {
                let items: Vec<String> = typed.split(',').map(str::trim).filter(|item| !item.is_empty()).map(str::to_owned).collect();

                json::texts(&self.text, &field.span, &items)
            }
            Value::Tokens(_) => match json::tokenize(typed) {
                Some(items) => json::tokens(&self.text, &field.span, &items),
                None => return false,
            },
            Value::Flag(_) => match typed.trim() {
                "true" => "true".to_owned(),
                "false" => "false".to_owned(),
                _ => return false,
            },
            Value::Other => return false,
        };

        let Some(text) = json::replace(&self.text, &field.span, &rendered) else {
            return false;
        };

        if text == self.text {
            return false;
        }

        self.text = text;
        self.index();

        if let Some(slot) = self.inputs.get_mut(index)
            && !matches!(field.value, Value::Number(_) | Value::Other)
        {
            typed.clone_into(slot);
        }

        true
    }

    fn restore(&mut self, vfs: &Vfs) -> bool {
        let Ok(vanilla) = fs::read_to_string(&self.plan.game) else {
            warn!(path = %self.plan.game.display(), "Record editor could not read the vanilla file");

            return false;
        };

        if self.plan.book.shallow(&self.plan.path) {
            let Some(fields) = own_fields(&vanilla, &self.plan.path) else {
                warn!(path = %self.plan.game.display(), "Record editor found no vanilla copy of this record");

                return false;
            };

            for (key, value) in fields {
                let span = self.records.first().and_then(|record| record.field(&key)).map(|field| field.span.clone());

                if let Some(text) = span.and_then(|span| json::replace(&self.text, &span, &value)) {
                    self.text = text;
                    self.index();
                }
            }

            return self.persist(vfs);
        }

        let Some(original) = object(&vanilla, &self.plan.path).map(str::to_owned) else {
            warn!(path = %self.plan.game.display(), "Record editor found no vanilla copy of this record");

            return false;
        };

        let Some(span) = self.span.clone() else {
            return false;
        };

        let Some(text) = json::replace(&self.text, &span, &original) else {
            return false;
        };

        self.text = text;
        self.index();

        self.persist(vfs)
    }

    fn destination(&self, vfs: &Vfs) -> Option<(PathBuf, Stamp)> {
        if self.read_from != self.plan.game {
            return Some((self.read_from.clone(), self.stamp));
        }

        let path = mods::ensure_as(vfs, &self.plan.target_mod, &self.plan.game, &self.plan.file)
            .inspect_err(|err| warn!(source = %self.plan.game.display(), "Record editor could not stage the file: {}", err))
            .ok()?;

        if let Err(err) = vfs.create((self.plan.target_mod.as_str(), path.as_path())) {
            warn!(path = %path.display(), "Record editor staged a file the index would not take: {}", err);
        }

        preview::stamp(&path).map(|stamp| (path, stamp))
    }

    fn persist(&mut self, vfs: &Vfs) -> bool {
        let Some((path, stamp)) = self.destination(vfs) else {
            self.failed = true;

            return false;
        };

        match preview::save(&path, self.text.as_bytes(), stamp) {
            Ok(fresh) => {
                self.read_from = path;
                self.stamp = fresh;
                self.failed = false;

                true
            }
            Err(err) => {
                warn!(path = %path.display(), "Record editor could not write the file: {}", err);
                self.failed = true;

                false
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct Choice {
    raw: i32,
    label: &'static str,
}

impl std::fmt::Display for Choice {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.label)
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct Pick {
    raw: i32,
    label: String,
}

impl std::fmt::Display for Pick {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.label)
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Popup(popup::Message),
    Changed(usize, String),
    Sync,
    ConfirmExpired,
    FailureExpired,
}

#[derive(Default)]
pub(super) struct State {
    draft: Option<Draft>,
    frame: popup::State,
    confirm: Slot<()>,
    failed: Slot<()>,
}

pub(super) fn kind() -> popup::Kind {
    POPUP.kind()
}

impl State {
    pub(super) fn begin(&mut self, plan: Plan, nudge: usize, vfs: &Vfs) {
        self.frame = popup::cascaded(nudge);
        self.confirm.clear();
        self.failed.clear();
        self.draft = Draft::load(plan, vfs);
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
            Message::Changed(index, typed) => {
                let Some(draft) = self.draft.as_mut() else {
                    return Task::none();
                };

                if draft.edit(index, &typed) && !draft.persist(vfs) {
                    return self.failed.set((), Message::FailureExpired);
                }

                Task::none()
            }
            Message::Sync => {
                if !self.confirm.take(&()) {
                    return self.confirm.set((), Message::ConfirmExpired);
                }

                let restored = self.draft.as_mut().is_some_and(|draft| draft.restore(vfs));

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
        let listing: Element<'_, Message> = match draft.records.is_empty() {
            true => container(theme::centered_text(MISSING_LABEL).size(LABEL_SIZE).style(text::danger))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into(),
            false => {
                let mut rows = Column::new().spacing(ROW_GAP).width(Length::Fill);
                let mut index = 0;
                let depth = draft.plan.path.len();

                for record in &draft.records {
                    if let Some(suffix) = record.path.get(depth..).filter(|suffix| !suffix.is_empty()) {
                        let heading = match draft.plan.values {
                            EditorMode::Raw => suffix.join(PATH_JOINT),
                            EditorMode::Resolved => draft.plan.book.section(suffix),
                        };

                        rows = rows.push(container(text(heading).size(HEADING_SIZE)).padding(Padding::default().top(SECTION_GAP)));
                    }

                    for field in &record.fields {
                        rows = rows.push(field_row(draft, index, &field.key, &field.value));
                        index += 1;
                    }
                }

                smooth_scroll(
                    scrollable(container(rows).padding(Padding::default().right(SCROLL_GAP)))
                        .width(Length::Fill)
                        .height(Length::Fill),
                )
                .into()
            }
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

        column![framed, sync]
            .spacing(GAP)
            .padding(BODY_PADDING)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

fn field_row<'a>(draft: &'a Draft, index: usize, key: &'a str, value: &'a Value) -> Element<'a, Message> {
    let label = match draft.plan.values {
        EditorMode::Raw => key.to_owned(),
        EditorMode::Resolved => draft.plan.book.label(key),
    };

    let input = draft.inputs.get(index).map_or("", String::as_str);

    let named = match draft.plan.values {
        EditorMode::Raw => None,
        EditorMode::Resolved => draft.plan.named.iter().find(|(held, _)| *held == key).map(|(_, options)| options),
    };

    let control: Element<'_, Message> = match (draft.rule(key), value) {
        (_, Value::Number(number)) if named.is_some() => {
            let options: Vec<Pick> = named.map_or_else(Vec::new, |options| options.iter().map(|(raw, label)| Pick { raw: *raw, label: label.clone() }).collect());
            let current = number.parse::<i32>().ok();
            let chosen = options.iter().find(|option| Some(option.raw) == current).cloned();

            match chosen {
                Some(chosen) => pick_list(options, Some(chosen), move |picked: Pick| Message::Changed(index, picked.raw.to_string()))
                    .text_size(INPUT_SIZE)
                    .width(Length::Fixed(INPUT_WIDTH))
                    .style(theme::combo_box)
                    .into(),
                None => numeric(index, input),
            }
        }
        (Rule::Choice(choices), Value::Number(number)) => {
            let options: Vec<Choice> = choices.iter().map(|(raw, label)| Choice { raw: *raw, label }).collect();
            let current = number.parse::<i32>().ok();
            let chosen = options.iter().find(|option| Some(option.raw) == current).cloned();

            match chosen {
                Some(chosen) => pick_list(options, Some(chosen), move |picked: Choice| Message::Changed(index, picked.raw.to_string()))
                    .text_size(INPUT_SIZE)
                    .width(Length::Fixed(INPUT_WIDTH))
                    .style(theme::combo_box)
                    .into(),
                None => numeric(index, input),
            }
        }
        (_, Value::Other) => text(input).size(INPUT_SIZE).width(Length::Fixed(INPUT_WIDTH)).into(),
        _ => numeric(index, input),
    };

    row![text(label).size(LABEL_SIZE).width(Length::Fill), control].spacing(GAP).align_y(Vertical::Center).into()
}

fn numeric(index: usize, input: &str) -> Element<'_, Message> {
    text_input("", input)
        .on_input(move |typed| Message::Changed(index, typed))
        .padding(INPUT_PADDING)
        .size(INPUT_SIZE)
        .width(Length::Fixed(INPUT_WIDTH))
        .style(theme::rounded_input)
        .into()
}

pub(super) fn mark(plan: &Plan, vfs: &Vfs, reader: &mut Reader) -> Mark {
    let Some(held) = plan.held(vfs) else {
        return Mark::Same;
    };

    let shallow = plan.book.shallow(&plan.path);
    let found = |path: &Path, reader: &mut Reader| reader.text(path).and_then(|body| snapshot(&body, &plan.path, shallow));

    let ours = found(&held, reader);
    let theirs = found(&plan.game, reader);

    Mark::of(ours, theirs)
}

pub(super) fn reset(plan: Plan, vfs: &Vfs) -> bool {
    Draft::load(plan, vfs).is_some_and(|mut draft| draft.restore(vfs))
}

#[cfg(test)]
mod tests {
    use super::{prettify, tag};

    #[test]
    fn a_record_is_tagged_by_the_numbers_in_its_path() {
        let path: Vec<String> = ["MapID", "39001", "-1", "BonusNum", "0"].iter().map(|segment| (*segment).to_owned()).collect();

        assert_eq!(tag(&path), "39001/-1/0");
        assert_eq!(prettify("BonusTargetID"), "Bonus Target ID");
    }
}
