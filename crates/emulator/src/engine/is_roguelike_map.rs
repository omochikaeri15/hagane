use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    rc::Rc,
};

use crate::Fault;

use super::{AppContext, LineupPreset, get_crown_level, get_global_map_id, get_scene_id};

#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct RoguelikeMapEntry {
    pub map_ids: Vec<i32>,
    pub levels: Vec<i32>,
    pub preset_file_name: Vec<u8>,
}

#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct RoguelikeEvent {
    pub entries: Vec<RoguelikeMapEntry>,
    pub reset_map_id: i32,
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct RoguelikeChara {
    pub unit_id: i32,
    pub form: i32,
    pub top_form: i32,
    pub level: i32,
    pub base_level: i32,
    pub plus_level: i32,
    pub base_plus: i32,
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct RoguelikeBonus {
    pub kind: i32,
    pub id: i32,
    pub level: i32,
}

#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct RoguelikePending {
    pub kind: i32,
    pub stage: i32,
    pub next_draw: i32,
    pub bonus_kind: i32,
    pub ids: Vec<i32>,
}

#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct RoguelikeProgress {
    pub roster: Vec<RoguelikeChara>,
    pub cannon_part: i32,
    pub style_part: i32,
    pub foundation_part: i32,
    pub stage_index: i32,
    pub pending_stage: i32,
    pub pending: VecDeque<RoguelikePending>,
    pub bonuses: Vec<RoguelikeBonus>,
    pub carried_bonuses: Vec<RoguelikeBonus>,
}

#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct RoguelikeRun {
    pub maps: BTreeMap<(i32, i32), Rc<RefCell<RoguelikeProgress>>>,
    pub permanent_bonuses: Vec<RoguelikeBonus>,
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct RoguelikeEffectAbility {
    pub kind: i32,
    pub order: i32,
    pub ability_id: i32,
    pub img1: i32,
    pub img2: i32,
    pub content: i32,
}

#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct RoguelikeBuffSkill {
    pub ability_id: i32,
    pub max_level: i32,
    pub params: Vec<i32>,
    pub effect: RoguelikeEffectAbility,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RoguelikeEffect {
    PowerUp { level: i32, plus: i32 },
    Instinct(Rc<RoguelikeBuffSkill>),
    Combo { effect: i32, band: i32 },
    CatCannon { cannon: i32, level: i32 },
}

impl RoguelikeEffect {
    pub fn kind(&self) -> i32 {
        match self {
            Self::PowerUp { .. } => 0,
            Self::Instinct(_) => 1,
            Self::Combo { .. } => 2,
            Self::CatCannon { .. } => 3,
        }
    }

    pub fn first(&self) -> i32 {
        match self {
            Self::PowerUp { level, .. } => *level,
            Self::Instinct(skill) => skill.ability_id,
            Self::Combo { effect, .. } => *effect,
            Self::CatCannon { cannon, .. } => *cannon,
        }
    }

    pub fn second(&self) -> i32 {
        match self {
            Self::PowerUp { plus, .. } => *plus,
            Self::Instinct(skill) => skill.max_level,
            Self::Combo { band, .. } => *band,
            Self::CatCannon { level, .. } => *level,
        }
    }
}

#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct RoguelikeBuff {
    pub id: i32,
    pub rarity: i32,
    pub limit: i32,
    pub target_rarity: i32,
    pub target_chara: i32,
    pub name: Vec<u8>,
    pub explanation: Vec<u8>,
    pub effects: Vec<Option<RoguelikeEffect>>,
}

#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct RoguelikeLotteryGroup {
    pub target: i32,
    pub target_ids: Vec<i32>,
}

#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct RoguelikeCharaBonus {
    pub id: i32,
    pub rarity: i32,
    pub chara_id: i32,
    pub level: i32,
    pub plus_level: i32,
}

#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct RoguelikeMaster {
    pub charas: BTreeMap<i32, RoguelikeCharaBonus>,
    pub effect_abilities: BTreeMap<i32, RoguelikeEffectAbility>,
    pub skills: BTreeMap<i32, Rc<RoguelikeBuffSkill>>,
    pub buffs: BTreeMap<i32, RoguelikeBuff>,
    pub lottery_groups: BTreeMap<i32, RoguelikeLotteryGroup>,
    pub permanent: BTreeMap<i32, BTreeMap<i32, BTreeMap<i32, Vec<i32>>>>,
}


#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct Roguelike {
    pub settings: BTreeMap<i32, RoguelikeEvent>,
    pub runs: BTreeMap<i32, RoguelikeRun>,
    pub lineup: LineupPreset,
    pub notice_step: i32,
    pub bonus_tutorial_seen: bool,
    pub clear_tutorial_seen: bool,
    pub lose_tutorial_seen: bool,
    pub master: Rc<RoguelikeMaster>,
}

pub fn is_roguelike_map(ctx: &mut AppContext, map_id: i32, crown: i32) -> Result<bool, Fault> {
    let mut map_id = map_id;
    let mut crown = crown;

    if get_scene_id(ctx)? == 0x64
        && (ctx.i32_at(AppContext::SCENE_0X64_PAGE)? == 3 || ctx.i32_at(AppContext::SCENE_0X64_PAGE)? == 0xa)
        && ctx.i32_at(AppContext::STAGE_SELECT_STATE)? != 3
        && ctx.i32_at(AppContext::STAGE_SELECT_STATE)? != 2
    {
        return Ok(false);
    }

    if map_id == -1 {
        map_id = get_global_map_id(ctx, 0)?;
    }

    if crown == -1 {
        crown = get_crown_level(ctx)?;
    }

    for event in ctx.roguelike.settings.values() {
        for entry in &event.entries {
            if !entry.map_ids.contains(&map_id) {
                continue;
            }

            if !entry.levels.contains(&crown) {
                continue;
            }

            return Ok(true);
        }
    }

    Ok(false)
}
