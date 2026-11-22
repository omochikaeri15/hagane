use std::cmp::Reverse;
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use codex::combat::{Faction, get_talent};
use codex::files::{Localizable, Param};
use sha2::{Digest, Sha256};
use tracing::{error, info, warn};

use foundry::Vault;
use foundry::common::architecture;
use foundry::common::context::GlobalContext;
use foundry::common::formats::{self, SpriteSheet};
use foundry::common::game;
use foundry::domains::cat::game::stats::seeded_level;
use foundry::domains::cat::level;
use foundry::domains::cat::scanner::{self as cat_scanner, CatEntry};
use foundry::domains::enemy::scanner::{self as enemy_scanner, EnemyEntry};
use foundry::domains::settings::{Settings, lang, source};
use foundry::systems::combat::NameBook;
use foundry::systems::treasure::Bonus;

use super::fuzzy::{self, Query};
use super::prefs::{ENGLISH, Prefs, Search};

const ICONS: &str = "img015";

pub(crate) struct Catalog {
    pub(crate) payload: String,
    books: Mutex<HashMap<String, Arc<Library>>>,
}

impl Catalog {
    pub(crate) fn new(payload: String) -> Self {
        Self { payload, books: Mutex::new(HashMap::new()) }
    }

    pub(crate) fn book(&self, language: &str) -> Arc<Library> {
        let language = if lang::choices().any(|(code, _)| code == language) { language } else { ENGLISH };
        let mut books = self.books.lock().unwrap_or_else(PoisonError::into_inner);

        Arc::clone(books.entry(language.to_owned()).or_insert_with(|| Arc::new(load(language))))
    }
}

pub(crate) struct Library {
    pub(crate) language: String,
    pub(crate) vault: Arc<Vault>,
    pub(crate) settings: Settings,
    pub(crate) cats: Vec<CatEntry>,
    pub(crate) enemies: Vec<EnemyEntry>,
    pub(crate) sheets: Vec<SpriteSheet>,
    pub(crate) icons: [u8; 32],
    param: Param,
    localizable: Localizable,
    names: NameBook,
    treasure: Bonus,
}

impl Library {
    pub(crate) fn context(&self) -> GlobalContext<'_> {
        GlobalContext { param: &self.param, localizable: &self.localizable, vault: &self.vault, treasure: &self.treasure, names: &self.names }
    }

    pub(crate) fn level_for(&self, cat: &CatEntry, form: usize) -> String {
        let (total, input) = seeded_level(cat, &self.settings);
        let needed = cat.unitbuy.form_level(form);

        if i64::from(needed) > i64::from(total) { level::capped(&needed.to_string(), cat.unitbuy.level_caps()) } else { input }
    }

    pub(crate) fn cat(&self, id: u32) -> Option<&CatEntry> {
        self.cats.iter().find(|cat| cat.id == id)
    }

    pub(crate) fn enemy(&self, id: u32) -> Option<&EnemyEntry> {
        self.enemies.iter().find(|enemy| enemy.id == id)
    }

    pub(crate) fn find_cat(&self, name: &str, prefs: &Prefs) -> Option<(&CatEntry, usize)> {
        let query = Query::new(name);

        if query.is_empty() {
            return None;
        }

        let scored: Vec<(&CatEntry, usize, u32)> = self
            .cats
            .iter()
            .flat_map(|cat| cat.names.iter().enumerate().filter(|(form, _)| cat.forms[*form]).filter_map(move |(form, held)| Some((cat, form, held.as_deref()?))))
            .filter_map(|(cat, form, held)| Some((cat, form, query.score(held)?)))
            .collect();
        let best = scored.iter().map(|(_, _, score)| *score).max()?;
        let (cat, _, _) = scored.iter().filter(|(_, _, score)| *score == best).min_by_key(|(cat, form, _)| (cat.id, *form))?;

        if prefs.search == Search::Preferred {
            return Some((cat, prefs.form_from(&cat.forms)));
        }

        let wanted = fuzzy::normalized(name);
        let exact: Vec<usize> = cat.names.iter().enumerate().filter(|(form, held)| cat.forms[*form] && held.as_deref().is_some_and(|held| fuzzy::normalized(held) == wanted)).map(|(form, _)| form).collect();

        if let Some(only) = exact.first().filter(|_| exact.len() == 1) {
            return Some((cat, *only));
        }
        let counts = |score: u32| fuzzy::strong(score) || !fuzzy::strong(best);
        let mut forms = [false; 4];

        for (held, form, score) in &scored {
            if held.id == cat.id && counts(*score) {
                forms[*form] = true;
            }
        }

        Some((cat, prefs.form_from(&forms)))
    }

    pub(crate) fn names_cat(&self, name: &str) -> bool {
        let wanted = fuzzy::normalized(name);

        self.cats.iter().any(|cat| cat.names.iter().flatten().any(|held| fuzzy::normalized(held) == wanted))
    }

    pub(crate) fn names_enemy(&self, name: &str) -> bool {
        let wanted = fuzzy::normalized(name);

        self.enemies.iter().any(|enemy| fuzzy::normalized(&enemy.name) == wanted)
    }

    pub(crate) fn find_enemy(&self, name: &str) -> Option<&EnemyEntry> {
        let query = Query::new(name);

        self.enemies.iter().filter_map(|enemy| Some((enemy, query.score(&enemy.name)?))).max_by_key(|(enemy, score)| (*score, Reverse(enemy.id))).map(|(enemy, _)| enemy)
    }

    pub(crate) fn talent_name(&self, ability: u8) -> String {
        self.names.talent(ability).map(str::to_owned).unwrap_or_else(|| get_talent(ability).map_or_else(|| format!("Talent {ability}"), |def| self.names.ability(def.identity, Faction::Cat).to_owned()))
    }
}

pub(crate) enum Remote {
    Current,
    Changed(String),
}

pub(crate) fn check(link: &str, held: Option<&str>) -> Result<Remote, String> {
    let remote = source::probe(link).map_err(|err| err.to_string())?;
    let present = architecture::game_present();

    if remote.size == 0 && present && held.is_some_and(|held| held == remote.id) {
        return Ok(Remote::Current);
    }

    Ok(Remote::Changed(remote.id))
}

pub(crate) fn download(link: &str) -> Result<String, String> {
    let started = Instant::now();
    let abort = AtomicBool::new(false);
    let stamp = source::install(link, None, |_| {}, &abort).map_err(|failure| failure.error.to_string())?;

    info!(payload = stamp.id, seconds = started.elapsed().as_secs(), "Game data is up to date");

    Ok(stamp.id)
}

fn load(language: &str) -> Library {
    let started = Instant::now();
    let mut settings = Settings::default();

    settings.general.language_priority = lang::led_by(language);

    let config = settings.scanner_config(None);
    let vault = Vault::with_priority(&config.language_priority);

    match vault.vfs.create(Path::new(architecture::GAME)) {
        Ok(conflicts) => {
            for conflict in &conflicts {
                warn!(key = %conflict.key, "Duplicate file name in the game data, every copy left out: {:?}", conflict.paths);
            }
        }
        Err(err) => error!("The game data could not be indexed: {}", err),
    }

    let vault = Arc::new(vault);
    let cats = cat_scanner::load(config.clone(), Arc::clone(&vault), |_, _| {}).data;
    let enemies = enemy_scanner::load(config, Arc::clone(&vault), |_, _| {}).data;
    let param = game::param(&vault.vfs).unwrap_or_default();
    let localizable = game::localizable(&vault.vfs);
    let names = if vault.vds.names.english(&vault.vfs) { NameBook::default() } else { vault.vds.names.book(&vault.vfs, &localizable) };
    let catalog = vault.vds.treasures.catalog(&vault.vfs);
    let treasure = Bonus::resolve(&catalog, &settings.general.treasures);
    let sheets = formats::sheets(&vault.vfs, ICONS);
    let icons = drawn_from(&sheets);

    info!(language, cats = cats.len(), enemies = enemies.len(), ms = started.elapsed().as_millis(), "Game data loaded");

    Library { language: language.to_owned(), vault, settings, cats, enemies, sheets, icons, param, localizable, names, treasure }
}

fn drawn_from(sheets: &[SpriteSheet]) -> [u8; 32] {
    let mut hasher = Sha256::new();

    for sheet in sheets {
        let mut cuts: Vec<_> = sheet.cuts_map.iter().collect();

        cuts.sort_by_key(|(id, _)| **id);
        hasher.update(sheet.sheet_name.as_bytes());
        hasher.update(format!("{cuts:?}").as_bytes());

        if let Some(image) = sheet.image_data.as_deref() {
            hasher.update(image.width().to_le_bytes());
            hasher.update(image.height().to_le_bytes());
            hasher.update(image.as_raw());
        }
    }

    hasher.finalize().into()
}
