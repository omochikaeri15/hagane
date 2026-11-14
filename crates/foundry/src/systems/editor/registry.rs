use std::path::Path;

use crate::common::io::APP_LANGUAGES;

pub fn language(name: &str) -> Option<&'static str> {
    let stem = Path::new(name).file_stem()?.to_str()?;
    let (_, code) = stem.rsplit_once('_')?;

    APP_LANGUAGES.iter().find(|(known, _)| *known == code).map(|(_, label)| *label)
}
