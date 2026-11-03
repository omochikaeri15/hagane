use std::cell::{Cell, RefCell};

use serde::{Deserialize, Serialize};
use tracing::warn;

use foundry::common::io::json;
use foundry::domains::settings::{Settings, lang, nightly};

const SETTINGS_FILE: &str = "settings.json";

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Orientation {
    Automatic,
    #[default]
    Portrait,
    Landscape,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Exports {
    AppFolder,
    #[serde(alias = "Downloads")]
    Ask,
}

impl Default for Exports {
    fn default() -> Self {
        if cfg!(target_os = "android") { Self::Ask } else { Self::AppFolder }
    }
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Saved {
    pub settings: Settings,
    pub orientation: Orientation,
    pub exports: Exports,
}

impl Saved {
    pub fn load() -> Self {
        let mut saved: Self = json::salvage(json::config_file(SETTINGS_FILE));

        lang::ensure_complete_list(&mut saved.settings.general.language_priority);
        nightly::settle(&mut saved.settings.general.enable_nightly);

        saved
    }
}

pub struct State {
    settings: RefCell<Settings>,
    orientation: Cell<Orientation>,
    exports: Cell<Exports>,
}

impl State {
    pub fn new(saved: Saved) -> Self {
        Self { settings: RefCell::new(saved.settings), orientation: Cell::new(saved.orientation), exports: Cell::new(saved.exports) }
    }

    pub fn settings(&self) -> std::cell::Ref<'_, Settings> {
        self.settings.borrow()
    }

    pub fn snapshot(&self) -> Settings {
        self.settings.borrow().clone()
    }

    pub fn orientation(&self) -> Orientation {
        self.orientation.get()
    }

    pub fn set_orientation(&self, orientation: Orientation) {
        self.orientation.set(orientation);
        self.save();
    }

    pub fn exports(&self) -> Exports {
        self.exports.get()
    }

    pub fn set_exports(&self, exports: Exports) {
        self.exports.set(exports);
        self.save();
    }

    pub fn apply(&self, change: impl FnOnce(&mut Settings)) {
        change(&mut self.settings.borrow_mut());
        self.save();
    }

    fn save(&self) {
        let saved = Saved { settings: self.settings.borrow().clone(), orientation: self.orientation.get(), exports: self.exports.get() };

        if let Err(err) = json::persist(json::config_file(SETTINGS_FILE), &saved) {
            warn!("The settings could not be saved: {err}");
        }
    }
}
