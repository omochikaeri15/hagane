use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

const ANIMATION: [&str; 27] = [
    "src/render.rs",
    "../codex/src/graphics.rs",
    "../codex/src/graphics/animate.rs",
    "../codex/src/graphics/engine.rs",
    "../codex/src/graphics/rig.rs",
    "../codex/src/graphics/rig",
    "../codex/src/graphics/tools.rs",
    "../codex/src/graphics/tools",
    "../foundry/src/systems/animation.rs",
    "../foundry/src/systems/animation/alpha.rs",
    "../foundry/src/systems/animation/collision.rs",
    "../foundry/src/systems/animation/overlay.rs",
    "../foundry/src/systems/animation/paint.rs",
    "../foundry/src/systems/animation/export.rs",
    "../foundry/src/systems/animation/export/avif.rs",
    "../foundry/src/systems/animation/export/avif",
    "../foundry/src/systems/animation/export/encoding.rs",
    "../foundry/src/systems/animation/export/find_bounds.rs",
    "../foundry/src/systems/animation/export/frames.rs",
    "../foundry/src/systems/animation/export/gif.rs",
    "../foundry/src/systems/animation/export/gif",
    "../foundry/src/systems/animation/export/leader.rs",
    "../foundry/src/systems/animation/export/process.rs",
    "../foundry/src/systems/animation/export/webp.rs",
    "../foundry/src/systems/animation/export/webp",
    "../foundry/src/systems/raster.rs",
    "../foundry/src/systems/raster",
];

const STATBLOCK: [&str; 10] = [
    "src/statblock.rs",
    "../foundry/src/systems/statblock.rs",
    "../foundry/src/systems/statblock",
    "../foundry/src/common/assets.rs",
    "../foundry/src/common/formats.rs",
    "../foundry/src/common/formats/imgcut.rs",
    "../foundry/src/common/frames.rs",
    "../foundry/src/common/gfx.rs",
    "../foundry/src/common/superscript.rs",
    "../codex/src/files/img015.rs",
];

const NOT_DRAWING: [&str; 7] = [
    "../codex/src/graphics/backdrop.rs",
    "../foundry/src/systems/animation/authoring.rs",
    "../foundry/src/systems/animation/posing.rs",
    "../foundry/src/systems/animation/export/apng.rs",
    "../foundry/src/systems/animation/export/find_loop.rs",
    "../foundry/src/systems/animation/export/jxl.rs",
    "../foundry/src/common/formats/json.rs",
];

const EMBEDS: [&str; 2] = ["include_bytes!(\"", "include_str!(\""];

const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

fn main() -> Result<(), Box<dyn Error>> {
    stamp("HAGANE_ANIMATION_STAMP", &ANIMATION)?;
    stamp("HAGANE_STATBLOCK_STAMP", &STATBLOCK)
}

fn stamp(name: &str, listed: &[&str]) -> Result<(), Box<dyn Error>> {
    let mut hash = FNV_OFFSET;

    for entry in listed {
        let path = Path::new(entry);

        if !path.exists() {
            return Err(format!("{entry} is on the bot's drawing list but does not exist").into());
        }

        println!("cargo::rerun-if-changed={entry}");

        if path.is_file() {
            unlisted(entry, listed)?;
        }

        for file in files(path)? {
            absorb_file(&file, &mut hash)?;
        }
    }

    println!("cargo::rustc-env={name}={hash:016x}");

    Ok(())
}

fn unlisted(entry: &str, listed: &[&str]) -> Result<(), Box<dyn Error>> {
    let Some(parent) = entry.strip_suffix(".rs") else {
        return Ok(());
    };

    for line in fs::read_to_string(entry)?.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();
        let Some(at) = words.iter().position(|word| *word == "mod") else { continue };
        let Some(name) = words.get(at + 1).and_then(|word| word.strip_suffix(';')) else { continue };
        let child = format!("{parent}/{name}.rs");
        let inside = |held: &&str| child.starts_with(&format!("{held}/"));

        if !listed.contains(&child.as_str()) && !NOT_DRAWING.contains(&child.as_str()) && !listed.iter().any(inside) {
            return Err(format!("{entry} declares `mod {name}`, which is on neither the bot's drawing lists nor NOT_DRAWING in build.rs").into());
        }
    }

    Ok(())
}

fn files(path: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }

    let mut children = fs::read_dir(path)?.map(|entry| entry.map(|entry| entry.path())).collect::<Result<Vec<_>, _>>()?;
    let mut found = Vec::new();

    children.sort();

    for child in children {
        found.extend(files(&child)?);
    }

    Ok(found)
}

fn absorb_file(file: &Path, hash: &mut u64) -> Result<(), Box<dyn Error>> {
    let bytes = fs::read(file)?;

    absorb(file.to_string_lossy().as_bytes(), hash);
    absorb(&bytes, hash);

    if file.extension().is_some_and(|extension| extension == "rs") {
        for embedded in embeds(file, &String::from_utf8_lossy(&bytes)) {
            let content = fs::read(&embedded).map_err(|err| format!("{} is built into {} but could not be read: {err}", embedded.display(), file.display()))?;

            println!("cargo::rerun-if-changed={}", embedded.display());
            absorb(embedded.to_string_lossy().as_bytes(), hash);
            absorb(&content, hash);
        }
    }

    Ok(())
}

fn embeds(file: &Path, text: &str) -> Vec<PathBuf> {
    let base = file.parent().unwrap_or(Path::new(""));

    EMBEDS.iter().flat_map(|marker| text.split(marker).skip(1)).filter_map(|rest| rest.split('"').next()).map(|relative| base.join(relative)).collect()
}

fn absorb(bytes: &[u8], hash: &mut u64) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(FNV_PRIME);
    }
}
