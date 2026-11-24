use std::io::Cursor;

use image::{ImageFormat, RgbaImage};
use tracing::warn;

use foundry::Source;
use foundry::domains::stage::sheet::{self, Subject};
use foundry::domains::stage::Stage;
use foundry::systems::statblock::{StageFonts, StagePlate, StageSheet, build_stage_pages, stage_file_name};

use super::cache::Cache;
use super::library::{Library, Staged};
use super::prose::Trouble;

const STAMP: &str = env!("HAGANE_STAGE_STAMP");
const EXTENSION: &str = "png";
const COUNT: &str = "pages";
const TOO_BIG: &str = "big";
const PAGE_HEIGHT: u32 = 4000;
pub(crate) const MAX_PAGES: usize = 10;

pub(crate) type Page = (Vec<u8>, String);

fn pictures(sheet: &StageSheet) -> Vec<&Source> {
    let drops = sheet.tables.iter().flat_map(|table| table.rows.iter().filter_map(|row| row.icon.as_ref()));
    let materials = sheet.materials.iter().flat_map(|held| held.chunks.iter().flatten().filter_map(|material| material.icon.as_ref()));
    let spawns = sheet.ground.spawns.iter().filter_map(|spawn| spawn.icon.as_ref());
    let leads = sheet.continuations.iter().filter_map(|lead| lead.plate.image.as_ref());

    sheet.map.image.iter().chain(sheet.stage.image.iter()).chain(materials).chain(drops).chain(leads).chain(spawns).collect()
}

fn encoded(image: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();

    image.write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png).map_err(|err| format!("the stage could not be encoded: {err}"))?;

    Ok(bytes)
}

fn named(sheet: &StageSheet, pages: usize, page: usize) -> String {
    stage_file_name(sheet, (pages > 1).then_some(page))
}

pub(crate) fn open(stage: &Stage, crown: u8) -> bool {
    stage.target_crowns == -1 || stage.target_crowns as u8 == crown
}

pub(crate) fn crown(stage: &Stage, asked: Option<u8>) -> Result<u8, Trouble> {
    let count = stage.max_crowns.max(1);

    match asked {
        None => Ok((0..count).find(|crown| open(stage, *crown)).unwrap_or(0)),
        Some(asked) if asked >= count => Err(Trouble::NoCrown(stage.name.clone(), count)),
        Some(asked) if count > 1 && !open(stage, asked) => Err(Trouble::LockedCrown(stage.name.clone(), asked + 1)),
        Some(asked) => Ok(asked),
    }
}

pub(crate) fn sheet(library: &Library, found: &Staged<'_>, crown: u8) -> Result<StageSheet, Trouble> {
    let grafted = found.host.filter(|_| found.grafted).map(|(_, host)| Stage { xp: host.xp, ..found.stage.clone() });
    let stage = grafted.as_ref().unwrap_or(found.stage);
    let map = library.stage_map(stage).ok_or(Trouble::NoStage)?;
    let dictionaries = &library.stages.dictionaries;
    let mut drawn = sheet::build(Subject {
        stage,
        map,
        stages: &library.stages.registry.stages,
        crown,
        enemies: &library.foes,
        drop_charas: &dictionaries.drop_chara_registry,
        unit_buys: &dictionaries.unit_buy_registry,
        lock_skips: &dictionaries.lock_skip_registry,
        cpu: &dictionaries.scat_cpu_setting,
        ctx: library.context(),
    });

    drawn.id = found.code();

    if let Some((_, host)) = found.host
        && let Some(seat) = library.stage_map(host)
    {
        let (map_plate, stage_plate) = sheet::plates(host, seat, &library.vault.vfs);

        drawn.map = map_plate;

        if found.grafted || drawn.stage.image.is_none() {
            drawn.stage = StagePlate { name: if found.grafted { stage_plate.name } else { drawn.stage.name }, image: stage_plate.image };
        }
    }

    Ok(drawn)
}

pub(crate) fn pages(library: &Library, cache: &Cache, code: &str, crown: u8) -> Result<Vec<Page>, Trouble> {
    let drawn = sheet(library, &library.stage(code).ok_or(Trouble::NoStage)?, crown)?;
    let mut recipe = cache.recipe(STAMP);
    let mut seen: Vec<&std::path::Path> = Vec::new();

    recipe.add(library.language.as_bytes());
    recipe.add(format!("{drawn:?}").as_bytes());
    recipe.add(&PAGE_HEIGHT.to_le_bytes());

    for picture in pictures(&drawn) {
        if seen.contains(&picture.path.as_path()) {
            continue;
        }

        seen.push(&picture.path);
        recipe.add(picture.read().as_deref().unwrap_or_default());
    }

    let key = recipe.finish();

    if cache.get(&key, TOO_BIG).is_some() {
        return Err(Trouble::StageTooBig);
    }

    let held = cache.get(&key, COUNT).and_then(|count| String::from_utf8(count).ok()).and_then(|count| count.parse::<usize>().ok());

    if let Some(count) = held
        && let Some(found) = (0..count).map(|page| cache.get(&key, &format!("{page}.{EXTENSION}"))).collect::<Option<Vec<_>>>()
    {
        return Ok(found.into_iter().enumerate().map(|(page, bytes)| (bytes, named(&drawn, count, page))).collect());
    }

    let images = build_stage_pages(&drawn, &StageFonts::embedded(), PAGE_HEIGHT)?;

    if images.len() > MAX_PAGES {
        if let Err(err) = cache.keep(&key, TOO_BIG, TOO_BIG.as_bytes()) {
            warn!("A stage refusal could not be cached: {}", err);
        }

        return Err(Trouble::StageTooBig);
    }

    let count = images.len();
    let made = images.iter().map(encoded).collect::<Result<Vec<_>, _>>()?;

    for (page, bytes) in made.iter().enumerate() {
        if let Err(err) = cache.keep(&key, &format!("{page}.{EXTENSION}"), bytes) {
            warn!("A stage picture could not be cached: {}", err);
        }
    }

    if let Err(err) = cache.keep(&key, COUNT, count.to_string().as_bytes()) {
        warn!("A stage's page count could not be cached: {}", err);
    }

    Ok(made.into_iter().enumerate().map(|(page, bytes)| (bytes, named(&drawn, count, page))).collect())
}
