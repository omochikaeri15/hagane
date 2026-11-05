use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::domains::settings::SavedBcvName;

use super::disk::{next_bundle, playable, read_save};
use super::{EXTENSION, clean_name};

const NEW_FOLDER: &str = "New Folder";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShelfRow {
    pub path: PathBuf,
    pub name: String,
    pub depth: u16,
    pub folder: bool,
    pub expanded: bool,
    pub parent: Option<usize>,
}

#[derive(Clone, Debug, Default)]
pub struct Shelf {
    root: PathBuf,
    limit: Option<usize>,
    folders: BTreeMap<PathBuf, Vec<PathBuf>>,
    bundles: BTreeMap<PathBuf, Vec<PathBuf>>,
}

fn sort_key(path: &Path) -> String {
    path.file_name().map_or_else(String::new, |name| name.to_string_lossy().to_lowercase())
}

fn stem(path: &Path) -> String {
    path.file_stem().map_or_else(String::new, |stem| stem.to_string_lossy().into_owned())
}

fn matches(name: &str, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }

    let (name, query) = (name.as_bytes(), query.as_bytes());

    name.len() >= query.len() && name.windows(query.len()).any(|window| window.eq_ignore_ascii_case(query))
}

impl Shelf {
    pub fn read(library: &Path) -> Self {
        Self::scanned(library, None, &playable)
    }

    pub fn read_to(library: &Path, limit: usize) -> Self {
        Self::scanned(library, Some(limit), &playable)
    }

    fn scanned(library: &Path, limit: Option<usize>, keeps: &dyn Fn(&Path) -> bool) -> Self {
        let mut shelf = Self { root: library.to_path_buf(), limit, ..Self::default() };

        shelf.scan(library, 0, keeps);
        shelf
    }

    fn scan(&mut self, dir: &Path, level: usize, keeps: &dyn Fn(&Path) -> bool) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        let (mut folders, mut bundles) = (Vec::new(), Vec::new());

        for entry in entries.flatten() {
            let path = entry.path();

            if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                if self.limit.is_none_or(|limit| level < limit) {
                    folders.push(path);
                }
            } else if path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case(EXTENSION)) && keeps(&path) {
                bundles.push(path);
            }
        }

        folders.sort_by_key(|path| sort_key(path));
        bundles.sort_by_key(|path| sort_key(path));

        for folder in &folders {
            self.scan(folder, level + 1, keeps);
        }

        self.folders.insert(dir.to_path_buf(), folders);
        self.bundles.insert(dir.to_path_buf(), bundles);
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn holds_bundle(&self, path: &Path) -> bool {
        path.parent().and_then(|parent| self.bundles.get(parent)).is_some_and(|held| held.iter().any(|bundle| bundle == path))
    }

    pub fn holds_folder(&self, path: &Path) -> bool {
        self.folders.contains_key(path)
    }

    pub fn room(&self, folder: &Path) -> usize {
        self.limit.map_or(usize::MAX, |limit| folder.strip_prefix(&self.root).map_or(0, |inner| limit.saturating_sub(inner.components().count())))
    }

    pub fn levels(&self, path: &Path) -> usize {
        self.folders.get(path).map_or(0, |inner| 1 + inner.iter().map(|folder| self.levels(folder)).max().unwrap_or(0))
    }

    pub fn takes(&self, folder: &Path, item: &Path) -> bool {
        self.levels(item) <= self.room(folder)
    }

    pub fn create_folder(&self, parent: &Path) -> Result<PathBuf, String> {
        if self.room(parent) == 0 {
            return Err(format!("{} is as deep as replay folders go", parent.display()));
        }

        let folder = free_path(parent, NEW_FOLDER, None);

        fs::create_dir_all(&folder).map_err(|error| format!("{} could not be created: {error}", folder.display()))?;

        Ok(folder)
    }

    pub fn move_into(&self, item: &Path, folder: &Path) -> Result<Option<PathBuf>, String> {
        if item.parent() == Some(folder) || folder.starts_with(item) || !self.takes(folder, item) {
            return Ok(None);
        }

        let target = if item.is_dir() {
            free_path(folder, item.file_name().map_or_else(String::new, |name| name.to_string_lossy().into_owned()).as_str(), None)
        } else {
            free_path(folder, &stem(item), Some(EXTENSION))
        };

        fs::rename(item, &target).map_err(|error| format!("{} could not be moved: {error}", item.display()))?;

        Ok(Some(target))
    }

    pub fn count(&self, folder: &Path) -> usize {
        let held = self.bundles.get(folder).map_or(0, Vec::len);

        held + self.folders.get(folder).map_or(0, |folders| folders.iter().map(|inner| self.count(inner)).sum())
    }

    fn any(&self, folder: &Path, query: &str) -> bool {
        self.bundles.get(folder).is_some_and(|held| held.iter().any(|bundle| matches(&stem(bundle), query)))
            || self.folders.get(folder).is_some_and(|inner| inner.iter().any(|inner| self.any(inner, query)))
    }

    pub fn rows(&self, expanded: &HashSet<PathBuf>, query: &str) -> Vec<ShelfRow> {
        let mut rows = Vec::new();

        self.walk(&self.root, 0, None, expanded, query.trim(), &mut rows);
        rows
    }

    fn walk(&self, dir: &Path, depth: u16, parent: Option<usize>, expanded: &HashSet<PathBuf>, query: &str, rows: &mut Vec<ShelfRow>) {
        for folder in self.folders.get(dir).into_iter().flatten() {
            if !query.is_empty() && !self.any(folder, query) {
                continue;
            }

            let open = expanded.contains(folder);
            let name = folder.file_name().map_or_else(String::new, |name| name.to_string_lossy().into_owned());
            let at = rows.len();

            rows.push(ShelfRow { path: folder.clone(), name, depth, folder: true, expanded: open, parent });

            if open {
                self.walk(folder, depth + 1, Some(at), expanded, query, rows);
            }
        }

        for bundle in self.bundles.get(dir).into_iter().flatten() {
            let name = stem(bundle);

            if matches(&name, query) {
                rows.push(ShelfRow { path: bundle.clone(), name, depth, folder: false, expanded: false, parent });
            }
        }
    }
}

fn free_path(folder: &Path, name: &str, extension: Option<&str>) -> PathBuf {
    let named = |name: String| folder.join(extension.map_or_else(|| name.clone(), |extension| format!("{name}.{extension}")));

    (1usize..).map(|copy| named(if copy == 1 { name.to_owned() } else { format!("{name}{copy}") })).find(|path| !path.exists()).unwrap_or_else(|| named(name.to_owned()))
}

fn location_name(latest: &Path) -> Option<String> {
    let save = read_save(latest).ok()?;
    let entry = &save.setup.stage;
    let map = if entry.map_name.is_empty() { format!("Map {}", entry.map_id) } else { entry.map_name.clone() };
    let stage = if entry.stage_name.is_empty() { format!("Stage {}", entry.stage + 1) } else { entry.stage_name.clone() };
    let name: String = clean_name(&format!("{map} - {stage}")).chars().filter(|part| !part.is_control()).collect();
    let name = name.trim();

    (!name.is_empty()).then(|| name.to_owned())
}

pub fn saved_bundle(library: &Path, naming: SavedBcvName, latest: &Path) -> PathBuf {
    match naming {
        SavedBcvName::Replay => next_bundle(library),
        SavedBcvName::Location => location_name(latest).map_or_else(|| next_bundle(library), |name| free_path(library, &name, Some(EXTENSION))),
    }
}

pub fn delete_folder(library: &Path, folder: &Path) -> Result<(), String> {
    if folder == library || !folder.starts_with(library) {
        return Err(format!("{} is not a folder inside the replay library", folder.display()));
    }

    fs::remove_dir_all(folder).map_err(|error| format!("{} could not be deleted: {error}", folder.display()))
}

pub fn rename_folder(folder: &Path, name: &str) -> Result<Option<PathBuf>, String> {
    let clean = clean_name(name);
    let clean = clean.trim();
    let current = folder.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();

    if clean.is_empty() || clean == current {
        return Ok(None);
    }

    let parent = folder.parent().ok_or("the folder has no parent")?;
    let target = parent.join(clean);

    if !current.eq_ignore_ascii_case(clean) && target.exists() {
        return Ok(None);
    }

    fs::rename(folder, &target).map_err(|error| format!("{} could not be renamed: {error}", folder.display()))?;

    Ok(Some(target))
}

impl Shelf {
    pub fn relocate(&mut self, old: &Path, fresh: &Path) {
        let moved = |path: &PathBuf| rebased(path, old, fresh).unwrap_or_else(|| path.clone());
        let carry = |lists: &mut BTreeMap<PathBuf, Vec<PathBuf>>| {
            *lists = std::mem::take(lists)
                .into_iter()
                .map(|(key, paths)| {
                    let mut paths: Vec<PathBuf> = paths.iter().map(moved).collect();

                    paths.sort_by_key(|path| sort_key(path));

                    (moved(&key), paths)
                })
                .collect();
        };

        carry(&mut self.folders);
        carry(&mut self.bundles);
    }
}

pub fn rebased(path: &Path, old: &Path, fresh: &Path) -> Option<PathBuf> {
    path.strip_prefix(old).ok().map(|rest| if rest.as_os_str().is_empty() { fresh.to_path_buf() } else { fresh.join(rest) })
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIMIT: usize = 4;

    fn library(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("hagane-shelf-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);

        fs::create_dir_all(root.join("Sharks/Deep")).unwrap();
        fs::create_dir_all(root.join("Empty")).unwrap();

        for file in ["Top.bcv", "Sharks/Great.bcv", "Sharks/Deep/Hammer.bcv", "notes.txt"] {
            fs::write(root.join(file), "x").unwrap();
        }

        root
    }

    fn shelf(root: &Path) -> Shelf {
        Shelf::scanned(root, None, &|_| true)
    }

    fn limited(root: &Path) -> Shelf {
        Shelf::scanned(root, Some(LIMIT), &|_| true)
    }

    fn names(rows: &[ShelfRow]) -> Vec<&str> {
        rows.iter().map(|row| row.name.as_str()).collect()
    }

    // Folders come first, the extension goes away, and anything that is not a replay stays hidden.
    #[test]
    fn the_tree_opens_only_what_is_expanded() {
        let root = library("tree");
        let shelf = shelf(&root);
        let mut open = HashSet::new();

        assert_eq!(names(&shelf.rows(&open, "")), ["Empty", "Sharks", "Top"]);

        open.insert(root.join("Sharks"));

        let rows = shelf.rows(&open, "");

        assert_eq!(names(&rows), ["Empty", "Sharks", "Deep", "Great", "Top"]);
        assert_eq!((rows[2].depth, rows[2].parent), (1, Some(1)));
        assert_eq!((rows[3].folder, rows[3].parent), (false, Some(1)));
        assert_eq!(rows[4].parent, None);
        assert_eq!(shelf.count(&root.join("Sharks")), 2);

        let _ = fs::remove_dir_all(&root);
    }

    // Like the Files page, a search keeps the folders leading to a match and leaves them as they were.
    #[test]
    fn a_search_keeps_the_folders_that_lead_to_a_match() {
        let root = library("search");
        let shelf = shelf(&root);
        let mut open = HashSet::new();

        assert_eq!(names(&shelf.rows(&open, "HAMMER")), ["Sharks"]);

        open.insert(root.join("Sharks"));
        assert_eq!(names(&shelf.rows(&open, "hammer")), ["Sharks", "Deep"]);

        open.insert(root.join("Sharks/Deep"));
        assert_eq!(names(&shelf.rows(&open, "hammer")), ["Sharks", "Deep", "Hammer"]);
        assert!(shelf.rows(&open, "nothing").is_empty());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_folder_can_never_move_into_itself() {
        let root = library("move");
        let sharks = root.join("Sharks");
        let shelf = shelf(&root);

        assert_eq!(shelf.move_into(&sharks, &sharks.join("Deep")).unwrap(), None);
        assert_eq!(shelf.move_into(&sharks, &sharks).unwrap(), None);
        assert_eq!(shelf.move_into(&root.join("Top.bcv"), &root).unwrap(), None);
        assert!(sharks.join("Deep/Hammer.bcv").exists());

        let moved = shelf.move_into(&sharks.join("Deep"), &root.join("Empty")).unwrap().unwrap();

        assert_eq!(moved, root.join("Empty/Deep"));
        assert!(moved.join("Hammer.bcv").exists());

        let _ = fs::remove_dir_all(&root);
    }

    // Two replays with one name both have to survive a move.
    #[test]
    fn a_move_onto_a_taken_name_keeps_both() {
        let root = library("taken");

        fs::write(root.join("Great.bcv"), "y").unwrap();

        let moved = shelf(&root).move_into(&root.join("Great.bcv"), &root.join("Sharks")).unwrap().unwrap();

        assert_eq!(moved, root.join("Sharks/Great2.bcv"));
        assert_eq!(fs::read_to_string(root.join("Sharks/Great.bcv")).unwrap(), "x");
        assert_eq!(fs::read_to_string(moved).unwrap(), "y");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn deleting_refuses_the_library_and_anything_outside_it() {
        let root = library("delete");

        assert!(delete_folder(&root, &root).is_err());
        assert!(delete_folder(&root, &std::env::temp_dir()).is_err());
        assert!(root.join("Top.bcv").exists());

        delete_folder(&root, &root.join("Sharks")).unwrap();
        assert!(!root.join("Sharks").exists());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_new_folder_never_takes_an_existing_name() {
        let root = library("create");
        let shelf = shelf(&root);

        assert_eq!(shelf.create_folder(&root).unwrap(), root.join("New Folder"));
        assert_eq!(shelf.create_folder(&root).unwrap(), root.join("New Folder2"));

        let renamed = rename_folder(&root.join("New Folder2"), "Bosses?").unwrap().unwrap();

        assert_eq!(renamed, root.join("Bosses"));
        assert_eq!(rename_folder(&renamed, "Empty").unwrap(), None);

        let _ = fs::remove_dir_all(&root);
    }

    fn chain(root: &Path, levels: usize) -> PathBuf {
        let folder = (0..levels).fold(root.to_path_buf(), |path, level| path.join(format!("L{level}")));

        fs::create_dir_all(&folder).unwrap();

        folder
    }

    // A folder past the limit stays on disk, but the list never sees it or what it holds.
    #[test]
    fn folders_past_the_limit_are_not_acknowledged() {
        let root = library("limit");
        let deepest = chain(&root, LIMIT);
        let past = chain(&root, LIMIT + 1);

        fs::write(deepest.join("Kept.bcv"), "x").unwrap();
        fs::write(past.join("Hidden.bcv"), "x").unwrap();

        let limited = limited(&root);

        assert!(limited.holds_folder(&deepest));
        assert!(limited.holds_bundle(&deepest.join("Kept.bcv")));
        assert!(!limited.holds_folder(&past));
        assert!(!limited.holds_bundle(&past.join("Hidden.bcv")));
        assert_eq!(limited.count(&root.join("L0")), 1);
        assert!(past.join("Hidden.bcv").exists());

        let open = shelf(&root);

        assert!(open.holds_bundle(&past.join("Hidden.bcv")));
        assert_eq!(open.count(&root.join("L0")), 2);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_folder_at_the_limit_refuses_new_folders() {
        let root = library("full");
        let deepest = chain(&root, LIMIT);
        let limited = limited(&root);

        assert_eq!(limited.room(&deepest), 0);
        assert!(limited.create_folder(&deepest).is_err());
        assert_eq!(fs::read_dir(&deepest).unwrap().count(), 0);
        assert!(limited.create_folder(deepest.parent().unwrap()).is_ok());

        assert!(shelf(&root).create_folder(&deepest).is_ok());

        let _ = fs::remove_dir_all(&root);
    }

    // Moving a folder carries everything inside it, so its deepest folder is what has to fit.
    #[test]
    fn a_move_past_the_limit_is_refused() {
        let root = library("deep-move");
        let deepest = chain(&root, LIMIT);
        let limited = limited(&root);
        let sharks = root.join("Sharks");

        assert_eq!(limited.levels(&sharks), 2);
        assert_eq!(limited.levels(&root.join("Top.bcv")), 0);
        assert!(!limited.takes(deepest.parent().unwrap(), &sharks));
        assert_eq!(limited.move_into(&sharks, deepest.parent().unwrap()).unwrap(), None);
        assert!(sharks.join("Deep/Hammer.bcv").exists());

        assert_eq!(limited.move_into(&root.join("Top.bcv"), &deepest).unwrap(), Some(deepest.join("Top.bcv")));
        assert_eq!(limited.move_into(&sharks, &root.join("Empty")).unwrap(), Some(root.join("Empty/Sharks")));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_renamed_folder_takes_its_replays_with_it() {
        let root = library("relocate");
        let mut shelf = shelf(&root);
        let open: HashSet<PathBuf> = [root.join("Whales")].into_iter().collect();

        shelf.relocate(&root.join("Sharks"), &root.join("Whales"));

        assert!(shelf.holds_bundle(&root.join("Whales/Great.bcv")));
        assert!(!shelf.holds_folder(&root.join("Sharks")));
        assert_eq!(names(&shelf.rows(&open, "")), ["Empty", "Whales", "Deep", "Great", "Top"]);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rebasing_follows_a_folder_and_leaves_everything_else() {
        let (old, fresh) = (Path::new("sandbox/A"), Path::new("sandbox/B"));

        assert_eq!(rebased(old, old, fresh).as_deref(), Some(fresh));
        assert_eq!(rebased(&old.join("x.bcv"), old, fresh), Some(fresh.join("x.bcv")));
        assert_eq!(rebased(Path::new("sandbox/AB"), old, fresh), None);
    }

    fn with_save(root: &Path, map: &str, stage: &str) -> PathBuf {
        let latest = root.join("latest");
        let mut save = super::super::Save::default();

        save.setup.stage.map_name = map.to_owned();
        save.setup.stage.stage_name = stage.to_owned();
        fs::create_dir_all(&latest).unwrap();
        fs::write(latest.join(super::super::SAVE), serde_json::to_string(&save).unwrap()).unwrap();

        latest
    }

    #[test]
    fn location_names_come_from_the_battle_and_stay_unique() {
        let root = library("naming");
        let latest = with_save(&root, "Empire of Cats: Ch. 1", "Korea?");

        assert_eq!(saved_bundle(&root, SavedBcvName::Location, &latest), root.join("Empire of Cats Ch. 1 - Korea.bcv"));

        fs::write(root.join("Empire of Cats Ch. 1 - Korea.bcv"), "x").unwrap();
        assert_eq!(saved_bundle(&root, SavedBcvName::Location, &latest), root.join("Empire of Cats Ch. 1 - Korea2.bcv"));

        assert_eq!(saved_bundle(&root, SavedBcvName::Replay, &latest), root.join("Replay1.bcv"));
        assert_eq!(saved_bundle(&root, SavedBcvName::Location, &root.join("missing")), root.join("Replay1.bcv"));

        let _ = fs::remove_dir_all(&root);
    }
}
