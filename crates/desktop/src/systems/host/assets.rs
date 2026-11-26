use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::mem;
use std::ops::{Bound, Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use emulator::engine::{AssetSource, SheetImage};
use emulator::runtime::HostLimits;
use foundry::common::io::APP_LANGUAGES;
use foundry::domains::sandbox::stash::Stash;
use foundry::{Source, Vfs};
use codex::combat::Separator;
use rayon::prelude::*;
use tracing::{debug, info, warn};

const UNIT_STEM: &str = "unit";
const TABLE_SUFFIX: &str = ".csv";
const ENEMY_TABLE: &str = "t_unit.csv";
const TALENT_TABLE: &str = "SkillAcquisition.csv";
const TALENT_LEAD: usize = 2;
const TALENT_GROUP: usize = 14;
const EX_MAP_STEM: &str = "MapStageDataRE_";
const CUT_SUFFIX: &str = ".imgcut";
const SHEET_SUFFIX: &str = ".png";
const CUT_HEADER: usize = 4;
const TABLE_KINDS: [&str; 2] = ["csv", "tsv"];
const KEYED_TABLES: [&str; 1] = ["localizable"];
const CLOSERS: [&[u8]; 2] = [b"@", "\u{ff20}".as_bytes()];
const MOST_KIN: usize = 32;

#[derive(Clone)]
pub struct Sheet {
    pub width: u32,
    pub height: u32,
    pub pixels: Arc<[u8]>,
    pub stamp: u64,
}

static NEXT_STAMP: AtomicU64 = AtomicU64::new(1);

impl Sheet {
    pub fn new(width: u32, height: u32, pixels: &[u8]) -> Self {
        Self { width, height, pixels: Arc::from(pixels), stamp: NEXT_STAMP.fetch_add(1, Ordering::Relaxed) }
    }
}

pub(super) const WIDE_COMMA: &str = "\u{ff0c}";

#[derive(Default)]
pub struct SheetCache {
    held: BTreeMap<Box<str>, Sheet>,
    survivors: Arc<[Box<str>]>,
    census: Option<BTreeSet<Box<str>>>,
    lately: BTreeSet<Box<str>>,
}

impl Deref for SheetCache {
    type Target = BTreeMap<Box<str>, Sheet>;

    fn deref(&self) -> &Self::Target {
        &self.held
    }
}

impl DerefMut for SheetCache {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.held
    }
}

impl SheetCache {
    pub fn settle(&mut self) {
        self.survivors = self.held.keys().cloned().collect();
    }

    pub fn survivors(&self) -> &Arc<[Box<str>]> {
        &self.survivors
    }

    pub fn count(&mut self) {
        self.census = Some(BTreeSet::new());
    }

    fn ask(&mut self, name: &str) {
        let asked = self.census.as_mut().unwrap_or(&mut self.lately);

        if !asked.contains(name) {
            asked.insert(Box::from(name));
        }
    }

    pub fn prune(&mut self, keep: impl Fn(&str) -> bool) {
        let Some(census) = self.census.take() else {
            return;
        };
        let lately = mem::take(&mut self.lately);
        let before = self.held.len();

        self.held.retain(|name, _| census.contains(name) || lately.contains(name) || keep(name));

        if self.held.len() < before {
            debug!(released = before - self.held.len(), held = self.held.len(), "emu: sheets the new battle did not ask for were released");
            self.settle();
        }
    }
}

pub type FileIndex = BTreeMap<Box<str>, Source>;

fn cut_rects(bytes: &[u8]) -> Vec<[u32; 4]> {
    String::from_utf8_lossy(bytes)
        .lines()
        .skip(CUT_HEADER)
        .filter_map(|line| {
            let mut cells = line.split(',').map(|cell| cell.trim().parse::<u32>().ok());

            Some([cells.next()??, cells.next()??, cells.next()??, cells.next()??])
        })
        .collect()
}

fn table_rows(bytes: &[u8]) -> (Vec<&[u8]>, Option<&[u8]>) {
    let mut rows: Vec<&[u8]> = bytes
        .split(|&byte| byte == b'\n')
        .map(|row| row.strip_suffix(b"\r").unwrap_or(row))
        .collect();

    while rows.last().is_some_and(|row| row.is_empty()) {
        rows.pop();
    }

    let closer = rows.pop_if(|row| CLOSERS.contains(row));

    (rows, closer)
}

fn blank_row(row: &[u8]) -> bool {
    row.iter().all(|byte| byte.is_ascii_whitespace() || *byte == b'|' || *byte == b',')
}

fn row_key(row: &[u8]) -> &[u8] {
    row.split(|&byte| byte == b'\t').next().unwrap_or(row)
}

fn stripped(name: &str) -> Option<(&str, &str)> {
    let (stem, extension) = name.rsplit_once('.')?;
    let (head, code) = stem.rsplit_once('_')?;

    APP_LANGUAGES.iter().any(|&(language, _)| language == code).then_some((head, extension))
}

#[derive(Default)]
pub struct Ledger {
    armed: bool,
    seen: BTreeSet<Box<str>>,
    fresh: Vec<(Box<str>, PathBuf)>,
}

impl Ledger {
    pub fn arm(&mut self) {
        self.armed = true;
        self.seen.clear();
        self.fresh.clear();
    }

    pub fn disarm(&mut self) {
        self.armed = false;
        self.seen.clear();
        self.fresh.clear();
    }

    pub fn note(&mut self, name: &str, path: &Path) {
        if self.armed && !self.seen.contains(name) {
            self.seen.insert(Box::from(name));
            self.fresh.push((Box::from(name), path.to_path_buf()));
        }
    }

    fn wants(&self, name: &str) -> bool {
        self.armed && !self.seen.contains(name)
    }

    pub fn drain(&mut self) -> Vec<(Box<str>, PathBuf)> {
        mem::take(&mut self.fresh)
    }
}

pub type SharedLedger = Rc<RefCell<Ledger>>;

pub type SharedStash = Rc<RefCell<Stash>>;

pub struct DiskAssets {
    files: Rc<RefCell<FileIndex>>,
    sheets: Rc<RefCell<SheetCache>>,
    ledger: SharedLedger,
    stash: SharedStash,
    missing: Vec<Box<str>>,
}

impl DiskAssets {
    pub fn new(files: Rc<RefCell<FileIndex>>, sheets: Rc<RefCell<SheetCache>>, ledger: SharedLedger, stash: SharedStash) -> Self {
        Self {
            files,
            sheets,
            ledger,
            stash,
            missing: Vec::new(),
        }
    }

    fn note(&self, name: &str) {
        if !self.ledger.borrow().wants(name) {
            return;
        }

        if let Some(source) = self.resolve(name).filter(|source| !source.in_memory()) {
            self.ledger.borrow_mut().note(name, &source.path);
        }
    }

    pub fn index(vfs: &Vfs) -> FileIndex {
        let started = std::time::Instant::now();
        let listed = vfs.glob("");
        let mut files: FileIndex = listed
            .par_iter()
            .filter_map(|name| vfs.locate(name).map(|path| (name.clone(), vfs.source(&path))))
            .collect();

        let bare: Vec<Box<str>> = files
            .keys()
            .filter_map(|name| stripped(name).map(|(head, extension)| Box::from(format!("{head}.{extension}").as_str())))
            .filter(|name| !files.contains_key(name))
            .collect::<BTreeSet<Box<str>>>()
            .into_iter()
            .collect();

        let localized = bare
            .into_par_iter()
            .filter_map(|name| {
                let path = vfs.variants(&name).into_iter().find_map(|variant| vfs.locate(&variant))?;

                Some((name, vfs.source(&path)))
            })
            .collect::<Vec<_>>();

        let aliased = localized.len();

        files.extend(localized);

        info!(
            "emu: {} files resolved in {:?}, {} through a regional variant",
            files.len(),
            started.elapsed(),
            aliased
        );

        files
    }

    pub fn limits(&self) -> HostLimits {
        let units = self
            .files
            .borrow()
            .keys()
            .filter_map(|name| name.strip_prefix(UNIT_STEM)?.strip_suffix(TABLE_SUFFIX)?.parse::<usize>().ok())
            .max()
            .unwrap_or(0);
        let enemy_rows = self.table(ENEMY_TABLE).map_or(0, |bytes| bytes.split(|&byte| byte == b'\n').filter(|row| !row.is_empty()).count());
        let talent_groups = self.table(TALENT_TABLE).map_or(0, |bytes| {
            bytes
                .split(|&byte| byte == b'\n')
                .map(|row| row.split(|&byte| byte == b',').count().saturating_sub(TALENT_LEAD) / TALENT_GROUP)
                .max()
                .unwrap_or(0)
        });

        let ex_maps = self
            .files
            .borrow()
            .keys()
            .filter_map(|name| name.strip_prefix(EX_MAP_STEM)?.strip_suffix(TABLE_SUFFIX)?.parse::<usize>().ok())
            .max()
            .map_or(0, |last| last + 1);

        HostLimits { units, enemy_rows, talent_groups, ex_maps, neg5_maps: 0 }
    }

    fn resolve(&self, name: &str) -> Option<Source> {
        let files = self.files.borrow();

        if let Some(path) = files.get(name) {
            return Some(path.clone());
        }

        let (head, extension) = stripped(name)?;

        files.get(format!("{head}.{extension}").as_str()).cloned()
    }

    fn read(&self, name: &str) -> Option<Vec<u8>> {
        self.stash.borrow_mut().read(&self.resolve(name)?, &self.files.borrow()).ok().map(|bytes| bytes.to_vec())
    }

    fn warm(&self, head: &str, tail: &str) {
        if !self.stash.borrow().keeping() {
            return;
        }

        let files = self.files.borrow();
        let copies: Vec<&Source> = APP_LANGUAGES.iter().filter_map(|&(code, _)| files.get(format!("{head}_{code}{tail}").as_str())).collect();

        self.stash.borrow_mut().warm(&copies);
    }

    fn gather(&self, name: &str) {
        if !self.stash.borrow().keeping() {
            return;
        }

        let Some((stem, _)) = name.split_once('.') else {
            return;
        };
        let files = self.files.borrow();
        let kin: Vec<&Source> = files
            .range::<str, _>((Bound::Included(stem), Bound::Unbounded))
            .take_while(|(held, _)| held.starts_with(stem))
            .filter(|(held, _)| held.get(stem.len()..).and_then(|rest| rest.split_once('.')).is_some_and(|(digits, _)| digits.bytes().all(|byte| byte.is_ascii_digit())))
            .map(|(_, source)| source)
            .take(MOST_KIN)
            .collect();

        self.stash.borrow_mut().warm(&kin);
    }

    fn fuller(&self, name: &str) -> Option<(Box<str>, Vec<[u32; 4]>)> {
        let stem = name.strip_suffix(CUT_SUFFIX)?;
        let head = stripped(name).map_or(stem, |(head, _)| head);
        let own = cut_rects(&self.read(name)?);

        self.warm(head, CUT_SUFFIX);

        let files = self.files.borrow();

        APP_LANGUAGES
            .iter()
            .filter_map(|&(code, _)| {
                let variant = format!("{head}_{code}");
                let rects = cut_rects(&self.stash.borrow_mut().read(files.get(format!("{variant}{CUT_SUFFIX}").as_str())?, &files).ok()?);

                (rects.len() > own.len() && rects.starts_with(&own)).then(|| (Box::from(variant.as_str()), rects.split_at(own.len()).1.to_vec()))
            })
            .max_by_key(|(_, extra)| extra.len())
    }

    fn plain(&self, name: &str) -> Option<Vec<u8>> {
        let source = self.resolve(name)?;
        let bytes = self.stash.borrow_mut().read(&source, &self.files.borrow()).ok()?.to_vec();
        let path = &source.path;
        let piped = path
            .file_name()
            .and_then(|found| found.to_str())
            .is_some_and(|found| Separator::localized(found) == Separator::Pipe);
        let tabular = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension == "csv");

        if !piped || !tabular || !bytes.contains(&b'|') {
            return Some(bytes);
        }

        let mut rewritten = Vec::with_capacity(bytes.len());

        for byte in bytes {
            match byte {
                b',' => rewritten.extend_from_slice(WIDE_COMMA.as_bytes()),
                b'|' => rewritten.push(b','),
                other => rewritten.push(other),
            }
        }

        Some(rewritten)
    }

    fn backfill(&self, name: &str, own: Vec<u8>) -> Vec<u8> {
        let keyed = name.rsplit_once('.').filter(|(head, _)| KEYED_TABLES.contains(head));
        let Some((head, extension)) = stripped(name).or(keyed).filter(|(_, extension)| TABLE_KINDS.contains(extension)) else {
            return own;
        };

        self.warm(head, &format!(".{extension}"));

        if KEYED_TABLES.contains(&head) {
            return self.keyed(name, head, extension, own);
        }

        let (rows, closer) = table_rows(&own);
        let donors: Vec<(String, Vec<u8>)> = APP_LANGUAGES
            .iter()
            .filter_map(|&(code, _)| {
                let variant = format!("{head}_{code}.{extension}");

                if variant == name || !self.files.borrow().contains_key(variant.as_str()) {
                    return None;
                }

                self.plain(&variant).map(|bytes| (variant, bytes))
            })
            .collect();
        let donor_rows: Vec<(&str, Vec<&[u8]>)> = donors.iter().map(|(variant, bytes)| (variant.as_str(), table_rows(bytes).0)).collect();
        let longest = donor_rows.iter().map(|(_, held)| held.len()).max().unwrap_or(0).max(rows.len());
        let mut borrowed: Vec<&str> = Vec::new();
        let mut filled: Vec<u8> = Vec::with_capacity(own.len());

        for at in 0..longest {
            let held = rows.get(at).copied();
            let row = if held.is_some_and(|row| !blank_row(row)) {
                held
            } else {
                donor_rows
                    .iter()
                    .find_map(|(variant, held)| {
                        let row = held.get(at).copied().filter(|row| !blank_row(row))?;

                        if !borrowed.contains(variant) {
                            borrowed.push(variant);
                        }

                        Some(row)
                    })
                    .or(held)
                    .or_else(|| donor_rows.iter().find_map(|(_, held)| held.get(at).copied()))
            };
            let Some(row) = row else {
                continue;
            };

            if at > 0 {
                filled.push(b'\n');
            }

            filled.extend_from_slice(row);
        }

        if borrowed.is_empty() && longest == rows.len() {
            return own;
        }

        for variant in borrowed {
            self.note(variant);
        }

        if let Some(closer) = closer {
            filled.push(b'\n');
            filled.extend_from_slice(closer);
        }

        filled.push(b'\n');
        filled
    }

    fn keyed(&self, name: &str, head: &str, extension: &str, own: Vec<u8>) -> Vec<u8> {
        let (rows, closer) = table_rows(&own);
        let mut held: BTreeSet<Vec<u8>> = rows.iter().map(|row| row_key(row).to_vec()).collect();
        let mut surplus: Vec<u8> = Vec::new();

        for &(code, _) in APP_LANGUAGES {
            let variant = format!("{head}_{code}.{extension}");

            if variant == name || !self.files.borrow().contains_key(variant.as_str()) {
                continue;
            }

            let Some(bytes) = self.plain(&variant) else {
                continue;
            };
            let before = surplus.len();

            for row in table_rows(&bytes).0 {
                if !row.is_empty() && held.insert(row_key(row).to_vec()) {
                    surplus.push(b'\n');
                    surplus.extend_from_slice(row);
                }
            }

            if surplus.len() > before {
                self.note(&variant);
            }
        }

        if surplus.is_empty() {
            return own;
        }

        let mut filled: Vec<u8> = rows.join(&b'\n');

        filled.append(&mut surplus);

        if let Some(closer) = closer {
            filled.push(b'\n');
            filled.extend_from_slice(closer);
        }

        filled.push(b'\n');
        filled
    }

    fn table(&self, name: &str) -> Option<Vec<u8>> {
        if let Some((variant, _)) = self.fuller(name) {
            let fuller = format!("{variant}{CUT_SUFFIX}");

            self.note(&fuller);

            return self.read(&fuller);
        }

        self.plain(name).map(|own| self.backfill(name, own))
    }

    fn decode(&mut self, name: &str) -> Option<Sheet> {
        {
            let mut sheets = self.sheets.borrow_mut();

            sheets.ask(name);

            if let Some(sheet) = sheets.get(name) {
                return Some(sheet.clone());
            }
        }

        let image = self.read(name).and_then(|bytes| {
            image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
                .inspect_err(|error| warn!("emu: {name} failed to decode: {error}"))
                .ok()
        })?;
        let mut decoded = image.to_rgba8();

        for pixel in decoded.pixels_mut() {
            let alpha = u32::from(pixel.0[3]);

            for channel in &mut pixel.0[..3] {
                *channel = (u32::from(*channel) * alpha / 0xff) as u8;
            }
        }

        let (width, height) = (decoded.width(), decoded.height());
        let patch = name
            .strip_suffix(SHEET_SUFFIX)
            .and_then(|stem| self.fuller(&format!("{stem}{CUT_SUFFIX}")))
            .and_then(|(variant, extra)| {
                let fuller = format!("{variant}{SHEET_SUFFIX}");

                self.note(&fuller);

                let bytes = self.read(&fuller)?;
                let donor = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png).ok()?.to_rgba8();

                (donor.width() == width && donor.height() == height).then_some((donor, extra))
            });

        if let Some((donor, extra)) = patch {
            for [left, top, wide, tall] in extra {
                for y in top..top.saturating_add(tall).min(height) {
                    for x in left..left.saturating_add(wide).min(width) {
                        let mut pixel = *donor.get_pixel(x, y);
                        let alpha = u32::from(pixel.0[3]);

                        for channel in &mut pixel.0[..3] {
                            *channel = (u32::from(*channel) * alpha / 0xff) as u8;
                        }

                        decoded.put_pixel(x, y, pixel);
                    }
                }
            }
        }

        let sheet = Sheet::new(width, height, decoded.into_raw().as_slice());

        self.sheets
            .borrow_mut()
            .insert(Box::from(name), sheet.clone());

        Some(sheet)
    }
}

impl AssetSource for DiskAssets {
    fn open(&mut self, name: &[u8], _packed: u8, _encrypted: u8) -> Option<Vec<u8>> {
        let name = std::str::from_utf8(name).ok()?;

        self.note(name);
        self.gather(name);

        let found = self.table(name);

        if found.is_none() && !self.missing.iter().any(|seen| seen.as_ref() == name) {
            debug!("emu: no file named {name}");
            self.missing.push(Box::from(name));
        }

        found
    }

    fn load_png(&mut self, name: &[u8]) -> Option<SheetImage> {
        let name = std::str::from_utf8(name).ok()?;

        self.note(name);
        self.gather(name);

        let sheet = self.decode(name)?;

        Some(SheetImage {
            width: sheet.width as i32,
            height: sheet.height as i32,
            whole: 0,
        })
    }

    fn upload(&mut self, name: &[u8]) {
        if let Ok(name) = std::str::from_utf8(name) {
            self.note(name);
            self.gather(name);
            self.decode(name);
        }
    }

    fn pack_entry(&mut self, _name: &[u8]) -> Vec<u8> {
        Vec::new()
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    fn sheet() -> Sheet {
        Sheet { width: 1, height: 1, pixels: Arc::from([0u8; 4].as_slice()), stamp: NEXT_STAMP.fetch_add(1, Ordering::Relaxed) }
    }

    fn held(cache: &SheetCache) -> Vec<&str> {
        cache.keys().map(AsRef::as_ref).collect()
    }

    #[test]
    fn a_new_stage_keeps_what_it_shares_and_what_the_last_battle_loaded_late() {
        let mut cache = SheetCache::default();

        cache.count();

        for name in ["shared.png", "old_stage.png"] {
            cache.ask(name);
            cache.insert(Box::from(name), sheet());
        }

        cache.prune(|_| false);

        // The pause dialog only loads mid-battle, so no load ever asks for it.
        cache.ask("pause.png");
        cache.insert(Box::from("pause.png"), sheet());

        cache.count();
        cache.ask("shared.png");
        cache.ask("new_stage.png");
        cache.insert(Box::from("new_stage.png"), sheet());
        cache.prune(|_| false);

        assert_eq!(held(&cache), ["new_stage.png", "pause.png", "shared.png"]);

        // A battle that never paused lets the dialog go at the next load.
        cache.count();
        cache.ask("shared.png");
        cache.ask("new_stage.png");
        cache.prune(|_| false);

        assert_eq!(held(&cache), ["new_stage.png", "shared.png"]);
    }
}
