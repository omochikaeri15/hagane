pub mod abilities;
pub mod lens;
pub mod names;
pub mod registry;

pub use names::NameBook;

use std::collections::{HashMap, HashSet};

use codex::cat::unit::{LevelCurve, Talent};
use codex::combat::{range_of, AttrUnit, AttrValue, Entity, Identity, REGISTRY};

use crate::common::context::GlobalContext;
use crate::systems::combat::registry::Magnification;

pub const ABILITY_X: f32 = 3.0;
pub const ABILITY_Y: f32 = 5.0;
pub const TRAIT_Y: f32 = 7.0;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
pub enum CustomIcon {
    #[default] None,
    Multihit,
    Kamikaze,
    BossWave,
    Dojo,
    StarredAlien,
    Burrow,
    Revive,
    Stop,
    DeathTimer,
    God,
    Unknown,
}

const TONE_HIT: [u8; 4] = [76, 196, 100, 255];
const TONE_MISS: [u8; 4] = [222, 76, 76, 255];
const TONE_INERT: [u8; 4] = [236, 164, 44, 255];

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
pub enum Mark {
    #[default] None,
    Hit,
    Miss,
    Inert,
    Mixed,
    Glancing,
}

impl Mark {
    pub fn tones(self) -> Option<([u8; 4], [u8; 4])> {
        match self {
            Mark::None => None,
            Mark::Hit => Some((TONE_HIT, TONE_HIT)),
            Mark::Miss => Some((TONE_MISS, TONE_MISS)),
            Mark::Inert => Some((TONE_INERT, TONE_INERT)),
            Mark::Mixed => Some((TONE_MISS, TONE_HIT)),
            Mark::Glancing => Some((TONE_HIT, TONE_INERT)),
        }
    }
}

#[derive(Clone, Debug)]
pub struct AbilityItem {
    pub identity: Identity,
    pub icon_id: Option<usize>,
    pub text: String,
    pub custom_icon: CustomIcon,
    pub border_id: Option<usize>,
    pub mark: Mark,
    pub added: bool,
    pub dimmed: bool,
    pub switchable: bool,
}

pub type AbilityGroups = (Vec<AbilityItem>, Vec<AbilityItem>, Vec<AbilityItem>, Vec<AbilityItem>, Vec<AbilityItem>, Vec<AbilityItem>);

#[derive(Clone, Copy)]
pub struct RenderContext<'a> {
    pub global: GlobalContext<'a>,
    pub base_stats: &'a Entity,
    pub final_stats: &'a Entity,
    pub magnification: Magnification,
    pub current_level: i32,
    pub level_curve: Option<&'a LevelCurve>,
    pub talent_data: Option<&'a Talent>,
    pub talent_levels: Option<&'a HashMap<u8, u8>>,
    pub is_conjure_unit: bool,
}

impl<'a> RenderContext<'a> {
    pub fn enemy(global: GlobalContext<'a>, stats: &'a Entity, magnification: Magnification) -> Self {
        Self {
            global,
            base_stats: stats,
            final_stats: stats,
            magnification,
            current_level: 0,
            level_curve: None,
            talent_data: None,
            talent_levels: None,
            is_conjure_unit: false,
        }
    }
}

pub fn present_identities<'a>(entities: impl Iterator<Item = &'a Entity>) -> HashSet<Identity> {
    let mut present = HashSet::new();

    for entity in entities {
        for ability in REGISTRY {
            if present.contains(&ability.identity) { continue; }

            if !(ability.attributes)(entity).is_empty() {
                present.insert(ability.identity);
            }
        }
    }

    present
}

#[derive(Clone, Default)]
pub struct TalentIdentities {
    pub normal: HashSet<Identity>,
    pub ultra: HashSet<Identity>,
}

pub fn talent_identities<'a>(talents: impl Iterator<Item = &'a Talent>) -> TalentIdentities {
    let mut referenced = TalentIdentities::default();

    for talent in talents {
        for group in &talent.groups {
            let side = if group.limit == 1 { &mut referenced.ultra } else { &mut referenced.normal };

            for ability in REGISTRY {
                let Some(talent_id) = ability.talent_id else { continue; };

                if abilities::talent_grants(group, talent.type_id, talent_id) {
                    side.insert(ability.identity);
                }
            }
        }
    }

    referenced
}

pub(crate) fn comparable(value: AttrValue) -> i32 {
    match value {
        AttrValue::Finite(amount) => amount,
        AttrValue::Infinite => i32::MAX,
    }
}

pub(crate) fn displayed(value: AttrValue, unit: AttrUnit) -> f64 {
    let amount = comparable(value);

    match unit {
        AttrUnit::Position => f64::from(range_of(amount)),
        AttrUnit::None | AttrUnit::Percent | AttrUnit::Frames | AttrUnit::Range => f64::from(amount),
    }
}
