use std::collections::HashMap;
use std::io::Cursor;

use image::ImageFormat;
use tracing::warn;

use foundry::domains::cat::scanner::CatEntry;
use foundry::domains::cat::statblock::{self as cat_statblock, Subject as CatSubject};
use foundry::domains::enemy::scanner::EnemyEntry;
use foundry::domains::enemy::statblock::{self as enemy_statblock, Subject as EnemySubject};
use foundry::systems::combat::registry::Magnification;
use foundry::systems::statblock::{Fonts, StatblockData, build_statblock_image};

use super::cache::Cache;
use super::library::Library;
use super::prose::Trouble;

const STAMP: &str = env!("HAGANE_STATBLOCK_STAMP");
const EXTENSION: &str = "png";

pub(crate) fn cat(library: &Library, cache: &Cache, cat: &CatEntry, form: usize, level: i32, level_input: &str, talents: Option<&HashMap<u8, u8>>) -> Result<Vec<u8>, Trouble> {
    let subject = CatSubject {
        cat,
        form,
        current_level: level,
        level_input,
        talent_levels: talents,
        is_conjure_expanded: true,
        global: library.context(),
        grants: None,
    };
    let data = cat_statblock::build(subject).ok_or_else(|| Trouble::NoStats(cat.display_name(form)))?;

    drawn(library, cache, data)
}

pub(crate) fn enemy(library: &Library, cache: &Cache, enemy: &EnemyEntry, magnification: Magnification) -> Result<Vec<u8>, Trouble> {
    let subject = EnemySubject { enemy, magnification, show_invalid: library.settings.show_invalid_enemies(), global: library.context(), vault: &library.vault };

    drawn(library, cache, enemy_statblock::build(subject))
}

fn drawn(library: &Library, cache: &Cache, data: StatblockData) -> Result<Vec<u8>, Trouble> {
    let icon = data.icon.as_ref().and_then(|icon| icon.read().ok());
    let shown = StatblockData { icon: None, ..data.clone() };
    let mut recipe = cache.recipe(STAMP);

    recipe.add(library.language.as_bytes());
    recipe.add(&library.icons);
    recipe.add(format!("{shown:?}").as_bytes());
    recipe.add(icon.as_deref().unwrap_or_default());

    let key = recipe.finish();

    if let Some(bytes) = cache.get(&key, EXTENSION) {
        return Ok(bytes);
    }

    let bytes = png(library, data)?;

    if let Err(err) = cache.keep(&key, EXTENSION, &bytes) {
        warn!("A statblock could not be cached: {}", err);
    }

    Ok(bytes)
}

fn png(library: &Library, data: StatblockData) -> Result<Vec<u8>, String> {
    let image = build_statblock_image(&library.settings.general.language_priority, &library.sheets, data, &Fonts::embedded())?;
    let mut bytes = Vec::new();

    image.write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png).map_err(|err| format!("the statblock could not be encoded: {err}"))?;

    Ok(bytes)
}
