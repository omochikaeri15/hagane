use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, PoisonError, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

const FILE: &str = "wiki.json";
const PARTIAL: &str = "json.partial";
const CATS: &str = "https://battlecats.miraheze.org/w/api.php?action=bucket&format=json&query=bucket('cat').select('id','name_index').limit(5000).run()";
const ENEMIES: &str =
    "https://battlecats.miraheze.org/w/index.php?title=Module:Enemies.csv&action=raw";
const AGENT: &str = "Hagane Discord Bot";
const TIMEOUT: Duration = Duration::from_secs(60);
const FRESH: Duration = Duration::from_secs(24 * 60 * 60);
const FEWEST_CATS: usize = 500;
const FEWEST_ENEMIES: usize = 400;
const ENEMY_MARK: &str = "E-";

#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Names {
    fetched: u64,
    cats: HashMap<u32, Vec<String>>,
    enemies: HashMap<u32, String>,
}

impl Names {
    pub(crate) fn cat(&self, id: u32, form: usize) -> Option<&str> {
        self.cats
            .get(&id)?
            .get(form)
            .map(String::as_str)
            .filter(|name| !name.is_empty())
    }

    pub(crate) fn enemy(&self, id: u32) -> Option<&str> {
        self.enemies.get(&id).map(String::as_str)
    }
}

#[derive(Deserialize)]
struct Bucket {
    bucket: Vec<Row>,
}

#[derive(Deserialize)]
struct Row {
    id: u32,
    #[serde(default)]
    name_index: Vec<String>,
}

fn numbered(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|glyph| glyph.is_ascii_digit() || glyph == '-')
}

fn cats_from(text: &str) -> Result<HashMap<u32, Vec<String>>, String> {
    let rows = serde_json::from_str::<Bucket>(text)
        .map_err(|err| format!("the cat table is not what it used to be: {err}"))?
        .bucket;

    if rows.len() < FEWEST_CATS {
        return Err(format!("the cat table only has {} cats", rows.len()));
    }

    Ok(rows
        .into_iter()
        .map(|row| {
            (
                row.id,
                row.name_index
                    .into_iter()
                    .map(|name| {
                        if numbered(name.trim()) {
                            String::new()
                        } else {
                            name.trim().to_owned()
                        }
                    })
                    .collect(),
            )
        })
        .collect())
}

fn enemies_from(text: &str) -> Result<HashMap<u32, String>, String> {
    let rows: Vec<(u32, &str)> = text
        .lines()
        .filter_map(|line| {
            line.split('\t')
                .collect::<Vec<_>>()
                .get(..2)
                .and_then(|held| Some((held[0].trim().parse::<u32>().ok()?, held[1].trim())))
        })
        .collect();

    if rows.len() < FEWEST_ENEMIES {
        return Err(format!("the enemy table only has {} enemies", rows.len()));
    }

    Ok(rows
        .into_iter()
        .filter(|(_, name)| {
            !name.is_empty() && !name.strip_prefix(ENEMY_MARK).is_some_and(numbered)
        })
        .map(|(id, name)| (id, name.to_owned()))
        .collect())
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

fn page(client: &Client, link: &str) -> Result<String, String> {
    client
        .get(link)
        .send()
        .and_then(|reply| reply.error_for_status())
        .and_then(|reply| reply.text())
        .map_err(|err| err.to_string())
}

pub(crate) struct Wiki {
    path: PathBuf,
    held: RwLock<Arc<Names>>,
}

impl Wiki {
    pub(crate) fn open() -> Self {
        Self::at(PathBuf::from(FILE))
    }

    fn at(path: PathBuf) -> Self {
        let held = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|err| {
                error!(
                    "The saved wiki names could not be read, fetching them again: {}",
                    err
                );
                Names::default()
            }),
            Err(_) => Names::default(),
        };

        Self {
            path,
            held: RwLock::new(Arc::new(held)),
        }
    }

    pub(crate) fn names(&self) -> Arc<Names> {
        Arc::clone(&self.held.read().unwrap_or_else(PoisonError::into_inner))
    }

    pub(crate) fn wait(&self) -> Duration {
        FRESH.saturating_sub(Duration::from_secs(
            now().saturating_sub(self.names().fetched),
        ))
    }

    fn keep(&self, names: Names) -> io::Result<()> {
        let bytes = serde_json::to_vec(&names)?;

        *self.held.write().unwrap_or_else(PoisonError::into_inner) = Arc::new(names);
        let partial = self.path.with_extension(PARTIAL);

        fs::write(&partial, bytes)?;
        fs::rename(&partial, &self.path)
    }

    pub(crate) fn refresh(&self) -> bool {
        let client = match Client::builder().user_agent(AGENT).timeout(TIMEOUT).build() {
            Ok(client) => client,
            Err(err) => {
                error!(
                    "The wiki names could not be fetched, no http client: {}",
                    err
                );
                return false;
            }
        };
        let cats = page(&client, CATS)
            .and_then(|text| cats_from(&text))
            .inspect_err(|reason| {
                warn!(
                    "The wiki's cat names could not be fetched, keeping the ones held: {}",
                    reason
                )
            });
        let enemies = page(&client, ENEMIES)
            .and_then(|text| enemies_from(&text))
            .inspect_err(|reason| {
                warn!(
                    "The wiki's enemy names could not be fetched, keeping the ones held: {}",
                    reason
                )
            });
        let held = self.names();
        let whole = cats.is_ok() && enemies.is_ok();
        let names = Names {
            fetched: if whole { now() } else { held.fetched },
            cats: cats.unwrap_or_else(|_| held.cats.clone()),
            enemies: enemies.unwrap_or_else(|_| held.enemies.clone()),
        };

        info!(
            cats = names.cats.len(),
            enemies = names.enemies.len(),
            whole,
            "Wiki names refreshed"
        );

        if let Err(err) = self.keep(names) {
            error!(
                "The wiki names could not be saved, they will be fetched again after a restart: {}",
                err
            );
        }

        whole
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cats(extra: &str) -> String {
        let filler: Vec<String> = (1000..1000 + FEWEST_CATS)
            .map(|id| format!(r#"{{"id":{id},"name_index":["Filler {id}"]}}"#))
            .collect();

        format!(
            r#"{{"bucketQuery":"","bucket":[{extra},{}]}}"#,
            filler.join(",")
        )
    }

    // A form the wiki has not named yet is written as its ID there, and that is not a name to search by.
    #[test]
    fn cat_names_keep_their_form_and_drop_placeholders() {
        let held = cats_from(&cats(r#"{"id":54,"name_index":["Type 10","Cat-Riding Type 10"]},{"id":882,"name_index":["882-1","882-2"]},{"id":68,"name_index":["Reaper","Reaper 999"]}"#)).unwrap();
        let names = Names {
            cats: held,
            ..Names::default()
        };

        assert_eq!(
            (names.cat(54, 0), names.cat(54, 1), names.cat(54, 2)),
            (Some("Type 10"), Some("Cat-Riding Type 10"), None)
        );
        assert_eq!((names.cat(882, 0), names.cat(882, 1)), (None, None));
        assert_eq!(names.cat(68, 1), Some("Reaper 999"));
        assert_eq!(names.cat(7, 0), None);
    }

    #[test]
    fn enemy_names_drop_placeholders() {
        let filler: String = (1000..1000 + FEWEST_ENEMIES)
            .map(|id| format!("{id}\tFiller {id}\t\t1\t1\n"))
            .collect();
        let held = enemies_from(&format!("Image\tName\tLink\tHP\tAttack\n068\tMonkey King\t\t90\t8\n797\tE-797\t\t1\t1\n799\t14th Anniv. Doge Cake\t\t30\t1\n{filler}")).unwrap();
        let names = Names {
            enemies: held,
            ..Names::default()
        };

        assert_eq!(
            (names.enemy(68), names.enemy(797), names.enemy(799)),
            (Some("Monkey King"), None, Some("14th Anniv. Doge Cake"))
        );
    }

    // What was fetched is there again after a restart, so the wiki is not asked on every boot.
    #[test]
    fn names_survive_a_restart() {
        let path = std::env::temp_dir().join(format!("hagane-wiki-{}.json", std::process::id()));
        let wiki = Wiki::at(path.clone());

        assert_eq!(
            wiki.wait(),
            Duration::ZERO,
            "nothing held yet, so it is due at once"
        );

        wiki.keep(Names {
            fetched: now(),
            enemies: HashMap::from([(68, "Monkey King".to_owned())]),
            ..Names::default()
        })
        .unwrap();

        let reopened = Wiki::at(path.clone());

        assert_eq!(reopened.names().enemy(68), Some("Monkey King"));
        assert!(reopened.wait() > FRESH - Duration::from_secs(60));

        fs::remove_file(path).unwrap();
    }

    // A page that stopped being the table must not wipe the names already held.
    #[test]
    fn a_changed_page_is_refused() {
        assert!(cats_from(r#"{"error":{"code":"badvalue"}}"#).is_err());
        assert!(cats_from(r#"{"bucket":[{"id":1,"name_index":["Tank Cat"]}]}"#).is_err());
        assert!(enemies_from("no update this page bad evil").is_err());
    }
}
