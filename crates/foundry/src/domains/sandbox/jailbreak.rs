use std::collections::{BTreeMap, BTreeSet, HashMap};

use codex::cat::unit::Talent;
use codex::chapter::map::{
    roguelike_form, roguelike_stacks, RoguelikeBonusBuff, RoguelikeBonusBuffSkill, RoguelikeBonusBuffSkills, RoguelikeBonusBuffs, RoguelikeBonusCharas, RoguelikeBonusEffect, RoguelikeBonusEffectAbilities,
    RoguelikeBonusLotteryGroups, RoguelikeBonusMapStageLottery, RoguelikeBonusPermanent, RoguelikeBonusTokens, RoguelikeMapSettings,
};
use codex::chapter::stage::CertificationPreset;
use codex::chapter::treasure::TreasureLevels;
use codex::combat::{get_talent, AttrUnit};
use codex::files::img015;
use serde::{Deserialize, Serialize};

use crate::common::context::GlobalContext;
use crate::domains::cat::scanner::CatEntry;
use crate::domains::cat::waiter::unitexplanation;
use crate::domains::sandbox::base;
use crate::domains::sandbox::lineup::{Arrangement, Cell, Edge, Lineup, Member, BENCH_SLOTS, LINEUP_SLOTS, UNNAMED};
use crate::systems::combat::registry::{get_display_def, AbilityIcon};
use crate::systems::treasure::{self, Catalog};
use crate::Vfs;

pub use crate::domains::cat::grants::Grants;

pub const FILE: &str = "RoguelikeMapSettings.json";
pub const GLOBAL_LABEL: &str = "Global Bonus";
pub const CAT_LABEL: &str = "Cat Bonus";
pub const GLOBAL_TITLE: &str = "Global Bonuses";
pub const CAT_TITLE: &str = "Cat Bonuses";
pub const GLOBAL_HINT: &str = "The bonuses that are not tied to one cat, each at the level it was picked up to\nPermanent bonuses are the ones bought with event points";
pub const CAT_HINT: &str = "The bonuses of the cats in this lineup, each at the level it was picked up to";
pub const OVERRIDDEN: &str = "A Jailbreak run brings its own, the ones here are not used";
pub const REGISTERED: &str = "REG";
pub const BENCH_NOTE: &str = "Not part of the run";
pub const PERMANENT: &str = "Permanent";
pub const CLEAR_TEMP_LABEL: &str = "Clear Temp";
pub const CLEAR_PERMA_LABEL: &str = "Clear Perma";
pub const CLEAR_CONFIRM: &str = "Continue?";
pub const NOTHING_TO_CLEAR: &str = "No Bonuses to clear!";
pub const RECRUITS_LABEL: &str = "Recruits";
pub const BONUSES_LABEL: &str = "Bonuses";
pub const TOO_MANY_RECRUITS: &str = "Too many Recruits!";
pub const TOO_MANY_BONUSES: &str = "Too many Bonuses!";
pub const EVERY_CAT: &str = "Every Cat";
pub const ABILITY_SHEET: &str = "img015";
pub const BASE_SHEET: &str = "img060_01";
const CHARA_FILE: &str = "RoguelikeBonusChara.json";
const BUFF_FILE: &str = "RoguelikeBonusBuff.json";
const SKILL_FILE: &str = "RoguelikeBonusBuffSkill.json";
const ABILITY_FILE: &str = "RoguelikeBonusEffectAbility.json";
const GROUP_FILE: &str = "RoguelikeBonusLotteryGroup.json";
const PERMANENT_FILE: &str = "RoguelikeBonusPermanent.json";
const LOTTERY_FILE: &str = "RoguelikeBonusMapStageLottery.json";
const BASIC_CANNON: i32 = 0;
const FIRST_STAGE: i32 = 0;
const EVERY_STAGE: i32 = -1;
const BREAK: &str = "<br>";
const LEVEL_FALLBACK: &str = "Lv+";
const COMBO_FALLBACK: &str = "Combo";
const CANNON_FALLBACK: &str = "Cannon";
const SKILL_FALLBACK: &str = "Skill";
const LEVEL_CUT: usize = 14;
const CANNON_CUT: usize = 245;
const NAMED: &str = "\u{1}";

const NEW_SKILLS: [(i32, &str); 14] = [
    (1000, "Cost Discount"),
    (1001, "Strong Effect Up"),
    (1002, "Massive Damage Effect Up"),
    (1003, "Tough Effect Up"),
    (1004, "Knockback Effect Up"),
    (1005, "Slow Effect Up"),
    (1006, "Freeze Effect Up"),
    (1007, "Weaken Effect Up"),
    (1008, "Attack Up On Kill"),
    (1009, "Extra Cat On Deploy"),
    (1010, "Insane Damage"),
    (1011, "Insanely Tough"),
    (1012, "Cost Down"),
    (1013, "Production Speed Up"),
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Run {
    pub name: String,
    pub slots: Vec<u32>,
    pub bench: [Option<u32>; BENCH_SLOTS],
    pub bonuses: BTreeMap<i32, i32>,
    pub cannon: Option<i32>,
    pub registered: BTreeSet<u32>,
}

impl Default for Run {
    fn default() -> Self {
        Self { name: UNNAMED.to_owned(), slots: Vec::new(), bench: Default::default(), bonuses: BTreeMap::new(), cannon: None, registered: BTreeSet::new() }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Runs {
    pub saved: Vec<Run>,
    pub selected: usize,
}

impl Default for Runs {
    fn default() -> Self {
        Self { saved: vec![Run::default()], selected: 0 }
    }
}

impl Runs {
    pub fn current(&self) -> Option<&Run> {
        self.saved.get(self.selected)
    }

    pub fn current_mut(&mut self) -> &mut Run {
        if self.saved.is_empty() {
            self.saved.push(Run::default());
        }

        let last = self.saved.len() - 1;

        self.selected = self.selected.min(last);

        &mut self.saved[self.selected]
    }

    pub fn create(&mut self) {
        self.saved.push(Run::default());
        self.selected = self.saved.len() - 1;
    }

    pub fn delete(&mut self) {
        if self.selected < self.saved.len() {
            self.saved.remove(self.selected);
        }

        if self.saved.is_empty() {
            self.saved.push(Run::default());
        }

        self.selected = self.selected.min(self.saved.len() - 1);
    }

    pub fn load(&mut self, index: usize) {
        if index < self.saved.len() {
            self.selected = index;
        }
    }
}

impl Run {
    pub fn is_fresh(&self) -> bool {
        self.slots.is_empty() && self.bench.iter().all(Option::is_none) && self.bonuses.is_empty() && self.cannon.is_none() && self.registered.is_empty()
    }

    pub fn level(&self, bonus: i32) -> i32 {
        self.bonuses.get(&bonus).copied().unwrap_or(0)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cat {
    pub unit: u32,
    pub chara: Option<i32>,
    pub starter: bool,
    pub names: [Option<String>; 4],
    pub level: i32,
    pub plus: i32,
    pub rarity: i32,
    pub forms: i32,
}

impl Cat {
    pub fn name_at(&self, form: usize) -> String {
        let told = |name: &&String| !name.is_empty();
        let named = self.names.get(form).and_then(Option::as_ref).filter(told).or_else(|| self.names.iter().flatten().find(told));

        named.map_or_else(|| format!("Cat {:03}", self.unit), String::clone)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bonus {
    pub id: i32,
    pub label: String,
    pub limit: i32,
    pub target: Option<u32>,
    pub permanent: bool,
    pub sheet: &'static str,
    pub icon: AbilityIcon,
    pub fallback: &'static str,
    pub rule: RoguelikeBonusBuff,
    pub skill: Option<RoguelikeBonusBuffSkill>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Standing {
    pub level: i32,
    pub plus: i32,
    pub form: usize,
}

impl Standing {
    pub fn label(&self) -> String {
        if self.plus > 0 { format!("{}+{}", self.level, self.plus) } else { self.level.to_string() }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Budget {
    pub recruits: i32,
    pub bonuses: i32,
}

impl Budget {
    pub fn covers(self, spent: Self) -> bool {
        spent.recruits <= self.recruits && spent.bonuses <= self.bonuses
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    pub recruits: String,
    pub bonuses: String,
    pub recruits_over: bool,
    pub bonuses_over: bool,
}

impl Tally {
    pub fn over(&self) -> bool {
        self.recruits_over || self.bonuses_over
    }

    pub fn recruits_note(&self) -> Option<&'static str> {
        self.recruits_over.then_some(TOO_MANY_RECRUITS)
    }

    pub fn bonuses_note(&self) -> Option<&'static str> {
        self.bonuses_over.then_some(TOO_MANY_BONUSES)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Board {
    pub map_id: i32,
    pub starters: Vec<u32>,
    pub cats: Vec<Cat>,
    pub bonuses: Vec<Bonus>,
    pub texts: HashMap<u8, (u8, i16)>,
    pub registered_plus: i32,
    pub picks: Option<BTreeMap<i32, Budget>>,
    pub preset: CertificationPreset,
}

pub fn is_challenge_map(vfs: &Vfs, map_id: i32) -> bool {
    vfs.load(FILE).is_some_and(|bytes| holds(&bytes, map_id))
}

pub fn preset(vfs: &Vfs, map_id: i32) -> Option<CertificationPreset> {
    let settings = RoguelikeMapSettings::parse(vfs.load(FILE)?).ok()?;

    CertificationPreset::parse(vfs.load(&settings.entry(map_id)?.preset_file_name)?).ok()
}

pub fn holds(bytes: &[u8], map_id: i32) -> bool {
    RoguelikeMapSettings::parse(bytes).is_ok_and(|settings| settings.entry(map_id).is_some())
}

struct Names<'a> {
    vfs: &'a Vfs,
    known: HashMap<u32, &'a CatEntry>,
}

impl Names<'_> {
    fn of(&self, unit: u32) -> [Option<String>; 4] {
        match self.known.get(&unit) {
            Some(entry) => entry.names.clone(),
            None => unitexplanation(self.vfs, unit).names,
        }
    }

    fn forms(&self, unit: u32) -> i32 {
        self.known.get(&unit).map_or(1, |entry| i32::try_from(entry.forms.iter().rposition(|held| *held).map_or(1, |last| last + 1)).unwrap_or(1))
    }
}

struct Texts<'a> {
    ctx: GlobalContext<'a>,
    skills: RoguelikeBonusBuffSkills,
    abilities: RoguelikeBonusEffectAbilities,
}

impl Texts<'_> {
    fn skill(&self, skill: i32, tokens: &RoguelikeBonusTokens) -> String {
        let ability = self.skills.skills.get(&skill).and_then(|skill| skill.ability_id);
        let name = if tokens.skill_name.is_empty() {
            ability
                .and_then(|id| NEW_SKILLS.iter().find(|(known, _)| *known == id))
                .map_or_else(|| format!("Ability {}", ability.unwrap_or(skill)), |(_, name)| (*name).to_owned())
        } else {
            tokens.skill_name.clone()
        };

        let held = self.skills.skills.get(&skill);
        let idle = held.is_some_and(|skill| skill.parameters.first() == Some(&0));
        let raised = held.filter(|_| idle).and_then(|skill| {
            let told = u8::try_from(skill.ability_id?).ok().and_then(get_talent)?.talent_values;
            let (index, amount) = skill.parameters.chunks(2).enumerate().find_map(|(index, pair)| pair.first().copied().filter(|amount| *amount != 0).map(|amount| (index, amount)))?;

            told.get(index).map(|value| (value.label, amount, value.unit == AttrUnit::Percent))
        });

        if let Some((label, amount, percent)) = raised {
            return format!("{name} ({label} +{amount}{})", if percent { "%" } else { "" });
        }

        let percent = held.is_some_and(RoguelikeBonusBuffSkill::percent);

        match (tokens.bonus_value.is_empty(), percent) {
            (true, _) => name,
            (false, true) => format!("{name} ({}%)", tokens.bonus_value),
            (false, false) => format!("{name} ({})", tokens.bonus_value),
        }
    }

    fn effect(&self, id: i32, buff: &RoguelikeBonusBuff, tokens: &RoguelikeBonusTokens) -> String {
        let vault = self.ctx.vault;

        match buff.effect() {
            RoguelikeBonusEffect::PowerUp { level, plus } if plus > 0 => format!("Level +{level}, Plus +{plus}"),
            RoguelikeBonusEffect::PowerUp { level, .. } => format!("Level +{level}"),
            RoguelikeBonusEffect::Instinct { skill } => self.skill(skill, tokens),
            RoguelikeBonusEffect::Combo { effect, volume } => {
                let text = |table: &[Option<String>], index: i32| usize::try_from(index).ok().and_then(|index| table.get(index)).cloned().flatten();
                let kind = text(&vault.vds.cats.combo_effects(&vault.vfs), effect);
                let band = text(&vault.vds.cats.combo_bands(&vault.vfs), volume).unwrap_or_default();

                kind.map_or_else(|| format!("Combo Effect {effect}-{volume}"), |kind| format!("{}{band}", kind.trim()))
            }
            RoguelikeBonusEffect::CatCannon { cannon, level } => {
                format!("{} Lv {level}", self.ctx.names.cannon(cannon).map_or_else(|| base::name(cannon), str::to_owned))
            }
            RoguelikeBonusEffect::Unknown(_) => format!("Bonus {id}"),
        }
    }

    fn icon(&self, buff: &RoguelikeBonusBuff) -> (&'static str, AbilityIcon, &'static str) {
        match buff.effect() {
            RoguelikeBonusEffect::PowerUp { .. } => (BASE_SHEET, AbilityIcon::Standard(LEVEL_CUT), LEVEL_FALLBACK),
            RoguelikeBonusEffect::Combo { .. } => (ABILITY_SHEET, AbilityIcon::Standard(img015::ICON_CAT_COMBO), COMBO_FALLBACK),
            RoguelikeBonusEffect::CatCannon { .. } => (BASE_SHEET, AbilityIcon::Standard(CANNON_CUT), CANNON_FALLBACK),
            RoguelikeBonusEffect::Unknown(_) => (ABILITY_SHEET, AbilityIcon::None, SKILL_FALLBACK),
            RoguelikeBonusEffect::Instinct { skill } => {
                let ability = self.skills.skills.get(&skill).and_then(|skill| skill.ability_id);
                let drawn = self
                    .abilities
                    .abilities
                    .values()
                    .find(|row| row.ability_id.is_some() && row.ability_id == ability)
                    .and_then(|row| usize::try_from(row.img2.or(row.img1)?).ok())
                    .map(|cut| (ABILITY_SHEET, AbilityIcon::Standard(cut), SKILL_FALLBACK));
                let known = || {
                    let shown = get_display_def(get_talent(u8::try_from(ability?).ok()?)?.identity);

                    Some((ABILITY_SHEET, shown.icon, shown.fallback))
                };

                drawn.or_else(known).unwrap_or((ABILITY_SHEET, AbilityIcon::None, SKILL_FALLBACK))
            }
        }
    }

    fn label(&self, id: i32, buff: &RoguelikeBonusBuff, chara: Option<&str>) -> String {
        let tokens = buff.tokens(&self.skills, self.ctx.localizable, chara);
        let told = buff.explanation.as_deref().and_then(|key| self.ctx.localizable.lookup(key)).map(|pattern| tokens.fill(pattern).replace(BREAK, " "));

        if let Some(told) = told.filter(|told| !told.trim().is_empty()) {
            return told;
        }

        let effect = self.effect(id, buff, &tokens);
        let rarity = buff.targeted_rarity().map(|rarity| if tokens.target_rarity.is_empty() { format!("Rarity {rarity}") } else { tokens.target_rarity.clone() });

        match chara.map(str::to_owned).or(rarity) {
            Some(target) => format!("{target}: {effect}"),
            None => effect,
        }
    }
}

impl Board {
    pub fn load(ctx: GlobalContext<'_>, cats: &[CatEntry], map_id: i32, crown: i32) -> Option<Self> {
        let vfs = &ctx.vault.vfs;
        let settings = RoguelikeMapSettings::parse(vfs.load(FILE)?).ok()?;
        let entry = settings.entry(map_id)?;
        let event = settings.event_of(map_id)?;
        let table = |file: &str| vfs.load(file);

        let charas = table(CHARA_FILE).and_then(|bytes| RoguelikeBonusCharas::parse(bytes).ok()).unwrap_or_default();
        let buffs = table(BUFF_FILE).and_then(|bytes| RoguelikeBonusBuffs::parse(bytes).ok()).unwrap_or_default();
        let skills = table(SKILL_FILE).and_then(|bytes| RoguelikeBonusBuffSkills::parse(bytes).ok()).unwrap_or_default();
        let abilities = table(ABILITY_FILE).and_then(|bytes| RoguelikeBonusEffectAbilities::parse(bytes).ok()).unwrap_or_default();
        let groups = table(GROUP_FILE).and_then(|bytes| RoguelikeBonusLotteryGroups::parse(bytes).ok()).unwrap_or_default();
        let permanent = table(PERMANENT_FILE).and_then(|bytes| RoguelikeBonusPermanent::parse(bytes).ok()).unwrap_or_default();
        let lottery = table(LOTTERY_FILE).and_then(|bytes| RoguelikeBonusMapStageLottery::parse(bytes).ok());
        let drawn = lottery.as_ref().and_then(|lottery| lottery.groups(map_id));
        let picks = lottery.as_ref().and_then(|lottery| {
            let stages = lottery.maps.get(&map_id)?;
            let picks_of = |stage: i32| {
                lottery.draws(map_id, stage).into_iter().fold(Budget::default(), |picks, draw| Budget {
                    recruits: picks.recruits + i32::from(!groups.charas(&draw.bonus_group_ids).is_empty()),
                    bonuses: picks.bonuses + i32::from(!groups.buffs(&draw.bonus_group_ids).is_empty()),
                })
            };

            Some(stages.keys().map(|stage| (*stage, picks_of(*stage))).collect())
        });
        let offered = drawn.as_ref().map(|held| (groups.charas(held), groups.buffs(held)));
        let preset = table(&entry.preset_file_name).and_then(|bytes| CertificationPreset::parse(bytes).ok()).unwrap_or_default();
        let unitbuy = ctx.vault.vds.cats.unitbuy(vfs);

        let names = Names { vfs, known: cats.iter().map(|entry| (entry.id, entry)).collect() };
        let texts = Texts { ctx, skills, abilities };
        let mut listed: Vec<Cat> = Vec::new();
        let seat = |unit: u32, chara: Option<i32>, starter: bool, level: i32, plus: i32, listed: &mut Vec<Cat>| {
            if listed.iter().any(|cat| cat.unit == unit) {
                return;
            }

            listed.push(Cat {
                unit,
                chara,
                starter,
                names: names.of(unit),
                level,
                plus,
                rarity: unitbuy.get(&unit).map_or(0, |row| row.rarity),
                forms: names.forms(unit),
            });
        };

        for unit in preset.slot_units.iter().take(LINEUP_SLOTS) {
            if let Some(chara) = preset.characters.get(unit) {
                seat(*unit, None, true, i32::from(chara.level), i32::from(chara.plus_level), &mut listed);
            }
        }

        let starters: Vec<u32> = listed.iter().map(|cat| cat.unit).collect();

        for (id, chara) in charas.charas.iter().filter(|(id, _)| offered.as_ref().is_none_or(|(charas, _)| charas.contains(id))) {
            if let Some(unit) = chara.chara_id.and_then(|unit| u32::try_from(unit).ok()) {
                seat(unit, Some(*id), false, chara.level.unwrap_or(1), chara.plus_level.unwrap_or(0), &mut listed);
            }
        }

        let kept = permanent.buffs(&groups, event, map_id, crown);
        let mut bonuses: Vec<Bonus> = buffs
            .buffs
            .iter()
            .filter(|(id, buff)| buff.limit.unwrap_or(0) > 0 && (kept.contains(id) || !permanent.keeps_buff(&groups, event, **id)))
            .filter(|(id, _)| kept.contains(id) || offered.as_ref().is_none_or(|(_, buffs)| buffs.contains(id)))
            .map(|(id, buff)| {
                let target = buff.targeted_chara().and_then(|unit| u32::try_from(unit).ok());
                let (sheet, icon, fallback) = texts.icon(buff);

                let skill = match buff.effect() {
                    RoguelikeBonusEffect::Instinct { skill } => texts.skills.skills.get(&skill).cloned(),
                    _ => None,
                };

                Bonus { id: *id, label: texts.label(*id, buff, target.map(|_| NAMED)), limit: buff.limit.unwrap_or(0), target, permanent: kept.contains(id), sheet, icon, fallback, rule: buff.clone(), skill }
            })
            .collect();

        bonuses.sort_by_key(|bonus| (bonus.target.is_some(), !bonus.permanent, bonus.id));

        let mut told: HashMap<u8, (u8, i16)> = HashMap::new();

        for group in cats.iter().filter_map(|entry| entry.talent_data.as_ref()).flat_map(|talent| talent.groups.iter()) {
            told.entry(group.ability_id).or_insert((group.text_id, group.name_id));
        }

        Some(Self { map_id, starters, cats: listed, bonuses, texts: told, registered_plus: ctx.param.registered_plus_levels, picks, preset })
    }

    pub fn cat(&self, unit: u32) -> Option<&Cat> {
        self.cats.iter().find(|cat| cat.unit == unit)
    }

    pub fn bonus(&self, id: i32) -> Option<&Bonus> {
        self.bonuses.iter().find(|bonus| bonus.id == id)
    }

    pub fn name(&self, run: &Run, unit: u32) -> String {
        self.cat(unit).map_or_else(|| format!("Cat {unit:03}"), |cat| cat.name_at(self.standing(run, unit).form))
    }

    pub fn title(&self, run: &Run, bonus: &Bonus) -> String {
        bonus.target.map_or_else(|| EVERY_CAT.to_owned(), |unit| self.name(run, unit))
    }

    pub fn label(&self, run: &Run, bonus: &Bonus) -> String {
        match bonus.target {
            Some(unit) => bonus.label.replace(NAMED, &self.name(run, unit)),
            None => bonus.label.clone(),
        }
    }

    pub fn detail(&self, run: &Run, bonus: &Bonus) -> String {
        let lead = format!("{NAMED}: ");
        let told = bonus.label.strip_prefix(lead.as_str()).unwrap_or(bonus.label.as_str());

        match bonus.target {
            Some(unit) => told.replace(NAMED, &self.name(run, unit)),
            None => told.to_owned(),
        }
    }

    pub fn deck(&self, run: &Run) -> Vec<(u32, Standing)> {
        self.fielded(run).iter().map(|unit| (*unit, self.standing(run, *unit))).collect()
    }

    pub fn fielded<'a>(&'a self, run: &'a Run) -> &'a [u32] {
        if run.slots.is_empty() { &self.starters } else { &run.slots }
    }

    pub fn offers(&self, run: &Run, bonus: &Bonus) -> bool {
        bonus.target.is_none_or(|unit| self.fielded(run).contains(&unit))
    }

    pub fn budget(&self, stage: i32) -> Option<Budget> {
        let picks = self.picks.as_ref()?;

        Some((FIRST_STAGE..stage).filter_map(|cleared| picks.get(&cleared).or_else(|| picks.get(&EVERY_STAGE))).fold(Budget::default(), |budget, picks| Budget {
            recruits: budget.recruits + picks.recruits,
            bonuses: budget.bonuses + picks.bonuses,
        }))
    }

    pub fn recruited(&self, unit: u32) -> bool {
        self.cat(unit).is_some_and(|cat| !cat.starter)
    }

    pub fn spent(&self, run: &Run) -> Budget {
        let recruits = self.fielded(run).iter().filter(|unit| self.recruited(**unit)).count();
        let bonuses = run.bonuses.iter().filter_map(|(id, level)| self.bonus(*id).filter(|bonus| !bonus.permanent).map(|_| *level)).sum();

        Budget { recruits: i32::try_from(recruits).unwrap_or(i32::MAX), bonuses }
    }

    pub fn tally(&self, run: &Run, stage: i32) -> Option<Tally> {
        let budget = self.budget(stage)?;
        let spent = self.spent(run);

        Some(Tally {
            recruits: format!("{RECRUITS_LABEL} {}/{}", spent.recruits, budget.recruits),
            bonuses: format!("{BONUSES_LABEL} {}/{}", spent.bonuses, budget.bonuses),
            recruits_over: spent.recruits > budget.recruits,
            bonuses_over: spent.bonuses > budget.bonuses,
        })
    }

    pub fn within_budget(&self, run: &Run, stage: i32) -> bool {
        self.budget(stage).is_none_or(|budget| budget.covers(self.spent(run)))
    }


    pub fn standing(&self, run: &Run, unit: u32) -> Standing {
        let Some(cat) = self.cat(unit) else {
            return Standing { level: 1, plus: 0, form: 0 };
        };
        let rarity = cat.rarity;
        let gains = run
            .bonuses
            .iter()
            .filter_map(|(id, held)| Some((self.bonus(*id)?, *held)))
            .filter(|(bonus, _)| i32::try_from(unit).is_ok_and(|unit| bonus.rule.applies_to(unit, rarity)))
            .filter_map(|(bonus, held)| bonus.rule.power_up().map(|(level, plus)| (level.saturating_mul(held), plus.saturating_mul(held))));
        let (level, plus) = gains.fold((cat.level.saturating_sub(1), cat.plus), |(level, plus), (more, extra)| (level.saturating_add(more), plus.saturating_add(extra)));
        let plus = if run.registered.contains(&unit) { plus.saturating_add(self.registered_plus) } else { plus };
        let form = roguelike_form(level.saturating_add(plus), cat.forms);

        Standing { level: level.saturating_add(1), plus, form: usize::try_from(form).unwrap_or(0) }
    }

    pub fn grants(&self, run: &Run, unit: u32) -> Grants {
        let rarity = self.cat(unit).map_or(0, |cat| cat.rarity);
        let mut grants = Grants { talent: Talent { id: unit, ..Talent::default() }, ..Grants::default() };
        let held: Vec<(&RoguelikeBonusBuffSkill, i32)> = run
            .bonuses
            .iter()
            .filter_map(|(id, level)| Some((self.bonus(*id)?, *level)))
            .filter(|(bonus, _)| i32::try_from(unit).is_ok_and(|unit| bonus.rule.applies_to(unit, rarity)))
            .filter_map(|(bonus, level)| Some((bonus.skill.as_ref()?, level)))
            .collect();

        for stack in roguelike_stacks(&held) {
            let Some((mut group, level)) = stack.talent else {
                grants.extras.push((stack.ability_id, stack.values, stack.maxed));

                continue;
            };
            let Ok(index) = u8::try_from(grants.talent.groups.len()) else {
                continue;
            };

            if let Some((text_id, name_id)) = self.texts.get(&group.ability_id) {
                group.text_id = *text_id;
                group.name_id = *name_id;
            }

            grants.levels.insert(index, level);
            grants.talent.groups.push(group);
        }

        grants
    }

    pub fn treasure(&self, catalog: &Catalog, levels: &TreasureLevels) -> treasure::Bonus {
        treasure::Bonus::preset(catalog, levels, &self.preset)
    }

    fn member(&self, run: &Run, unit: u32) -> Member {
        let standing = self.standing(run, unit);

        Member { id: unit, form: standing.form, level: standing.label(), talents: self.grants(run, unit).levels, ..Member::default() }
    }

    pub fn lineup(&self, run: &Run) -> Lineup {
        Lineup {
            slots: self.fielded(run).iter().map(|unit| self.member(run, *unit)).collect(),
            bench: run.bench.map(|held| held.map(|unit| self.member(run, unit))),
            ..Lineup::default()
        }
    }

    pub fn arrange(&self, lineup: &Lineup, lifted: Option<Cell>, target: Cell, edge: Edge) -> Option<Arrangement> {
        match (lifted, target) {
            (Some(from @ Cell::Slot(_)), Cell::Bench(_)) => lineup.swap(from, target),
            _ => lineup.arrange(lifted, target, edge),
        }
    }

    fn keep(&self, run: &mut Run, lineup: &Lineup) {
        run.slots = lineup.slots.iter().map(|member| member.id).collect();
        run.bench = lineup.bench.clone().map(|held| held.map(|member| member.id));
        self.settle(run);
    }

    pub fn commit(&self, run: &mut Run, plan: &Arrangement, fresh: Option<u32>) {
        let mut lineup = self.lineup(run);
        let fresh = fresh.filter(|unit| self.cat(*unit).is_some() && lineup.find(*unit).is_none()).map(|unit| self.member(run, unit));

        if plan.lifted.is_none() && fresh.is_none() {
            return;
        }

        lineup.commit(plan, fresh);
        self.keep(run, &lineup);
    }

    pub fn add(&self, run: &mut Run, unit: u32) {
        if self.cat(unit).is_none() {
            return;
        }

        let mut lineup = self.lineup(run);

        lineup.add(self.member(run, unit));
        self.keep(run, &lineup);
    }

    pub fn add_over(&self, run: &mut Run, unit: u32) {
        if self.cat(unit).is_none() {
            return;
        }

        let mut lineup = self.lineup(run);

        if lineup.add(self.member(run, unit)).is_none()
            && let Some(last) = lineup.bench.last_mut()
        {
            *last = Some(self.member(run, unit));
        }

        self.keep(run, &lineup);
    }

    pub fn unlocked(&self, run: &Run) -> Vec<i32> {
        let held = run.bonuses.keys().filter_map(|id| self.bonus(*id)).filter_map(|bonus| match bonus.rule.effect() {
            RoguelikeBonusEffect::CatCannon { cannon, .. } => Some(cannon),
            _ => None,
        });

        std::iter::once(BASIC_CANNON).chain(held).fold(Vec::new(), |mut seen, cannon| {
            if !seen.contains(&cannon) {
                seen.push(cannon);
            }

            seen
        })
    }

    pub fn cannon(&self, run: &Run) -> i32 {
        let unlocked = self.unlocked(run);

        run.cannon.filter(|cannon| unlocked.contains(cannon)).or_else(|| unlocked.last().copied()).unwrap_or(BASIC_CANNON)
    }

    pub fn enlist(&self, run: &mut Run, unit: u32, registered: bool) {
        if registered && self.cat(unit).is_some() {
            run.registered.insert(unit);
        } else {
            run.registered.remove(&unit);
        }
    }

    pub fn toggle_registered(&self, run: &mut Run, unit: u32) {
        self.enlist(run, unit, !run.registered.contains(&unit));
    }

    pub fn registered(&self, run: &Run) -> Vec<i32> {
        self.fielded(run).iter().filter(|unit| run.registered.contains(unit)).filter_map(|unit| i32::try_from(*unit).ok()).collect()
    }

    pub fn run_from(&self, roster: &[(bool, i32)], bonuses: &[(i32, i32)], cannon: i32, registered: &[i32]) -> Run {
        let slots = roster
            .iter()
            .filter_map(|(starter, id)| match starter {
                true => u32::try_from(*id).ok(),
                false => self.cats.iter().find(|cat| cat.chara == Some(*id)).map(|cat| cat.unit),
            })
            .collect();
        let registered = registered.iter().filter_map(|unit| u32::try_from(*unit).ok()).collect();
        let mut run = Run { slots, bonuses: bonuses.iter().copied().collect(), cannon: Some(cannon), registered, ..Run::default() };

        self.settle(&mut run);

        run
    }

    pub fn discard(&self, run: &mut Run, cell: Cell) {
        if let Cell::Bench(index) = cell
            && let Some(held) = run.bench.get_mut(index)
        {
            *held = None;
        }

        self.settle(run);
    }

    pub fn raise(&self, run: &mut Run, id: i32, level: i32) {
        let Some(bonus) = self.bonus(id).filter(|bonus| self.offers(run, bonus)) else {
            return;
        };

        let unlocks = match bonus.rule.effect() {
            RoguelikeBonusEffect::CatCannon { cannon, .. } => Some(cannon),
            _ => None,
        };
        match level.clamp(0, bonus.limit) {
            0 => run.bonuses.remove(&id),
            level => run.bonuses.insert(id, level),
        };

        if let Some(cannon) = unlocks.filter(|_| run.bonuses.contains_key(&id)) {
            run.cannon = Some(cannon);
        }

        run.cannon = run.cannon.filter(|cannon| *cannon != BASIC_CANNON && self.unlocked(run).contains(cannon));
    }

    fn is_kind(&self, id: i32, global: bool, permanent: bool) -> bool {
        self.bonus(id).is_some_and(|bonus| bonus.target.is_none() == global && bonus.permanent == permanent)
    }

    pub fn has_bonuses(&self, run: &Run, global: bool, permanent: bool) -> bool {
        run.bonuses.keys().any(|id| self.is_kind(*id, global, permanent))
    }

    pub fn clear_bonuses(&self, run: &mut Run, global: bool, permanent: bool) {
        let held: Vec<i32> = run.bonuses.keys().copied().filter(|id| self.is_kind(*id, global, permanent)).collect();

        for id in held {
            self.raise(run, id, 0);
        }
    }

    pub fn settle(&self, run: &mut Run) {
        let seats = self.starters.len().min(LINEUP_SLOTS);
        let mut slots: Vec<u32> = Vec::new();

        for unit in std::mem::take(&mut run.slots) {
            if self.cat(unit).is_some() && !slots.contains(&unit) && slots.len() < LINEUP_SLOTS {
                slots.push(unit);
            }
        }

        if !slots.is_empty() {
            for unit in &self.starters {
                if slots.len() < seats && !slots.contains(unit) {
                    slots.push(*unit);
                }
            }
        }

        if slots != self.starters {
            run.slots = slots;
        }

        let fielded = self.fielded(run).to_vec();
        let mut benched: Vec<u32> = Vec::new();

        for held in &mut run.bench {
            *held = held.filter(|unit| self.cat(*unit).is_some() && !fielded.contains(unit) && !benched.contains(unit));
            benched.extend(*held);
        }

        let held = std::mem::take(&mut run.bonuses);

        run.bonuses = held
            .into_iter()
            .filter_map(|(id, level)| {
                let bonus = self.bonus(id).filter(|bonus| bonus.target.is_none_or(|unit| fielded.contains(&unit)))?;
                let level = level.min(bonus.limit);

                (level > 0).then_some((id, level))
            })
            .collect();
        run.cannon = run.cannon.filter(|cannon| *cannon != BASIC_CANNON && self.unlocked(run).contains(cannon));
        run.registered.retain(|unit| self.cat(*unit).is_some());
    }

    pub fn settled(&self, run: &Run) -> Run {
        let mut run = run.clone();

        self.settle(&mut run);

        run
    }

    pub fn roster(&self, run: &Run) -> Vec<(bool, i32)> {
        run.slots
            .iter()
            .filter_map(|unit| {
                let cat = self.cat(*unit)?;

                match cat.chara {
                    Some(chara) if !cat.starter => Some((false, chara)),
                    _ => Some((true, i32::try_from(*unit).ok()?)),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use codex::combat::Entity;

    use super::*;

    fn cat(unit: u32, chara: Option<i32>, level: i32, forms: i32) -> Cat {
        Cat { unit, chara, starter: chara.is_none(), names: [Some(format!("Cat {unit}")), Some(format!("Big Cat {unit}")), None, None], level, plus: 0, rarity: 2, forms }
    }

    fn bonus(id: i32, limit: i32, target: Option<u32>, rule: RoguelikeBonusBuff) -> Bonus {
        Bonus { id, label: format!("{NAMED}: Bonus"), limit, target, permanent: false, sheet: ABILITY_SHEET, icon: AbilityIcon::None, fallback: "", rule, skill: None }
    }

    fn board() -> Board {
        let level_up = RoguelikeBonusBuff { buff_type: Some(0), level_up: Some(5), target_rarity: Some(-1), ..RoguelikeBonusBuff::default() };
        let skill = |unit: i32| RoguelikeBonusBuff { buff_type: Some(1), buff_skill_id: Some(0), target_chara_id: Some(unit), ..RoguelikeBonusBuff::default() };

        Board {
            map_id: 39001,
            starters: (0..10).collect(),
            cats: (0..10).map(|unit| cat(unit, None, 20, 3)).chain([cat(75, Some(4), 20, 3), cat(76, Some(6), 5, 2)]).collect(),
            bonuses: vec![bonus(0, 5, None, level_up), bonus(50, 1, Some(75), skill(75)), bonus(86, 1, Some(9), skill(9))],
            texts: HashMap::from([(32, (7, 3))]),
            registered_plus: 5,
            picks: Some(BTreeMap::from([(-1, Budget { recruits: 1, bonuses: 1 })])),
            preset: CertificationPreset::default(),
        }
    }

    // Each button clears one kind in one popup and has to leave the other three alone.
    #[test]
    fn clearing_bonuses_stops_at_its_own_kind() {
        let mut board = board();
        let rule = RoguelikeBonusBuff { buff_type: Some(0), level_up: Some(5), target_rarity: Some(-1), ..RoguelikeBonusBuff::default() };

        board.bonuses.push(Bonus { permanent: true, ..bonus(90, 3, None, rule.clone()) });
        board.bonuses.push(Bonus { permanent: true, ..bonus(91, 3, Some(9), rule) });

        let mut run = Run { bonuses: BTreeMap::from([(0, 3), (86, 1), (90, 2), (91, 1)]), ..Run::default() };

        board.clear_bonuses(&mut run, true, false);

        assert_eq!(run.bonuses, BTreeMap::from([(86, 1), (90, 2), (91, 1)]));
        assert!(!board.has_bonuses(&run, true, false));
        assert!(board.has_bonuses(&run, true, true));

        board.clear_bonuses(&mut run, false, true);

        assert_eq!(run.bonuses, BTreeMap::from([(86, 1), (90, 2)]));
        assert!(board.has_bonuses(&run, false, false));
        assert!(!board.has_bonuses(&run, false, true));
    }

    // Only the maps the settings name are challenges, and a broken file names none.
    #[test]
    fn a_map_is_a_challenge_only_when_the_settings_name_it() {
        let settings = br#"{"EventID":{"0":{"Data":[{"MapID":[39001],"Level":[0],"PresetFileName":"A.preset"}]}}}"#;

        assert!(holds(settings, 39001));
        assert!(!holds(settings, 39000));
        assert!(!holds(b"", 39001));
    }

    // The lineup is always full, so a new cat can only take a seat by pushing its holder to the bench.
    #[test]
    fn a_recruit_takes_a_seat_and_the_cat_it_replaced_goes_to_the_bench() {
        let board = board();
        let mut run = Run::default();
        let lineup = board.lineup(&run);

        board.raise(&mut run, 86, 1);

        let plan = board.arrange(&lineup, None, Cell::Slot(9), Edge::Before).unwrap();

        board.commit(&mut run, &plan, Some(75));

        assert_eq!(run.slots, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 75]);
        assert_eq!(run.bench[0], Some(9));
        assert!(run.bonuses.is_empty(), "the starter left the lineup, so its bonus went with it");
        assert_eq!(board.roster(&run).last(), Some(&(false, 4)));
        assert_eq!(board.roster(&run).first(), Some(&(true, 0)));

        let lineup = board.lineup(&run);

        assert!(board.arrange(&lineup, Some(Cell::Slot(0)), Cell::Bench(1), Edge::Before).is_none(), "a seated cat never leaves for an empty bench seat");

        let traded = board.arrange(&lineup, Some(Cell::Slot(0)), Cell::Bench(0), Edge::Before).unwrap();
        let mut swapped = run.clone();

        board.commit(&mut swapped, &traded, None);

        assert_eq!((swapped.slots[0], swapped.bench[0]), (9, Some(0)), "over a benched cat the two trade places");

        let back = board.arrange(&lineup, Some(Cell::Bench(0)), Cell::Slot(9), Edge::Before).unwrap();

        board.commit(&mut run, &back, None);

        assert!(run.slots.is_empty(), "the starter lineup again is no lineup of its own");
        assert_eq!(run.bench[0], Some(75));

        board.discard(&mut run, Cell::Slot(0));
        board.discard(&mut run, Cell::Bench(0));

        assert!(run.is_fresh());
    }

    // Level bonuses move a cat's form, and the form is what every picture of it has to show.
    #[test]
    fn a_level_bonus_raises_the_form_a_cat_stands_in() {
        let board = board();
        let mut run = Run::default();

        board.add(&mut run, 76);

        assert_eq!(run.bench[0], Some(76), "a full lineup sends a picked cat to the bench");
        assert_eq!(board.standing(&run, 76), Standing { level: 5, plus: 0, form: 0 });
        assert_eq!(board.standing(&run, 0), Standing { level: 20, plus: 0, form: 1 });

        board.raise(&mut run, 0, 9);

        assert_eq!(run.level(0), 5, "a level stops at the bonus's own limit");
        assert_eq!(board.standing(&run, 0), Standing { level: 45, plus: 0, form: 2 });
        assert_eq!(board.standing(&run, 76), Standing { level: 30, plus: 0, form: 1 }, "two forms is all this cat has");
        assert_eq!(board.lineup(&run).slots[0], Member { id: 0, form: 2, level: "45".to_owned(), ..Member::default() });
        assert_eq!(board.deck(&run)[0], (0, Standing { level: 45, plus: 0, form: 2 }));

        let own = &board.bonuses[2];

        assert_eq!(board.label(&Run::default(), own), "Big Cat 9: Bonus", "a starter at level 20 is already in its second form");
        assert_eq!(board.title(&run, own), "Cat 9", "a cat with no name for its form falls back to the first it has");
        assert_eq!(board.title(&run, &board.bonuses[0]), EVERY_CAT);
    }

    // What a cat is shown with has to be what the run gave it: talents the cat list can draw, and the rest as stat changes.
    #[test]
    fn a_skill_bonus_reaches_the_cat_as_a_talent_or_a_stat_change() {
        let mut board = board();
        let health = RoguelikeBonusBuffSkill { ability_id: Some(32), max_level: Some(5), parameters: vec![40, 200] };
        let cheaper = RoguelikeBonusBuffSkill { ability_id: Some(1012), max_level: Some(5), parameters: vec![15, 75] };

        board.bonuses[1].skill = Some(health);
        board.bonuses[1].limit = 5;
        board.bonuses.push(Bonus { skill: Some(cheaper), ..bonus(7, 5, None, RoguelikeBonusBuff { buff_type: Some(1), ..RoguelikeBonusBuff::default() }) });

        let run = Run { slots: vec![75, 1, 2, 3, 4, 5, 6, 7, 8, 9], bonuses: BTreeMap::from([(50, 3), (7, 2)]), ..Run::default() };
        let grants = board.grants(&run, 75);

        assert_eq!(grants.talent.groups.len(), 1);
        assert_eq!((grants.talent.groups[0].ability_id, grants.talent.groups[0].text_id, grants.talent.groups[0].name_id), (32, 7, 3));
        assert_eq!(grants.levels, HashMap::from([(0, 3)]));
        assert_eq!(grants.extras, vec![(1012, vec![30], false)]);
        assert_eq!(board.lineup(&run).slots[0].talents, HashMap::from([(0, 3)]));
        assert!(board.grants(&run, 1).talent.groups.is_empty(), "a bonus for one cat is not another cat's");

        let mut stats = Entity { eoc1_cost: 750, ..Entity::default() };

        grants.apply(&mut stats);

        assert_eq!(stats.eoc1_cost, 525);
    }

    // The base fires the cannon bonus switched on last, and falls back when that bonus is dropped.
    #[test]
    fn the_cannon_follows_the_bonus_that_unlocks_it() {
        let mut board = board();
        let cannon = |id: i32| RoguelikeBonusBuff { buff_type: Some(3), id: Some(id), level: Some(20), ..RoguelikeBonusBuff::default() };

        board.bonuses.push(bonus(255, 1, None, cannon(1)));
        board.bonuses.push(bonus(256, 1, None, cannon(3)));

        let mut run = Run::default();

        assert_eq!((board.cannon(&run), board.unlocked(&run)), (0, vec![0]));

        board.raise(&mut run, 256, 1);
        board.raise(&mut run, 255, 1);

        assert_eq!((board.cannon(&run), board.unlocked(&run)), (1, vec![0, 1, 3]), "a later map keeps the earlier maps' cannons, and the last one switched on fires");

        board.raise(&mut run, 255, 0);

        assert_eq!(board.cannon(&run), 3, "the other cannon is still held");

        board.raise(&mut run, 256, 0);

        assert!(run.is_fresh(), "dropping the bonus takes its cannon with it");

        let replayed = board.run_from(&[(false, 4), (true, 1)], &[(255, 1)], 1, &[]);

        assert_eq!((replayed.slots[0], replayed.slots[1], board.cannon(&replayed)), (75, 1, 1));

        let mut full = Run { bench: [Some(75), None, None, None, None], ..Run::default() };

        for unit in [76, 75] {
            board.add_over(&mut full, unit);
        }

        assert_eq!(full.bench, [Some(75), Some(76), None, None, None], "a cat already on the bench is not added twice");
    }

    // Every cleared stage hands out one cat and one bonus level, so the stage picked is the budget; permanent bonuses and the bench are free.
    #[test]
    fn the_picks_of_the_stages_cleared_so_far_are_the_budget() {
        let mut board = board();

        board.bonuses[0].limit = 5;
        board.bonuses.push(Bonus { permanent: true, ..bonus(90, 3, None, RoguelikeBonusBuff { buff_type: Some(0), level_up: Some(5), target_rarity: Some(-1), ..RoguelikeBonusBuff::default() }) });

        let mut run = Run::default();

        assert_eq!(board.budget(0), Some(Budget::default()));
        assert_eq!(board.budget(3), Some(Budget { recruits: 3, bonuses: 3 }));

        board.raise(&mut run, 0, 3);
        board.raise(&mut run, 90, 3);
        board.add(&mut run, 76);

        assert_eq!(board.spent(&run), Budget { recruits: 0, bonuses: 3 }, "the permanent bonus and the benched cat cost nothing");
        assert!(board.within_budget(&run, 3) && !board.within_budget(&run, 2));

        let plan = board.arrange(&board.lineup(&run), None, Cell::Slot(9), Edge::Before).unwrap();

        board.commit(&mut run, &plan, Some(75));

        assert_eq!(board.spent(&run).recruits, 1);

        let tally = board.tally(&run, 1).unwrap();

        assert_eq!((tally.recruits.as_str(), tally.bonuses.as_str(), tally.recruits_over, tally.bonuses_over), ("Recruits 1/1", "Bonuses 3/1", false, true), "backtracking to an earlier stage leaves the run over budget");
        assert!(tally.over());
        assert_eq!((tally.recruits_note(), tally.bonuses_note()), (None, Some(TOO_MANY_BONUSES)));
        assert_eq!(board.tally(&run, 0).unwrap().recruits_note(), Some(TOO_MANY_RECRUITS));

        board.picks = None;

        assert!(board.tally(&run, 0).is_none() && board.within_budget(&run, 0), "a map the lottery does not list has no budget");
    }

    // Under a cat's own header the text does not need to say whose bonus it is again.
    #[test]
    fn a_bonus_under_its_cats_header_drops_the_name() {
        let board = board();
        let run = Run::default();
        let owned = bonus(7, 1, Some(1), RoguelikeBonusBuff::default());
        let woven = Bonus { label: format!("Gives {NAMED} a boost"), ..owned.clone() };

        assert_eq!(board.label(&run, &owned), "Big Cat 1: Bonus");
        assert_eq!(board.detail(&run, &owned), "Bonus");
        assert_eq!(board.detail(&run, &woven), "Gives Big Cat 1 a boost", "a name inside the sentence stays");
    }

    // A cat in the player's Cat Guide joins the run with extra plus levels, which can carry it into its next form.
    #[test]
    fn a_registered_cat_gains_its_plus_levels() {
        let board = board();
        let mut run = Run::default();
        let before = board.standing(&run, 75);

        board.enlist(&mut run, 75, true);

        let after = board.standing(&run, 75);

        assert_eq!((after.level, after.plus), (before.level, before.plus + 5));
        assert_eq!(after.label(), format!("{}+{}", before.level, before.plus + 5));

        board.enlist(&mut run, 75, false);

        assert!(run.is_fresh());
    }

    // A map keeps a list of runs the way the Lineup tab keeps lineups, and is never left without one.
    #[test]
    fn a_map_always_has_a_run_to_play() {
        let mut runs = Runs::default();

        runs.current_mut().slots = vec![75];
        runs.create();

        assert_eq!((runs.saved.len(), runs.selected, runs.current().map(Run::is_fresh)), (2, 1, Some(true)));

        runs.load(0);
        runs.load(9);
        runs.delete();

        assert_eq!((runs.saved.len(), runs.selected), (1, 0), "the second run moved up");

        runs.delete();

        assert_eq!(runs, Runs::default(), "deleting the last run leaves a fresh one");
    }

    // A run saved against older data must not hand the battle picks the tables no longer hold.
    #[test]
    fn a_saved_run_is_cut_back_to_what_the_board_holds() {
        let board = board();
        let run = Run { slots: vec![75, 75, 400, 3], bench: [Some(75), Some(76), Some(76), Some(9), None], bonuses: BTreeMap::from([(0, 40), (50, 1), (400, 1), (86, 1)]), cannon: Some(5), ..Run::default() };
        let settled = board.settled(&run);

        assert_eq!(settled.slots, vec![75, 3, 0, 1, 2, 4, 5, 6, 7, 8]);
        assert_eq!(settled.bench, [None, Some(76), None, Some(9), None]);
        assert_eq!(settled.bonuses, BTreeMap::from([(0, 5), (50, 1)]));
        assert_eq!(settled.cannon, None, "a cannon no bonus unlocks is not the run's to fire");
    }
}
