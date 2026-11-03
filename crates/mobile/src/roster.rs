use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::sync::Arc;

use slint::{Image, Model, ModelNotify, ModelTracker, SharedString};

use foundry::Vault;
use foundry::common::assets;

use crate::images::Images;
use crate::{AppWindow, RosterRow};

const PLACEHOLDER: &str = "udi_f";

pub struct Entry {
    pub id: u32,
    pub banner: Option<PathBuf>,
    pub tooltip: SharedString,
}

pub type Loader = Rc<dyn Fn(&Images, &Arc<Vault>, &Path) -> (String, Option<Image>)>;

pub struct Roster {
    entries: RefCell<Vec<Entry>>,
    shown: RefCell<Vec<usize>>,
    vault: RefCell<Option<Arc<Vault>>>,
    images: Images,
    load: Loader,
    keys: RefCell<HashMap<String, usize>>,
    notify: ModelNotify,
    watch: RefCell<Option<u64>>,
}

impl Roster {
    pub fn new(images: &Images, load: Loader) -> Rc<Self> {
        let roster = Rc::new(Self {
            entries: RefCell::new(Vec::new()),
            shown: RefCell::new(Vec::new()),
            vault: RefCell::new(None),
            images: images.clone(),
            load,
            keys: RefCell::new(HashMap::new()),
            notify: ModelNotify::default(),
            watch: RefCell::new(None),
        });
        let weak: Weak<Self> = Rc::downgrade(&roster);
        let id = images.watch(Rc::new(move |_: &AppWindow, key: &str| {
            let Some(roster) = weak.upgrade() else {
                return;
            };
            let row = roster.keys.borrow().get(key).copied();

            if let Some(row) = row {
                roster.notify.row_changed(row);
            }
        }));

        *roster.watch.borrow_mut() = Some(id);
        roster
    }

    pub fn fill(&self, vault: Arc<Vault>, entries: Vec<Entry>, shown: Vec<usize>) {
        *self.vault.borrow_mut() = Some(vault);
        *self.entries.borrow_mut() = entries;
        *self.shown.borrow_mut() = shown;
        self.keys.borrow_mut().clear();
        self.notify.reset();
    }

    pub fn show(&self, shown: Vec<usize>) {
        *self.shown.borrow_mut() = shown;
        self.keys.borrow_mut().clear();
        self.notify.reset();
    }
}

impl Drop for Roster {
    fn drop(&mut self) {
        if let Some(id) = self.watch.borrow_mut().take() {
            self.images.unwatch(id);
        }
    }
}

impl Model for Roster {
    type Data = RosterRow;

    fn row_count(&self) -> usize {
        self.shown.borrow().len()
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        let at = *self.shown.borrow().get(row)?;
        let entries = self.entries.borrow();
        let entry = entries.get(at)?;
        let vault = self.vault.borrow();
        let banner = entry.banner.as_ref().zip(vault.as_ref()).map(|(path, vault)| {
            let (key, image) = (self.load)(&self.images, vault, path);

            self.keys.borrow_mut().insert(key, row);
            image
        });
        let (banner, has_banner) = match banner.flatten().or_else(|| self.images.asset(PLACEHOLDER, assets::UDI_F)) {
            Some(image) => (image, true),
            None => (Image::default(), false),
        };

        Some(RosterRow { id: entry.id as i32, banner, has_banner, tooltip: entry.tooltip.clone() })
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.notify
    }
}
