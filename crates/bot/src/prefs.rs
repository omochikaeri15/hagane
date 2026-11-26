use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use serde::{Deserialize, Serialize};
use tracing::error;

use foundry::domains::settings::lang;

use super::alias::Aliases;
use super::prose::{self, Line, Prose};
use super::render::Format;

const FOLDER: &str = "users";
const HELD: &str = "json";
const PARTIAL: &str = "partial";
pub(crate) const ENGLISH: &str = "en";
pub(crate) const PREFIX: &str = "hg!";
pub(crate) const PREFIX_LIMIT: usize = 16;
pub(crate) const FORMS: [&str; 4] = ["Normal", "Evolved", "True", "Ultra"];
const TRUE_FORM: usize = 2;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Search {
    #[default]
    Exact,
    Preferred,
}

impl Search {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Search::Exact => "Exact",
            Search::Preferred => "Preferred",
        }
    }

    pub(crate) fn slug(self) -> &'static str {
        match self {
            Search::Exact => "exact",
            Search::Preferred => "preferred",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Mode {
    #[default]
    Dormant,
    Interactive,
}

impl Mode {
    pub(crate) const ALL: [Mode; 2] = [Mode::Dormant, Mode::Interactive];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Mode::Dormant => "Dormant",
            Mode::Interactive => "Interactive",
        }
    }

    pub(crate) fn slug(self) -> &'static str {
        match self {
            Mode::Dormant => "dormant",
            Mode::Interactive => "interactive",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Prefs {
    pub(crate) language: String,
    pub(crate) format: Format,
    pub(crate) form: usize,
    pub(crate) search: Search,
    pub(crate) prefix: Option<String>,
    pub(crate) prose: Prose,
    pub(crate) mode: Mode,
    pub(crate) aliases: Aliases,
}

impl Default for Prefs {
    fn default() -> Self {
        Self { language: ENGLISH.to_owned(), format: Format::Avif, form: TRUE_FORM, search: Search::Exact, prefix: None, prose: Prose::Human, mode: Mode::Dormant, aliases: Aliases::default() }
    }
}

impl Prefs {
    pub(crate) fn prefix(&self) -> &str {
        self.prefix.as_deref().unwrap_or(PREFIX)
    }

    pub(crate) fn form_from(&self, forms: &[bool; 4]) -> usize {
        (0..=self.form.min(FORMS.len() - 1)).rev().find(|form| forms[*form]).or_else(|| forms.iter().position(|held| *held)).unwrap_or(0)
    }
}

pub(crate) fn prefix_from(typed: &str) -> Result<Option<String>, &'static Line> {
    let typed = typed.trim();

    if typed.is_empty() || typed.eq_ignore_ascii_case(PREFIX) {
        return Ok(None);
    }

    if typed.chars().any(char::is_whitespace) {
        return Err(&prose::PREFIX_SPACED);
    }

    if typed.chars().count() > PREFIX_LIMIT {
        return Err(&prose::PREFIX_LONG);
    }

    Ok(Some(typed.to_owned()))
}

pub(crate) fn language_name(code: &str) -> &'static str {
    lang::choices().find(|(held, _)| *held == code).map_or("English", |(_, name)| name)
}

pub(crate) struct Book {
    folder: PathBuf,
    users: Mutex<HashMap<u64, Prefs>>,
}

fn held(folder: &Path) -> io::Result<HashMap<u64, Prefs>> {
    let mut users = HashMap::new();

    fs::create_dir_all(folder)?;

    for entry in fs::read_dir(folder)? {
        let path = entry?.path();
        let user = path.file_stem().filter(|_| path.extension().is_some_and(|extension| extension == HELD)).and_then(|stem| stem.to_str()).and_then(|stem| stem.parse::<u64>().ok());
        let Some(user) = user else { continue };

        match fs::read(&path).map_err(|err| err.to_string()).and_then(|bytes| serde_json::from_slice(&bytes).map_err(|err| err.to_string())) {
            Ok(prefs) => {
                users.insert(user, prefs);
            }
            Err(reason) => error!(user, "One user's settings could not be read, so they start over: {}", reason),
        }
    }

    Ok(users)
}

impl Book {
    pub(crate) fn open() -> Self {
        Self::at(PathBuf::from(FOLDER))
    }

    fn at(folder: PathBuf) -> Self {
        let users = held(&folder).unwrap_or_else(|err| {
            error!("The user settings folder could not be read, starting with none: {}", err);
            HashMap::new()
        });

        Self { folder, users: Mutex::new(users) }
    }

    pub(crate) fn get(&self, user: u64) -> Prefs {
        self.users.lock().unwrap_or_else(PoisonError::into_inner).get(&user).cloned().unwrap_or_default()
    }

    pub(crate) fn prefix(&self, user: u64) -> Option<String> {
        self.users.lock().unwrap_or_else(PoisonError::into_inner).get(&user).and_then(|prefs| prefs.prefix.clone())
    }

    pub(crate) fn prose(&self, user: u64) -> Prose {
        self.users.lock().unwrap_or_else(PoisonError::into_inner).get(&user).map_or(Prose::Human, |prefs| prefs.prose)
    }

    pub(crate) fn aliases(&self, user: u64) -> Aliases {
        self.users.lock().unwrap_or_else(PoisonError::into_inner).get(&user).map_or_else(Aliases::default, |prefs| prefs.aliases.clone())
    }

    fn save(&self, user: u64, prefs: &Prefs) -> io::Result<()> {
        let partial = self.folder.join(format!("{user}.{PARTIAL}"));

        fs::write(&partial, serde_json::to_vec(prefs)?)?;
        fs::rename(&partial, self.folder.join(format!("{user}.{HELD}")))
    }

    pub(crate) fn update(&self, user: u64, change: impl FnOnce(&mut Prefs)) -> Prefs {
        let mut users = self.users.lock().unwrap_or_else(PoisonError::into_inner);
        let prefs = users.entry(user).or_default();
        let before = prefs.clone();

        change(prefs);

        if *prefs != before
            && let Err(err) = self.save(user, prefs)
        {
            error!(user, "One user's settings could not be saved: {}", err);
        }

        prefs.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Empty or the default itself means "back to hg!", so it is never stored.
    #[test]
    fn an_empty_prefix_is_the_default() {
        assert_eq!(prefix_from(""), Ok(None));
        assert_eq!(prefix_from("  HG! "), Ok(None));
        assert_eq!(prefix_from(" hagane "), Ok(Some("hagane".to_owned())));
        assert_eq!(prefix_from("ha gane"), Err(&prose::PREFIX_SPACED));
        assert_eq!(prefix_from("abcdefghijklmnopq"), Err(&prose::PREFIX_LONG));
        assert_eq!(Prefs::default().prefix(), "hg!");
        assert_eq!(Prefs { prefix: Some("hagane".to_owned()), ..Prefs::default() }.prefix(), "hagane");
    }

    // Any field can be missing from a saved file and any unknown one is skipped, so adding or dropping a
    // setting never costs anyone the rest of theirs.
    #[test]
    fn a_file_missing_a_field_still_loads() {
        let mut prefs = Prefs { language: "ja".to_owned(), form: 1, prose: Prose::Tsundere, ..Prefs::default() };

        prefs.aliases.add("Baha", "Bahamut Cat");

        let saved = serde_json::to_value(&prefs).unwrap();
        let fresh = serde_json::to_value(Prefs::default()).unwrap();

        for key in saved.as_object().unwrap().keys() {
            let mut trimmed = saved.clone();

            trimmed.as_object_mut().unwrap().remove(key);

            let loaded = serde_json::to_value(serde_json::from_value::<Prefs>(trimmed).unwrap()).unwrap();

            assert!(saved.as_object().unwrap().keys().all(|held| &loaded[held] == if held == key { &fresh[held] } else { &saved[held] }), "only {key} goes back to its default");
        }

        let mut grown = saved.clone();

        grown["retired"] = serde_json::json!(1);
        grown["aliases"].as_object_mut().unwrap().remove("hidden");
        assert_eq!(serde_json::from_value::<Prefs>(grown).unwrap(), prefs);
    }

    // Each user is a file of their own, so a change rewrites one file and a damaged one costs one user.
    #[test]
    fn each_user_is_saved_alone() {
        let folder = std::env::temp_dir().join(format!("hagane-users-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        let book = Book::at(folder.clone());

        book.update(7, |prefs| prefs.form = 1);
        book.update(8, |prefs| prefs.aliases.add("Baha", "Bahamut Cat"));
        book.update(9, |_| {});

        let mut files: Vec<String> = fs::read_dir(&folder).unwrap().map(|entry| entry.unwrap().file_name().into_string().unwrap()).collect();

        files.sort();
        assert_eq!(files, ["7.json", "8.json"], "someone who changed nothing is not written");

        fs::write(folder.join("7.json"), "{ not json").unwrap();

        let reopened = Book::at(folder.clone());

        assert_eq!(reopened.get(7), Prefs::default());
        assert_eq!(reopened.get(8), book.get(8));
        assert_eq!(reopened.aliases(8).count(), book.aliases(9).count() + 1);

        fs::remove_dir_all(&folder).unwrap();
    }

    // A missing form falls back to the next one down, never up.
    #[test]
    fn a_missing_form_degrades_downward() {
        let prefs = Prefs { form: 3, ..Prefs::default() };

        assert_eq!(prefs.form_from(&[true, true, true, true]), 3);
        assert_eq!(prefs.form_from(&[true, true, true, false]), 2);
        assert_eq!(prefs.form_from(&[true, true, false, false]), 1);
        assert_eq!(Prefs { form: 0, ..Prefs::default() }.form_from(&[false, true, true, false]), 1);
    }
}
