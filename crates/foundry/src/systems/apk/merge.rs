mod manifest;
mod pool;
mod table;

use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use tracing::{debug, info, warn};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use manifest::Identity;
use table::Table;

const EXTENSIONS: [&str; 3] = ["xapk", "apkm", "apks"];
const APK: &str = ".apk";
const MANIFEST: &str = "AndroidManifest.xml";
const TABLE: &str = "resources.arsc";
const SIGNATURES: &str = "META-INF/";
const SIGNATURE_ENDS: [&str; 5] = [".SF", ".RSA", ".DSA", ".EC", "MANIFEST.MF"];
const STAMP: &str = "stamp-cert-sha256";
const CODE: &str = "classes";
const DEX: &str = ".dex";
const LIBRARY: &str = ".so";
const PARTS: &str = "parts";
const PAGE: u16 = 16384;
const WORD: u16 = 4;
const TRUNCATED: &str = "A resource file is cut short";

fn half(bytes: &[u8], at: usize) -> Result<u16, String> {
    bytes.get(at..).and_then(|held| held.first_chunk()).copied().map(u16::from_le_bytes).ok_or_else(|| TRUNCATED.to_string())
}

fn word(bytes: &[u8], at: usize) -> Result<u32, String> {
    bytes.get(at..).and_then(|held| held.first_chunk()).copied().map(u32::from_le_bytes).ok_or_else(|| TRUNCATED.to_string())
}

fn span(bytes: &[u8], at: usize) -> Result<usize, String> {
    usize::try_from(word(bytes, at)?).map_err(|_| TRUNCATED.to_string())
}

fn put_half(bytes: &mut [u8], at: usize, value: u16) -> Result<(), String> {
    let held: &mut [u8; 2] = bytes.get_mut(at..).and_then(|held| held.first_chunk_mut()).ok_or_else(|| TRUNCATED.to_string())?;

    *held = value.to_le_bytes();

    Ok(())
}

fn put_word(bytes: &mut [u8], at: usize, value: u32) -> Result<(), String> {
    let held: &mut [u8; 4] = bytes.get_mut(at..).and_then(|held| held.first_chunk_mut()).ok_or_else(|| TRUNCATED.to_string())?;

    *held = value.to_le_bytes();

    Ok(())
}

fn fit(value: usize) -> Result<u32, String> {
    u32::try_from(value).map_err(|_| "A resource file grew past what its format can hold".to_string())
}

struct Part {
    archive: ZipArchive<File>,
    identity: Identity,
}

pub(crate) fn is_split(path: &Path) -> bool {
    path.extension().and_then(OsStr::to_str).is_some_and(|held| EXTENSIONS.iter().any(|known| held.eq_ignore_ascii_case(known)))
}

pub(crate) fn run(bundle: &Path, output: &Path, log: &impl Fn(String)) -> Result<(), String> {
    info!("Merging split APKs: {:?} -> {:?}", bundle, output);

    let scratch = output.with_extension(PARTS);
    let _ = fs::remove_dir_all(&scratch);

    fs::create_dir_all(&scratch).map_err(|error| format!("Could not create a folder for the split APKs: {error}"))?;

    let outcome = unpack(bundle, &scratch).and_then(|paths| assemble(&paths, output, log));
    let _ = fs::remove_dir_all(&scratch);

    if outcome.is_err() {
        let _ = fs::remove_file(output);
    }

    outcome
}

fn unpack(bundle: &Path, scratch: &Path) -> Result<Vec<PathBuf>, String> {
    let file = File::open(bundle).map_err(|error| format!("Failed to open {}: {}", bundle.display(), error))?;
    let mut archive = ZipArchive::new(file).map_err(|_| format!("{} is not a valid xapk/apkm/apks archive.", bundle.display()))?;
    let mut paths = Vec::new();

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| error.to_string())?;

        if entry.is_dir() || !entry.name().to_ascii_lowercase().ends_with(APK) {
            continue;
        }

        let path = scratch.join(format!("{}{}", paths.len(), APK));
        let mut held = File::create(&path).map_err(|error| format!("Could not unpack {}: {}", entry.name(), error))?;

        io::copy(&mut entry, &mut held).map_err(|error| format!("Could not unpack {}: {}", entry.name(), error))?;
        debug!("Unpacked {} to {:?}", entry.name(), path);
        paths.push(path);
    }

    if paths.is_empty() {
        return Err(format!("{} does not contain any split APKs.\nThe download may be incomplete or corrupted.", bundle.display()));
    }

    Ok(paths)
}

fn bytes_of(archive: &mut ZipArchive<File>, name: &str) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();

    archive.by_name(name).ok()?.read_to_end(&mut bytes).ok()?;

    Some(bytes)
}

fn open(path: &Path) -> Option<Part> {
    let mut archive = File::open(path).ok().and_then(|file| ZipArchive::new(file).ok())?;
    let identity = bytes_of(&mut archive, MANIFEST).and_then(|bytes| manifest::identity(&bytes).ok())?;

    Some(Part { archive, identity })
}

fn signature(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();

    upper.starts_with(SIGNATURES) && SIGNATURE_ENDS.iter().any(|end| upper.ends_with(end))
}

fn code(name: &str) -> bool {
    name.strip_prefix(CODE).and_then(|rest| rest.strip_suffix(DEX)).is_some_and(|number| number.chars().all(|held| held.is_ascii_digit()))
}

fn assemble(paths: &[PathBuf], output: &Path, log: &impl Fn(String)) -> Result<(), String> {
    let mut parts: Vec<Part> = paths
        .iter()
        .filter_map(|path| {
            let part = open(path);

            if part.is_none() {
                warn!("{:?} is not a readable APK, leaving it out of the merge", path);
            }

            part
        })
        .collect();
    let Some(base) = parts.iter().position(|part| part.identity.split.is_none()) else {
        return Err("None of the split APKs is a base APK.\nThe download may be incomplete or corrupted.".to_string());
    };
    let mut base = parts.remove(base);
    let mut names = HashSet::new();

    parts.retain(|part| {
        let wanted = part.identity.package == base.identity.package && part.identity.split.as_ref().is_some_and(|name| names.insert(name.clone()));

        if !wanted {
            warn!(package = %part.identity.package, split = ?part.identity.split, "Split does not belong to the base APK or is a repeat, leaving it out");
        }

        wanted
    });
    parts.sort_by(|left, right| left.identity.split.cmp(&right.identity.split));
    log(format!("Found {} split APKs for {}.", parts.len(), base.identity.package));

    let mut table = bytes_of(&mut base.archive, TABLE).map(|bytes| Table::parse(&bytes)).transpose()?;

    for part in &mut parts {
        let Some(bytes) = bytes_of(&mut part.archive, TABLE) else {
            continue;
        };

        match table.as_mut() {
            Some(held) => held.absorb(Table::parse(&bytes)?)?,
            None => table = Some(Table::parse(&bytes)?),
        }
    }

    let fused: Vec<String> = parts.iter().filter(|part| part.identity.feature).filter_map(|part| part.identity.split.clone()).collect();
    let original = bytes_of(&mut base.archive, MANIFEST).ok_or("The base APK carries no AndroidManifest.xml")?;
    let cleaned = manifest::clean(&original, &fused)?;
    let dropped: HashSet<String> = match (cleaned.listing, table.as_mut()) {
        (Some(resource), Some(held)) => held.forget(resource)?.into_iter().collect(),
        _ => HashSet::new(),
    };
    let table = table.map(|held| held.write()).transpose()?;
    let file = File::create(output).map_err(|error| format!("Could not create the merged APK: {error}"))?;
    let mut writer = ZipWriter::new(file);
    let mut written = HashSet::new();
    let mut dexes = 0;

    written.insert(MANIFEST.to_string());
    written.insert(TABLE.to_string());
    stored(&mut writer, MANIFEST, &cleaned.bytes, false)?;

    if let Some(bytes) = &table {
        stored(&mut writer, TABLE, bytes, true)?;
    }

    for part in std::iter::once(&mut base).chain(&mut parts) {
        for index in 0..part.archive.len() {
            let entry = part.archive.by_index(index).map_err(|error| error.to_string())?;
            let name = entry.name().to_string();

            if entry.is_dir() || name == STAMP || signature(&name) || dropped.contains(&name) {
                continue;
            }

            let name = if code(&name) && part.identity.split.is_some() {
                format!("{}{}{}", CODE, dexes + 1, DEX)
            } else {
                name
            };

            if !written.insert(name.clone()) {
                continue;
            }

            if code(&name) {
                dexes += 1;
            }

            copy(&mut writer, entry, &name)?;
        }
    }

    writer.finish().map_err(|error| format!("Could not finish the merged APK: {error}"))?;
    info!("Merged {} files into {:?}", written.len(), output);

    Ok(())
}

fn stored(writer: &mut ZipWriter<File>, name: &str, bytes: &[u8], raw: bool) -> Result<(), String> {
    let options = if raw {
        SimpleFileOptions::default().compression_method(CompressionMethod::Stored).with_alignment(WORD)
    } else {
        SimpleFileOptions::default().compression_method(CompressionMethod::Deflated)
    };

    writer.start_file(name, options).map_err(|error| format!("Could not write {name}: {error}"))?;
    writer.write_all(bytes).map_err(|error| format!("Could not write {name}: {error}"))
}

fn copy<R: Read>(writer: &mut ZipWriter<File>, mut entry: zip::read::ZipFile<'_, R>, name: &str) -> Result<(), String> {
    if entry.compression() != CompressionMethod::Stored {
        return writer.raw_copy_file_rename(entry, name).map_err(|error| format!("Could not copy {name}: {error}"));
    }

    let alignment = if name.ends_with(LIBRARY) { PAGE } else { WORD };
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .large_file(entry.size() > u64::from(u32::MAX))
        .with_alignment(alignment);

    writer.start_file(name, options).map_err(|error| format!("Could not write {name}: {error}"))?;
    io::copy(&mut entry, writer).map(|_| ()).map_err(|error| format!("Could not copy {name}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_packages_are_told_from_plain_ones_by_extension() {
        assert!(is_split(Path::new("/pick/BCEN.xapk")));
        assert!(is_split(Path::new("/pick/BCEN.APKM")));
        assert!(is_split(Path::new("/pick/BCEN.apks")));
        assert!(!is_split(Path::new("/pick/BCEN.apk")));
        assert!(!is_split(Path::new("/pick/xapk")));
    }

    // Only a feature split's own code is renumbered; anything else named like it is left alone.
    #[test]
    fn only_root_dex_files_count_as_code() {
        assert!(code("classes.dex") && code("classes12.dex"));
        assert!(!code("assets/classes.dex") && !code("classes-old.dex") && !code("classes.dex.bak"));
    }

    #[test]
    fn only_signing_files_are_dropped_from_meta_inf() {
        assert!(signature("META-INF/BNDLTOOL.RSA") && signature("META-INF/MANIFEST.MF") && signature("meta-inf/cert.sf"));
        assert!(!signature("META-INF/services/kotlinx.coroutines.CoroutineExceptionHandler"));
        assert!(!signature("assets/MANIFEST.MF"));
    }
}
