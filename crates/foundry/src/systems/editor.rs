pub mod audio;
pub mod captions;
pub mod cuts;
pub mod catfruit;
pub mod dungeon;
pub mod evolve;
pub mod figures;
pub mod filter;
pub mod finder;
pub mod gift;
pub mod ground;
pub mod group;
pub mod ledger;
pub mod logins;
pub mod maplayout;
pub mod plans;
pub mod preset;
pub mod prose;
pub mod records;
pub mod registry;
pub mod roster;
pub mod sheet;
pub mod tables;
pub mod treasure;
pub mod unused;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use rustc_hash::FxHashMap;
use tracing::warn;

use codex::chapter::stage::RewardStructure;
use codex::chapter::Category;
use codex::combat::{Faction, Identity};
use codex::common;
use codex::files::Localizable;

use crate::common::architecture;
use crate::domains::cat::scanner::CatEntry;
use crate::domains::mods;
use crate::domains::enemy::scanner::EnemyEntry;
use crate::domains::sandbox::orb;
use crate::domains::stage::{GlobalMapId, GlobalStageId, Stage, StageRegistry};
use crate::systems::combat::names::NameBook;
use crate::{Vault, Vfs};

use tables::*;

pub const SEPARATOR: &str = " :: ";

const FILE_TITLE: &str = "File";

pub struct Sources<'a> {
    pub vault: &'a Vault,
    pub cats: &'a [CatEntry],
    pub enemies: &'a [EnemyEntry],
    pub registry: &'a StageRegistry,
    pub drops: &'a HashMap<u32, u32>,
    pub cat_names: &'a HashMap<u32, Vec<String>>,
    pub names: &'a NameBook,
    pub localizable: &'a Localizable,
    pub category: fn(&Category) -> &str,
    pub order: fn(&Category) -> u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Realm {
    Home,
    Cats,
    Enemies,
    Stages,
    Files,
    Elsewhere,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Focus {
    pub realm: Realm,
    pub cat: Option<u32>,
    pub form: usize,
    pub enemy: Option<u32>,
    pub category: Option<Category>,
    pub map: Option<GlobalMapId>,
    pub stage: Option<GlobalStageId>,
    pub active_mod: Option<String>,
    pub unlocked: bool,
}

impl Focus {
    pub fn mount(&self) -> Option<&str> {
        self.active_mod.as_deref()
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Pick {
    pub pane: Pane,
    pub cat: Option<u32>,
    pub form: usize,
    pub enemy: Option<u32>,
    pub map: Option<GlobalMapId>,
    pub stage: Option<GlobalStageId>,
    pub key: Option<u32>,
    pub line: Option<usize>,
    pub aim: Option<(Identity, Faction)>,
    pub file: String,
    pub label: String,
    pub path: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Art {
    DeployIcon,
    Banner,
    Gacha,
    EnemyIcon,
    MapBanner,
    StageBanner,
    Castle,
    Background,
    ItemIcon,
    Image,
}

impl Art {
    pub fn title(self) -> &'static str {
        match self {
            Art::DeployIcon => CAT_ICON_TITLE,
            Art::Gacha => GACHA_IMAGE_TITLE,
            Art::EnemyIcon => ICON_TITLE,
            Art::Banner | Art::MapBanner | Art::StageBanner => BANNER_TITLE,
            Art::Castle => CASTLE_IMAGE_TITLE,
            Art::Background => BACKGROUND_IMAGE_TITLE,
            Art::ItemIcon => ITEM_ICON_TITLE,
            Art::Image => IMAGE_TITLE,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Pane {
    Figures(figures::Subject),
    Prose(prose::Subject),
    Ground,
    Artwork(Art),
    Sheet(sheet::Sheet),
    Ledger(ledger::Ledger),
    Records(records::Book),
    Preset,
    Group,
    Treasure,
    Gift,
    Filter,
    Evolve,
    Roster(roster::Tier),
    Captions(captions::Layout),
    Catfruit,
    MapLayout,
    Dungeon,
    Cuts,
    Logins,
    Unused,
    Audio,
    File,
}

impl Pane {
    pub fn title(self) -> &'static str {
        match self {
            Pane::Figures(subject) => subject.title(),
            Pane::Prose(subject) => subject.title(),
            Pane::Ground => ground::TITLE,
            Pane::Artwork(art) => art.title(),
            Pane::Sheet(kind) => kind.title(),
            Pane::Ledger(kind) => kind.title(),
            Pane::Records(book) => book.title(),
            Pane::Preset => preset::TITLE,
            Pane::Group => group::TITLE,
            Pane::Treasure => treasure::TITLE,
            Pane::Gift => gift::TITLE,
            Pane::Filter => filter::TITLE,
            Pane::Evolve => evolve::TITLE,
            Pane::Roster(tier) => tier.title(),
            Pane::Captions(layout) => layout.title(),
            Pane::Catfruit => catfruit::TITLE,
            Pane::MapLayout => maplayout::TITLE,
            Pane::Dungeon => dungeon::TITLE,
            Pane::Cuts => cuts::TITLE,
            Pane::Logins => logins::TITLE,
            Pane::Unused => unused::TITLE,
            Pane::Audio => audio::TITLE,
            Pane::File => FILE_TITLE,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mark {
    Same,
    Changed,
    Added,
}

impl Mark {
    pub fn of<T: PartialEq>(ours: Option<T>, theirs: Option<T>) -> Mark {
        match (ours, theirs) {
            (None, None) => Mark::Same,
            (Some(_), None) => Mark::Added,
            (ours, theirs) if ours == theirs => Mark::Same,
            _ => Mark::Changed,
        }
    }
}

#[derive(Default)]
pub struct Reader {
    held: FxHashMap<PathBuf, Option<Rc<str>>>,
}

impl Reader {
    pub fn text(&mut self, path: &Path) -> Option<Rc<str>> {
        if let Some(held) = self.held.get(path) {
            return held.clone();
        }

        let read = fs::read(path).ok().map(|bytes| Rc::<str>::from(common::scrub(&bytes)));

        self.held.insert(path.to_path_buf(), read.clone());

        read
    }
}

pub fn file_mark(vfs: &Vfs, target: &str, file: &str) -> Mark {
    let Some(held) = mods::find(vfs, target, Path::new(file)) else {
        return Mark::Same;
    };

    let ours = fs::read(&held).ok();
    let theirs = vfs.rooted(architecture::GAME, file).and_then(|game| fs::read(game).ok());

    Mark::of(ours, theirs)
}

pub fn file_reset(vfs: &Vfs, target: &str, file: &str) -> bool {
    mods::find(vfs, target, Path::new(file)).is_some_and(|held| {
        fs::remove_file(&held)
            .inspect_err(|err| warn!(path = %held.display(), "The mod's copy could not be removed: {}", err))
            .is_ok()
    })
}

fn regional(vfs: &Vfs, base: &str, name: &str) -> bool {
    name == base || vfs.stripped(name).is_some_and(|held| held == base)
}

pub fn copies(vfs: &Vfs, base: &str) -> Vec<String> {
    vfs.variants(base).into_iter().filter(|name| regional(vfs, base, name)).collect()
}

fn internal_name(sources: &Sources<'_>, visible: &str) -> String {
    sources.vault.vfs.stripped(visible).unwrap_or_else(|| visible.to_owned())
}

pub fn visible_name(sources: &Sources<'_>, wanted: &str) -> String {
    sources
        .vault
        .vfs
        .find(wanted)
        .as_deref()
        .and_then(std::path::Path::file_name)
        .map_or_else(|| wanted.to_owned(), |name| name.to_string_lossy().into_owned())
}

fn altar_key(sources: &Sources<'_>, stage: &GlobalStageId) -> Option<u32> {
    let map = GlobalMapId { category: stage.category.clone(), map: stage.map };
    let global = sources.registry.addresses.get(&map)?.global?;

    global.checked_mul(ALTAR_STAGE_SCALE)?.checked_add(stage.stage)
}

fn combo_effect(sources: &Sources<'_>, line: usize) -> String {
    sources
        .vault
        .vds
        .cats
        .combo_effects(&sources.vault.vfs)
        .get(line)
        .and_then(Option::as_deref)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map_or_else(|| format!("{COMBO_EFFECT_LABEL} {line}"), str::to_owned)
}

fn glossary_file(faction: Faction) -> &'static str {
    match faction {
        Faction::Cat => GLOSSARY_FILES[0],
        Faction::Enemy => GLOSSARY_FILES[1],
    }
}

pub fn item_label(sources: &Sources<'_>, item: u32) -> String {
    sources
        .vault
        .vds
        .items
        .name(&sources.vault.vfs, item)
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| format!("Item {item}"))
}

fn drop_ids(stage: &Stage) -> impl Iterator<Item = u32> + '_ {
    let timed = match &stage.rewards {
        RewardStructure::Timed { scores, .. } => Some(scores.iter().map(|score| score.item_id)),
        _ => None,
    };

    stage.rewards.drops().iter().map(|drop| drop.item_id).chain(timed.into_iter().flatten())
}

pub fn orb_named(sources: &Sources<'_>) -> records::Named {
    let numbered = |at: usize| i32::try_from(at).unwrap_or(i32::MAX);

    let grades = orb::GRADES.iter().enumerate().map(|(at, grade)| (numbered(at), (*grade).to_owned())).collect();

    let effects = orb::effect_names(&sources.vault.vfs)
        .into_iter()
        .enumerate()
        .filter(|(_, name)| !name.is_empty())
        .map(|(at, name)| (numbered(at), name))
        .collect();

    let traits = orb::TRAITS.iter().enumerate().map(|(at, identity)| (numbered(at), sources.names.trait_label(*identity).to_owned())).collect();

    vec![(ORB_GRADE, grades), (ORB_EFFECT, effects), (ORB_TRAIT, traits)]
}

impl Art {
    pub fn realm(self) -> Realm {
        match self {
            Art::DeployIcon | Art::Banner | Art::Gacha => Realm::Cats,
            Art::EnemyIcon => Realm::Enemies,
            Art::ItemIcon | Art::Image => Realm::Home,
            Art::MapBanner | Art::StageBanner | Art::Castle | Art::Background => Realm::Stages,
        }
    }
}

impl Pane {
    pub fn realm(self) -> Realm {
        match self {
            Pane::Figures(subject) => subject.realm(),
            Pane::Prose(subject) => subject.realm(),
            Pane::Ground => Realm::Stages,
            Pane::Artwork(art) => art.realm(),
            Pane::Sheet(_) => Realm::Home,
            Pane::Ledger(_) => Realm::Stages,
            Pane::Records(_) => Realm::Home,
            Pane::Preset | Pane::Group | Pane::Treasure | Pane::MapLayout | Pane::Dungeon => Realm::Stages,
            Pane::Gift | Pane::Roster(_) | Pane::Captions(_) | Pane::Catfruit | Pane::Cuts | Pane::Logins | Pane::Unused | Pane::Audio => Realm::Home,
            Pane::Filter | Pane::Evolve => Realm::Cats,
            Pane::File => Realm::Files,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Vfs;

    use super::regional;

    #[test]
    fn only_a_language_copy_counts_as_the_same_table() {
        let vfs = Vfs::with_priority(&[]);

        assert!(regional(&vfs, "Text_1.csv", "Text_1.csv"));
        assert!(regional(&vfs, "Text_1.csv", "Text_1_en.csv"));
        assert!(regional(&vfs, "Text_1_Later.csv", "Text_1_Later_en.csv"));

        // A longer name is another table, with or without a language on its end.
        assert!(!regional(&vfs, "Text_1.csv", "Text_1_Later_en.csv"));
        assert!(!regional(&vfs, "Text_1.csv", "Text_1_group.csv"));
    }
}
