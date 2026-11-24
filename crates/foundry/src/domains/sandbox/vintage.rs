use std::cmp::Ordering;
use std::fs;

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::common::io::json;

const FILE: &str = "vintage.json";

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, Eq)]
#[serde(default)]
pub struct Vintage {
    highest: Option<String>,
    cleared: Option<(String, String)>,
}

fn parts(version: &str) -> Option<Vec<u32>> {
    let numbered: Option<Vec<u32>> = version.trim().trim_start_matches(['v', 'V']).split('.').map(|part| part.parse().ok()).collect();

    numbered.filter(|parts| !parts.is_empty())
}

fn compared(left: &str, right: &str) -> Option<Ordering> {
    let (left, right) = (parts(left)?, parts(right)?);
    let at = |held: &[u32], place: usize| held.get(place).copied().unwrap_or(0);

    Some((0..left.len().max(right.len())).map(|place| at(&left, place).cmp(&at(&right, place))).find(|order| order.is_ne()).unwrap_or(Ordering::Equal))
}

impl Vintage {
    pub fn raised<'a>(mut self, names: impl IntoIterator<Item = &'a str>) -> Self {
        for name in names {
            let name = name.trim().trim_start_matches(['v', 'V']);
            let higher = self.highest.as_deref().is_none_or(|held| compared(name, held) == Some(Ordering::Greater));

            if parts(name).is_some() && higher {
                self.highest = Some(name.to_owned());
            }
        }

        self
    }

    pub fn stale(&self, replicated: &str) -> Option<&str> {
        let highest = self.highest.as_deref()?;
        let cleared = self.cleared.as_ref().is_some_and(|(files, game)| files == highest && game == replicated);

        (compared(highest, replicated) == Some(Ordering::Less) && !cleared).then_some(highest)
    }

    pub fn cleared(mut self, replicated: &str) -> Self {
        self.cleared = self.highest.clone().map(|highest| (highest, replicated.to_owned()));
        self
    }
}

fn load() -> Vintage {
    json::load_state(FILE).unwrap_or_default()
}

fn save(vintage: &Vintage) {
    if let Err(err) = json::save_state(FILE, vintage) {
        warn!("The highest imported game version could not be saved: {}", err);
    }
}

pub fn note<'a>(names: impl IntoIterator<Item = &'a str>) {
    let held = load();
    let raised = held.clone().raised(names);

    if raised != held {
        info!(version = raised.highest.as_deref().unwrap_or_default(), "A higher game version was imported");
        save(&raised);
    }
}

pub(crate) fn forget() {
    if let Some(path) = json::state_file(FILE)
        && path.exists()
        && let Err(err) = fs::remove_file(&path)
    {
        warn!("The highest imported game version could not be forgotten: {}", err);
    }
}

pub fn stale(replicated: &str) -> Option<String> {
    load().stale(replicated).map(str::to_owned)
}

pub fn clear(replicated: &str) {
    save(&load().cleared(replicated));
}

#[cfg(test)]
mod tests {
    use super::*;

    // The highest version ever imported is kept, whatever order the regions arrive in, and an import that names no version changes nothing.
    #[test]
    fn the_highest_imported_version_is_kept() {
        let held = Vintage::default().raised(["15.7.1", "16.0.3", "v15.10.0", "", "Varies with device"]);

        assert_eq!(held.highest.as_deref(), Some("16.0.3"));
        assert_eq!(held.clone().raised(["15.7.2"]).highest.as_deref(), Some("16.0.3"));
        assert_eq!(Vintage::default().raised(["15.9.2", "15.10.0"]).highest.as_deref(), Some("15.10.0"), "by number, not by text");
        assert_eq!(Vintage::default().raised(["", "unknown"]), Vintage::default());
    }

    // Files behind the game ask once; Ignore quiets that exact pair, and a newer import or a newer game asks again.
    #[test]
    fn older_files_ask_until_that_pair_is_ignored() {
        let held = Vintage::default().raised(["15.7.1"]);

        assert_eq!(held.stale("16.0.0"), Some("15.7.1"));
        assert_eq!(held.stale("15.7.1"), None);
        assert_eq!(held.stale("15.7"), None, "a version at or past the game's never asks");
        assert_eq!(Vintage::default().stale("16.0.0"), None, "an import that named no version never asks");

        let ignored = held.cleared("16.0.0");

        assert_eq!(ignored.stale("16.0.0"), None);
        assert_eq!(ignored.stale("16.1.0"), Some("15.7.1"), "a newer game asks again");
        assert_eq!(ignored.clone().raised(["15.8.0"]).stale("16.0.0"), Some("15.8.0"), "and so does a newer import that is still behind");
        assert_eq!(ignored.raised(["16.0.3"]).stale("16.0.0"), None);
    }
}
