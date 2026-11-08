mod apng;
pub(crate) mod encoding;

use std::fs;
use std::path::PathBuf;

use tracing::{error, info};

use crate::common::platform;

use super::DownloadConfig;
use super::paths::{get_tools_dir, AddonStatus, CJXL_BIN};

pub struct CjxlManager {
    pub status: AddonStatus,
}

impl Default for CjxlManager {
    fn default() -> Self {
        Self {
            status: if get_cjxl_path().is_some() { AddonStatus::Installed } else { AddonStatus::NotInstalled },
        }
    }
}

impl CjxlManager {
    pub fn install(&mut self) -> DownloadConfig {
        info!("Starting cjxl installation...");
        self.status = AddonStatus::Downloading(0.0, "Starting...".to_string());

        DownloadConfig {
            folder_name: "cjxl".to_string(),
            asset_name: format!("cjxl_{}.zip", platform::suffix()),
            binary_name: CJXL_BIN.to_string(),
        }
    }

    pub fn uninstall(&mut self) {
        info!("Uninstalling cjxl...");
        let dir = get_cjxl_dir();

        if dir.exists()
            && let Err(err) = fs::remove_dir_all(&dir) {
            error!("Failed to remove cjxl directory: {}", err);
        }

        self.status = AddonStatus::NotInstalled;
    }
}

fn get_cjxl_dir() -> PathBuf {
    get_tools_dir().join("cjxl")
}

fn get_cjxl_path() -> Option<PathBuf> {
    let bin = get_cjxl_dir().join(CJXL_BIN);
    if bin.exists() { Some(bin) } else { None }
}
