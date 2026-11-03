use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, ModelRc, SharedString, StyledText, VecModel};
use tracing::{error, info};

use foundry::common::changelog;

use crate::pages::Context;
use crate::{AppWindow, Home, bridge, platform};

const REPO_OWNER: &str = "omochikaeri15";
const REPO_NAME: &str = "battle-cats-complete";
const UNKNOWN: &str = "Unknown";
const NO_RELEASES: &str = "No releases found.";
const NO_NOTES: &str = "No notes available.";

type Releases = Rc<RefCell<Vec<(String, String)>>>;

fn show(ui: &AppWindow, releases: &[(String, String)], index: usize) {
    let home = ui.global::<Home>();
    let body = releases.get(index).map_or(NO_NOTES, |(_, body)| body.as_str());

    home.set_changelog_current(index as i32);
    home.set_changelog_body(StyledText::from_markdown(body).unwrap_or_else(|_| StyledText::from_plain_text(body)));
}

pub fn wire(window: &AppWindow, _context: &Rc<Context>) {
    let home = window.global::<Home>();
    let releases: Releases = Rc::default();
    let fetching = Rc::new(std::cell::Cell::new(false));

    home.set_version(env!("CARGO_PKG_VERSION").into());
    home.on_open_url(|url| platform::open_url(&url));

    let picked = Rc::clone(&releases);
    let picker = window.as_weak();

    home.on_pick_changelog(move |index| {
        if let Some(ui) = picker.upgrade() {
            show(&ui, &picked.borrow(), index.max(0) as usize);
        }
    });

    let opener = window.as_weak();

    home.on_open_changelog(move || {
        let Some(ui) = opener.upgrade() else {
            return;
        };

        if !releases.borrow().is_empty() || fetching.get() {
            return;
        }

        fetching.set(true);
        ui.global::<Home>().set_changelog_loading(true);
        ui.global::<Home>().set_changelog_failed(false);

        let held = Rc::clone(&releases);
        let busy = Rc::clone(&fetching);

        bridge::offload(
            &ui,
            "changelogs",
            || changelog::load(REPO_OWNER, REPO_NAME, env!("CARGO_PKG_VERSION")).map_err(|err| err.to_string()),
            move |ui, outcome| {
                let home = ui.global::<Home>();

                busy.set(false);
                home.set_changelog_loading(false);

                match outcome {
                    Ok(mut found) => {
                        info!(count = found.len(), "Changelogs loaded");

                        if found.is_empty() {
                            found.push((UNKNOWN.to_owned(), NO_RELEASES.to_owned()));
                        }

                        let current = env!("CARGO_PKG_VERSION").trim_start_matches('v');
                        let index = found.iter().position(|(version, _)| version == current).unwrap_or(0);
                        let versions: Vec<SharedString> = found.iter().map(|(version, _)| version.as_str().into()).collect();

                        home.set_changelog_versions(ModelRc::new(VecModel::from(versions)));
                        show(ui, &found, index);
                        *held.borrow_mut() = found;
                    }
                    Err(reason) => {
                        error!("The changelogs could not be fetched: {reason}");
                        home.set_changelog_failed(true);
                    }
                }
            },
        );
    });
}
