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
const AGENT: &str = concat!("BattleCatsComplete/", env!("CARGO_PKG_VERSION"));
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

#[derive(Clone, Debug, Deserialize)]
pub struct Asset {
    pub name: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Release {
    pub tag_name: String,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub assets: Vec<Asset>,
}

impl Release {
    pub fn version(&self) -> &str {
        self.tag_name.trim_start_matches('v')
    }

    pub fn is_versioned(&self) -> bool {
        let version = self.version();
        !version.is_empty() && version.chars().all(|c| c.is_ascii_digit() || c == '.')
    }

    pub fn has_asset(&self, name: &str) -> bool {
        self.assets.iter().any(|asset| asset.name == name)
    }
}

pub fn latest_release(owner: &str, repo: &str) -> Result<Option<Release>, Error> {
    let client = client()?;
    let url = format!("{}/repos/{}/{}/releases/latest", API_ROOT, owner, repo);
    let response = send(&client, &url)?;

    if response.status() == StatusCode::NOT_FOUND {
        debug!("{}/{} has no published releases", owner, repo);
        return Ok(None);
    }

    let body = response
        .text()
        .map_err(|err| Error::new(ErrorKind::Network, format!("could not read the latest release: {}", err)))?;

    serde_json::from_str(&body)
        .map(Some)
        .map_err(|err| Error::new(ErrorKind::Malformed, format!("could not parse the latest release: {}", err)))
}

pub fn latest_tag(owner: &str, repo: &str) -> Result<Option<String>, Error> {
    let url = format!("{}/{}/{}/releases/latest", WEB_ROOT, owner, repo);
    let response = probe(&url)?;
    let status = response.status();

    if status == StatusCode::NOT_FOUND {
        return Err(Error::new(ErrorKind::InvalidUrl, format!("GitHub has no release page at {}", url)));
    }

    if !status.is_redirection() {
        return Err(Error::new(ErrorKind::Malformed, format!("{} answered {} instead of pointing at a release", url, status)));
    }

    Ok(response.headers().get(LOCATION).and_then(|location| location.to_str().ok()).and_then(tag_of))
}

pub fn has_download(owner: &str, repo: &str, tag: &str, asset: &str) -> Result<bool, Error> {
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

pub fn list_releases(owner: &str, repo: &str) -> Result<Vec<Release>, Error> {
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

pub fn newer(current: &str, candidate: &str) -> bool {
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

    #[test]
    fn the_latest_redirect_names_its_tag() {
        assert_eq!(tag_of("https://github.com/owner/repo/releases/tag/v4.1.2"), Some("v4.1.2".to_owned()));
        assert_eq!(tag_of("https://github.com/owner/repo/releases/tag/v4.1.2/"), Some("v4.1.2".to_owned()));
    }

    // A repo with no releases bounces back to the listing, which names no tag at all.
    #[test]
    fn a_redirect_without_a_tag_means_no_release() {
        assert_eq!(tag_of("https://github.com/owner/repo/releases"), None);
        assert_eq!(tag_of("https://github.com/owner/repo/releases/tag/"), None);
    }
}
