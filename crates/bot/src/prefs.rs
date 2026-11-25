use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};

use serde::{Deserialize, Serialize};
use tracing::{error, warn};

use foundry::domains::settings::lang;

use super::prose::{self, Line, Prose};
use super::render::Format;

const FILE: &str = "users.json";
const PARTIAL: &str = "users.json.partial";
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
}

impl Default for Prefs {
    fn default() -> Self {
        Self { language: ENGLISH.to_owned(), format: Format::Avif, form: TRUE_FORM, search: Search::Exact, prefix: None, prose: Prose::Human, mode: Mode::Dormant }
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
    path: PathBuf,
    users: Mutex<HashMap<u64, Prefs>>,
}

impl Book {
    pub(crate) fn open() -> Self {
        let path = PathBuf::from(FILE);
        let users = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|err| {
                error!("The user settings could not be read, starting with none: {}", err);
                HashMap::new()
            }),
            Err(_) => HashMap::new(),
        };

        Self { path, users: Mutex::new(users) }
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

    pub(crate) fn update(&self, user: u64, change: impl FnOnce(&mut Prefs)) -> Prefs {
        let mut users = self.users.lock().unwrap_or_else(PoisonError::into_inner);
        let prefs = users.entry(user).or_default();

        change(prefs);

        let changed = prefs.clone();

        match serde_json::to_vec(&*users) {
            Ok(bytes) => {
                if let Err(err) = fs::write(PARTIAL, &bytes).and_then(|()| fs::rename(PARTIAL, &self.path)) {
                    error!("The user settings could not be saved: {}", err);
                }
            }
            Err(err) => warn!("The user settings could not be encoded: {}", err),
        }

        changed
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

    // Settings saved before the prefix existed still load.
    #[test]
    fn older_settings_still_load() {
        let prefs: Prefs = serde_json::from_str(r#"{"language":"ja","format":"WebP","form":1,"search":"Preferred"}"#).unwrap();

        assert_eq!((prefs.language.as_str(), prefs.form, prefs.search, prefs.prefix, prefs.prose, prefs.mode), ("ja", 1, Search::Preferred, None, Prose::Human, Mode::Dormant));
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
