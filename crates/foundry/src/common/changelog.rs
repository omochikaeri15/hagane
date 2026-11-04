use tracing::{debug, warn};

use super::github;
use super::io::cache::{self, CacheSpec};

const UNKEYED: u64 = 0;

struct Cache;

impl CacheSpec for Cache {
    type Data = Vec<(String, String)>;
    const FILE: &'static str = "changelog.bin";
}

pub fn load(owner: &str, repo: &str, current_version: &str) -> Result<Vec<(String, String)>, github::Error> {
    let Some((_, cached)) = cache::read::<Cache>() else {
        return github::list_releases(owner, repo).map(store);
    };

    if cached.iter().any(|(version, _)| version == current_version) {
        debug!("Serving {} changelog entries from the cache", cached.len());
        return Ok(cached);
    }

    match github::list_releases(owner, repo) {
        Ok(releases) => Ok(store(releases)),
        Err(err) => {
            warn!("Serving the stale cached changelog, GitHub was unreachable: {}", err);
            Ok(cached)
        }
    }
}

fn store(releases: Vec<github::Release>) -> Vec<(String, String)> {
    let entries = entries(releases, &github::TARGET);

    cache::write::<Cache>(UNKEYED, &entries);
    entries
}

fn entries(releases: Vec<github::Release>, target: &github::Target) -> Vec<(String, String)> {
    releases
        .into_iter()
        .filter(|release| release.is_versioned() && release.offers(target))
        .map(|release| {
            let version = release.version().to_string();
            let notes = release
                .body
                .filter(|body| !body.trim().is_empty())
                .unwrap_or_else(|| "No notes.".to_string());

            (version, notes)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_release_without_this_target_has_no_entry() {
        let target = github::Target { asset: "hagane-mine.zip", former: &["bcc_mine.zip"] };
        let releases: Vec<github::Release> = serde_json::from_str(
            r#"[
                {"tag_name": "v3.0.0", "body": "other only", "assets": [{"name": "hagane-other.zip"}]},
                {"tag_name": "v2.9.0", "body": "ours", "assets": [{"name": "hagane-mine.zip"}]},
                {"tag_name": "v2.8.0", "body": "  ", "assets": [{"name": "bcc_mine.zip"}]},
                {"tag_name": "v0.2.0", "body": "no builds at all", "assets": []},
                {"tag_name": "tools", "body": "not a version", "assets": [{"name": "hagane-mine.zip"}]}
            ]"#,
        )
        .unwrap();

        assert_eq!(entries(releases, &target), [("2.9.0".to_owned(), "ours".to_owned()), ("2.8.0".to_owned(), "No notes.".to_owned())]);
    }
}
