pub mod values;
use std::fmt;
use std::sync::LazyLock;
use codex::combat::{get_talent, targets_traits, Faction, Identity};
use crate::systems::combat::NameBook;
use crate::systems::combat::registry::get_display_def;
use crate::Vfs;
use super::resolved::{Rule, PERCENT_CEILING};
use super::schema::{slot_of, TALENT_HEAD, TALENT_SLOTS, TALENT_STRIDE};
use super::Draft;
use values::{Unit, Value};

pub const ABILITY: usize = 0;

pub const MAX_LEVEL: usize = 1;

pub const VALUES: usize = 4;

pub const MIN_1: usize = 2;

pub const TEXT_ID: usize = 10;

pub const COST_ID: usize = 11;

pub const NAME_ID: usize = 12;

pub const LIMIT: usize = 13;

pub const TYPE_ID: usize = 1;

pub const TRAITS: [Identity; 12] = [
    Identity::TraitRed,
    Identity::TraitFloating,
    Identity::TraitDark,
    Identity::TraitMetal,
    Identity::TraitAngel,
    Identity::TraitAlien,
    Identity::TraitZombie,
    Identity::TraitRelic,
    Identity::TraitTraitless,
    Identity::TraitWitch,
    Identity::TraitEva,
    Identity::TraitAku,
];

pub const ULTRA: i32 = 1;

const RANGE_STEP: i32 = 4;

const MONEY_NUMERATOR: i32 = 3;

const MONEY_DENOMINATOR: i32 = 2;

pub const HEADER_LINES: usize = 3;

pub const ABILITY_COLUMN: usize = 0;

pub fn split(words: &[usize], lines: usize) -> usize {
    let per_line = words.len().div_ceil(lines);

    words
        .chunks(per_line)
        .map(|chunk| chunk.iter().sum::<usize>() + chunk.len().saturating_sub(1))
        .max()
        .unwrap_or_default()
}

pub const CHOOSER_COLUMNS: usize = 7;

pub const SEARCH_PLACEHOLDER: &str = "Search fields";

pub const IDLE_TYPE_ID: &str = "No Talents utilize Type ID";

const NO_IMAGE: &str = "None";

pub const IMAGE_HUNT: &str = "Search ID...";

pub const WORDING_HUNT: &str = "Search ID or Content...";

pub const TYPE_ID_TITLE: &str = "Type ID";

pub fn offset(index: usize) -> usize {
    index.saturating_sub(TALENT_HEAD) % TALENT_STRIDE
}

pub fn image_id(named: i32, ability: i32) -> i32 {
    if named > 0 { named } else { ability }
}

pub fn arming(draft: &Draft) -> bool {
    (0..TALENT_SLOTS).any(|slot| {
        draft
            .reads_at(TALENT_HEAD + slot * TALENT_STRIDE + ABILITY)
            .and_then(|id| u8::try_from(id).ok())
            .is_some_and(targets_traits)
    })
}

pub fn spare(draft: &Draft) -> Option<usize> {
    let used = filled(draft);

    (used < TALENT_SLOTS).then(|| TALENT_HEAD + used * TALENT_STRIDE)
}

pub fn dim_from(spare: Option<usize>, shown: &[usize]) -> Option<usize> {
    let first = spare?;

    shown.contains(&first).then_some(first)
}

pub fn filled(draft: &Draft) -> usize {
    (0..TALENT_SLOTS)
        .take_while(|slot| draft.reads_at(TALENT_HEAD + slot * TALENT_STRIDE) != Some(0))
        .count()
}

#[derive(Clone, PartialEq, Eq)]
pub struct Skill {
    pub id: i32,
    pub name: Option<String>,
}

impl Skill {
    pub fn of(id: i32, glossary: &NameBook) -> Skill {
        catalog(glossary).into_iter().find(|skill| skill.id == id).unwrap_or(Skill { id, name: None })
    }
}

impl fmt::Display for Skill {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.name {
            Some(name) => formatter.write_str(name),
            None => write!(formatter, "{}", self.id),
        }
    }
}

pub static CATALOG: LazyLock<Vec<Skill>> = LazyLock::new(|| {
    let mut listed = vec![Skill { id: 0, name: Some("None".to_owned()) }];

    listed.extend((1..=u8::MAX).filter_map(|id| {
        let ability = get_talent(id)?;
        let name = get_display_def(ability.identity).name;

        Some(Skill { id: i32::from(id), name: (!name.is_empty()).then(|| name.to_owned()) })
    }));

    listed
});

pub fn catalog(glossary: &NameBook) -> Vec<Skill> {
    CATALOG
        .iter()
        .map(|skill| {
            let named = u8::try_from(skill.id).ok().filter(|id| *id > 0).and_then(|id| {
                glossary.talent(id).map(str::to_owned).or_else(|| get_talent(id).map(|ability| glossary.ability(ability.identity, Faction::Cat).to_owned()))
            });

            Skill { id: skill.id, name: named.or_else(|| skill.name.clone()) }
        })
        .collect()
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Named(pub i32);

impl Named {
    pub fn matches(self, wanted: &str) -> bool {
        wanted.is_empty() || self.to_string().to_lowercase().contains(&wanted.to_lowercase())
    }
}

impl fmt::Display for Named {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 < 0 {
            return formatter.write_str(NO_IMAGE);
        }

        write!(formatter, "{}", self.0)
    }
}

pub fn available(vfs: &Vfs) -> Vec<i32> {
    let mut found: Vec<i32> = vfs
        .glob("Skill_name_")
        .into_iter()
        .filter_map(|name| {
            let digits: String =
                name.strip_prefix("Skill_name_")?.chars().take_while(char::is_ascii_digit).collect();

            (digits.len() == 3).then(|| digits.parse().ok())?
        })
        .collect();

    found.sort_unstable();
    found.dedup();

    found
}

pub fn rule(index: usize, cells: &[i32]) -> Rule {
    if slot_of(index).is_none() {
        return Rule::Plain;
    }

    if offset(index) == NAME_ID {
        return Rule::Floor(-1);
    }

    meaning(index, cells).map_or(Rule::Plain, |value| shape(value.unit()))
}

fn shape(unit: Unit) -> Rule {
    match unit {
        Unit::Percent => Rule::Percent,
        Unit::Inverted => Rule::Inverted(PERCENT_CEILING),
        Unit::Range => Rule::Ratio(1, RANGE_STEP),
        Unit::Money => Rule::Ratio(MONEY_NUMERATOR, MONEY_DENOMINATOR),
        Unit::Multiplier | Unit::Frames | Unit::Count => Rule::Plain,
    }
}

pub fn meaning(index: usize, cells: &[i32]) -> Option<Value> {
    let slot = slot_of(index)?;
    let pair =
        offset(index).checked_sub(MIN_1).map(|span| span / 2).filter(|pair| *pair < VALUES)?;
    let ability = cells.get(TALENT_HEAD + slot * TALENT_STRIDE).copied().unwrap_or_default();

    values::value(ability, pair)
}

pub fn mirror(index: usize, cells: &[i32]) -> Option<usize> {
    if !meaning(index, cells)?.is_fixed() {
        return None;
    }

    let least = offset(index).saturating_sub(MIN_1).is_multiple_of(2);

    Some(if least { index + 1 } else { index - 1 })
}
