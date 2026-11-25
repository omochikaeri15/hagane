use std::cmp::Reverse;
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use codex::chapter::Category;
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
use foundry::domains::stage::scanner::{self as stage_scanner, StageBundle};
use foundry::domains::stage::{GlobalMapId, GlobalStageId, Map, Stage, StageRegistry, navigate};
use foundry::domains::settings::{Settings, lang, source};
use foundry::systems::combat::NameBook;
use foundry::systems::treasure::Bonus;

use super::fuzzy::{self, Query};
use super::prefs::{ENGLISH, Prefs, Search};

const ICONS: &str = "img015";
const COINS: &str = "img022";
const GRAFT_MARK: char = 'i';

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
    pub(crate) foes: HashMap<u32, EnemyEntry>,
    pub(crate) stages: StageBundle,
    hosts: HashMap<GlobalStageId, GlobalStageId>,
    filibusters: HashMap<GlobalMapId, GlobalStageId>,
    pub(crate) sheets: Vec<SpriteSheet>,
    pub(crate) icons: [u8; 32],
    pub(crate) coins: Vec<SpriteSheet>,
    pub(crate) coin_mark: [u8; 32],
    param: Param,
    localizable: Localizable,
    names: NameBook,
    treasure: Bonus,
}

impl Library {
    pub(crate) fn context(&self) -> GlobalContext<'_> {
        GlobalContext { param: &self.param, localizable: &self.localizable, vault: &self.vault, treasure: &self.treasure, names: &self.names }
    }

    pub(crate) fn level_for(&self, cat: &CatEntry, form: usize, talents: Option<&HashMap<u8, u8>>) -> String {
        let (total, input) = seeded_level(cat, &self.settings);
        let groups = cat.talent_data.iter().flat_map(|data| data.groups.iter().enumerate());
        let learned = groups.filter(|(index, _)| talents.is_some_and(|levels| u8::try_from(*index).ok().and_then(|index| levels.get(&index)).is_some_and(|level| *level > 0)));
        let needed = learned.map(|(_, group)| group.unlock_level()).fold(cat.unitbuy.form_level(form), u32::max);

        if i64::from(needed) > i64::from(total) { level::capped(&needed.to_string(), cat.unitbuy.level_caps()) } else { input }
    }

    pub(crate) fn cat(&self, id: u32) -> Option<&CatEntry> {
        self.cats.iter().find(|cat| cat.id == id)
    }

    pub(crate) fn enemy(&self, id: u32) -> Option<&EnemyEntry> {
        self.enemies.iter().find(|enemy| enemy.id == id)
    }

    fn scored_cats(&self, query: &Query, closeness: u32) -> Vec<(&CatEntry, usize, u32)> {
        self.cats
            .iter()
            .flat_map(|cat| cat.names.iter().enumerate().filter(|(form, _)| cat.forms[*form]).filter_map(move |(form, held)| Some((cat, form, held.as_deref()?))))
            .filter_map(|(cat, form, held)| Some((cat, form, query.score_within(held, closeness)?)))
            .collect()
    }

    pub(crate) fn search_cats(&self, name: &str, closeness: u32) -> Vec<(&CatEntry, usize, u32)> {
        let mut scored = self.scored_cats(&Query::new(name), closeness);

        scored.sort_by_key(|(cat, form, score)| (Reverse(*score), cat.id, *form));
        scored
    }

    pub(crate) fn search_enemies(&self, name: &str, closeness: u32) -> Vec<(&EnemyEntry, u32)> {
        let query = Query::new(name);
        let mut scored: Vec<(&EnemyEntry, u32)> = self.enemies.iter().filter_map(|enemy| Some((enemy, query.score_within(&enemy.name, closeness)?))).collect();

        scored.sort_by_key(|(enemy, score)| (Reverse(*score), enemy.id));
        scored
    }

    pub(crate) fn find_cat(&self, name: &str, prefs: &Prefs) -> Option<(&CatEntry, usize)> {
        let query = Query::new(name);

        if query.is_empty() {
            return None;
        }

        let scored = self.scored_cats(&query, fuzzy::CLOSENESS);
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

    fn staged<'a>(&'a self, id: &'a GlobalStageId, stage: &'a Stage) -> Staged<'a> {
        let host = self.hosts.get(id).and_then(|host| self.stages.registry.stages.get_key_value(host));

        Staged { id, stage, host, grafted: false }
    }

    fn grafts<'a>(&'a self, id: &'a GlobalStageId, stage: &'a Stage) -> Option<Staged<'a>> {
        let (graft, held) = self.stages.registry.stages.get_key_value(self.filibusters.get(&GlobalMapId { category: id.category.clone(), map: id.map })?)?;

        Some(Staged { id: graft, stage: held, host: Some((id, stage)), grafted: true })
    }

    pub(crate) fn search_stages(&self, name: &str, closeness: u32) -> Vec<(Staged<'_>, u32)> {
        let query = Query::new(name);
        let mut scored: Vec<(Staged<'_>, u32, String)> = Vec::new();

        for (id, stage) in self.stages.registry.stages.iter().filter(|(id, _)| !navigate::is_invasion(&self.stages.registry, id)) {
            let Some(score) = query.score_within(&stage.name, closeness) else { continue };

            for found in std::iter::once(self.staged(id, stage)).chain(self.grafts(id, stage)) {
                let code = found.code();

                scored.push((found, score, code));
            }
        }

        scored.sort_by(|one, other| other.1.cmp(&one.1).then_with(|| one.2.cmp(&other.2)));
        scored.into_iter().map(|(found, score, _)| (found, score)).collect()
    }

    pub(crate) fn stage(&self, code: &str) -> Option<Staged<'_>> {
        let grafted = code.strip_suffix(['i', 'I']);
        let (id, stage) = self.stages.registry.stages.iter().find(|(id, _)| stage_code(id).eq_ignore_ascii_case(grafted.unwrap_or(code)))?;

        match grafted {
            Some(_) => self.grafts(id, stage),
            None => (!navigate::is_invasion(&self.stages.registry, id)).then(|| self.staged(id, stage)),
        }
    }

    pub(crate) fn stage_map(&self, stage: &Stage) -> Option<&Map> {
        self.stages.registry.maps.get(&GlobalMapId { category: stage.category.clone(), map: stage.map_id })
    }

    fn area<'a>(&'a self, id: &'a GlobalMapId, map: &'a Map) -> Area<'a> {
        let registry = &self.stages.registry;
        let host = registry.stages.keys().filter(|stage| stage.category == id.category && stage.map == id.map).find_map(|stage| self.hosts.get(stage)).and_then(|host| registry.maps.get_key_value(&GlobalMapId { category: host.category.clone(), map: host.map }));

        Area { id, map, host, grafted: false }
    }

    pub(crate) fn areas(&self) -> impl Iterator<Item = Area<'_>> {
        let registry = &self.stages.registry;
        let grafted = self.filibusters.keys().filter_map(|id| registry.maps.get_key_value(id)).map(|(id, map)| Area { id, map, host: None, grafted: true });

        registry.maps.iter().map(|(id, map)| self.area(id, map)).chain(grafted)
    }

    pub(crate) fn search_maps(&self, name: &str, closeness: u32) -> Vec<(Area<'_>, u32)> {
        let query = Query::new(name);
        let mut scored: Vec<(Area<'_>, u32, String)> = self
            .areas()
            .filter_map(|area| {
                let score = [Some(area.name()), area.host.map(|_| area.map.name.as_str())].into_iter().flatten().filter_map(|held| query.score_within(held, closeness)).max()?;
                let code = area.code();

                Some((area, score, code))
            })
            .collect();

        scored.sort_by(|one, other| other.1.cmp(&one.1).then_with(|| one.2.cmp(&other.2)));
        scored.into_iter().map(|(area, score, _)| (area, score)).collect()
    }

    pub(crate) fn map(&self, code: &str) -> Option<Area<'_>> {
        let grafted = code.strip_suffix(['i', 'I']);
        let (id, map) = self.stages.registry.maps.iter().find(|(id, _)| map_code(id).eq_ignore_ascii_case(grafted.unwrap_or(code)))?;

        match grafted {
            Some(_) => self.filibusters.contains_key(id).then_some(Area { id, map, host: None, grafted: true }),
            None => Some(self.area(id, map)),
        }
    }

    pub(crate) fn map_stages(&self, area: &Area<'_>) -> Vec<Staged<'_>> {
        let registry = &self.stages.registry;
        let mut held: Vec<(&GlobalStageId, &Stage)> = registry.stages.iter().filter(|(id, _)| id.category == area.id.category && id.map == area.id.map && !navigate::is_invasion(registry, id)).collect();

        held.sort_by_key(|(id, _)| id.stage);

        match area.grafted {
            true => held.into_iter().filter_map(|(id, stage)| self.grafts(id, stage)).collect(),
            false => held.into_iter().map(|(id, stage)| self.staged(id, stage)).collect(),
        }
    }

    pub(crate) fn names_map(&self, name: &str) -> bool {
        let wanted = fuzzy::normalized(name);

        self.areas().any(|area| fuzzy::normalized(area.name()) == wanted)
    }

    pub(crate) fn names_stage(&self, name: &str) -> bool {
        let wanted = fuzzy::normalized(name);

        self.stages.registry.stages.values().any(|stage| fuzzy::normalized(&stage.name) == wanted)
    }

    pub(crate) fn talent_name(&self, ability: u8) -> String {
        self.names.talent(ability).map(str::to_owned).unwrap_or_else(|| get_talent(ability).map_or_else(|| format!("Talent {ability}"), |def| self.names.ability(def.identity, Faction::Cat).to_owned()))
    }
}

fn variety(category: &Category, invasion: bool) -> Option<&'static str> {
    match (category, invasion) {
        (Category::ZombieOutbreaks, true) => Some("Outbreak Invasion"),
        (Category::ZombieOutbreaks, false) => Some("Outbreak"),
        (Category::CataminStages, true) => Some("Catamin Invasion"),
        (Category::CataminStages, false) => Some("Catamin"),
        (_, true) => Some("Invasion"),
        (_, false) => None,
    }
}

pub(crate) struct Area<'a> {
    pub(crate) id: &'a GlobalMapId,
    pub(crate) map: &'a Map,
    pub(crate) host: Option<(&'a GlobalMapId, &'a Map)>,
    pub(crate) grafted: bool,
}

impl Area<'_> {
    pub(crate) fn code(&self) -> String {
        if self.grafted { format!("{}{GRAFT_MARK}", map_code(self.id)) } else { map_code(self.id) }
    }

    pub(crate) fn name(&self) -> &str {
        self.host.map_or(self.map.name.as_str(), |(_, host)| host.name.as_str())
    }

    pub(crate) fn category(&self) -> &Category {
        self.host.map_or(&self.id.category, |(host, _)| &host.category)
    }

    pub(crate) fn invasion(&self) -> bool {
        self.grafted || self.host.is_some()
    }

    pub(crate) fn kind(&self) -> Option<&'static str> {
        variety(&self.id.category, self.invasion())
    }
}

pub(crate) struct Staged<'a> {
    pub(crate) id: &'a GlobalStageId,
    pub(crate) stage: &'a Stage,
    pub(crate) host: Option<(&'a GlobalStageId, &'a Stage)>,
    pub(crate) grafted: bool,
}

impl Staged<'_> {
    pub(crate) fn code(&self) -> String {
        match (self.grafted, self.host) {
            (true, Some((host, _))) => format!("{}{GRAFT_MARK}", stage_code(host)),
            _ => stage_code(self.id),
        }
    }

    pub(crate) fn name(&self) -> &str {
        match (self.grafted, self.host) {
            (true, Some((_, host))) => &host.name,
            _ => &self.stage.name,
        }
    }

    pub(crate) fn kind(&self) -> Option<&'static str> {
        variety(&self.id.category, self.host.is_some())
    }
}

fn invasions(registry: &StageRegistry) -> (HashMap<GlobalStageId, GlobalStageId>, HashMap<GlobalMapId, GlobalStageId>) {
    let mut hosts = HashMap::new();
    let mut filibusters = HashMap::new();

    for id in registry.stages.keys() {
        if navigate::is_invasion(registry, id) {
            filibusters.insert(GlobalMapId { category: id.category.clone(), map: id.map }, id.clone());
            continue;
        }

        let Some(target) = navigate::invaded_stage(registry, id) else { continue };
        let host = registry.addresses.iter().find(|(_, address)| address.global == Some(target.map_id)).map(|(map, _)| GlobalStageId { category: map.category.clone(), map: map.map, stage: target.stage });

        if let Some(host) = host.filter(|host| registry.stages.contains_key(host)) {
            hosts.insert(id.clone(), host);
        }
    }

    (hosts, filibusters)
}

pub(crate) fn map_code(id: &GlobalMapId) -> String {
    format!("{}{:03}", id.category.map_prefix(), id.map)
}

pub(crate) fn stage_code(id: &GlobalStageId) -> String {
    format!("{}{:03}-{:02}", id.category.map_prefix(), id.map, id.stage)
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
    let enemies = enemy_scanner::load(config.clone(), Arc::clone(&vault), |_, _| {}).data;
    let stages = stage_scanner::load(config, Arc::clone(&vault), |_, _| {}).data;
    let foes = enemies.iter().map(|enemy| (enemy.id, enemy.clone())).collect();
    let (hosts, filibusters) = invasions(&stages.registry);
    let param = game::param(&vault.vfs).unwrap_or_default();
    let localizable = game::localizable(&vault.vfs);
    let names = if vault.vds.names.english(&vault.vfs) { NameBook::default() } else { vault.vds.names.book(&vault.vfs, &localizable) };
    let catalog = vault.vds.treasures.catalog(&vault.vfs);
    let treasure = Bonus::resolve(&catalog, &settings.general.treasures);
    let sheets = formats::sheets(&vault.vfs, ICONS);
    let icons = drawn_from(&sheets);
    let coins = formats::sheets(&vault.vfs, COINS);
    let coin_mark = drawn_from(&coins);

    info!(language, cats = cats.len(), enemies = enemies.len(), stages = stages.registry.stages.len(), ms = started.elapsed().as_millis(), "Game data loaded");

    Library { language: language.to_owned(), vault, settings, cats, enemies, foes, stages, hosts, filibusters, sheets, icons, coins, coin_mark, param, localizable, names, treasure }
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
