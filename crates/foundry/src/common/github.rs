use std::fmt;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::blocking::{Client, Response};
use reqwest::header::LOCATION;
use reqwest::redirect::Policy;
use reqwest::StatusCode;
use serde::Deserialize;
use tracing::{debug, warn};

use super::http;

const API_ROOT: &str = "https://api.github.com";
const WEB_ROOT: &str = "https://github.com";
const TAG_PATH: &str = "/releases/tag/";
const AGENT: &str = concat!("Hagane/", env!("CARGO_PKG_VERSION"));
const PER_PAGE: usize = 100;
const MAX_PAGES: usize = 200;
const TIMEOUT: Duration = Duration::from_secs(20);
const CHUNK: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    RateLimited,
    InvalidUrl,
    Network,
    Malformed,
    Disk,
}

#[derive(Clone, Debug)]
pub struct Error {
    pub kind: ErrorKind,
    message: String,
}

impl Error {
    fn new(kind: ErrorKind, message: String) -> Self {
        Self { kind, message }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Target {
    pub asset: &'static str,
    pub(super) former: &'static [&'static str],
}

pub const TARGET: Target = if cfg!(target_os = "android") {
    Target { asset: "hagane-android-apk.zip", former: &[] }
} else if cfg!(target_os = "ios") {
    Target { asset: "hagane-ios-ipa.zip", former: &[] }
} else if cfg!(target_os = "windows") {
    Target { asset: "hagane-windows-x64.zip", former: &["bcc_windows_x64.zip", "bcc_windows.zip"] }
} else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
    Target { asset: "hagane-mac-arm.zip", former: &["bcc_mac_arm.zip", "bcc_mac.zip"] }
} else if cfg!(target_os = "macos") {
    Target { asset: "hagane-mac-x64.zip", former: &["bcc_mac_x64.zip", "bcc_mac_intel.zip"] }
} else if cfg!(target_arch = "aarch64") {
    Target { asset: "hagane-linux-arm.zip", former: &["bcc_linux_arm.zip"] }
} else {
    Target { asset: "hagane-linux-x64.zip", former: &["bcc_linux_x64.zip", "bcc_linux.zip"] }
};

#[derive(Clone, Debug)]
pub struct Offering {
    pub version: String,
    pub tag: String,
}

#[derive(Clone, Debug, Deserialize)]
struct Asset {
    name: String,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct Release {
    tag_name: String,
    #[serde(default)]
    pub(super) body: Option<String>,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<Asset>,
}

impl Release {
    pub(super) fn version(&self) -> &str {
        self.tag_name.trim_start_matches('v')
    }

    pub(super) fn is_versioned(&self) -> bool {
        let version = self.version();
        !version.is_empty() && version.chars().all(|c| c.is_ascii_digit() || c == '.')
    }

    fn has_asset(&self, name: &str) -> bool {
        self.assets.iter().any(|asset| asset.name == name)
    }

    pub(super) fn offers(&self, target: &Target) -> bool {
        self.has_asset(target.asset) || target.former.iter().any(|name| self.has_asset(name))
    }

    fn upgrades(&self, current: &str) -> bool {
        self.is_versioned() && !self.prerelease && newer(current, self.version())
    }
}

pub fn newest_offering(owner: &str, repo: &str, current: &str) -> Result<Option<Offering>, Error> {
    let tag = match latest_tag(owner, repo) {
        Ok(tag) => tag,
        Err(err) => {
            warn!("The latest release page could not be read, listing every release instead: {}", err);
            return listed_offering(owner, repo, current);
        }
    };
    let latest = Release { tag_name: tag, body: None, prerelease: false, assets: Vec::new() };

    if latest.is_versioned() && !newer(current, latest.version()) {
        return Ok(None);
    }

    if latest.upgrades(current) {
        match has_download(owner, repo, &latest.tag_name, TARGET.asset) {
            Ok(true) => return Ok(Some(Offering { version: latest.version().to_owned(), tag: latest.tag_name })),
            Ok(false) => debug!("{} ships nothing for {}, listing every release", latest.tag_name, TARGET.asset),
            Err(err) => warn!("{} could not be probed for {}, listing every release instead: {}", latest.tag_name, TARGET.asset, err),
        }
    }

    listed_offering(owner, repo, current)
}

fn listed_offering(owner: &str, repo: &str, current: &str) -> Result<Option<Offering>, Error> {
    let releases = list_releases(owner, repo)?;

    Ok(best_offering(&releases, current, TARGET.asset))
}

fn best_offering(releases: &[Release], current: &str, asset: &str) -> Option<Offering> {
    releases
        .iter()
        .filter(|release| release.upgrades(current) && release.has_asset(asset))
        .reduce(|held, release| if newer(held.version(), release.version()) { release } else { held })
        .map(|release| Offering { version: release.version().to_owned(), tag: release.tag_name.clone() })
}

fn latest_tag(owner: &str, repo: &str) -> Result<String, Error> {
    let url = format!("{}/{}/{}/releases/latest", WEB_ROOT, owner, repo);
    let response = probe(&url)?;
    let status = response.status();

    if status == StatusCode::NOT_FOUND {
        return Err(Error::new(ErrorKind::InvalidUrl, format!("GitHub has no release page at {}", url)));
    }

    if !status.is_redirection() {
        return Err(Error::new(ErrorKind::Malformed, format!("{} answered {} instead of pointing at a release", url, status)));
    }

    response
        .headers()
        .get(LOCATION)
        .and_then(|location| location.to_str().ok())
        .and_then(tag_of)
        .ok_or_else(|| Error::new(ErrorKind::Malformed, format!("{} redirected somewhere that names no release", url)))
}

fn has_download(owner: &str, repo: &str, tag: &str, asset: &str) -> Result<bool, Error> {
    let url = format!("{}/{}/{}/releases/download/{}/{}", WEB_ROOT, owner, repo, tag, asset);
    let response = probe(&url)?;

    match response.status() {
        StatusCode::NOT_FOUND => Ok(false),
        status if status.is_redirection() || status.is_success() => Ok(true),
        status => Err(Error::new(ErrorKind::Network, format!("{} answered {}", url, status))),
    }
}

fn tag_of(location: &str) -> Option<String> {
    let (_, tag) = location.split_once(TAG_PATH)?;
    let tag = tag.trim_end_matches('/');

    (!tag.is_empty()).then(|| tag.to_owned())
}

fn probe(url: &str) -> Result<Response, Error> {
    let client = http::builder()
        .user_agent(AGENT)
        .timeout(TIMEOUT)
        .redirect(Policy::none())
        .build()
        .map_err(|err| Error::new(ErrorKind::Network, format!("could not build the http client: {}", err)))?;

    let response = client.head(url).send().map_err(|err| {
        let kind = if err.is_builder() { ErrorKind::InvalidUrl } else { ErrorKind::Network };
        Error::new(kind, format!("request to GitHub failed: {}", err))
    })?;

    if response.status() == StatusCode::TOO_MANY_REQUESTS {
        return Err(Error::new(ErrorKind::RateLimited, format!("GitHub is throttling requests to {}", url)));
    }

    Ok(response)
}

pub(super) fn list_releases(owner: &str, repo: &str) -> Result<Vec<Release>, Error> {
    let client = client()?;
    let url = format!("{}/repos/{}/{}/releases", API_ROOT, owner, repo);
    let mut releases = Vec::new();

    for page in 1..=MAX_PAGES {
        let batch = fetch_page(&client, &url, page)?;
        let is_last = batch.len() < PER_PAGE;
        releases.extend(batch);

        if is_last {
            debug!("Listed {} releases for {}/{} over {} page(s)", releases.len(), owner, repo, page);
            return Ok(releases);
        }
    }

    warn!("Stopped listing {}/{} releases at the {} page ceiling", owner, repo, MAX_PAGES);
    Ok(releases)
}

fn fetch_page(client: &Client, url: &str, page: usize) -> Result<Vec<Release>, Error> {
    let paged = format!("{}?per_page={}&page={}", url, PER_PAGE, page);
    let response = send(client, &paged)?;

    if response.status() == StatusCode::NOT_FOUND {
        return Err(Error::new(ErrorKind::InvalidUrl, format!("GitHub has no release listing at {}", url)));
    }

    let body = response
        .text()
        .map_err(|err| Error::new(ErrorKind::Network, format!("could not read release page {}: {}", page, err)))?;

    serde_json::from_str(&body)
        .map_err(|err| Error::new(ErrorKind::Malformed, format!("could not parse release page {}: {}", page, err)))
}

fn client() -> Result<Client, Error> {
    http::builder()
        .user_agent(AGENT)
        .timeout(TIMEOUT)
        .build()
        .map_err(|err| Error::new(ErrorKind::Network, format!("could not build the http client: {}", err)))
}

fn send(client: &Client, url: &str) -> Result<Response, Error> {
    let response = client
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .map_err(|err| {
            let kind = if err.is_builder() { ErrorKind::InvalidUrl } else { ErrorKind::Network };
            Error::new(kind, format!("request to GitHub failed: {}", err))
        })?;

    let status = response.status();

    if status.is_success() || status == StatusCode::NOT_FOUND {
        return Ok(response);
    }

    if is_rate_limited(&response) {
        return Err(Error::new(ErrorKind::RateLimited, rate_limit_message(&response)));
    }

    Err(Error::new(ErrorKind::Network, format!("GitHub answered {}", status)))
}

fn is_rate_limited(response: &Response) -> bool {
    if !matches!(response.status(), StatusCode::FORBIDDEN | StatusCode::TOO_MANY_REQUESTS) {
        return false;
    }

    response
        .headers()
        .get("x-ratelimit-remaining")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|remaining| remaining == "0")
}

fn rate_limit_message(response: &Response) -> String {
    let seconds = response
        .headers()
        .get("x-ratelimit-reset")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .zip(SystemTime::now().duration_since(UNIX_EPOCH).ok())
        .map(|(reset, now)| reset.saturating_sub(now.as_secs()));

    seconds.map_or_else(
        || "GitHub rate limit reached (60 requests an hour without a token)".to_string(),
        |seconds| {
            format!(
                "GitHub rate limit reached (60 requests an hour without a token); it clears in {}m {}s",
                seconds / 60,
                seconds % 60
            )
        },
    )
}

pub fn release_page(owner: &str, repo: &str, tag: &str) -> String {
    format!("{WEB_ROOT}/{owner}/{repo}{TAG_PATH}{tag}")
}

fn newer(current: &str, candidate: &str) -> bool {
    let parts = |version: &str| -> Vec<u64> { version.trim_start_matches('v').split('.').map(|part| part.trim().parse().unwrap_or(0)).collect() };
    let held = parts(current);
    let offered = parts(candidate);
    let width = held.len().max(offered.len());
    let at = |list: &[u64], index: usize| list.get(index).copied().unwrap_or(0);

    (0..width).find_map(|index| (at(&held, index) != at(&offered, index)).then(|| at(&offered, index) > at(&held, index))).unwrap_or(false)
}

pub fn download_asset(owner: &str, repo: &str, tag: &str, asset: &str, into: &mut impl Write, progress: &dyn Fn(u64, Option<u64>)) -> Result<(), Error> {
    let url = format!("{WEB_ROOT}/{owner}/{repo}/releases/download/{tag}/{asset}");
    let client = http::builder()
        .user_agent(AGENT)
        .connect_timeout(TIMEOUT)
        .build()
        .map_err(|err| Error::new(ErrorKind::Network, format!("could not build the http client: {}", err)))?;
    let mut response = client.get(&url).send().map_err(|err| Error::new(ErrorKind::Network, format!("the download of {asset} failed: {err}")))?;
    let status = response.status();

    if status == StatusCode::NOT_FOUND {
        return Err(Error::new(ErrorKind::InvalidUrl, format!("{tag} has no asset named {asset}")));
    }

    if !status.is_success() {
        return Err(Error::new(ErrorKind::Network, format!("GitHub answered {status} for {asset}")));
    }

    let total = response.content_length();
    let mut done = 0u64;
    let mut chunk = vec![0u8; CHUNK];

    progress(0, total);

    loop {
        let read = response.read(&mut chunk).map_err(|err| Error::new(ErrorKind::Network, format!("the download of {asset} was cut short: {err}")))?;

        if read == 0 {
            break;
        }

        into.write_all(&chunk[..read]).map_err(|err| Error::new(ErrorKind::Disk, format!("{asset} could not be written: {err}")))?;
        done += read as u64;
        progress(done, total);
    }

    into.flush().map_err(|err| Error::new(ErrorKind::Disk, format!("{asset} could not be written: {err}")))?;

    Ok(())
}

pub fn extract_asset(archive: &Path, extension: &str, into: &Path) -> Result<PathBuf, Error> {
    let file = fs::File::open(archive).map_err(|err| Error::new(ErrorKind::Disk, format!("{} could not be opened: {err}", archive.display())))?;
    let mut zipped = zip::ZipArchive::new(file).map_err(|err| Error::new(ErrorKind::Malformed, format!("{} is not a zip archive: {err}", archive.display())))?;
    let wanted = (0..zipped.len())
        .find(|&index| zipped.by_index(index).ok().is_some_and(|entry| entry.is_file() && Path::new(entry.name()).extension().is_some_and(|held| held == extension)))
        .ok_or_else(|| Error::new(ErrorKind::Malformed, format!("{} holds no .{extension} file", archive.display())))?;
    let mut entry = zipped.by_index(wanted).map_err(|err| Error::new(ErrorKind::Malformed, format!("{} could not be read: {err}", archive.display())))?;
    let name = Path::new(entry.name()).file_name().map(PathBuf::from).ok_or_else(|| Error::new(ErrorKind::Malformed, "the archived file has no name".to_string()))?;
    let target = into.join(name);
    let mut out = fs::File::create(&target).map_err(|err| Error::new(ErrorKind::Disk, format!("{} could not be created: {err}", target.display())))?;

    std::io::copy(&mut entry, &mut out).map_err(|err| Error::new(ErrorKind::Disk, format!("{} could not be extracted: {err}", target.display())))?;

    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str, assets: &[&str]) -> Release {
        Release { tag_name: tag.to_owned(), body: None, prerelease: false, assets: assets.iter().map(|name| Asset { name: (*name).to_owned() }).collect() }
    }

    const MINE: &str = "hagane-mine.zip";
    const OTHER: &str = "hagane-other.zip";

    #[test]
    fn offers_the_newest_release_with_this_target() {
        let releases = [release("v3.0.0", &[MINE]), release("v2.9.0", &[MINE])];
        let offered = best_offering(&releases, "2.8.0", MINE).unwrap();

        assert_eq!(offered.tag, "v3.0.0");
        assert_eq!(offered.version, "3.0.0");
    }

    // A newer release that only shipped another front end must not hide the one that shipped ours.
    #[test]
    fn skips_a_newer_release_without_this_target() {
        let releases = [release("v3.0.0", &[OTHER]), release("v2.9.0", &[MINE, OTHER])];

        assert_eq!(best_offering(&releases, "2.8.0", MINE).unwrap().tag, "v2.9.0");
    }

    #[test]
    fn offers_nothing_when_only_other_targets_moved_on() {
        let releases = [release("v3.0.0", &[OTHER]), release("v2.8.0", &[MINE]), release("v2.7.0", &[MINE])];

        assert!(best_offering(&releases, "2.8.0", MINE).is_none());
    }

    // GitHub lists by date, so a hotfix cut for an old line can come first.
    #[test]
    fn picks_by_version_and_not_by_listing_order() {
        let releases = [release("v2.9.1", &[MINE]), release("v3.0.0", &[MINE]), release("v2.9.0", &[MINE])];

        assert_eq!(best_offering(&releases, "2.8.0", MINE).unwrap().tag, "v3.0.0");
    }

    #[test]
    fn ignores_prereleases_and_unversioned_tags() {
        let mut beta = release("v3.0.0", &[MINE]);
        beta.prerelease = true;
        let releases = [beta, release("tools", &[MINE]), release("v2.9.0", &[MINE])];

        assert_eq!(best_offering(&releases, "2.8.0", MINE).unwrap().tag, "v2.9.0");
    }

    #[test]
    fn a_former_asset_name_counts_as_offering_the_target() {
        let target = Target { asset: MINE, former: &["bcc_mine.zip"] };

        assert!(release("v4.0.0", &["bcc_mine.zip"]).offers(&target));
        assert!(!release("v4.0.0", &["bcc_other.zip"]).offers(&target));
        assert!(!release("v0.2.0", &[]).offers(&target));
    }

    #[test]
    fn the_latest_redirect_names_its_tag() {
        assert_eq!(tag_of("https://github.com/owner/repo/releases/tag/v4.1.2"), Some("v4.1.2".to_owned()));
        assert_eq!(tag_of("https://github.com/owner/repo/releases/tag/v4.1.2/"), Some("v4.1.2".to_owned()));
    }

    // An empty repo bounces to its listing and a renamed one to its new name, so the updater asks the API.
    #[test]
    fn a_redirect_without_a_tag_names_nothing() {
        assert_eq!(tag_of("https://github.com/owner/repo/releases"), None);
        assert_eq!(tag_of("https://github.com/owner/repo/releases/tag/"), None);
        assert_eq!(tag_of("https://github.com/owner/renamed/releases/latest"), None);
    }
}
