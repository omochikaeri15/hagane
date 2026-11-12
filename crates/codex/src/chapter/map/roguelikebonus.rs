use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::cat::unit::TalentGroup;
use crate::combat::{get_talent, AttrUnit, Entity, Identity};
use crate::common::file::scrub;
use crate::files::Localizable;

const CHARA_ROOT: &str = "BonusCharaID";
const BUFF_ROOT: &str = "BonusBuffID";
const SKILL_ROOT: &str = "BuffSkillID";
const GROUP_ROOT: &str = "BonusGroupID";
const EVENT_ROOT: &str = "EventID";
const MAP_ROOT: &str = "MapID";
const EVERY: i32 = -1;
const CHARA_TARGET: i32 = 0;
const BUFF_TARGET: i32 = 1;
const ULTRA_TOTAL: i32 = 0x3a;
const TRUE_TOTAL: i32 = 0x1c;
const EVOLVED_TOTAL: i32 = 9;
const COST_DISCOUNT: i32 = 1000;
const FIRST_EFFECT_UP: i32 = 1001;
const KILL_ATTACK_UP: i32 = 1008;
const EXTRA_SPAWN: i32 = 1009;
const INSANE_DAMAGE: i32 = 1010;
const INSANELY_TOUGH: i32 = 1011;
const COST_DOWN: i32 = 1012;
const PRODUCTION_UP: i32 = 1013;
const WHOLE: i32 = 100;

pub fn roguelike_form(total: i32, forms: i32) -> i32 {
    let reached = match total {
        total if total > ULTRA_TOTAL => 3,
        total if total > TRUE_TOTAL => 2,
        total if total >= EVOLVED_TOTAL => 1,
        _ => 0,
    };

    reached.min(forms.saturating_sub(1))
}

pub fn roguelike_grants(stats: &mut Entity, grants: &[(i32, Vec<i32>, bool)]) {
    for (index, (ability, _, _)) in grants.iter().enumerate() {
        if grants[..index].iter().any(|(earlier, _, _)| earlier == ability) {
            continue;
        }

        let mut total: Vec<i32> = Vec::new();
        let mut maxed = true;

        for (_, values, full) in grants.iter().filter(|(other, _, _)| other == ability) {
            if total.len() < values.len() {
                total.resize(values.len(), 0);
            }

            for (cell, value) in total.iter_mut().zip(values) {
                *cell = cell.saturating_add(*value);
            }

            maxed &= *full;
        }

        grant(stats, *ability, &total, maxed);
    }
}

fn grant(stats: &mut Entity, ability: i32, values: &[i32], maxed: bool) {
    let first = values.first().copied().unwrap_or(0);
    let cut = WHOLE.saturating_sub(first);
    let raised = usize::try_from(ability.saturating_sub(FIRST_EFFECT_UP)).ok().filter(|index| *index < stats.effect_ups.len());
    let granted = match ability {
        COST_DOWN | COST_DISCOUNT => {
            stats.eoc1_cost = (cut.saturating_mul(stats.eoc1_cost.max(0)) / WHOLE).max(0);

            None
        }
        PRODUCTION_UP => {
            stats.cooldown = cut.saturating_mul(stats.cooldown.max(0)) / WHOLE;

            None
        }
        KILL_ATTACK_UP => {
            stats.kill_attack_up = stats.kill_attack_up.saturating_add(first);

            Some(Identity::KillAttackUp)
        }
        EXTRA_SPAWN => {
            stats.extra_spawn = stats.extra_spawn.saturating_add(first);
            stats.extra_spawn_delay = values.get(1).copied().unwrap_or(0);

            Some(Identity::ExtraSpawn)
        }
        INSANE_DAMAGE => {
            stats.insane_damage = 1;

            Some(Identity::InsaneDamage)
        }
        INSANELY_TOUGH => {
            stats.insanely_tough = 1;

            Some(Identity::InsanelyTough)
        }
        _ => raised.and_then(|index| {
            stats.effect_ups[index] = stats.effect_ups[index].saturating_add(first);

            Identity::effect_up_at(index)
        }),
    };

    if let Some(identity) = granted {
        stats.run_marks.push((identity, maxed));
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RoguelikeBonusError {
    InvalidJson,
}

impl fmt::Display for RoguelikeBonusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "The provided byte slice could not be parsed as valid JSON.")
    }
}

impl std::error::Error for RoguelikeBonusError {}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoguelikeBonusChara {
    pub rarity: Option<i32>,
    pub chara_id: Option<i32>,
    pub level: Option<i32>,
    pub plus_level: Option<i32>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoguelikeBonusCharas {
    pub charas: BTreeMap<i32, RoguelikeBonusChara>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoguelikeBonusBuffSkill {
    pub ability_id: Option<i32>,
    pub max_level: Option<i32>,
    pub parameters: Vec<i32>,
}

impl RoguelikeBonusBuffSkill {
    pub fn percent(&self) -> bool {
        let ability = self.ability_id.unwrap_or(0);
        let told = u8::try_from(ability).ok().and_then(get_talent).and_then(|talent| talent.talent_values.first()).map(|value| value.unit == AttrUnit::Percent);

        told.unwrap_or((COST_DISCOUNT..=PRODUCTION_UP).contains(&ability) && ![EXTRA_SPAWN, INSANE_DAMAGE, INSANELY_TOUGH].contains(&ability))
    }

    pub fn values(&self, level: i32) -> Vec<i32> {
        let steps = self.max_level.unwrap_or(0).saturating_sub(1);

        self.parameters
            .chunks(2)
            .map(|pair| match pair {
                [low, high] if steps > 0 => high.saturating_sub(*low).saturating_mul(level.saturating_sub(1)) / steps + low,
                [low, ..] => *low,
                [] => 0,
            })
            .collect()
    }

    pub fn talent(&self) -> Option<TalentGroup> {
        let ability_id = u8::try_from(self.ability_id?).ok().filter(|id| get_talent(*id).is_some())?;
        let cell = |index: usize| self.parameters.get(index).map_or(0, |value| u16::try_from(*value).unwrap_or(u16::MAX));

        Some(TalentGroup {
            ability_id,
            max_level: u8::try_from(self.max_level.unwrap_or(1).max(1)).unwrap_or(u8::MAX),
            min_1: cell(0),
            max_1: cell(1),
            min_2: cell(2),
            max_2: cell(3),
            min_3: cell(4),
            max_3: cell(5),
            min_4: cell(6),
            max_4: cell(7),
            name_id: -1,
            ..TalentGroup::default()
        })
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoguelikeBonusBuffSkills {
    pub skills: BTreeMap<i32, RoguelikeBonusBuffSkill>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoguelikeBonusEffectAbility {
    pub order: Option<i32>,
    pub kind: Option<i32>,
    pub ability_id: Option<i32>,
    pub img1: Option<i32>,
    pub img2: Option<i32>,
    pub content: Option<i32>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoguelikeBonusEffectAbilities {
    pub abilities: BTreeMap<i32, RoguelikeBonusEffectAbility>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoguelikeBonusEffect {
    PowerUp { level: i32, plus: i32 },
    Instinct { skill: i32 },
    Combo { effect: i32, volume: i32 },
    CatCannon { cannon: i32, level: i32 },
    Unknown(i32),
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoguelikeBonusBuff {
    pub rarity: Option<i32>,
    pub limit: Option<i32>,
    pub target_rarity: Option<i32>,
    pub target_chara_id: Option<i32>,
    pub name: Option<String>,
    pub explanation: Option<String>,
    pub buff_type: Option<i32>,
    pub level_up: Option<i32>,
    pub plus_up: Option<i32>,
    pub buff_skill_id: Option<i32>,
    pub effect_id: Option<i32>,
    pub volume_id: Option<i32>,
    pub id: Option<i32>,
    pub level: Option<i32>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct RoguelikeBonusTokens {
    pub skill_name: String,
    pub bonus_value: String,
    pub target_rarity: String,
    pub chara_name: String,
    pub allow_duplicates: String,
}

impl RoguelikeBonusTokens {
    pub fn fill(&self, pattern: &str) -> String {
        [
            ("${skillName}", &self.skill_name),
            ("${bonusValue}", &self.bonus_value),
            ("${targetRarity}", &self.target_rarity),
            ("${charaName}", &self.chara_name),
            ("${allowDuplicates}", &self.allow_duplicates),
        ]
        .into_iter()
        .fold(pattern.to_owned(), |text, (token, value)| text.replace(token, value))
    }
}

impl RoguelikeBonusBuff {
    pub fn effect(&self) -> RoguelikeBonusEffect {
        match self.buff_type.unwrap_or(EVERY) {
            0 => RoguelikeBonusEffect::PowerUp { level: self.level_up.unwrap_or(0), plus: self.plus_up.unwrap_or(0) },
            1 => RoguelikeBonusEffect::Instinct { skill: self.buff_skill_id.unwrap_or(EVERY) },
            2 => RoguelikeBonusEffect::Combo { effect: self.effect_id.unwrap_or(0), volume: self.volume_id.unwrap_or(0) },
            3 => RoguelikeBonusEffect::CatCannon { cannon: self.id.unwrap_or(0), level: self.level.unwrap_or(0) },
            other => RoguelikeBonusEffect::Unknown(other),
        }
    }

    pub fn targeted_chara(&self) -> Option<i32> {
        self.target_chara_id.filter(|chara| *chara != EVERY)
    }

    pub fn targeted_rarity(&self) -> Option<i32> {
        self.target_rarity.filter(|rarity| *rarity != EVERY)
    }

    pub fn applies_to(&self, unit: i32, rarity: i32) -> bool {
        self.targeted_chara().is_none_or(|chara| chara == unit) && self.targeted_rarity().is_none_or(|wanted| wanted == rarity)
    }

    pub fn power_up(&self) -> Option<(i32, i32)> {
        match self.effect() {
            RoguelikeBonusEffect::PowerUp { level, plus } => Some((level, plus)),
            _ => None,
        }
    }

    pub fn tokens(&self, skills: &RoguelikeBonusBuffSkills, localizable: &Localizable, chara_name: Option<&str>) -> RoguelikeBonusTokens {
        let text = |key: String| localizable.lookup(&key).unwrap_or_default().to_owned();
        let mut tokens = RoguelikeBonusTokens {
            chara_name: self.targeted_chara().and(chara_name).unwrap_or_default().to_owned(),
            target_rarity: self.targeted_rarity().map(|rarity| text(format!("rarity_name_{}", rarity.saturating_add(1)))).unwrap_or_default(),
            ..RoguelikeBonusTokens::default()
        };

        match self.effect() {
            RoguelikeBonusEffect::PowerUp { level, plus } => tokens.bonus_value = level.saturating_add(plus).to_string(),
            RoguelikeBonusEffect::Instinct { skill } => {
                if let Some(skill) = skills.skills.get(&skill) {
                    let repeats = skill.max_level.unwrap_or(0) != 1;

                    tokens.skill_name = text(format!("potential_skill_name{:02}", skill.ability_id.unwrap_or(0)));
                    tokens.bonus_value = skill.parameters.first().map(i32::to_string).unwrap_or_default();
                    tokens.allow_duplicates = text(format!("RoguelikeBonusAllowDuplicates_{repeats}"));
                }
            }
            RoguelikeBonusEffect::Combo { .. } | RoguelikeBonusEffect::CatCannon { .. } | RoguelikeBonusEffect::Unknown(_) => {}
        }

        tokens
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoguelikeBonusBuffs {
    pub buffs: BTreeMap<i32, RoguelikeBonusBuff>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoguelikeBonusLotteryGroup {
    pub bonus_target: Option<i32>,
    pub bonus_target_ids: Vec<i32>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoguelikeBonusLotteryGroups {
    pub groups: BTreeMap<i32, RoguelikeBonusLotteryGroup>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoguelikeBonusDraw {
    pub draw_count: Option<i32>,
    pub bonus_group_ids: Vec<i32>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoguelikeBonusMapStageLottery {
    pub maps: BTreeMap<i32, BTreeMap<i32, BTreeMap<i32, RoguelikeBonusDraw>>>,
}

impl RoguelikeBonusMapStageLottery {
    pub fn parse<B: AsRef<[u8]>>(bytes: B) -> Result<Self, RoguelikeBonusError> {
        parse_lottery(bytes.as_ref())
    }

    pub fn groups(&self, map_id: i32) -> Option<Vec<i32>> {
        let stages = self.maps.get(&map_id)?;

        Some(stages.values().flat_map(BTreeMap::values).flat_map(|draw| draw.bonus_group_ids.iter().copied()).collect())
    }

    pub fn draws(&self, map_id: i32, stage: i32) -> Vec<&RoguelikeBonusDraw> {
        let Some(stages) = self.maps.get(&map_id) else {
            return Vec::new();
        };

        stages.get(&stage).or_else(|| stages.get(&EVERY)).map_or_else(Vec::new, |draws| draws.values().collect())
    }
}

fn parse_lottery(bytes: &[u8]) -> Result<RoguelikeBonusMapStageLottery, RoguelikeBonusError> {
    let maps = table(root(bytes)?.get(MAP_ROOT), |stages| {
        table(Some(stages), |stage| table(stage.get("BonusNum"), |draw| RoguelikeBonusDraw { draw_count: number(draw, "DrawCount"), bonus_group_ids: numbers(draw, "BonusGroupID") }))
    });

    Ok(RoguelikeBonusMapStageLottery { maps })
}

impl RoguelikeBonusLotteryGroups {
    pub fn targets(&self, groups: &[i32], target: i32) -> Vec<i32> {
        groups.iter().filter_map(|group| self.groups.get(group)).filter(|group| group.bonus_target == Some(target)).flat_map(|group| group.bonus_target_ids.iter().copied()).collect()
    }

    pub fn charas(&self, groups: &[i32]) -> Vec<i32> {
        self.targets(groups, CHARA_TARGET)
    }

    pub fn buffs(&self, groups: &[i32]) -> Vec<i32> {
        self.targets(groups, BUFF_TARGET)
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoguelikeBonusPermanent {
    pub events: BTreeMap<i32, BTreeMap<i32, BTreeMap<i32, Vec<i32>>>>,
}

impl RoguelikeBonusPermanent {
    pub fn groups(&self, event_id: i32, map_id: i32, crown: i32) -> Vec<i32> {
        let Some(maps) = self.events.get(&event_id) else {
            return Vec::new();
        };

        [map_id, EVERY]
            .into_iter()
            .filter_map(|map| maps.get(&map))
            .flat_map(|crowns| [crown, EVERY].into_iter().filter_map(|crown| crowns.get(&crown)))
            .flatten()
            .copied()
            .collect()
    }

    pub fn buffs(&self, groups: &RoguelikeBonusLotteryGroups, event_id: i32, map_id: i32, crown: i32) -> Vec<i32> {
        self.groups(event_id, map_id, crown)
            .iter()
            .filter_map(|group| groups.groups.get(group))
            .filter(|group| group.bonus_target == Some(BUFF_TARGET))
            .flat_map(|group| group.bonus_target_ids.iter().copied())
            .collect()
    }

    pub fn charas(&self, groups: &RoguelikeBonusLotteryGroups, event_id: i32, map_id: i32, crown: i32) -> Vec<i32> {
        self.groups(event_id, map_id, crown)
            .iter()
            .filter_map(|group| groups.groups.get(group))
            .filter(|group| group.bonus_target == Some(CHARA_TARGET))
            .flat_map(|group| group.bonus_target_ids.iter().copied())
            .collect()
    }

    pub fn keeps_buff(&self, groups: &RoguelikeBonusLotteryGroups, event_id: i32, buff_id: i32) -> bool {
        self.events.get(&event_id).is_some_and(|maps| {
            maps.values()
                .flat_map(BTreeMap::values)
                .flatten()
                .filter_map(|group| groups.groups.get(group))
                .any(|group| group.bonus_target == Some(BUFF_TARGET) && group.bonus_target_ids.contains(&buff_id))
        })
    }
}

fn root(bytes: &[u8]) -> Result<Value, RoguelikeBonusError> {
    serde_json::from_str(&scrub(bytes)).map_err(|_| RoguelikeBonusError::InvalidJson)
}

fn number(row: &Value, key: &str) -> Option<i32> {
    row.get(key).and_then(Value::as_i64).and_then(|number| i32::try_from(number).ok())
}

fn numbers(row: &Value, key: &str) -> Vec<i32> {
    row.get(key)
        .and_then(Value::as_array)
        .map(|cells| cells.iter().filter_map(|cell| cell.as_i64().and_then(|number| i32::try_from(number).ok())).collect())
        .unwrap_or_default()
}

fn words(row: &Value, key: &str) -> Option<String> {
    row.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn table<T>(listed: Option<&Value>, read: impl Fn(&Value) -> T) -> BTreeMap<i32, T> {
    listed
        .and_then(Value::as_object)
        .map(|rows| rows.iter().filter_map(|(key, row)| Some((key.parse::<i32>().ok()?, read(row)))).collect())
        .unwrap_or_default()
}

impl RoguelikeBonusCharas {
    pub fn parse<B: AsRef<[u8]>>(bytes: B) -> Result<Self, RoguelikeBonusError> {
        parse_charas(bytes.as_ref())
    }
}

fn parse_charas(bytes: &[u8]) -> Result<RoguelikeBonusCharas, RoguelikeBonusError> {
    let charas = table(root(bytes)?.get(CHARA_ROOT), |row| RoguelikeBonusChara {
        rarity: number(row, "Rarity"),
        chara_id: number(row, "CharaID"),
        level: number(row, "Level"),
        plus_level: number(row, "PlusLevel"),
    });

    Ok(RoguelikeBonusCharas { charas })
}

impl RoguelikeBonusBuffSkills {
    pub fn parse<B: AsRef<[u8]>>(bytes: B) -> Result<Self, RoguelikeBonusError> {
        parse_skills(bytes.as_ref())
    }
}

fn parse_skills(bytes: &[u8]) -> Result<RoguelikeBonusBuffSkills, RoguelikeBonusError> {
    let skills = table(root(bytes)?.get(SKILL_ROOT), |row| RoguelikeBonusBuffSkill {
        ability_id: number(row, "AbilityID"),
        max_level: number(row, "MaxLevel"),
        parameters: numbers(row, "Parameters"),
    });

    Ok(RoguelikeBonusBuffSkills { skills })
}

impl RoguelikeBonusEffectAbilities {
    pub fn parse<B: AsRef<[u8]>>(bytes: B) -> Result<Self, RoguelikeBonusError> {
        parse_abilities(bytes.as_ref())
    }
}

fn parse_abilities(bytes: &[u8]) -> Result<RoguelikeBonusEffectAbilities, RoguelikeBonusError> {
    let abilities = table(Some(&root(bytes)?), |row| RoguelikeBonusEffectAbility {
        order: number(row, "Order"),
        kind: number(row, "Kind"),
        ability_id: number(row, "AbilityID"),
        img1: number(row, "Img1"),
        img2: number(row, "Img2"),
        content: number(row, "Content"),
    });

    Ok(RoguelikeBonusEffectAbilities { abilities })
}

impl RoguelikeBonusBuffs {
    pub fn parse<B: AsRef<[u8]>>(bytes: B) -> Result<Self, RoguelikeBonusError> {
        parse_buffs(bytes.as_ref())
    }
}

fn parse_buffs(bytes: &[u8]) -> Result<RoguelikeBonusBuffs, RoguelikeBonusError> {
    let buffs = table(root(bytes)?.get(BUFF_ROOT), |row| RoguelikeBonusBuff {
        rarity: number(row, "Rarity"),
        limit: number(row, "Limit"),
        target_rarity: number(row, "TargetRarity"),
        target_chara_id: number(row, "TargetCharaID"),
        name: words(row, "RoguelikeBonusBuffName"),
        explanation: words(row, "RoguelikeBonusBuffExplanation"),
        buff_type: number(row, "BuffType"),
        level_up: number(row, "LevelUp"),
        plus_up: number(row, "PlusUp"),
        buff_skill_id: number(row, "BuffSkillID"),
        effect_id: number(row, "EffectID"),
        volume_id: number(row, "VolumeID"),
        id: number(row, "ID"),
        level: number(row, "Level"),
    });

    Ok(RoguelikeBonusBuffs { buffs })
}

impl RoguelikeBonusLotteryGroups {
    pub fn parse<B: AsRef<[u8]>>(bytes: B) -> Result<Self, RoguelikeBonusError> {
        parse_groups(bytes.as_ref())
    }
}

fn parse_groups(bytes: &[u8]) -> Result<RoguelikeBonusLotteryGroups, RoguelikeBonusError> {
    let groups = table(root(bytes)?.get(GROUP_ROOT), |row| RoguelikeBonusLotteryGroup {
        bonus_target: number(row, "BonusTarget"),
        bonus_target_ids: numbers(row, "BonusTargetID"),
    });

    Ok(RoguelikeBonusLotteryGroups { groups })
}

impl RoguelikeBonusPermanent {
    pub fn parse<B: AsRef<[u8]>>(bytes: B) -> Result<Self, RoguelikeBonusError> {
        parse_permanent(bytes.as_ref())
    }
}

fn parse_permanent(bytes: &[u8]) -> Result<RoguelikeBonusPermanent, RoguelikeBonusError> {
    let events = table(root(bytes)?.get(EVENT_ROOT), |maps| {
        table(Some(maps), |crowns| table(Some(crowns), |row| numbers(row, "BonusGroupID")))
    });

    Ok(RoguelikeBonusPermanent { events })
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUFFS: &str = r#"{"BonusBuffID":{
        "0":{"Rarity":3,"Limit":5,"TargetRarity":-1,"RoguelikeBonusBuffName":"N0","RoguelikeBonusBuffExplanation":"E0","BuffType":0,"LevelUp":5},
        "1":{"Rarity":10,"Limit":5,"TargetRarity":2,"RoguelikeBonusBuffName":"N1","RoguelikeBonusBuffExplanation":"E1","BuffType":1,"BuffSkillID":7},
        "2":{"Rarity":0,"Limit":1,"TargetRarity":-1,"TargetCharaID":44,"RoguelikeBonusBuffName":"N1","RoguelikeBonusBuffExplanation":"E2","BuffType":1,"BuffSkillID":8},
        "3":{"Rarity":0,"Limit":1,"TargetRarity":-1,"BuffType":3,"ID":5,"Level":20}
    }}"#;
    const SKILLS: &str = r#"{"BuffSkillID":{"7":{"AbilityID":32,"MaxLevel":5,"Parameters":[40,200]},"8":{"AbilityID":5,"MaxLevel":1}}}"#;
    const TEXT: &str = "rarity_name_3\tRare\npotential_skill_name32\tDefense Buff\npotential_skill_name05\tStrong\nRoguelikeBonusAllowDuplicates_true\t(stacks)\nRoguelikeBonusAllowDuplicates_false\t(once)\n";

    #[test]
    fn a_buff_reads_as_the_effect_its_type_names() {
        let buffs = RoguelikeBonusBuffs::parse(BUFFS).unwrap().buffs;

        assert_eq!(buffs[&0].effect(), RoguelikeBonusEffect::PowerUp { level: 5, plus: 0 });
        assert_eq!(buffs[&1].effect(), RoguelikeBonusEffect::Instinct { skill: 7 });
        assert_eq!(buffs[&3].effect(), RoguelikeBonusEffect::CatCannon { cannon: 5, level: 20 });
        assert_eq!(buffs[&1].targeted_rarity(), Some(2));
        assert_eq!(buffs[&0].targeted_rarity(), None, "-1 is every rarity, not a rarity");
        assert_eq!(buffs[&2].targeted_chara(), Some(44));
    }

    // The rarity key is one past the column, and a skill that tops out at one level reads as not stacking.
    #[test]
    fn the_text_tokens_follow_the_first_effect() {
        let buffs = RoguelikeBonusBuffs::parse(BUFFS).unwrap().buffs;
        let skills = RoguelikeBonusBuffSkills::parse(SKILLS).unwrap();
        let text = Localizable::parse(TEXT).unwrap();

        let rare = buffs[&1].tokens(&skills, &text, Some("ignored"));

        assert_eq!(rare.fill("${targetRarity}: ${skillName} +${bonusValue}${allowDuplicates}${charaName}"), "Rare: Defense Buff +40(stacks)");

        let named = buffs[&2].tokens(&skills, &text, Some("Cat"));

        assert_eq!(named.fill("[${charaName}] ${skillName}${bonusValue}${allowDuplicates}"), "[Cat] Strong(once)");
        assert_eq!(buffs[&0].tokens(&skills, &text, None).bonus_value, "5");
    }

    #[test]
    fn permanent_groups_are_the_maps_own_plus_the_shared_ones() {
        let permanent = RoguelikeBonusPermanent::parse(r#"{"EventID":{"0":{"-1":{"-1":{"BonusGroupID":[13]}},"39001":{"-1":{"BonusGroupID":[12]},"2":{"BonusGroupID":[40]}}}}}"#).unwrap();
        let groups = RoguelikeBonusLotteryGroups::parse(
            r#"{"BonusGroupID":{"12":{"BonusTarget":1,"BonusTargetID":[86]},"13":{"BonusTarget":1,"BonusTargetID":[90]},"40":{"BonusTarget":0,"BonusTargetID":[4]}}}"#,
        )
        .unwrap();

        assert_eq!(permanent.groups(0, 39001, 2), vec![40, 12, 13]);
        assert_eq!(permanent.buffs(&groups, 0, 39001, 0), vec![86, 90]);
        assert_eq!(permanent.buffs(&groups, 0, 39002, 0), vec![90]);
        assert_eq!(permanent.charas(&groups, 0, 39001, 2), vec![4]);
        assert!(permanent.keeps_buff(&groups, 0, 86));
        assert!(!permanent.keeps_buff(&groups, 0, 1));
        assert!(permanent.groups(1, 39001, 0).is_empty());
    }

    // The form a run cat fights in comes from its level and plus level together, never past the forms it has.
    #[test]
    fn a_run_cat_evolves_at_the_totals_the_game_uses() {
        assert_eq!([8, 9, 28, 29, 58, 59].map(|total| roguelike_form(total, 4)), [0, 1, 1, 2, 2, 3]);
        assert_eq!(roguelike_form(59, 2), 1);
        assert_eq!(roguelike_form(59, 0), -1, "a unit with no forms on record has no form to stand in");

        let buffs = RoguelikeBonusBuffs::parse(BUFFS).unwrap().buffs;

        assert!(buffs[&0].applies_to(7, 3) && buffs[&1].applies_to(7, 2) && !buffs[&1].applies_to(7, 3));
        assert!(buffs[&2].applies_to(44, 0) && !buffs[&2].applies_to(45, 0));
        assert_eq!(buffs[&0].power_up(), Some((5, 0)));
        assert_eq!(buffs[&1].power_up(), None);

        let abilities = RoguelikeBonusEffectAbilities::parse(r#"{"0":{"Order":0},"1001":{"Order":1005,"Kind":0,"AbilityID":1001,"Img1":401,"Img2":408,"Content":2}}"#).unwrap().abilities;

        assert_eq!(abilities[&1001].img1, Some(401));
        assert_eq!(abilities[&0], RoguelikeBonusEffectAbility { order: Some(0), ..RoguelikeBonusEffectAbility::default() });
    }

    // A stacking skill climbs from its first value to its last, and the cost and recharge cuts are percentages.
    #[test]
    fn a_skill_grows_by_level_and_trims_what_the_game_trims() {
        let skills = RoguelikeBonusBuffSkills::parse(SKILLS).unwrap().skills;

        assert_eq!([1, 3, 5].map(|level| skills[&7].values(level)), [vec![40], vec![120], vec![200]]);
        assert!(skills[&8].values(1).is_empty());

        let group = skills[&7].talent().unwrap();

        assert_eq!((group.ability_id, group.max_level, group.min_1, group.max_1), (32, 5, 40, 200));
        assert_eq!(TalentGroup::calculate_value(group.min_1, group.max_1, 3, group.max_level), 120);
        assert!(RoguelikeBonusBuffSkill { ability_id: Some(1012), ..RoguelikeBonusBuffSkill::default() }.talent().is_none());

        let mut stats = Entity { eoc1_cost: 75, cooldown: 300, ..Entity::default() };

        roguelike_grants(&mut stats, &[(1012, vec![45], false), (1013, vec![15], false), (1010, vec![], true), (1009, vec![2, 60], false)]);

        assert_eq!((stats.eoc1_cost, stats.cooldown, stats.insane_damage), (41, 255, 1), "a cheap cat keeps a price, it does not round down to nothing");
        roguelike_grants(&mut stats, &[(1008, vec![100], true), (1005, vec![20], false)]);

        assert_eq!((stats.extra_spawn, stats.extra_spawn_delay, stats.kill_attack_up), (2, 60, 100));
        assert_eq!(stats.effect_ups, [0, 0, 0, 0, 20, 0, 0], "1005 is the slow one");
        assert_eq!(stats.run_marks, vec![(Identity::InsaneDamage, true), (Identity::ExtraSpawn, false), (Identity::KillAttackUp, true), (Identity::SlowUp, false)], "a price or a recharge is a number, not an ability to mark");
        assert!(skills[&7].percent() && !RoguelikeBonusBuffSkill { ability_id: Some(1009), ..RoguelikeBonusBuffSkill::default() }.percent());
        assert!(RoguelikeBonusBuffSkill { ability_id: Some(1012), ..RoguelikeBonusBuffSkill::default() }.percent());
    }

    // The game adds every bonus for one ability into a single percentage before it cuts, so two cuts never compound.
    #[test]
    fn bonuses_for_one_ability_add_up_before_they_cut() {
        let mut stats = Entity { eoc1_cost: 3600, cooldown: 300, ..Entity::default() };

        roguelike_grants(&mut stats, &[(1012, vec![75], true), (1013, vec![15], false), (1012, vec![20], true), (1013, vec![60], true)]);

        assert_eq!((stats.eoc1_cost, stats.cooldown), (180, 75));

        let mut free = Entity { eoc1_cost: 3600, ..Entity::default() };

        roguelike_grants(&mut free, &[(1012, vec![75], true), (1012, vec![60], true)]);

        assert_eq!(free.eoc1_cost, 0, "past a full cut the price bottoms out at nothing");

        let mut spawns = Entity::default();

        roguelike_grants(&mut spawns, &[(1009, vec![1, 60], true), (1009, vec![2, 60], false)]);

        assert_eq!((spawns.extra_spawn, spawns.extra_spawn_delay), (3, 120));
        assert_eq!(spawns.run_marks, vec![(Identity::ExtraSpawn, false)], "one mark per ability, gold only when every bonus behind it is maxed");
    }

    // Each map draws from its own groups, which is what keeps two maps' copies of a bonus apart.
    #[test]
    fn a_map_offers_only_what_its_own_groups_hold() {
        let lottery = RoguelikeBonusMapStageLottery::parse(r#"{"MapID":{"39001":{"-1":{"BonusNum":{"0":{"DrawCount":3,"BonusGroupID":[40]},"1":{"DrawCount":3,"BonusGroupID":[12]}}}}}}"#).unwrap();
        let groups = RoguelikeBonusLotteryGroups::parse(r#"{"BonusGroupID":{"12":{"BonusTarget":1,"BonusTargetID":[86]},"13":{"BonusTarget":1,"BonusTargetID":[90]},"40":{"BonusTarget":0,"BonusTargetID":[4]}}}"#).unwrap();
        let held = lottery.groups(39001).unwrap();

        assert_eq!(held, vec![40, 12]);
        assert_eq!((groups.charas(&held), groups.buffs(&held)), (vec![4], vec![86]));
        assert!(lottery.groups(39002).is_none());

        // A stage with no row of its own draws the map's -1 row, in BonusNum order.
        let drawn: Vec<Vec<i32>> = lottery.draws(39001, 7).into_iter().map(|draw| draw.bonus_group_ids.clone()).collect();

        assert_eq!(drawn, vec![vec![40], vec![12]]);
        assert!(lottery.draws(39002, 0).is_empty());
    }

    #[test]
    fn a_broken_file_is_an_error_and_a_missing_table_is_empty() {
        assert_eq!(RoguelikeBonusCharas::parse("{"), Err(RoguelikeBonusError::InvalidJson));
        assert!(RoguelikeBonusCharas::parse("{}").unwrap().charas.is_empty());

        let charas = RoguelikeBonusCharas::parse(r#"{"BonusCharaID":{"4":{"Rarity":5,"CharaID":75,"Level":20}}}"#).unwrap().charas;

        assert_eq!(charas[&4], RoguelikeBonusChara { rarity: Some(5), chara_id: Some(75), level: Some(20), plus_level: None });
    }
}
