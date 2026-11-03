use std::sync::Arc;

use image::imageops;
use slint::Image;

use codex::cat::unit::Equipment;
use codex::combat::Identity;
use foundry::Vault;
use foundry::domains::sandbox::orb::{self, Allowance, GRADES, Kind};

use crate::context::Globals;
use crate::images::Images;
use crate::OrbSlot;

const EXPLANATION: &str = "equipment_explonation.tsv";
const PLACEHOLDER: &str = "%@";
const UNIVERSAL: i32 = 12;
pub const BEST_GRADE: usize = 4;
const ATTRIBUTE_SHEET: &str = "equipment_attribute";
const EFFECT_SHEET: &str = "equipment_effect";
const GRADE_SHEET: &str = "equipment_grade";
const TRAITS: [Identity; 12] = [
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

pub struct Book {
    pub orbs: Arc<Vec<Equipment>>,
    kinds: Vec<Kind>,
    effects: Vec<String>,
    traits: Vec<String>,
}

fn effect_names(vault: &Vault) -> Vec<String> {
    let vfs = &vault.vfs;
    let named = vfs.variants(EXPLANATION).into_iter().next().unwrap_or_else(|| EXPLANATION.to_owned());

    vfs.find(named.as_str()).and_then(|path| vfs.read(&path).ok()).and_then(|bytes| String::from_utf8(bytes.to_vec()).ok()).map_or_else(Vec::new, |content| {
        content
            .lines()
            .map(|line| {
                let head = line.split('\t').next().unwrap_or_default();

                head.split(PLACEHOLDER).next().unwrap_or_default().trim().trim_end_matches(':').trim().to_owned()
            })
            .collect()
    })
}

impl Book {
    pub fn load(vault: &Vault, globals: &Globals) -> Self {
        let orbs = vault.vds.cats.orbs(&vault.vfs);
        let kinds = orb::kinds(&orbs);
        let effects = effect_names(vault);
        let traits = TRAITS.iter().map(|identity| globals.names.trait_label(*identity).to_owned()).collect();

        Self { orbs, kinds, effects, traits }
    }

    pub fn effect_label(&self, content: i32) -> String {
        usize::try_from(content).ok().and_then(|index| self.effects.get(index)).filter(|name| !name.is_empty()).map_or_else(|| format!("Effect {content}"), Clone::clone)
    }

    pub fn trait_label(&self, attribute: i32) -> String {
        usize::try_from(attribute).ok().and_then(|index| self.traits.get(index)).map_or_else(|| format!("Trait {attribute}"), Clone::clone)
    }

    pub fn aims(&self, content: i32, allowance: Allowance) -> Vec<i32> {
        orb::attributes(&self.kinds, content).into_iter().filter(|attribute| allowance.permits(Kind { content, attribute: Some(*attribute) })).collect()
    }

    pub fn offered(&self, allowance: Allowance) -> Vec<i32> {
        orb::effects(&self.kinds).into_iter().filter(|content| orb::attributes(&self.kinds, *content).is_empty() || !self.aims(*content, allowance).is_empty()).collect()
    }

    pub fn grades(&self, kind: Kind) -> Vec<usize> {
        let mut offered: Vec<usize> = orb::grades(&self.orbs, kind).into_iter().map(|(grade, _)| grade).collect();

        offered.sort_unstable();
        offered.dedup();
        offered
    }

    pub fn grade_label(grade: usize) -> &'static str {
        GRADES.get(grade).copied().unwrap_or("?")
    }

    pub fn picture(&self, images: &Images, vault: &Vault, orb: Option<u32>) -> OrbSlot {
        let icon = orb.and_then(|orb| self.face(images, vault, orb));

        OrbSlot { has_icon: icon.is_some(), icon: icon.unwrap_or_default() }
    }

    pub fn face(&self, images: &Images, vault: &Vault, orb: u32) -> Option<Image> {
        let held = self.orbs.get(orb as usize)?;
        let index = |value: Option<i32>| value.and_then(|value| usize::try_from(value).ok());

        compose(images, vault, index(Some(held.attribute.unwrap_or(UNIVERSAL)))?, index(held.content), index(held.grade_id))
    }
}

fn compose(images: &Images, vault: &Vault, attribute: usize, content: Option<usize>, grade: Option<usize>) -> Option<Image> {
    let key = format!("orb:{attribute}:{}:{}", content.map_or(-1, |value| value as i64), grade.map_or(-1, |value| value as i64));
    let layers = [(ATTRIBUTE_SHEET, Some(attribute)), (EFFECT_SHEET, content), (GRADE_SHEET, grade)];
    let sheets: Vec<_> = layers.iter().map(|(sheet, index)| (images.sheets(vault, sheet), *index)).collect();

    images.local(&key, move || {
        let mut face = sheets[0].1.and_then(|index| sheets[0].0.iter().find_map(|layer| layer.crop(index)))?;

        for (sheet, index) in &sheets[1..] {
            if let Some(over) = index.and_then(|index| sheet.iter().find_map(|layer| layer.crop(index))) {
                let (across, down) = (face.width().saturating_sub(over.width()) / 2, face.height().saturating_sub(over.height()) / 2);

                imageops::overlay(&mut face, &over, i64::from(across), i64::from(down));
            }
        }

        Some(face)
    })
}
