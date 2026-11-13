use std::ops::Range;
use std::path::Path;

use rustc_hash::FxHashMap;

use codex::cat::unit::NyancomboData;
use codex::chapter::Category;
use codex::combat::{Faction, Identity, REGISTRY};
use foundry::common::architecture;
use foundry::domains::cat::combo as cat_combo;
use foundry::domains::cat::files as cat_files;
use foundry::domains::cat::scanner::CatEntry;
use foundry::domains::enemy::files as enemy_files;
use foundry::domains::enemy::scanner::EnemyEntry;
use foundry::domains::settings::EditorMode;
use foundry::domains::stage::files as stage_files;
use foundry::domains::stage::{GlobalMapId, GlobalStageId};

use crate::app::{theme, HaganeApp};
use crate::domains::stage::category::CategoryExt;
use crate::editor::{
    drop_ids, figures, glossary_file, internal_name, item_label, prose, registry, visible_name, Art, Focus, Pane,
    Pick, NO_EGGS,
};
use crate::widget::{Guide, Tracer};

const CATS: &str = "Cats";
const ENEMIES: &str = "Enemies";
const STAGES: &str = "Stages";
const OTHER: &str = "Other";
const COMBOS: &str = "Combos";
const COMBO_SETUP: &str = "Setup";
const COMBO_NAME: &str = "Name";

const UNITS: &str = "Units";
const EGGS: &str = "Eggs";
const EGG_NAMES: [&str; 11] =
    ["Dirty", "Traitless", "Red", "Floating", "Dark", "Metal", "Angel", "Alien", "Zombie", "Aku", "Relic"];
const EGG_MARK: &str = "m";
const EGG_STAGES: usize = 2;
const NO_EGG: i32 = -1;

const ITEM_CATALOG: &str = "Item Catalog";
const ITEM_TEXT: &str = "Item Name & Description";

const ENEMY_MARK: &str = "e";
const ITEM_TAG: &str = "Item";
const LINE_TAG: &str = "Line";

const KEY_JOINT: char = '\u{1f}';

type Maps = Vec<(u32, Vec<u32>)>;
type Sets = FxHashMap<(Pane, &'static str), FxHashMap<Box<str>, Range<usize>>>;

enum Who {
    Cat { id: u32, form: usize },
    Egg,
    Combo { id: u32, form: usize, line: usize },
    Enemy(u32),
    Map(GlobalMapId),
    Stage(GlobalStageId),
    Item(u32),
    Ability(Identity, Faction),
}

struct Unit {
    who: Who,
    folder: u32,
    tag: Box<str>,
    named: Box<str>,
    tag_key: Box<str>,
    name_key: Box<str>,
}

struct Folder {
    raw: Box<str>,
    resolved: Box<str>,
    raw_key: Box<str>,
    resolved_key: Box<str>,
    parent: Option<u32>,
    folders: Vec<u32>,
    leaves: Vec<u32>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Face {
    File,
    Title,
    Unit,
}

#[derive(Clone, Copy)]
struct Row {
    pane: Pane,
    unit: u32,
    file: u32,
    title: u32,
    folder: u32,
    face: Face,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Entry {
    Folder(u32),
    Leaf(u32),
}

pub(super) struct Line {
    pub(super) entry: Entry,
    pub(super) depth: u16,
    pub(super) open: bool,
    pub(super) guide: Guide,
}

#[derive(PartialEq)]
pub(super) struct Lead {
    folder: String,
    pane: Pane,
    file: Box<str>,
    unit: Box<str>,
}

#[derive(Default)]
pub(super) struct Filter {
    hits: Option<Vec<bool>>,
    counts: Vec<u32>,
}

impl Filter {
    pub(super) fn searching(&self) -> bool {
        self.hits.is_some()
    }

    pub(super) fn keeps(&self, leaf: u32) -> bool {
        self.hits.as_ref().is_none_or(|hits| hits.get(leaf as usize).copied().unwrap_or_default())
    }

    fn holds(&self, folder: u32) -> bool {
        self.counts.get(folder as usize).is_some_and(|count| *count > 0)
    }
}

#[derive(Default)]
pub(crate) struct Catalog {
    units: Vec<Unit>,
    folders: Vec<Folder>,
    roots: Vec<u32>,
    files: Vec<Box<str>>,
    file_keys: Vec<Box<str>>,
    titles: Vec<Box<str>>,
    title_keys: Vec<Box<str>>,
    rows: Vec<Row>,
}

struct Hits {
    files: Vec<bool>,
    titles: Vec<bool>,
    units: Vec<bool>,
    folders: Vec<bool>,
}

impl Hits {
    fn reach(&self, row: &Row) -> bool {
        let held = |hits: &[bool], at: u32| hits.get(at as usize).copied().unwrap_or_default();

        held(&self.files, row.file)
            || held(&self.titles, row.title)
            || held(&self.units, row.unit)
            || held(&self.folders, row.folder)
    }
}

impl Catalog {
    pub(crate) fn build(app: &HaganeApp) -> Catalog {
        let mut builder = Builder {
            app,
            catalog: Catalog::default(),
            files_at: FxHashMap::default(),
            titles_at: FxHashMap::default(),
            sets: FxHashMap::default(),
            arena: Vec::new(),
        };

        let cats = builder.folder(None, CATS.to_owned(), CATS.to_owned());
        let units = builder.folder(Some(cats), UNITS.to_owned(), UNITS.to_owned());
        builder.cats(units);

        let eggs = builder.folder(Some(cats), EGGS.to_owned(), EGGS.to_owned());
        builder.eggs(eggs);
        builder.abilities(cats, Faction::Cat);

        let enemies = builder.folder(None, ENEMIES.to_owned(), ENEMIES.to_owned());
        let units = builder.folder(Some(enemies), UNITS.to_owned(), UNITS.to_owned());
        builder.enemies(units);
        builder.abilities(enemies, Faction::Enemy);

        let stages = builder.folder(None, STAGES.to_owned(), STAGES.to_owned());
        builder.stages(stages);

        let other = builder.folder(None, OTHER.to_owned(), OTHER.to_owned());
        builder.items(other);

        builder.catalog
    }

    pub(super) fn label(&self, entry: Entry, values: EditorMode) -> &str {
        match entry {
            Entry::Folder(id) => self.folder_label(id, values),
            Entry::Leaf(at) => self.leaf_label(at, values).unwrap_or_default(),
        }
    }

    fn folder_label(&self, id: u32, values: EditorMode) -> &str {
        self.folders.get(id as usize).map_or("", |folder| match values {
            EditorMode::Raw => &folder.raw,
            EditorMode::Resolved => &folder.resolved,
        })
    }

    fn leaf_label(&self, at: u32, values: EditorMode) -> Option<&str> {
        let row = self.rows.get(at as usize)?;
        let unit = self.units.get(row.unit as usize)?;

        match (row.face, values) {
            (Face::File, EditorMode::Raw) => self.files.get(row.file as usize).map(AsRef::as_ref),
            (Face::File, EditorMode::Resolved) | (Face::Title, _) => self.titles.get(row.title as usize).map(AsRef::as_ref),
            (Face::Unit, EditorMode::Raw) => Some(&unit.tag),
            (Face::Unit, EditorMode::Resolved) => Some(&unit.named),
        }
    }

    pub(super) fn filter(&self, query: &str) -> Filter {
        let lowered = query.to_lowercase();

        let tokens: Vec<&str> = lowered
            .split_whitespace()
            .map(|token| token.trim_matches(['(', ')']))
            .filter(|token| !token.is_empty())
            .collect();

        let hits: Option<Vec<bool>> = (!tokens.is_empty()).then(|| {
            let found: Vec<Hits> = tokens.iter().map(|token| self.hits(token)).collect();

            self.rows.iter().map(|row| found.iter().all(|hit| hit.reach(row))).collect()
        });

        let mut counts = vec![0u32; self.folders.len()];

        for (at, row) in self.rows.iter().enumerate() {
            let kept = hits.as_ref().is_none_or(|hits| hits.get(at).copied().unwrap_or_default());

            if let Some(count) = counts.get_mut(row.folder as usize)
                && kept
            {
                *count += 1;
            }
        }

        for at in (0..self.folders.len()).rev() {
            let Some(parent) = self.folders[at].parent else {
                continue;
            };

            let held = counts[at];

            if let Some(count) = counts.get_mut(parent as usize) {
                *count += held;
            }
        }

        Filter { hits, counts }
    }

    fn hits(&self, token: &str) -> Hits {
        let mut folders = vec![false; self.folders.len()];

        for (at, folder) in self.folders.iter().enumerate() {
            let inherited = folder.parent.is_some_and(|parent| folders.get(parent as usize).copied().unwrap_or_default());

            folders[at] = inherited || folder.raw_key.contains(token) || folder.resolved_key.contains(token);
        }

        Hits {
            files: self.file_keys.iter().map(|key| key.contains(token)).collect(),
            titles: self.title_keys.iter().map(|key| key.contains(token)).collect(),
            units: self.units.iter().map(|unit| unit.tag_key.contains(token) || unit.name_key.contains(token)).collect(),
            folders,
        }
    }

    pub(super) fn lines(&self, filter: &Filter, open: impl Fn(u32) -> bool) -> Vec<Line> {
        let mut lines = Vec::new();

        for root in &self.roots {
            self.walk(*root, 0, filter, &open, &mut lines);
        }

        let mut tracer = Tracer::default();

        for index in (0..lines.len()).rev() {
            let depth = lines[index].depth;
            let above = index.checked_sub(1).map(|prev| lines[prev].depth);

            lines[index].guide = tracer.back(depth, above);
        }

        lines
    }

    fn walk(&self, id: u32, depth: u16, filter: &Filter, open: &impl Fn(u32) -> bool, lines: &mut Vec<Line>) {
        if !filter.holds(id) {
            return;
        }

        let Some(folder) = self.folders.get(id as usize) else {
            return;
        };

        let opened = open(id);

        lines.push(Line { entry: Entry::Folder(id), depth, open: opened, guide: Guide::default() });

        if !opened {
            return;
        }

        for child in &folder.folders {
            self.walk(*child, depth + 1, filter, open, lines);
        }

        for leaf in &folder.leaves {
            if !filter.keeps(*leaf) {
                continue;
            }

            lines.push(Line { entry: Entry::Leaf(*leaf), depth: depth + 1, open: false, guide: Guide::default() });
        }
    }

    pub(super) fn folder_key(&self, id: u32) -> Option<String> {
        let mut parts: Vec<&str> = Vec::new();
        let mut at = Some(id);

        while let Some(held) = at.and_then(|id| self.folders.get(id as usize)) {
            parts.push(&held.raw);
            at = held.parent;
        }

        if parts.is_empty() {
            return None;
        }

        parts.reverse();

        Some(parts.join(&KEY_JOINT.to_string()))
    }

    pub(super) fn folder_ids(&self) -> FxHashMap<String, u32> {
        (0..u32::try_from(self.folders.len()).unwrap_or(u32::MAX))
            .filter_map(|id| Some((self.folder_key(id)?, id)))
            .collect()
    }

    pub(super) fn lead(&self, leaf: u32) -> Option<Lead> {
        let row = self.rows.get(leaf as usize)?;

        Some(Lead {
            folder: self.folder_key(row.folder)?,
            pane: row.pane,
            file: self.files.get(row.file as usize)?.clone(),
            unit: self.units.get(row.unit as usize)?.tag_key.clone(),
        })
    }

    pub(super) fn follow(&self, lead: &Lead, folders: &FxHashMap<String, u32>) -> Option<u32> {
        let folder = self.folders.get(*folders.get(&lead.folder)? as usize)?;

        folder.leaves.iter().copied().find(|leaf| self.lead(*leaf).as_ref() == Some(lead))
    }

    pub(super) fn pick(&self, row: u32, target: String) -> Option<Pick> {
        let row = self.rows.get(row as usize)?;
        let unit = self.units.get(row.unit as usize)?;
        let file = self.files.get(row.file as usize)?;

        let mut focus = Focus {
            page: row.pane.page(),
            cat: None,
            form: 0,
            enemy: None,
            map: None,
            stage: None,
            active_mod: Some(target),
            unlocked: false,
        };

        let mut item = None;
        let mut line = None;
        let mut aim = None;

        match &unit.who {
            Who::Cat { id, form } => {
                focus.cat = Some(*id);
                focus.form = *form;
            }
            Who::Combo { id, form, line: held } => {
                focus.cat = Some(*id);
                focus.form = *form;
                line = Some(*held);
            }
            Who::Enemy(id) => focus.enemy = Some(*id),
            Who::Map(map) => focus.map = Some(map.clone()),
            Who::Stage(stage) => {
                focus.map = Some(GlobalMapId { category: stage.category.clone(), map: stage.map });
                focus.stage = Some(stage.clone());
            }
            Who::Item(id) => item = Some(*id),
            Who::Egg => {}
            Who::Ability(identity, faction) => aim = Some((*identity, *faction)),
        }

        let label = [unit.named.as_ref(), file.as_ref()].join(theme::HEADER_SEPARATOR);

        Some(Pick { pane: row.pane, focus, item, line, aim, file: file.to_string(), label })
    }
}

#[derive(Clone, Copy)]
enum Source {
    Single,
    Variants,
    Exception,
    Image,
}

struct Builder<'a> {
    app: &'a HaganeApp,
    catalog: Catalog,
    files_at: FxHashMap<Box<str>, u32>,
    titles_at: FxHashMap<Box<str>, u32>,
    sets: Sets,
    arena: Vec<(u32, u32)>,
}

impl Builder<'_> {
    fn cats(&mut self, root: u32) {
        let app = self.app;
        let mut cats: Vec<&CatEntry> = app.cat_state.data.cats.iter().collect();
        let combos = app.vault.vds.cats.combos(&app.vault.vfs);
        let names = app.vault.vds.cats.combo_names(&app.vault.vfs);

        cats.sort_by_key(|cat| cat.id);

        for cat in cats {
            let stats = cat_files::stats_file(cat.id);
            let explanation = cat_files::explanation_file(cat.id);
            let talented = cat.talent_data.is_some();
            let parent = self.folder(Some(root), format!("{:03}", cat.id), cat.display_name(0));

            for (form, held) in cat.forms.iter().enumerate() {
                if !held {
                    continue;
                }

                let tag = format!("{:03}_{}", cat.id, cat_files::form_letter(form));
                let folder = self.folder(Some(parent), tag.clone(), cat.display_name(form));
                let unit = self.unit(Who::Cat { id: cat.id, form }, folder, tag, cat.display_name(form));

                self.rows(unit, Pane::Figures(figures::Subject::Cat), &stats, Source::Single);
                self.rows(unit, Pane::Prose(prose::Subject::Explanation), &explanation, Source::Variants);
                self.rows(unit, Pane::Figures(figures::Subject::Buy), cat_files::UNIT_BUY, Source::Variants);
                self.rows(unit, Pane::Figures(figures::Subject::Curve), cat_files::UNIT_LEVEL, Source::Variants);
                self.rows(unit, Pane::Figures(figures::Subject::Talents), cat_files::SKILL_ACQUISITION, Source::Variants);

                if talented {
                    self.rows(unit, Pane::Figures(figures::Subject::Costs), cat_files::SKILL_LEVEL, Source::Variants);
                    self.rows(unit, Pane::Prose(prose::Subject::TalentText), cat_files::SKILL_DESCRIPTIONS, Source::Exception);
                }

                self.combos(folder, (cat.id, form), &combos, &names);

                let banner = cat_files::banner_file(cat.id, form, cat.egg_ids.unwrap_or(NO_EGGS));

                if let Some(icon) = cat.deploy_icon_paths.get(form).and_then(Option::as_deref).and_then(Path::file_name) {
                    self.rows(unit, Pane::Artwork(Art::DeployIcon), &icon.to_string_lossy(), Source::Image);
                }

                self.rows(unit, Pane::Artwork(Art::Banner), &banner, Source::Image);
            }
        }
    }

    fn combos(&mut self, parent: u32, (id, form): (u32, usize), rows: &[NyancomboData], names: &[Option<String>]) {
        let group = self.folder(Some(parent), COMBOS.to_owned(), COMBOS.to_owned());

        for line in cat_combo::combo_lines(&self.app.vault, id, form) {
            let key = figures::combo_key(rows, line);
            let title = figures::combo_title(rows, names, line);
            let folder = self.folder(Some(group), key.clone(), title.clone());
            let unit = self.unit(Who::Combo { id, form, line }, folder, key, title);

            self.headed(unit, Pane::Prose(prose::Subject::ComboName), cat_files::NYANCOMBO_NAME, COMBO_NAME);
            self.headed(unit, Pane::Figures(figures::Subject::Combo), cat_files::NYANCOMBO_DATA, COMBO_SETUP);
        }

        let tag = format!("{id:03}_{}", cat_files::form_letter(form));
        let unit = self.unit(Who::Cat { id, form }, group, tag, figures::NEW_COMBO.to_owned());

        self.fresh(unit, Pane::Figures(figures::Subject::Combo), cat_files::NYANCOMBO_DATA, figures::NEW_COMBO);
    }

    fn eggs(&mut self, parent: u32) {
        let mut used: Vec<u32> = self
            .app
            .cat_state
            .data
            .cats
            .iter()
            .filter_map(|cat| cat.egg_ids)
            .flat_map(|(norm, evol)| [norm, evol])
            .filter_map(|egg| u32::try_from(egg).ok())
            .collect();

        used.sort_unstable();
        used.dedup();

        for id in used {
            let Ok(raw) = i32::try_from(id) else {
                continue;
            };

            let tag = format!("{id:03}_{EGG_MARK}");
            let name = EGG_NAMES.get(id as usize).map_or_else(|| format!("{id:03}"), |named| (*named).to_owned());
            let folder = self.folder(Some(parent), tag, name.clone());

            for stage in 0..EGG_STAGES {
                let eggs = match stage {
                    0 => (raw, NO_EGG),
                    _ => (NO_EGG, raw),
                };

                let held = format!("{id:03}_{EGG_MARK}{stage:02}");
                let titled = figures::FORMS.get(stage).copied().unwrap_or_default().to_owned();
                let sub = self.folder(Some(folder), held.clone(), titled);
                let unit = self.unit(Who::Egg, sub, held, name.clone());

                self.rows(unit, Pane::Artwork(Art::DeployIcon), &cat_files::icon_file(0, stage, eggs), Source::Image);
                self.rows(unit, Pane::Artwork(Art::Banner), &cat_files::banner_file(0, stage, eggs), Source::Image);
            }
        }
    }

    fn enemies(&mut self, root: u32) {
        let mut enemies: Vec<&EnemyEntry> = self.app.enemy_state.data.enemies.iter().collect();

        enemies.sort_by_key(|enemy| enemy.id);

        for enemy in enemies {
            let tag = format!("{:03}_{ENEMY_MARK}", enemy.id);
            let folder = self.folder(Some(root), tag.clone(), enemy.display_name());
            let unit = self.unit(Who::Enemy(enemy.id), folder, tag, enemy.display_name());

            self.rows(unit, Pane::Figures(figures::Subject::Enemy), enemy_files::STATS, Source::Single);
            self.rows(unit, Pane::Prose(prose::Subject::EnemyName), enemy_files::NAMES, Source::Exception);
            self.rows(unit, Pane::Prose(prose::Subject::EnemyDescription), enemy_files::PICTURE_BOOK, Source::Variants);

            if let Some(icon) = enemy.icon_path.as_deref().and_then(Path::file_name) {
                self.rows(unit, Pane::Artwork(Art::EnemyIcon), &icon.to_string_lossy(), Source::Image);
            }
        }
    }

    fn stages(&mut self, root: u32) {
        let app = self.app;
        let registry = &app.stage_state.data.registry;

        let mut held: FxHashMap<GlobalMapId, Vec<u32>> = FxHashMap::default();

        for key in registry.maps.keys() {
            held.entry(key.clone()).or_default();
        }

        for key in registry.stages.keys() {
            held.entry(GlobalMapId { category: key.category.clone(), map: key.map }).or_default().push(key.stage);
        }

        let mut grouped: FxHashMap<Category, Maps> = FxHashMap::default();

        for (key, stages) in held {
            grouped.entry(key.category).or_default().push((key.map, stages));
        }

        let mut categories: Vec<(Category, Maps)> = grouped.into_iter().collect();

        categories.sort_by(|(left, _), (right, _)| {
            (left.sort_order(), left.map_prefix()).cmp(&(right.sort_order(), right.map_prefix()))
        });

        for (category, mut maps) in categories {
            let prefix = category.map_prefix();
            let raw = if prefix.is_empty() { category.display_name().to_owned() } else { prefix.clone() };
            let folder = self.folder(Some(root), raw, category.display_name().to_owned());

            maps.sort_unstable_by_key(|(map, _)| *map);

            for (map, stages) in maps {
                self.map(folder, GlobalMapId { category: category.clone(), map }, &prefix, stages);
            }
        }
    }

    fn map(&mut self, parent: u32, map: GlobalMapId, prefix: &str, mut stages: Vec<u32>) {
        let app = self.app;
        let data = &app.stage_state.data;
        let registry = &data.registry;
        let address = registry.addresses.get(&map);
        let global = address.and_then(|held| held.global);

        let map_tag = match prefix.is_empty() {
            true => format!("{:03}", map.map),
            false => format!("{prefix}_{:03}", map.map),
        };

        let chapter = registry
            .maps
            .get(&map)
            .map(|found| found.name.trim())
            .filter(|name| !name.is_empty())
            .map_or_else(|| map_tag.clone(), str::to_owned);

        let dropping = registry.maps.get(&map).is_some_and(|found| found.drop_items.is_some());
        let folder = self.folder(Some(parent), map_tag.clone(), chapter.clone());
        let unit = self.unit(Who::Map(map.clone()), folder, map_tag.clone(), chapter.clone());

        if global.is_some() {
            self.rows(unit, Pane::Prose(prose::Subject::MapName), stage_files::MAP_NAME, Source::Exception);
        }

        if global.is_some() && dropping {
            self.rows(unit, Pane::Figures(figures::Subject::MapDrops), stage_files::DROP_ITEM, Source::Variants);
        }

        let image = map.category.image_prefix();

        self.rows(unit, Pane::Artwork(Art::MapBanner), &stage_files::map_banner_file(map.map, &image), Source::Variants);

        stages.sort_unstable();

        for stage in stages {
            let key = GlobalStageId { category: map.category.clone(), map: map.map, stage };
            let entry = registry.stages.get(&key);

            let named = entry
                .map(|found| found.name.trim())
                .filter(|name| !name.is_empty())
                .map_or_else(|| format!("{stage:02}"), str::to_owned);

            let tag = format!("{map_tag}_{stage:02}");
            let name = [chapter.as_str(), named.as_str()].join(theme::HEADER_SEPARATOR);
            let ground = registry.grounds.get(&key).map(|file| file.to_string());
            let drops = entry.is_some_and(|found| drop_ids(found).any(|id| data.drop_chara_registry.contains_key(&id)));

            let at = self.folder(Some(folder), tag.clone(), named);
            let unit = self.unit(Who::Stage(key), at, tag, name);

            if let Some(file) = address.and_then(|held| held.data_file.as_deref()) {
                self.rows(unit, Pane::Figures(figures::Subject::MapStage), file, Source::Variants);
            }

            if let Some(file) = ground {
                self.rows(unit, Pane::Ground, &file, Source::Single);
            }

            if let Some(file) = address.and_then(|held| held.name_file.as_deref()) {
                self.rows(unit, Pane::Prose(prose::Subject::StageName), file, Source::Variants);
            }

            if drops {
                self.rows(unit, Pane::Figures(figures::Subject::DropChara), stage_files::DROP_CHARA, Source::Variants);
            }

            let plate = stage_files::stage_banner_file(&map.category, map.map, stage, &image);

            self.rows(unit, Pane::Artwork(Art::StageBanner), &plate, Source::Variants);
        }
    }

    fn items(&mut self, parent: u32) {
        let app = self.app;
        let catalog = app.vault.vds.items.catalog(&app.vault.vfs);

        let mut ids: Vec<u32> = catalog.iter().filter_map(|row| u32::try_from(row.stage_drop_item_id).ok()).collect();

        ids.sort_unstable();
        ids.dedup();

        let attributes = [
            (Pane::Figures(figures::Subject::ItemBuy), stage_files::GATYA_ITEM_BUY, Source::Variants, ITEM_CATALOG),
            (Pane::Prose(prose::Subject::ItemName), stage_files::GATYA_ITEM_NAME, Source::Exception, ITEM_TEXT),
        ];

        for (pane, base, source, title) in attributes {
            let folder = self.folder(Some(parent), base.to_owned(), title.to_owned());

            for id in &ids {
                let unit = self.unit(Who::Item(*id), folder, format!("{ITEM_TAG} {id:03}"), item_label(app, *id));

                self.listed(unit, pane, base, source);
            }
        }
    }

    fn abilities(&mut self, parent: u32, faction: Faction) {
        let names = &self.app.names;
        let file = glossary_file(faction);

        let pane = match faction {
            Faction::Cat => Pane::Prose(prose::Subject::CatGlossary),
            Faction::Enemy => Pane::Prose(prose::Subject::EnemyGlossary),
        };

        let folder = self.folder(Some(parent), file.to_owned(), pane.title().to_owned());

        for ability in REGISTRY {
            let Some(line) = names.glossary_line(ability.identity, faction).filter(|line| *line > 0) else {
                continue;
            };

            let named = names.ability(ability.identity, faction).to_owned();
            let who = Who::Ability(ability.identity, faction);
            let unit = self.unit(who, folder, format!("{LINE_TAG} {line}"), named);

            self.listed(unit, pane, file, Source::Exception);
        }
    }

    fn folder(&mut self, parent: Option<u32>, raw: String, resolved: String) -> u32 {
        let at = u32::try_from(self.catalog.folders.len()).unwrap_or(u32::MAX);

        self.catalog.folders.push(Folder {
            raw_key: raw.to_lowercase().into(),
            resolved_key: resolved.to_lowercase().into(),
            raw: raw.into(),
            resolved: resolved.into(),
            parent,
            folders: Vec::new(),
            leaves: Vec::new(),
        });

        match parent.and_then(|parent| self.catalog.folders.get_mut(parent as usize)) {
            Some(held) => held.folders.push(at),
            None => self.catalog.roots.push(at),
        }

        at
    }

    fn unit(&mut self, who: Who, folder: u32, tag: String, name: String) -> u32 {
        let at = u32::try_from(self.catalog.units.len()).unwrap_or(u32::MAX);

        self.catalog.units.push(Unit {
            who,
            folder,
            tag_key: tag.to_lowercase().into(),
            name_key: name.to_lowercase().into(),
            tag: tag.into(),
            named: name.into(),
        });

        at
    }

    fn rows(&mut self, unit: u32, pane: Pane, base: &str, source: Source) {
        self.placed(unit, pane, base, source, pane.title(), Face::File);
    }

    fn headed(&mut self, unit: u32, pane: Pane, base: &str, heading: &'static str) {
        self.placed(unit, pane, base, Source::Variants, heading, Face::File);
    }

    fn listed(&mut self, unit: u32, pane: Pane, base: &str, source: Source) {
        self.placed(unit, pane, base, source, pane.title(), Face::Unit);
    }

    fn placed(&mut self, unit: u32, pane: Pane, base: &str, source: Source, heading: &'static str, face: Face) {
        let Some(home) = self.catalog.units.get(unit as usize).map(|held| held.folder) else {
            return;
        };

        let set = self.set(pane, base, source, heading);

        let folder = match set.len() > 1 {
            true => self.folder(Some(home), base.to_owned(), heading.to_owned()),
            false => home,
        };

        for at in set {
            let Some(&(file, title)) = self.arena.get(at) else {
                continue;
            };

            let next = u32::try_from(self.catalog.rows.len()).unwrap_or(u32::MAX);

            self.catalog.rows.push(Row { pane, unit, file, title, folder, face });

            if let Some(held) = self.catalog.folders.get_mut(folder as usize) {
                held.leaves.push(next);
            }
        }
    }

    fn fresh(&mut self, unit: u32, pane: Pane, base: &str, label: &str) {
        let Some(folder) = self.catalog.units.get(unit as usize).map(|held| held.folder) else {
            return;
        };

        let Some(&(file, _)) =
            self.set(pane, base, Source::Variants, pane.title()).next().and_then(|at| self.arena.get(at))
        else {
            return;
        };

        let title = self.title(label);
        let next = u32::try_from(self.catalog.rows.len()).unwrap_or(u32::MAX);

        self.catalog.rows.push(Row { pane, unit, file, title, folder, face: Face::Title });

        if let Some(held) = self.catalog.folders.get_mut(folder as usize) {
            held.leaves.push(next);
        }
    }

    fn set(&mut self, pane: Pane, base: &str, source: Source, heading: &'static str) -> Range<usize> {
        if let Some(range) = self.sets.get(&(pane, heading)).and_then(|held| held.get(base)) {
            return range.clone();
        }

        let vfs = &self.app.vault.vfs;

        let named: Vec<(String, String)> = match source {
            Source::Single => match vfs.rooted(architecture::GAME, base) {
                Some(_) => vec![(base.to_owned(), heading.to_owned())],
                None => Vec::new(),
            },
            Source::Exception => match vfs.find(base) {
                Some(_) => vec![(internal_name(self.app, &visible_name(self.app, base)), heading.to_owned())],
                None => Vec::new(),
            },
            Source::Image => vec![(internal_name(self.app, &visible_name(self.app, base)), heading.to_owned())],
            Source::Variants => {
                let names = vfs.variants(base);
                let shared = names.len() > 1;

                names
                    .into_iter()
                    .map(|name| {
                        let title = match shared {
                            true => registry::language(&name).map_or_else(|| name.clone(), str::to_owned),
                            false => heading.to_owned(),
                        };

                        (name, title)
                    })
                    .collect()
            }
        };

        let start = self.arena.len();

        for (name, title) in named {
            let file = self.file(&name);
            let title = self.title(&title);

            self.arena.push((file, title));
        }

        let range = start..self.arena.len();

        self.sets.entry((pane, heading)).or_default().insert(base.into(), range.clone());

        range
    }

    fn file(&mut self, name: &str) -> u32 {
        if let Some(at) = self.files_at.get(name) {
            return *at;
        }

        let at = u32::try_from(self.catalog.files.len()).unwrap_or(u32::MAX);

        self.catalog.files.push(name.into());
        self.catalog.file_keys.push(name.to_lowercase().into());
        self.files_at.insert(name.into(), at);

        at
    }

    fn title(&mut self, title: &str) -> u32 {
        if let Some(at) = self.titles_at.get(title) {
            return *at;
        }

        let at = u32::try_from(self.catalog.titles.len()).unwrap_or(u32::MAX);

        self.catalog.titles.push(title.into());
        self.catalog.title_keys.push(title.to_lowercase().into());
        self.titles_at.insert(title.into(), at);

        at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(raw: &str, resolved: &str, parent: Option<u32>, folders: Vec<u32>, leaves: Vec<u32>) -> Folder {
        Folder {
            raw: raw.into(),
            resolved: resolved.into(),
            raw_key: raw.to_lowercase().into(),
            resolved_key: resolved.to_lowercase().into(),
            parent,
            folders,
            leaves,
        }
    }

    fn unit(id: u32, folder: u32, name: &str) -> Unit {
        Unit {
            who: Who::Cat { id, form: 0 },
            folder,
            tag: format!("{id:03}_f").into(),
            named: name.into(),
            tag_key: format!("{id:03}_f").into(),
            name_key: name.to_lowercase().into(),
        }
    }

    fn row(unit: u32, file: u32, folder: u32) -> Row {
        Row { pane: Pane::Figures(figures::Subject::Buy), unit, file, title: 0, folder, face: Face::File }
    }

    // Cats > 000 > Cat           unitbuy.csv, unit001.csv
    //      > 002 > Tank Cat      unitbuy.csv
    fn roster() -> Catalog {
        Catalog {
            units: vec![unit(0, 2, "Cat"), unit(2, 4, "Tank Cat")],
            folders: vec![
                folder("Cats", "Cats", None, vec![1, 3], Vec::new()),
                folder("000", "Cat", Some(0), vec![2], Vec::new()),
                folder("000_f", "Cat", Some(1), Vec::new(), vec![0, 1]),
                folder("002", "Tank Cat", Some(0), vec![4], Vec::new()),
                folder("002_f", "Tank Cat", Some(3), Vec::new(), vec![2]),
            ],
            roots: vec![0],
            files: vec!["unitbuy.csv".into(), "unit001.csv".into()],
            file_keys: vec!["unitbuy.csv".into(), "unit001.csv".into()],
            titles: vec!["Progression".into()],
            title_keys: vec!["progression".into()],
            rows: vec![row(0, 0, 2), row(0, 1, 2), row(1, 0, 4)],
        }
    }

    fn shown(catalog: &Catalog, query: &str) -> Vec<Entry> {
        let filter = catalog.filter(query);
        let searching = filter.searching();

        catalog.lines(&filter, |_| searching).into_iter().map(|line| line.entry).collect()
    }

    #[test]
    fn a_folder_name_narrows_the_files_beneath_it() {
        // "tank" only lives on the folder, "unitbuy" only on the file
        assert_eq!(
            shown(&roster(), "tank unitbuy"),
            vec![Entry::Folder(0), Entry::Folder(3), Entry::Folder(4), Entry::Leaf(2)],
        );
    }

    #[test]
    fn a_rename_keeps_your_place_in_the_tree() {
        let before = roster();
        let mut after = roster();

        // renaming only touches the resolved side, never the ids the place is kept by
        after.folders[3].resolved = "The Grim Reaper".into();
        after.folders[3].resolved_key = "the grim reaper".into();

        let folders = after.folder_ids();
        let lead = before.lead(2).expect("leaf 2 exists in the fixture");

        assert_eq!(folders.get(&before.folder_key(4).unwrap_or_default()), Some(&4));
        assert_eq!(after.follow(&lead, &folders), Some(2));
        assert_eq!(shown(&after, "grim unitbuy"), vec![Entry::Folder(0), Entry::Folder(3), Entry::Folder(4), Entry::Leaf(2)]);
    }

    #[test]
    fn browsing_starts_shut_and_a_search_opens_every_path_to_a_hit() {
        assert_eq!(shown(&roster(), ""), vec![Entry::Folder(0)]);

        assert_eq!(
            shown(&roster(), "(unit001"),
            vec![Entry::Folder(0), Entry::Folder(1), Entry::Folder(2), Entry::Leaf(1)],
        );

        assert!(shown(&roster(), "nothing").is_empty(), "a search with no hits leaves no empty folders behind");
    }
}
