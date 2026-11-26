use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;

use image::{ImageFormat, RgbaImage};
use tracing::warn;

use codex::combat::Entity;
use foundry::Source;

use foundry::domains::cat::scanner::CatEntry;
use foundry::domains::cat::statblock::{self as cat_statblock, Subject as CatSubject};
use foundry::domains::enemy::scanner::EnemyEntry;
use foundry::domains::enemy::statblock::{self as enemy_statblock, Subject as EnemySubject};
use foundry::systems::combat::lens::Lens;
use foundry::systems::combat::registry::Magnification;
use foundry::systems::statblock::{DetailsData, Fonts, PageFonts, StatblockData, build_details_image, build_statblock_image, build_talents_image};

use super::cache::{Cache, Key};
use super::grammar::Versus;
use super::library::Library;
use super::prose::Trouble;

const STAMP: &str = env!("HAGANE_STATBLOCK_STAMP");
const EXTENSION: &str = "png";

#[derive(Clone, Copy)]
pub(crate) struct Shown<'a> {
    pub(crate) cat: &'a CatEntry,
    pub(crate) form: usize,
    pub(crate) level: i32,
    pub(crate) level_input: &'a str,
    pub(crate) talents: Option<&'a HashMap<u8, u8>>,
}

impl<'a> Shown<'a> {
    fn subject(self, library: &'a Library) -> CatSubject<'a> {
        CatSubject {
            cat: self.cat,
            form: self.form,
            current_level: self.level,
            level_input: self.level_input,
            talent_levels: self.talents,
            is_conjure_expanded: true,
            global: library.context(),
            grants: None,
            lens: Lens::default(),
        }
    }
}

pub(crate) fn cat_stats(library: &Library, shown: Shown<'_>) -> Option<Entity> {
    cat_statblock::final_stats(&shown.subject(library))
}

pub(crate) fn cat(library: &Library, cache: &Cache, shown: Shown<'_>, versus: Versus, lens: Lens) -> Result<Vec<u8>, Trouble> {
    let data = cat_statblock::build_against(CatSubject { lens, ..shown.subject(library) }, |stats| versus.against(stats)).ok_or_else(|| Trouble::NoStats(shown.cat.display_name(shown.form)))?;

    drawn(library, cache, data)
}

pub(crate) fn enemy(library: &Library, cache: &Cache, enemy: &EnemyEntry, magnification: Magnification, lens: Lens) -> Result<Vec<u8>, Trouble> {
    let subject = EnemySubject { enemy, magnification, show_invalid: library.settings.show_invalid_enemies(), global: library.context(), vault: &library.vault, lens };

    drawn(library, cache, enemy_statblock::build(subject))
}

pub(crate) fn talents(library: &Library, cache: &Cache, shown: Shown<'_>) -> Result<Vec<u8>, Trouble> {
    let data = cat_statblock::talents(shown.subject(library)).ok_or_else(|| Trouble::NoTalents(shown.cat.display_name(shown.form)))?;
    let pictures = data.head.icon.iter().chain(data.cards.iter().filter_map(|card| card.plate.as_ref()));
    let key = keyed(library, cache, &format!("{data:?}"), pictures, &[&library.icons, &library.coin_mark]);

    stored(cache, &key, || {
        build_talents_image(&library.settings.general.language_priority, &library.sheets, &library.coins, &data, &PageFonts::embedded())
    })
}

fn details(library: &Library, cache: &Cache, data: &DetailsData) -> Result<Vec<u8>, Trouble> {
    let evolved = data.evolve.iter().flat_map(|held| held.items.iter().filter_map(|item| item.icon.as_ref()).chain(held.cost.iter().filter_map(|cost| cost.icon.as_ref())));
    let seated = data.combos.iter().flat_map(|combo| combo.seats.iter().filter_map(|seat| seat.icon.as_ref()));
    let key = keyed(library, cache, &format!("{data:?}"), data.head.icon.iter().chain(evolved).chain(seated), &[]);

    stored(cache, &key, || build_details_image(&library.settings.general.language_priority, data, &PageFonts::embedded()))
}

pub(crate) fn cat_details(library: &Library, cache: &Cache, cat: &CatEntry, form: usize) -> Result<Vec<u8>, Trouble> {
    details(library, cache, &cat_statblock::details(cat, form, library.context()))
}

pub(crate) fn enemy_details(library: &Library, cache: &Cache, enemy: &EnemyEntry) -> Result<Vec<u8>, Trouble> {
    details(library, cache, &enemy_statblock::details(enemy, library.context()))
}

fn keyed<'a>(library: &Library, cache: &Cache, shown: &str, pictures: impl Iterator<Item = &'a Source>, marks: &[&[u8; 32]]) -> Key {
    let mut recipe = cache.recipe(STAMP);
    let mut seen: Vec<&Path> = Vec::new();

    recipe.add(library.language.as_bytes());
    recipe.add(shown.as_bytes());

    for mark in marks {
        recipe.add(*mark);
    }

    for picture in pictures {
        if seen.contains(&picture.path.as_path()) {
            continue;
        }

        seen.push(&picture.path);
        recipe.add(picture.read().as_deref().unwrap_or_default());
    }

    recipe.finish()
}

fn stored(cache: &Cache, key: &Key, draw: impl FnOnce() -> Result<RgbaImage, String>) -> Result<Vec<u8>, Trouble> {
    if let Some(bytes) = cache.get(key, EXTENSION) {
        return Ok(bytes);
    }

    let bytes = encoded(&draw()?)?;

    if let Err(err) = cache.keep(key, EXTENSION, &bytes) {
        warn!("A statblock could not be cached: {}", err);
    }

    Ok(bytes)
}

fn drawn(library: &Library, cache: &Cache, data: StatblockData) -> Result<Vec<u8>, Trouble> {
    let icon = data.icon.as_ref().and_then(|icon| icon.read().ok());
    let shown = StatblockData { icon: None, ..data.clone() };
    let mut recipe = cache.recipe(STAMP);

    recipe.add(library.language.as_bytes());
    recipe.add(&library.icons);
    recipe.add(format!("{shown:?}").as_bytes());
    recipe.add(icon.as_deref().unwrap_or_default());

    stored(cache, &recipe.finish(), || build_statblock_image(&library.settings.general.language_priority, &library.sheets, data, &Fonts::embedded()))
}

fn encoded(image: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();

    image.write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png).map_err(|err| format!("the statblock could not be encoded: {err}"))?;

    Ok(bytes)
}
