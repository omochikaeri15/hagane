use std::cmp::Reverse;
use std::sync::Arc;

use codex::cat::unit::Equipment;
use codex::combat::Foe;
use codex::combat::orb::{bits, FIRST_FORM};
use foundry::domains::cat::orbs::{self, Allowance, Kind, Titles, GRADES};
use foundry::domains::cat::scanner::CatEntry;

use super::fuzzy::{self, Query};
use super::grammar;
use super::library::Library;
use super::prose::Trouble;

const EMPTY: &str = "-";
const NONE: char = 'x';
const SPLIT: char = '.';

pub(crate) fn tag(slots: &[Option<u32>]) -> String {
    if slots.iter().all(Option::is_none) {
        return EMPTY.to_owned();
    }

    slots.iter().map(|slot| slot.map_or_else(|| NONE.to_string(), |orb| format!("{orb:x}"))).collect::<Vec<_>>().join(&SPLIT.to_string())
}

pub(crate) fn tagged(text: &str) -> Vec<Option<u32>> {
    if text == EMPTY {
        return Vec::new();
    }

    text.split(SPLIT).map(|slot| u32::from_str_radix(slot, 16).ok()).collect()
}

pub(crate) const BEST_GRADE: usize = 4;

pub(crate) struct Bench {
    list: Arc<Vec<Equipment>>,
    kinds: Vec<Kind>,
    masks: Arc<Vec<u32>>,
    allowance: Allowance,
    titles: Titles,
    effects: Vec<String>,
    pub(crate) doors: Vec<bool>,
}

impl Bench {
    pub(crate) fn of(library: &Library, cat: &CatEntry, form: usize, level: i32) -> Self {
        let vault = &library.vault;
        let count = if form >= FIRST_FORM { vault.vds.cats.orb_slots(&vault.vfs).get(&cat.id).copied().unwrap_or(0) } else { 0 };
        let list = vault.vds.cats.orbs(&vault.vfs);
        let masks = vault.vds.cats.orb_masks(&vault.vfs);

        Self {
            kinds: orbs::kinds(&list),
            list,
            allowance: Allowance::of(cat, Arc::clone(&masks)),
            masks,
            titles: Titles::load(&vault.vfs),
            effects: orbs::effect_names(&vault.vfs),
            doors: orbs::open_slots(count, vault.vds.cats.orb_gates(&vault.vfs).get(&cat.id), level),
        }
    }

    pub(crate) fn kind(&self, orb: u32) -> Option<Kind> {
        orbs::kind(&self.list, orb)
    }

    pub(crate) fn grade(&self, orb: u32) -> Option<usize> {
        orbs::grade(&self.list, orb)
    }

    pub(crate) fn aims(&self, content: i32) -> Vec<i32> {
        orbs::attributes(&self.kinds, content).into_iter().filter(|attribute| self.allowance.permits(Kind { content, attribute: Some(*attribute) })).collect()
    }

    pub(crate) fn offered(&self) -> Vec<i32> {
        orbs::effects(&self.kinds).into_iter().filter(|content| orbs::attributes(&self.kinds, *content).is_empty() || !self.aims(*content).is_empty()).collect()
    }

    pub(crate) fn grades(&self, kind: Kind) -> Vec<usize> {
        let mut held: Vec<usize> = orbs::grades(&self.list, kind).into_iter().map(|(grade, _)| grade).collect();

        held.sort_unstable();
        held.dedup();
        held
    }

    pub(crate) fn effect_label(&self, content: i32) -> String {
        usize::try_from(content).ok().and_then(|at| self.effects.get(at)).filter(|name| !name.is_empty()).map_or_else(|| format!("Effect {content}"), Clone::clone)
    }

    pub(crate) fn trait_label(library: &Library, attribute: i32) -> String {
        usize::try_from(attribute).ok().and_then(|row| library.context().names.orb_trait(row)).map_or_else(|| format!("Trait {attribute}"), str::to_owned)
    }

    pub(crate) fn with_effect(&self, held: Option<u32>, content: i32) -> Option<u32> {
        let grade = held.and_then(|orb| self.grade(orb)).unwrap_or(BEST_GRADE);
        let aimed = held.and_then(|orb| self.kind(orb)).and_then(|kind| kind.attribute);
        let aims = self.aims(content);
        let attribute = aims.iter().find(|attribute| Some(**attribute) == aimed).or_else(|| aims.first()).copied();

        orbs::pick(&self.list, Kind { content, attribute }, grade)
    }

    pub(crate) fn with_trait(&self, held: Option<u32>, attribute: i32) -> Option<u32> {
        let kind = held.and_then(|orb| self.kind(orb))?;
        let grade = held.and_then(|orb| self.grade(orb)).unwrap_or(BEST_GRADE);

        orbs::pick(&self.list, Kind { attribute: Some(attribute), ..kind }, grade)
    }

    pub(crate) fn with_grade(&self, held: Option<u32>, grade: usize) -> Option<u32> {
        orbs::pick(&self.list, held.and_then(|orb| self.kind(orb))?, grade)
    }

    fn trait_row(&self, library: &Library, word: &str) -> Option<i32> {
        let named = grammar::trait_of(&word.to_lowercase()).map(|label| bits(Foe::default().with(label))).filter(|bit| *bit != 0);
        let typed = fuzzy::normalized(word);
        let names = library.context().names;

        self.kinds.iter().filter_map(|kind| kind.attribute).find(|row| {
            let at = usize::try_from(*row).ok();

            named.is_some_and(|bit| at.and_then(|at| self.masks.get(at)) == Some(&bit)) || at.and_then(|at| names.orb_trait(at)).is_some_and(|label| fuzzy::normalized(label) == typed)
        })
    }

    fn read(&self, library: &Library, typed: &str) -> Result<u32, String> {
        let said = || typed.trim().to_owned();
        let mut words: Vec<&str> = typed.split(|glyph: char| !glyph.is_alphanumeric()).filter(|word| !word.is_empty()).collect();
        let grade = words.iter().position(|word| grade_of(word).is_some()).and_then(|at| grade_of(words.remove(at)));
        let aimed = words.iter().position(|word| self.trait_row(library, word).is_some()).and_then(|at| self.trait_row(library, words.remove(at)));
        let query = Query::new(&words.join(" "));
        let content = orbs::effects(&self.kinds)
            .into_iter()
            .filter_map(|content| Some((query.score(&self.effect_label(content))?, content)))
            .max_by_key(|(score, content)| (*score, Reverse(*content)))
            .map(|(_, content)| content)
            .ok_or_else(said)?;
        let traits = orbs::attributes(&self.kinds, content);
        let aims = self.aims(content);
        let attribute = match aimed {
            _ if traits.is_empty() => None,
            Some(row) if traits.contains(&row) => Some(row),
            Some(_) => return Err(said()),
            None => aims.first().or(traits.first()).copied(),
        };
        let kind = Kind { content, attribute };
        let offered = grade.is_none_or(|grade| self.grades(kind).contains(&grade));
        let orb = orbs::pick(&self.list, kind, grade.unwrap_or(BEST_GRADE)).filter(|_| offered).ok_or_else(said)?;

        match self.list.get(orb as usize) {
            Some(_) if self.allowance.permits(kind) => Ok(orb),
            Some(held) => Err(self.titles.title(library.context().names, held)),
            None => Err(said()),
        }
    }
}

fn grade_of(word: &str) -> Option<usize> {
    GRADES.iter().position(|grade| grade.eq_ignore_ascii_case(word))
}

pub(crate) fn grade_label(grade: usize) -> &'static str {
    GRADES.get(grade).copied().unwrap_or("?")
}

pub(crate) fn assign(library: &Library, cat: &CatEntry, form: usize, level: i32, typed: &[Option<String>; 2]) -> Result<Vec<Option<u32>>, Trouble> {
    let bench = Bench::of(library, cat, form, level);
    let unit = cat.display_name(form);

    if bench.doors.is_empty() {
        return Err(Trouble::NoOrbs(unit));
    }

    let mut slots = vec![None; bench.doors.len()];

    for (slot, text) in typed.iter().enumerate().filter_map(|(slot, text)| Some((slot, text.as_deref()?))) {
        match bench.doors.get(slot) {
            None => return Err(Trouble::OneOrbSlot(unit)),
            Some(false) => return Err(Trouble::OrbLocked(unit)),
            Some(true) => {
                let orb = bench.read(library, text).map_err(|name| Trouble::NoOrbLike(unit.clone(), name))?;

                if let Some(held) = slots.get_mut(slot) {
                    *held = Some(orb);
                }
            }
        }
    }

    Ok(slots)
}

#[cfg(test)]
mod tests {
    use super::*;

    // The slots ride in a button's id, so they have to come back exactly as they went in.
    #[test]
    fn slots_survive_their_tag() {
        assert_eq!(tag(&[]), "-");
        assert_eq!(tag(&[None, None]), "-");
        assert_eq!(tagged(&tag(&[Some(0xa3), None])), [Some(0xa3), None]);
        assert_eq!(tagged(&tag(&[None, Some(309)])), [None, Some(309)]);
        assert!(tagged("-").is_empty());
    }
}
