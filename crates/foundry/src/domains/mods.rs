#[cfg(feature = "mods")]
mod disk;
#[cfg(feature = "mods")]
pub mod export;
#[cfg(feature = "mods")]
pub mod import;
#[cfg(feature = "bcm")]
mod package;

use std::path::{Component, Path};

use serde::{Deserialize, Serialize};

#[cfg(feature = "mods")]
pub use disk::{ModData, ModDataState, adopt, create, disable, enable, ensure, ensure_as, find, find_all, locate, patch_root, place, reserved, taken};
#[cfg(feature = "bcm")]
pub use package::Package;

pub const METADATA: &str = "metadata.json";

pub const ICON: &str = "icon.png";

pub const PATCH: &str = "patch";

const LOOSE: &str = "loose";
const NESTED: [&str; 3] = ["patch/", "loose/", "icons/"];
const PACK_PARTS: [&str; 2] = ["assets/DownloadLocal.list", "assets/DownloadLocal.pack"];
const ASSETS: &str = "assets";
const JUNK: [&str; 5] = [".dat", ".lock", ".pack", ".list", ".dex"];

pub fn placement(name: &str) -> Option<(String, bool)> {
    let path = Path::new(name);
    let file = path.file_name()?.to_string_lossy().into_owned();

    if NESTED.iter().any(|nested| name.starts_with(nested)) {
        let nested = path.components().all(|part| matches!(part, Component::Normal(_)));

        return nested.then(|| (name.to_owned(), false));
    }

    if PACK_PARTS.contains(&name) {
        return Some((file, true));
    }

    if file == METADATA {
        return Some((format!("{PATCH}/{file}"), false));
    }

    if path.parent() != Some(Path::new(ASSETS)) {
        return None;
    }

    let is_download_tsv = file.starts_with("download") && file.ends_with(".tsv");
    let is_junk = JUNK.iter().any(|junk| file.ends_with(junk));

    (!is_download_tsv && !is_junk).then(|| (format!("{LOOSE}/{file}"), false))
}

pub fn kept_placement(name: &str) -> Option<(String, bool)> {
    placement(name).or_else(|| {
        let mut parts = Path::new(name).components();

        match (parts.next(), parts.next()) {
            (Some(Component::Normal(file)), None) => Some((file.to_string_lossy().into_owned(), false)),
            _ => None,
        }
    })
}

fn default_source() -> String {
    "Hagane".to_string()
}

fn default_package() -> String {
    "".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModMetadata {
    #[serde(default)] pub title: String,
    #[serde(default)] pub author: String,
    #[serde(default)] pub version: String,
    #[serde(default)] pub description: String,
    #[serde(default = "default_package")] pub package: String,
    #[serde(default = "default_source")] pub source: String,
}

impl Default for ModMetadata {
    fn default() -> Self {
        Self {
            title: String::new(),
            author: String::new(),
            version: String::new(),
            description: String::new(),
            package: default_package(),
            source: default_source(),
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    // A modder drops sprites and animations straight into the mod's folder, and the exporter packs
    // them at the top of the BCM. They have to come back out of it.
    #[test]
    fn a_file_at_the_top_of_a_bcm_survives_the_round_trip() {
        assert_eq!(kept_placement("269_c.maanim"), Some(("269_c.maanim".to_owned(), false)));
        assert_eq!(kept_placement("patch/udi038_f.png"), Some(("patch/udi038_f.png".to_owned(), false)));
        assert_eq!(kept_placement("metadata.json"), Some(("patch/metadata.json".to_owned(), false)));
        assert_eq!(kept_placement("../escape.png"), None);
        assert_eq!(kept_placement("other/udi038_f.png"), None);
    }

    // An APK is a zip with its own files at the top, and none of those are mod files.
    #[test]
    fn the_top_of_an_apk_is_not_mod_content() {
        assert_eq!(placement("classes.dex"), None);
        assert_eq!(placement("AndroidManifest.xml"), None);
        assert_eq!(placement("assets/udi038_f.png"), Some(("loose/udi038_f.png".to_owned(), false)));
    }
}
