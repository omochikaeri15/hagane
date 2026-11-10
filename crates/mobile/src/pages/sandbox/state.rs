use std::cell::{Ref, RefCell};

use serde::{Deserialize, Serialize};
use tracing::warn;

use foundry::common::io::json;
use foundry::domains::sandbox::{Config, Roster};

use crate::settings::Orientation;
use crate::host::{self, Lag};

const STATE_FILE: &str = "sandbox.json";
pub const QUALITY_RANGE: std::ops::RangeInclusive<u8> = 15..=100;
const DEFAULT_QUALITY: u8 = 100;
const CPU_QUALITY: u8 = 65;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Volume {
    Off,
    Low,
    #[default]
    Medium,
    High,
}

impl Volume {
    pub const ALL: [Self; 4] = [Self::Off, Self::Low, Self::Medium, Self::High];

    pub fn percent(self) -> i32 {
        match self {
            Self::Off => 0,
            Self::Low => 20,
            Self::Medium => 50,
            Self::High => 100,
        }
    }

    pub fn nearest(percent: i32) -> Self {
        Self::ALL.into_iter().min_by_key(|step| (step.percent() - percent).abs()).unwrap_or_default()
    }

    pub fn index(self) -> i32 {
        Self::ALL.iter().position(|step| *step == self).map_or(0, |index| index as i32)
    }

    pub fn at(index: i32) -> Self {
        usize::try_from(index).ok().and_then(|index| Self::ALL.get(index).copied()).unwrap_or_default()
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Device {
    Tablet,
    #[default]
    Phone,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Scaling {
    #[default]
    Stretch,
    Pad,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Tab {
    #[default]
    Lineup,
    Stage,
    Config,
    Replay,
}

impl Tab {
    pub fn index(self) -> i32 {
        match self {
            Self::Lineup => 0,
            Self::Stage => 1,
            Self::Config => 2,
            Self::Replay => 3,
        }
    }

    pub fn at(index: i32) -> Self {
        match index {
            1 => Self::Stage,
            2 => Self::Config,
            3 => Self::Replay,
            _ => Self::Lineup,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct Saved {
    pub agreement: String,
    pub tab: Tab,
    pub panel: u8,
    pub roster: Roster,
    pub config: Config,
    pub search: String,
    pub music_volume: i32,
    pub effects_volume: i32,
    pub two_rows: bool,
    pub vibrate: bool,
    pub device_type: Option<Device>,
    pub image_quality: Option<u8>,
    pub scaling: Option<Scaling>,
    pub lag: Lag,
    pub battle_orientation: Orientation,
    pub fullscreen: bool,
    pub show_collision: bool,
}

impl Default for Saved {
    fn default() -> Self {
        Self {
            agreement: String::new(),
            tab: Tab::default(),
            panel: 0,
            roster: Roster::default(),
            config: Config::default(),
            search: String::new(),
            music_volume: Volume::Medium.percent(),
            effects_volume: Volume::Medium.percent(),
            two_rows: true,
            vibrate: false,
            device_type: None,
            image_quality: None,
            scaling: None,
            lag: Lag::default(),
            battle_orientation: Orientation::Landscape,
            fullscreen: true,
            show_collision: false,
        }
    }
}

impl Saved {
    pub fn quality(&self) -> u8 {
        self.image_quality.unwrap_or_else(|| if host::hardware_drawn() { DEFAULT_QUALITY } else { CPU_QUALITY }).clamp(*QUALITY_RANGE.start(), *QUALITY_RANGE.end())
    }

    pub fn device(&self) -> Device {
        self.device_type.unwrap_or_default()
    }

    pub fn scaling(&self) -> Scaling {
        self.scaling.unwrap_or_default()
    }

    pub fn render_scale(&self) -> f32 {
        f32::from(self.quality()) / f32::from(*QUALITY_RANGE.end())
    }

    pub fn native(&self) -> bool {
        self.quality() >= *QUALITY_RANGE.end()
    }

    pub fn padded(&self) -> bool {
        !self.native() && self.scaling() == Scaling::Pad
    }
}

pub struct Store {
    held: RefCell<Saved>,
}

impl Store {
    pub fn load() -> Self {
        Self { held: RefCell::new(json::salvage(json::state_file(STATE_FILE))) }
    }

    pub fn get(&self) -> Ref<'_, Saved> {
        self.held.borrow()
    }

    pub fn update(&self, change: impl FnOnce(&mut Saved)) {
        change(&mut self.held.borrow_mut());

        if let Err(err) = json::persist(json::state_file(STATE_FILE), &*self.held.borrow()) {
            warn!("The sandbox state could not be saved: {err}");
        }
    }
}
