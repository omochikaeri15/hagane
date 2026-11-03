use std::cell::RefCell;

use serde::{Deserialize, Serialize};
use tracing::warn;

use foundry::common::io::json;

const STATE_FILE: &str = "state.json";

#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(default)]
pub struct Remembered {
    pub mounted: Option<String>,
    pub last_cat: Option<u32>,
    pub last_cat_form: usize,
    pub last_enemy: Option<u32>,
    pub last_stage: Option<(String, u32, u32)>,
    pub last_mod: Option<String>,
    pub files_mount: Option<String>,
    pub files_mode: u8,
    pub import_tab: u8,
    pub device_type: u8,
    pub device_region: u8,
    pub source_agreement: String,
    pub utilities_tool: u8,
    pub help_page: usize,
    pub settings_tab: u8,
    pub export_format: u8,
    pub export_quality: Option<i32>,
    pub export_compression: Option<i32>,
    pub export_background: bool,
    pub background_quality: Option<i32>,
    pub background_compression: Option<i32>,
    pub background_format: u8,
}

pub struct Memory {
    held: RefCell<Remembered>,
}

impl Memory {
    pub fn load() -> Self {
        Self { held: RefCell::new(json::salvage(json::state_file(STATE_FILE))) }
    }

    pub fn get(&self) -> Remembered {
        self.held.borrow().clone()
    }

    pub fn mounted(&self) -> Option<String> {
        self.held.borrow().mounted.clone()
    }

    pub fn update(&self, change: impl FnOnce(&mut Remembered)) {
        change(&mut self.held.borrow_mut());

        if let Err(err) = json::persist(json::state_file(STATE_FILE), &*self.held.borrow()) {
            warn!("The app state could not be saved: {err}");
        }
    }
}
