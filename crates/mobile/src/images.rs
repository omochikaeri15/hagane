use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use image::RgbaImage;
use slint::{ComponentHandle, Image, Rgba8Pixel, SharedPixelBuffer, Weak, Timer};
use tracing::{debug, warn};

use foundry::common::formats::{self, SpriteSheet};
use foundry::common::gfx;
use foundry::Vault;

use crate::AppWindow;
use crate::bridge::{self, Ticket};

pub type Watcher = Rc<dyn Fn(&AppWindow, &str)>;

#[derive(Clone)]
pub enum Slot {
    Pending,
    Ready(Image),
    Missing,
}

struct Inner {
    window: Weak<AppWindow>,
    cache: RefCell<HashMap<String, Slot>>,
    watchers: RefCell<Vec<(u64, Watcher)>>,
    sheets: RefCell<HashMap<String, Rc<Vec<SpriteSheet>>>>,
    frame: RefCell<Option<Arc<RgbaImage>>>,
    next: RefCell<u64>,
    ticket: Cell<Option<Ticket>>,
}

const SETTLE_POLL: Duration = Duration::from_millis(50);
pub const SETTLE_LIMIT: Duration = Duration::from_millis(500);

type Decoded = (String, Option<RgbaImage>);

#[derive(Clone)]
pub struct Images(Rc<Inner>);

pub fn to_slint(image: &RgbaImage) -> Image {
    Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(image.as_raw(), image.width(), image.height()))
}

pub fn decode(vault: &Vault, path: &Path) -> Option<RgbaImage> {
    let bytes = vault.vfs.read(path).inspect_err(|err| debug!(path = %path.display(), "An image could not be read: {err}")).ok()?;

    image::load_from_memory(&bytes).inspect_err(|err| warn!(path = %path.display(), "An image could not be decoded: {err}")).ok().map(|image| image.to_rgba8())
}

impl Images {
    pub fn new(window: &AppWindow) -> Self {
        let inner = Rc::new(Inner {
            window: window.as_weak(),
            cache: RefCell::new(HashMap::new()),
            watchers: RefCell::new(Vec::new()),
            sheets: RefCell::new(HashMap::new()),
            frame: RefCell::new(None),
            next: RefCell::new(1),
            ticket: Cell::new(None),
        });
        let landing = Rc::downgrade(&inner);
        let ticket = bridge::subscribe::<Decoded>(move |ui, (key, decoded)| {
            let Some(inner) = landing.upgrade() else {
                return;
            };
            let slot = decoded.as_ref().map_or(Slot::Missing, |image| Slot::Ready(to_slint(image)));

            inner.cache.borrow_mut().insert(key.clone(), slot);

            let watchers: Vec<Watcher> = inner.watchers.borrow().iter().map(|(_, watcher)| Rc::clone(watcher)).collect();

            for watcher in watchers {
                watcher(ui, &key);
            }
        });

        inner.ticket.set(Some(ticket));
        Self(inner)
    }

    pub fn watch(&self, watcher: Watcher) -> u64 {
        let id = {
            let mut next = self.0.next.borrow_mut();
            let id = *next;

            *next += 1;
            id
        };

        self.0.watchers.borrow_mut().push((id, watcher));
        id
    }

    pub fn unwatch(&self, id: u64) {
        self.0.watchers.borrow_mut().retain(|(held, _)| *held != id);
    }

    pub fn pending(&self) -> usize {
        self.0.cache.borrow().values().filter(|slot| matches!(slot, Slot::Pending)).count()
    }

    pub fn settled(&self, ui: &AppWindow, deadline: Instant, then: Rc<dyn Fn(&AppWindow)>) {
        if self.pending() == 0 || Instant::now() >= deadline {
            then(ui);
            return;
        }

        let (weak, images) = (ui.as_weak(), self.clone());

        Timer::single_shot(SETTLE_POLL, move || {
            if let Some(ui) = weak.upgrade() {
                images.settled(&ui, deadline, then);
            }
        });
    }

    pub fn forget_all(&self) {
        self.0.cache.borrow_mut().clear();
        self.0.sheets.borrow_mut().clear();
    }

    pub fn request(&self, key: &str, load: impl FnOnce() -> Option<RgbaImage> + Send + 'static) -> Option<Image> {
        if let Some(slot) = self.0.cache.borrow().get(key) {
            return match slot {
                Slot::Ready(image) => Some(image.clone()),
                _ => None,
            };
        }

        self.0.cache.borrow_mut().insert(key.to_owned(), Slot::Pending);

        let ticket = self.0.ticket.get()?;
        let weak = self.0.window.clone();
        let landing = key.to_owned();

        rayon::spawn(move || {
            let decoded = load();

            bridge::post(&weak, ticket, (landing, decoded));
        });

        None
    }

    pub fn local(&self, key: &str, build: impl FnOnce() -> Option<RgbaImage>) -> Option<Image> {
        if let Some(slot) = self.0.cache.borrow().get(key) {
            return match slot {
                Slot::Ready(image) => Some(image.clone()),
                _ => None,
            };
        }

        let slot = build().map_or(Slot::Missing, |image| Slot::Ready(to_slint(&image)));
        let image = match &slot {
            Slot::Ready(image) => Some(image.clone()),
            _ => None,
        };

        self.0.cache.borrow_mut().insert(key.to_owned(), slot);
        image
    }

    pub fn frame(&self) -> Option<Arc<RgbaImage>> {
        if let Some(frame) = self.0.frame.borrow().as_ref() {
            return Some(Arc::clone(frame));
        }

        let frame = Arc::new(gfx::banner_frame()?);

        *self.0.frame.borrow_mut() = Some(Arc::clone(&frame));
        Some(frame)
    }

    pub fn cat_banner(&self, vault: &Arc<Vault>, path: &Path) -> Option<Image> {
        let key = format!("cat-banner:{}", path.display());
        let (vault, path, frame) = (Arc::clone(vault), path.to_path_buf(), self.frame()?);

        self.request(&key, move || decode(&vault, &path).map(|unit| gfx::cat_banner(&frame, unit)))
    }

    pub fn enemy_banner(&self, vault: &Arc<Vault>, path: &Path) -> Option<Image> {
        let key = format!("enemy-banner:{}", path.display());
        let (vault, path, frame) = (Arc::clone(vault), path.to_path_buf(), self.frame()?);

        self.request(&key, move || decode(&vault, &path).map(|unit| gfx::enemy_banner(&frame, unit)))
    }

    pub fn trimmed(&self, vault: &Arc<Vault>, path: &Path) -> Option<Image> {
        let key = format!("trim:{}", path.display());
        let (vault, path) = (Arc::clone(vault), path.to_path_buf());

        self.request(&key, move || decode(&vault, &path).and_then(gfx::trimmed))
    }

    pub fn boxed(&self, vault: &Arc<Vault>, path: &Path, canvas: u32) -> Option<Image> {
        let key = format!("box{canvas}:{}", path.display());
        let (vault, path) = (Arc::clone(vault), path.to_path_buf());

        self.request(&key, move || decode(&vault, &path).and_then(|image| gfx::boxed(image, canvas)))
    }

    pub fn sheets(&self, vault: &Vault, name: &str) -> Rc<Vec<SpriteSheet>> {
        if let Some(sheets) = self.0.sheets.borrow().get(name) {
            return Rc::clone(sheets);
        }

        let layers = Rc::new(formats::sheets(&vault.vfs, name));

        if !layers.is_empty() && layers.iter().all(|layer| layer.image_data.is_some()) {
            self.0.sheets.borrow_mut().insert(name.to_owned(), Rc::clone(&layers));
        }

        layers
    }

    pub fn cut(&self, vault: &Vault, sheet: &str, icon: usize) -> Option<Image> {
        let key = format!("cut:{sheet}:{icon}");
        let layers = self.sheets(vault, sheet);

        self.local(&key, || layers.iter().find_map(|layer| layer.crop(icon)))
    }

    pub fn asset(&self, name: &str, bytes: &[u8]) -> Option<Image> {
        self.local(&format!("asset:{name}"), || image::load_from_memory(bytes).ok().map(|image| image.to_rgba8()))
    }

    pub fn size_of(image: &Image) -> (u32, u32) {
        let size = image.size();

        (size.width, size.height)
    }
}
