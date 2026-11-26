use std::collections::HashMap;
use std::sync::Arc;

use codex::cat::unit::Equipment;
use codex::combat::orb::{self as rules, FIRST_FORM};
use codex::combat::{Entity, Foe, Switch, Worn};
use image::{imageops, RgbaImage};

use crate::common::formats::{self, SpriteSheet};
use crate::systems::combat::lens::Lens;
use crate::systems::combat::NameBook;
use crate::Vfs;

use super::game::talents::apply_talent_stats;
use super::scanner::CatEntry;

pub use rules::TRAITS;

pub const GRADES: [&str; 5] = ["D", "C", "B", "A", "S"];
pub const LAYERS: [&str; 3] = ["equipment_attribute", "equipment_effect", "equipment_grade"];
pub const PLAIN_FACE: i32 = 12;

const EXPLANATION: &str = "equipment_explonation.tsv";
const PLACEHOLDER: &str = "%@";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Kind {
    pub content: i32,
    pub attribute: Option<i32>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Allowance {
    forms: Arc<[Entity]>,
    masks: Arc<Vec<u32>>,
}

impl Allowance {
    pub fn of(entry: &CatEntry, masks: Arc<Vec<u32>>) -> Self {
        let levels: Option<HashMap<u8, u8>> =
            entry.talent_data.as_ref().map(|talents| talents.groups.iter().enumerate().map(|(group, talent)| (group as u8, talent.max_level.max(1))).collect());
        let forms = entry.stats.iter().skip(FIRST_FORM).flatten().map(|stats| match (entry.talent_data.as_ref(), levels.as_ref()) {
            (Some(talents), Some(levels)) => apply_talent_stats(stats, talents, levels),
            _ => stats.clone(),
        });

        Self::over(forms, masks)
    }

    fn over(forms: impl IntoIterator<Item = Entity>, masks: Arc<Vec<u32>>) -> Self {
        Self { forms: forms.into_iter().collect(), masks }
    }

    pub fn permits(&self, kind: Kind) -> bool {
        let mask = mask_of(&self.masks, kind.attribute);

        self.forms.iter().any(|stats| stats.orb_fits(kind.content, mask))
    }
}

fn mask_of(masks: &[u32], attribute: Option<i32>) -> u32 {
    attribute.and_then(|row| usize::try_from(row).ok()).and_then(|row| masks.get(row)).copied().unwrap_or(0)
}

pub fn kind(orbs: &[Equipment], orb: u32) -> Option<Kind> {
    let held = orbs.get(orb as usize)?;

    Some(Kind { content: held.content?, attribute: held.attribute })
}

pub fn grade(orbs: &[Equipment], orb: u32) -> Option<usize> {
    orbs.get(orb as usize).and_then(|held| held.grade_id).and_then(|grade| usize::try_from(grade).ok())
}

pub fn kinds(orbs: &[Equipment]) -> Vec<Kind> {
    let mut seen: Vec<Kind> = Vec::new();

    for held in orbs {
        let Some(content) = held.content else {
            continue;
        };
        let found = Kind { content, attribute: held.attribute };

        if !seen.contains(&found) {
            seen.push(found);
        }
    }

    seen
}

pub fn effects(kinds: &[Kind]) -> Vec<i32> {
    let mut seen: Vec<i32> = Vec::new();

    for kind in kinds {
        if !seen.contains(&kind.content) {
            seen.push(kind.content);
        }
    }

    seen
}

pub fn attributes(kinds: &[Kind], content: i32) -> Vec<i32> {
    kinds.iter().filter(|kind| kind.content == content).filter_map(|kind| kind.attribute).collect()
}

pub fn grades(orbs: &[Equipment], wanted: Kind) -> Vec<(usize, u32)> {
    orbs.iter()
        .enumerate()
        .filter(|(_, held)| held.content == Some(wanted.content) && held.attribute == wanted.attribute)
        .filter_map(|(orb, held)| Some((usize::try_from(held.grade_id?).ok()?, u32::try_from(orb).ok()?)))
        .collect()
}

pub fn pick(orbs: &[Equipment], wanted: Kind, grade: usize) -> Option<u32> {
    let offered = grades(orbs, wanted);

    offered
        .iter()
        .find(|(held, _)| *held == grade)
        .or_else(|| offered.iter().max_by_key(|(held, _)| *held))
        .map(|(_, orb)| *orb)
}

fn heads(vfs: &Vfs) -> Vec<String> {
    let named = vfs.variants(EXPLANATION).into_iter().next().unwrap_or_else(|| EXPLANATION.to_owned());

    vfs.find(named.as_str())
        .and_then(|path| vfs.read(&path).ok())
        .and_then(|bytes| String::from_utf8(bytes.to_vec()).ok())
        .map_or_else(Vec::new, |content| content.lines().map(|line| line.split('\t').next().unwrap_or_default().to_owned()).collect())
}

pub fn effect_names(vfs: &Vfs) -> Vec<String> {
    heads(vfs).into_iter().map(|head| head.split(PLACEHOLDER).next().unwrap_or_default().trim().trim_end_matches(':').trim().to_owned()).collect()
}

#[derive(Clone, Debug, Default)]
pub struct Titles {
    heads: Vec<String>,
}

impl Titles {
    pub fn load(vfs: &Vfs) -> Self {
        Self { heads: heads(vfs) }
    }

    pub fn title(&self, names: &NameBook, held: &Equipment) -> String {
        let content = held.content.unwrap_or(-1);
        let grade = held.grade_id.and_then(|grade| usize::try_from(grade).ok()).and_then(|grade| GRADES.get(grade)).copied().unwrap_or("?");
        let aimed = held.attribute.and_then(|row| usize::try_from(row).ok()).and_then(|row| names.orb_trait(row)).unwrap_or_default();
        let head = usize::try_from(content).ok().and_then(|at| self.heads.get(at)).filter(|head| !head.trim().is_empty());

        let Some(head) = head else {
            return format!("Orb {content} {grade}");
        };

        let mut fills = [grade, aimed].into_iter();
        let mut parts = head.split(PLACEHOLDER);
        let mut title = parts.next().unwrap_or_default().to_owned();

        for part in parts {
            title.push_str(fills.next().unwrap_or_default());
            title.push_str(part);
        }

        title.trim().to_owned()
    }
}

pub fn open_slots(count: usize, gates: Option<&Vec<i32>>, level: i32) -> Vec<bool> {
    (0..count).map(|slot| rules::unlocked(gates.and_then(|held| held.get(slot)).copied().unwrap_or(0), level)).collect()
}

pub fn worn(orbs: &[Equipment], masks: &[u32], held: impl IntoIterator<Item = (usize, u32)>) -> Vec<Worn> {
    held.into_iter()
        .filter_map(|(slot, id)| {
            let equipment = orbs.get(id as usize)?;

            Some(Worn { slot, id, content: equipment.content?, mask: mask_of(masks, equipment.attribute), grade: equipment.grade_id.unwrap_or(0), values: equipment.value.clone() })
        })
        .collect()
}

pub fn picked(held: &[Option<u32>], doors: &[bool]) -> Vec<(usize, u32)> {
    held.iter().zip(doors).enumerate().filter(|(_, (_, open))| **open).filter_map(|(slot, (orb, _))| orb.map(|orb| (slot, orb))).collect()
}

pub fn dress(stats: &mut Entity, base: &Entity, form: usize, lens: Lens, worn: &[Worn]) {
    if form >= FIRST_FORM && !worn.is_empty() {
        stats.wear(worn.to_vec(), [base.attack_1_damage, base.attack_2_damage, base.attack_3_damage], lens.switches, lens.mode);
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Shine {
    pub lit: bool,
    pub ringed: bool,
}

pub fn shine(stats: &Entity, lens: Lens, foe: Option<Foe>, worn: &Worn) -> Shine {
    let lit = stats.orb_works(worn) && (worn.switch().is_none() || worn.toggled(lens.switches, lens.mode));
    let foe = foe.filter(|foe| !foe.is_empty());
    let ringed = lit
        && match worn.content {
            rules::STRONG_UP | rules::MASSIVE_UP | rules::TOUGH_UP => foe.is_some_and(|foe| worn.strikes(foe) && stats.targets(foe)),
            rules::ATTACK_UP | rules::DEFENSE_UP => foe.is_some_and(|foe| worn.strikes(foe)),
            _ => false,
        };

    Shine { lit, ringed }
}

pub fn toggles(worn: &[Worn]) -> Vec<(Switch, &Worn)> {
    worn.iter().filter_map(|held| Some((held.switch()?, held))).collect()
}

#[derive(Default)]
pub struct Faces {
    layers: [Vec<SpriteSheet>; 3],
}

impl Faces {
    pub fn load(vfs: &Vfs) -> Self {
        Self { layers: LAYERS.map(|name| formats::sheets(vfs, name)) }
    }

    fn cut(&self, layer: usize, index: i32) -> Option<RgbaImage> {
        let index = usize::try_from(index).ok()?;

        self.layers.get(layer)?.iter().find_map(|sheet| sheet.crop(index))
    }

    pub fn face(&self, held: &Equipment) -> Option<RgbaImage> {
        let mut face = self.cut(0, held.attribute.unwrap_or(PLAIN_FACE))?;

        for (layer, index) in [(1, held.content), (2, held.grade_id)] {
            if let Some(over) = index.and_then(|index| self.cut(layer, index)) {
                let (across, down) = (face.width().saturating_sub(over.width()) / 2, face.height().saturating_sub(over.height()) / 2);

                imageops::overlay(&mut face, &over, i64::from(across), i64::from(down));
            }
        }

        Some(face)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex::combat::orb::{ATTACK_UP, MASSIVE_UP, STRONG_UP};

    // Massive vs Floating goes on Windy because her True form has it, even though her Ultra form does not.
    #[test]
    fn an_ability_orb_needs_the_ability_on_some_talent_form() {
        let floating = Entity { trait_floating: 1, massive_damage: 1, ..Entity::default() };
        let insane = Entity { trait_floating: 1, insane_damage: 1, ..Entity::default() };
        let masks = Arc::new(vec![0b1, 0b10]);
        let windy = Allowance::over([floating, insane], Arc::clone(&masks));

        assert!(windy.permits(Kind { content: MASSIVE_UP, attribute: Some(1) }));
        assert!(!windy.permits(Kind { content: MASSIVE_UP, attribute: Some(0) }));
        assert!(!windy.permits(Kind { content: STRONG_UP, attribute: Some(1) }));
        assert!(windy.permits(Kind { content: ATTACK_UP, attribute: Some(0) }));

        let early = Allowance::over([Entity::default()], masks);

        assert!(!early.permits(Kind { content: STRONG_UP, attribute: Some(0) }), "only True and Ultra forms count");
    }

    #[test]
    fn a_marked_second_slot_opens_at_sixty() {
        let gates = vec![0, 1];

        assert_eq!(open_slots(2, Some(&gates), 59), [true, false]);
        assert_eq!(open_slots(2, Some(&gates), 60), [true, true]);
        assert_eq!(open_slots(1, None, 1), [true]);
    }

    #[test]
    fn a_title_fills_in_its_grade() {
        let titles = Titles { heads: vec!["Attack Up %@: %@".to_owned()] };
        let held = Equipment { grade_id: Some(4), content: Some(0), value: vec![20], attribute: None };

        assert_eq!(titles.title(&NameBook::default(), &held), "Attack Up S:");
    }
}
