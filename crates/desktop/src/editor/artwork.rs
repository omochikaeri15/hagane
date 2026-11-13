use std::fs;
use std::path::{Path, PathBuf};

use iced::widget::{button, column, container, row, text};
use iced::{Element, Length, Size, Task};
use tracing::{info, warn};

use foundry::common::preview::{self, Preview, Stamp};
use foundry::domains::mods;
use foundry::Vfs;

use crate::app::theme;
use crate::common::dialog;
use crate::common::feedback::{Slot, CONFIRM_LABEL, FAILURE_LABEL};
use crate::widget::{picture, popup};

use super::Mark;

const SIZE: Size = Size::new(440.0, 480.0);
const POPUP: popup::Spec = popup::Spec::new(popup::Kind::Artwork, SIZE);

const BODY_PADDING: f32 = 12.0;
const GAP: f32 = 8.0;
const BUTTON_SIZE: f32 = 13.0;
const BUTTON_PADDING: f32 = 8.0;
const FRAME_GAP: f32 = 3.0;
const FRAME_INSET: f32 = theme::CARD_BORDER_WIDTH + FRAME_GAP;
const NOTICE_SIZE: f32 = 14.0;

const UPLOAD_LABEL: &str = "Upload New PNG";
const SYNC_LABEL: &str = "Sync With \"game\"";
const MISSING_LABEL: &str = "No image yet";
const PNG_FILTER: &str = "PNG Image";
const PNG_EXTENSIONS: &[&str] = &["png"];

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
}

#[derive(Debug, Clone)]
pub enum Message {
    Popup(popup::Message),
    Picture(picture::Message),
    Upload,
    Chosen(Option<PathBuf>),
    Sync,
    ConfirmExpired,
    FailureExpired,
}

struct Draft {
    plan: Plan,
    shown: Option<PathBuf>,
    owned: bool,
    stamp: Option<Stamp>,
    image: Option<picture::Source>,
}

impl Draft {
    fn load(plan: Plan, vfs: &Vfs) -> Draft {
        let held = mod_copy(&plan, vfs);
        let owned = held.is_some();
        let shown = held.or_else(|| plan.game.clone());
        let stamp = shown.as_deref().and_then(preview::stamp);

        let image = shown.as_deref().and_then(|path| match preview::load(path) {
            Preview::Image { bytes, width, height, .. } => Some(picture::Source::new(bytes, width, height)),
            Preview::Text { .. } | Preview::Oversized | Preview::Binary | Preview::Unavailable => {
                warn!(path = %path.display(), "Image editor could not show the file as an image");

                None
            }
        });

        Draft { plan, shown, owned, stamp, image }
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

#[derive(Default)]
pub(super) struct State {
    draft: Option<Draft>,
    frame: popup::State,
    picture: picture::State,
    confirm: Slot<Intent>,
    failed: Slot<Intent>,
}

impl State {
    pub(super) fn begin(&mut self, plan: Plan, nudge: usize, vfs: &Vfs) {
        self.frame = popup::cascaded(nudge);
        self.picture.reset();
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
            self.draft = None;

            return;
        };

        if current.plan.file != plan.file {
            self.picture.reset();
        }

        if current.plan != plan || current.drifted() || current.moved(vfs) {
            self.draft = Some(Draft::load(plan, vfs));
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
            Message::Picture(msg) => {
                self.picture.update(msg);

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

                Task::perform(dialog::file(PNG_FILTER, PNG_EXTENSIONS), Message::Chosen)
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

        let surface: Element<'_, Message> = match &draft.image {
            Some(source) => self.picture.view(source).map(Message::Picture),
            None => container(theme::centered_text(MISSING_LABEL).size(NOTICE_SIZE).style(text::danger))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into(),
        };

        let framed = container(surface)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(FRAME_INSET)
            .style(theme::card_container_outlined);

        column![row![upload, sync].spacing(GAP), framed]
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

fn upload(plan: &Plan, source: &Path, vfs: &Vfs) -> Option<PathBuf> {
    let bytes = fs::read(source)
        .inspect_err(|err| warn!(path = %source.display(), "Image editor could not read the upload: {}", err))
        .ok()?;

    if !preview::is_png(&bytes) {
        warn!(path = %source.display(), "Image editor refused an upload that is not a PNG");

        return None;
    }

    place(plan, source, vfs)
}

fn restore(plan: &Plan, vfs: &Vfs) -> Option<PathBuf> {
    let Some(game) = plan.game.as_deref() else {
        warn!(file = plan.file, "Image editor has no game copy to sync from");

        return None;
    };

    place(plan, game, vfs)
}

fn place(plan: &Plan, source: &Path, vfs: &Vfs) -> Option<PathBuf> {
    let placed = mods::place(&plan.target_mod, source, &plan.file)
        .inspect_err(|err| warn!(file = plan.file, "Image editor could not write into the mod: {}", err))
        .ok()?;

    if let Err(err) = vfs.create((plan.target_mod.as_str(), placed.as_path())) {
        warn!(path = %placed.display(), "Image editor wrote a file the index would not take: {}", err);
    }

    info!(path = %placed.display(), "Image editor placed an image into the mod");

    Some(placed)
}
