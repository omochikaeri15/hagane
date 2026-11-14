use std::fs;
use std::path::{Path, PathBuf};

use iced::widget::{button, column, container, row, text};
use iced::{Element, Length, Size, Task};
use tracing::{info, warn};

use foundry::common::preview::{self, Stamp};
use foundry::domains::mods;
use foundry::Vfs;

use crate::app::theme;
use crate::common::dialog;
use crate::common::feedback::{Slot, CONFIRM_LABEL, FAILURE_LABEL};
use crate::systems::host::Audition;
use crate::widget::popup;

use super::Mark;

const SIZE: Size = Size::new(440.0, 260.0);
const POPUP: popup::Spec = popup::Spec::new(popup::Kind::Audio, SIZE);

const BODY_PADDING: f32 = 12.0;
const GAP: f32 = 8.0;
const BUTTON_SIZE: f32 = 13.0;
const BUTTON_PADDING: f32 = 8.0;
const NOTICE_SIZE: f32 = 14.0;
const DETAIL_SIZE: f32 = 12.0;
const KILOBYTE: u64 = 1024;

pub(super) const TITLE: &str = "Sound";
pub(super) const EXTENSIONS: [&str; 2] = ["ogg", "caf"];

const UPLOAD_LABEL: &str = "Upload New Sound";
const SYNC_LABEL: &str = "Sync With \"game\"";
const PLAY_LABEL: &str = "Play";
const STOP_LABEL: &str = "Stop";
const MISSING_LABEL: &str = "No sound yet";
const MOD_LABEL: &str = "Playing from the mod";
const GAME_LABEL: &str = "Playing from the game";
const FILTER: &str = "Sound";

struct Format {
    picks: &'static [&'static str],
    magic: &'static [u8],
}

const FORMATS: [Format; 2] = [Format { picks: &["ogg"], magic: b"OggS" }, Format { picks: &["caf"], magic: b"caff" }];

pub(super) fn kind() -> popup::Kind {
    POPUP.kind()
}

#[derive(Clone, PartialEq)]
pub(crate) struct Plan {
    label: String,
    file: String,
    game: Option<PathBuf>,
    target_mod: String,
}

pub(super) fn plan(label: String, file: String, game: Option<PathBuf>, target_mod: String) -> Plan {
    Plan { label, file, game, target_mod }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Intent {
    Upload,
    Sync,
    Play,
}

#[derive(Debug, Clone)]
pub enum Message {
    Popup(popup::Message),
    Upload,
    Chosen(Option<PathBuf>),
    Sync,
    Play,
    Stop,
    ConfirmExpired,
    FailureExpired,
}

struct Draft {
    plan: Plan,
    shown: Option<PathBuf>,
    owned: bool,
    stamp: Option<Stamp>,
    detail: String,
}

impl Draft {
    fn load(plan: Plan, vfs: &Vfs) -> Draft {
        let held = mod_copy(&plan, vfs);
        let owned = held.is_some();
        let shown = held.or_else(|| plan.game.clone());
        let stamp = shown.as_deref().and_then(preview::stamp);
        let size = shown.as_deref().and_then(|path| fs::metadata(path).ok()).map_or(0, |held| held.len());
        let detail = format!("{} ({} KB)", plan.file, size.div_ceil(KILOBYTE));

        Draft { plan, shown, owned, stamp, detail }
    }

    fn drifted(&self) -> bool {
        self.shown.as_deref().and_then(preview::stamp) != self.stamp
    }

    fn moved(&self, vfs: &Vfs) -> bool {
        mod_copy(&self.plan, vfs).is_some() != self.owned
    }
}

fn mod_copy(plan: &Plan, vfs: &Vfs) -> Option<PathBuf> {
    mods::find(vfs, &plan.target_mod, Path::new(&plan.file))
}

fn format(file: &str) -> Option<&'static Format> {
    let (_, held) = file.rsplit_once('.')?;

    FORMATS.iter().find(|format| format.picks.iter().any(|known| held.eq_ignore_ascii_case(known)))
}

#[derive(Default)]
pub(super) struct State {
    draft: Option<Draft>,
    frame: popup::State,
    audition: Audition,
    confirm: Slot<Intent>,
    failed: Slot<Intent>,
}

impl State {
    pub(super) fn begin(&mut self, plan: Plan, nudge: usize, vfs: &Vfs) {
        self.frame = popup::cascaded(nudge);
        self.audition.stop();
        self.confirm.clear();
        self.failed.clear();
        self.draft = Some(Draft::load(plan, vfs));
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
            self.audition.stop();
            self.draft = None;

            return;
        };

        if current.plan != plan || current.drifted() || current.moved(vfs) {
            self.audition.stop();
            self.draft = Some(Draft::load(plan, vfs));
        }
    }

    pub(super) fn update(&mut self, message: Message, vfs: &Vfs) -> Task<Message> {
        match message {
            Message::Popup(msg) => {
                if self.frame.update(msg, POPUP) {
                    self.audition.stop();
                    self.draft = None;
                    self.confirm.clear();
                    self.failed.clear();
                }

                Task::none()
            }
            Message::Upload => {
                let Some(draft) = self.draft.as_ref() else {
                    return Task::none();
                };

                if draft.owned && !self.confirm.take(&Intent::Upload) {
                    return self.confirm.set(Intent::Upload, Message::ConfirmExpired);
                }

                self.confirm.clear();

                let Some(wanted) = format(&draft.plan.file) else {
                    return self.failed.set(Intent::Upload, Message::FailureExpired);
                };

                Task::perform(dialog::file(FILTER, wanted.picks), Message::Chosen)
            }
            Message::Chosen(None) => Task::none(),
            Message::Chosen(Some(source)) => {
                let placed = self.draft.as_ref().and_then(|draft| upload(&draft.plan, &source, vfs));

                self.settle(placed, Intent::Upload, vfs)
            }
            Message::Sync => {
                if !self.confirm.take(&Intent::Sync) {
                    return self.confirm.set(Intent::Sync, Message::ConfirmExpired);
                }

                let placed = self.draft.as_ref().and_then(|draft| restore(&draft.plan, vfs));

                self.settle(placed, Intent::Sync, vfs)
            }
            Message::Play => {
                let played = self.draft.as_ref().and_then(|draft| {
                    let path = draft.shown.as_deref()?;
                    let bytes = fs::read(path)
                        .inspect_err(|err| warn!(path = %path.display(), "Sound editor could not read the file to play it: {}", err))
                        .ok()?;

                    Some((draft.plan.file.as_str(), bytes))
                });

                match played.is_some_and(|(name, bytes)| self.audition.play(name, bytes)) {
                    true => Task::none(),
                    false => self.failed.set(Intent::Play, Message::FailureExpired),
                }
            }
            Message::Stop => {
                self.audition.stop();

                Task::none()
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

    fn settle(&mut self, placed: Option<PathBuf>, intent: Intent, vfs: &Vfs) -> Task<Message> {
        let Some(draft) = self.draft.take() else {
            return Task::none();
        };

        if placed.is_none() {
            self.draft = Some(draft);

            return self.failed.set(intent, Message::FailureExpired);
        }

        self.audition.stop();
        self.failed.clear();
        self.draft = Some(Draft::load(draft.plan, vfs));

        Task::none()
    }

    pub(super) fn view(&self, window: Size) -> Option<Element<'_, Message>> {
        let draft = self.draft.as_ref()?;

        Some(self.frame.view(&draft.plan.label, POPUP, window, Message::Popup, move || self.body(draft), None))
    }

    fn body<'a>(&'a self, draft: &'a Draft) -> Element<'a, Message> {
        let upload = self.action(Intent::Upload, UPLOAD_LABEL, theme::primary_button)
            .on_press(Message::Upload);

        let sync = self.action(Intent::Sync, SYNC_LABEL, theme::danger_button)
            .on_press_maybe(draft.plan.game.is_some().then_some(Message::Sync));

        let play = self.action(Intent::Play, PLAY_LABEL, theme::primary_button)
            .on_press_maybe(draft.shown.is_some().then_some(Message::Play));

        let stop = button(theme::button_label(STOP_LABEL).size(BUTTON_SIZE))
            .width(Length::Fill)
            .padding(BUTTON_PADDING)
            .style(theme::neutral_button)
            .on_press(Message::Stop);

        let facts: Element<'_, Message> = match (draft.shown.is_some(), draft.owned) {
            (false, _) => theme::centered_text(MISSING_LABEL).size(NOTICE_SIZE).style(text::danger).into(),
            (true, owned) => column![
                theme::centered_text(draft.detail.as_str()).size(NOTICE_SIZE),
                theme::centered_text(if owned { MOD_LABEL } else { GAME_LABEL }).size(DETAIL_SIZE).style(text::secondary),
            ]
            .spacing(GAP)
            .width(Length::Fill)
            .into(),
        };

        let framed = container(facts)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(theme::card_container_outlined);

        column![row![upload, sync].spacing(GAP), framed, row![play, stop].spacing(GAP)]
            .spacing(GAP)
            .padding(BODY_PADDING)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn action<'a>(&self, intent: Intent, idle: &'a str, style: theme::ButtonStyleFn) -> button::Button<'a, Message> {
        let (label, style): (&str, theme::ButtonStyleFn) = match (self.failed.armed_for(&intent), self.confirm.armed_for(&intent)) {
            (true, _) => (FAILURE_LABEL, theme::danger_button),
            (false, true) => (CONFIRM_LABEL, theme::danger_button),
            (false, false) => (idle, style),
        };

        button(theme::button_label(label).size(BUTTON_SIZE))
            .width(Length::Fill)
            .padding(BUTTON_PADDING)
            .style(style)
    }
}

pub(super) fn mark(plan: &Plan, vfs: &Vfs) -> Mark {
    let Some(held) = mod_copy(plan, vfs) else {
        return Mark::Same;
    };

    let ours = fs::read(&held).ok();
    let theirs = plan.game.as_deref().and_then(|game| fs::read(game).ok());

    Mark::of(ours, theirs)
}

pub(super) fn reset(plan: &Plan, vfs: &Vfs) -> bool {
    restore(plan, vfs).is_some()
}

fn fits(file: &str, bytes: &[u8]) -> bool {
    format(file).is_some_and(|wanted| bytes.starts_with(wanted.magic))
}

fn upload(plan: &Plan, source: &Path, vfs: &Vfs) -> Option<PathBuf> {
    let bytes = fs::read(source)
        .inspect_err(|err| warn!(path = %source.display(), "Sound editor could not read the upload: {}", err))
        .ok()?;

    if !fits(&plan.file, &bytes) {
        warn!(path = %source.display(), file = plan.file, "Sound editor refused an upload that is not the format the file holds");

        return None;
    }

    place(plan, source, vfs)
}

fn restore(plan: &Plan, vfs: &Vfs) -> Option<PathBuf> {
    let Some(game) = plan.game.as_deref() else {
        warn!(file = plan.file, "Sound editor has no game copy to sync from");

        return None;
    };

    place(plan, game, vfs)
}

fn place(plan: &Plan, source: &Path, vfs: &Vfs) -> Option<PathBuf> {
    let placed = mods::place(&plan.target_mod, source, &plan.file)
        .inspect_err(|err| warn!(file = plan.file, "Sound editor could not write into the mod: {}", err))
        .ok()?;

    if let Err(err) = vfs.create((plan.target_mod.as_str(), placed.as_path())) {
        warn!(path = %placed.display(), "Sound editor wrote a file the index would not take: {}", err);
    }

    info!(path = %placed.display(), "Sound editor placed a sound into the mod");

    Some(placed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_upload_must_be_the_format_the_file_holds() {
        assert!(fits("012.ogg", b"OggS\0\x02"));
        assert!(fits("snd003.CAF", b"caff\0\x01"));

        // A renamed file would reach the game as noise or a crash.
        assert!(!fits("012.ogg", b"caff\0\x01"));
        assert!(!fits("012.wav", b"RIFF"));
    }
}
