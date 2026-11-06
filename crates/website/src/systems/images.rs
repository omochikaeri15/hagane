use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::path::Path;
use std::rc::Rc;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use dioxus::core::spawn_forever;
use dioxus::prelude::*;
use image::{ImageFormat, RgbaImage};
use tracing::{debug, warn};

use foundry::common::formats::{self, SpriteSheet};
use foundry::common::gfx;
use foundry::Vfs;

use crate::platform::{Picture, Platform};

const PACE_MS: u32 = 40;
const PER_PACE: usize = 6;
const PAYLOAD_PREFIX: usize = 16;

struct Wanted {
    key: String,
    kind: Picture,
    name: Box<str>,
    folder: String,
}

#[derive(Clone)]
pub(crate) struct Pictures {
    platform: Platform,
    urls: Rc<RefCell<HashMap<String, Rc<str>>>>,
    sizes: Rc<RefCell<HashMap<String, (u32, u32)>>>,
    sheets: Rc<RefCell<HashMap<String, Rc<Vec<SpriteSheet>>>>>,
    banner_frame: Rc<RefCell<Option<Rc<RgbaImage>>>>,
    wanted: Rc<RefCell<Vec<Wanted>>>,
    inflight: Rc<RefCell<HashSet<String>>>,
    failed: Rc<RefCell<HashSet<String>>>,
    scheduling: Rc<Cell<bool>>,
    bumping: Rc<Cell<bool>>,
    tick: Signal<u32>,
    epoch: Rc<RefCell<Option<Epoch>>>,
    pruned: Rc<RefCell<Option<String>>>,
    session: Rc<Cell<u32>>,
}

type Epoch = Rc<dyn Fn() -> Option<String>>;

pub(crate) fn epoch_of(payload: &str) -> String {
    payload.chars().take(PAYLOAD_PREFIX).collect()
}

fn lasting(kind: &Picture) -> bool {
    !matches!(kind, Picture::Cut { .. })
}

pub(crate) struct Sized {
    pub(crate) url: Rc<str>,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

fn file_name(path: &Path) -> Option<Box<str>> {
    path.file_name().and_then(|name| name.to_str()).map(Box::from)
}

fn encode(image: &RgbaImage) -> Option<Vec<u8>> {
    let mut png = Vec::new();

    image.write_to(&mut Cursor::new(&mut png), ImageFormat::Png).inspect_err(|err| warn!("An image could not be encoded: {}", err)).ok()?;

    Some(png)
}

fn data_url(png: &[u8]) -> Rc<str> {
    Rc::from(format!("data:image/png;base64,{}", STANDARD.encode(png)))
}

pub(crate) fn provide(platform: &Platform) -> Pictures {
    let platform = platform.clone();

    use_context_provider(move || Pictures {
        platform,
        urls: Rc::default(),
        sizes: Rc::default(),
        sheets: Rc::default(),
        banner_frame: Rc::default(),
        wanted: Rc::default(),
        inflight: Rc::default(),
        failed: Rc::default(),
        scheduling: Rc::default(),
        bumping: Rc::default(),
        tick: Signal::new(0),
        epoch: Rc::default(),
        pruned: Rc::default(),
        session: Rc::default(),
    })
}

pub(crate) fn pictures() -> Pictures {
    use_context::<Pictures>()
}

impl Pictures {
    pub(crate) fn attach(&self, epoch: impl Fn() -> Option<String> + 'static) {
        *self.epoch.borrow_mut() = Some(Rc::new(epoch));
    }

    fn kept_under(&self) -> Option<String> {
        let source = self.epoch.borrow().clone()?;
        let epoch = source()?;

        if self.pruned.borrow().as_deref() != Some(epoch.as_str()) {
            *self.pruned.borrow_mut() = Some(epoch.clone());

            let pruning = self.platform.host().prune_images(&epoch);

            spawn_forever(pruning);
        }

        Some(epoch)
    }

    pub(crate) fn forget_all(&self) {
        self.session.set(self.session.get().wrapping_add(1));
        self.urls.borrow_mut().clear();
        self.sizes.borrow_mut().clear();
        self.sheets.borrow_mut().clear();
        self.wanted.borrow_mut().clear();
        self.failed.borrow_mut().clear();
        self.bump();
    }

    pub(crate) fn watch(&self) -> u32 {
        (self.tick)()
    }

    pub(crate) fn settled(&self) -> bool {
        self.wanted.borrow().is_empty() && self.inflight.borrow().is_empty()
    }

    fn bump(&self) {
        if self.bumping.replace(true) {
            return;
        }

        let platform = self.platform.clone();
        let bumping = Rc::clone(&self.bumping);
        let mut tick = self.tick;

        spawn_forever(async move {
            platform.host().frame().await;
            bumping.set(false);
            tick += 1;
        });
    }

    fn remote(&self, key: String, kind: Picture, name: Box<str>, folder: String) -> Option<Rc<str>> {
        let _ = (self.tick)();

        if let Some(url) = self.urls.borrow().get(&key) {
            return Some(Rc::clone(url));
        }

        if self.failed.borrow().contains(&key) || self.inflight.borrow().contains(&key) || self.wanted.borrow().iter().any(|held| held.key == key) {
            return None;
        }

        let held = Wanted { key, kind, name, folder };

        match self.kept_under().filter(|_| lasting(&held.kind) && self.platform.host().can_publish()) {
            Some(epoch) => self.recall(held, epoch),
            None => {
                self.wanted.borrow_mut().push(held);
                self.schedule();
            }
        }

        None
    }

    fn recall(&self, held: Wanted, epoch: String) {
        let session = self.session.get();
        let lookup = self.platform.host().kept_image(&format!("{epoch}/{}", held.folder), &held.name);
        let pictures = self.clone();

        self.inflight.borrow_mut().insert(held.key.clone());
        spawn_forever(async move {
            let kept = lookup.await;

            pictures.inflight.borrow_mut().remove(&held.key);

            if pictures.session.get() != session {
                return;
            }

            match kept {
                Some(kept) => {
                    pictures.sizes.borrow_mut().insert(held.key.clone(), (kept.width, kept.height));
                    pictures.urls.borrow_mut().insert(held.key, Rc::from(kept.url));
                    pictures.bump();
                }
                None => {
                    pictures.wanted.borrow_mut().push(held);
                    pictures.schedule();
                }
            }
        });
    }

    pub(crate) fn unwant(&self, key: &str) {
        self.wanted.borrow_mut().retain(|held| held.key != key);
    }

    fn schedule(&self) {
        if self.scheduling.replace(true) {
            return;
        }

        let pictures = self.clone();

        spawn_forever(async move {
            loop {
                pictures.platform.host().sleep(PACE_MS).await;

                let batch: Vec<Wanted> = {
                    let mut wanted = pictures.wanted.borrow_mut();
                    let take = wanted.len().min(PER_PACE);

                    wanted.drain(..take).collect()
                };

                for held in batch {
                    if pictures.urls.borrow().contains_key(&held.key) || !pictures.inflight.borrow_mut().insert(held.key.clone()) {
                        continue;
                    }

                    pictures.request(held);
                }

                if pictures.wanted.borrow().is_empty() {
                    pictures.scheduling.set(false);

                    return;
                }
            }
        });
    }

    fn request(&self, held: Wanted) {
        let folder = lasting(&held.kind).then(|| self.kept_under()).flatten().map(|epoch| format!("{epoch}/{}", held.folder));
        let pending = self.platform.host().picture(held.kind, held.name.clone());
        let pictures = self.clone();
        let started = self.platform.host().now();
        let session = self.session.get();

        spawn_forever(async move {
            let outcome = pending.await;

            if pictures.session.get() != session {
                pictures.inflight.borrow_mut().remove(&held.key);

                return;
            }

            match outcome {
                Ok(Some(prepared)) => {
                    let url = match folder.filter(|_| pictures.platform.host().can_publish()) {
                        Some(folder) => match pictures.platform.host().publish_image(&folder, &held.name, prepared.png.clone(), (prepared.width, prepared.height)).await {
                            Ok(Some(url)) => Rc::from(url),
                            Ok(None) => data_url(&prepared.png),
                            Err(fault) => {
                                warn!("An image could not be published: {fault}");
                                data_url(&prepared.png)
                            }
                        },
                        None => data_url(&prepared.png),
                    };

                    pictures.sizes.borrow_mut().insert(held.key.clone(), (prepared.width, prepared.height));
                    pictures.urls.borrow_mut().insert(held.key.clone(), url);
                    debug!(key = %held.key, ms = (pictures.platform.host().now() - started) as u32, "A picture is ready");
                }
                Ok(None) => {
                    warn!("The host has no image for {}", held.key);
                    pictures.failed.borrow_mut().insert(held.key.clone());
                }
                Err(fault) => {
                    warn!("An image could not be prepared: {fault}");
                    pictures.failed.borrow_mut().insert(held.key.clone());
                }
            }

            pictures.inflight.borrow_mut().remove(&held.key);
            pictures.bump();
        });
    }

    pub(crate) fn trimmed(&self, path: &Path) -> Option<Sized> {
        let name = file_name(path)?;
        let key = format!("trim:{name}");
        let url = self.remote(key.clone(), Picture::Trimmed, name, "trimmed".to_owned())?;
        let (width, height) = *self.sizes.borrow().get(&key)?;

        Some(Sized { url, width, height })
    }

    pub(crate) fn boxed(&self, path: &Path, canvas: u32) -> Option<Rc<str>> {
        let name = file_name(path)?;

        self.remote(format!("box{canvas}:{name}"), Picture::Boxed(canvas), name, format!("boxed{canvas}"))
    }

    pub(crate) fn cut(&self, sheet: &str, icon: usize) -> Option<Rc<str>> {
        let name: Box<str> = format!("{sheet}_{icon}.png").into();

        self.remote(format!("cut:{sheet}:{icon}"), Picture::Cut { sheet: Box::from(sheet), icon: icon as u32 }, name, "cut".to_owned())
    }

    pub(crate) fn cut_sized(&self, sheet: &str, icon: usize) -> Option<Sized> {
        let url = self.cut(sheet, icon)?;
        let (width, height) = *self.sizes.borrow().get(&format!("cut:{sheet}:{icon}"))?;

        Some(Sized { url, width, height })
    }

    pub(crate) fn banner_key(kind: &Picture, path: &Path) -> Option<String> {
        Some(format!("{kind:?}:{}", file_name(path)?))
    }

    pub(crate) fn banner(&self, kind: Picture, path: &Path) -> Option<Rc<str>> {
        let name = file_name(path)?;
        let folder = match kind {
            Picture::EnemyBanner => "enemy",
            _ => "banner",
        };

        self.remote(format!("{kind:?}:{name}"), kind, name, folder.to_owned())
    }

    fn local(&self, key: String, build: impl FnOnce() -> Option<RgbaImage>) -> Option<Rc<str>> {
        if let Some(url) = self.urls.borrow().get(&key) {
            return Some(Rc::clone(url));
        }

        let url = data_url(&encode(&build()?)?);

        self.urls.borrow_mut().insert(key, Rc::clone(&url));

        Some(url)
    }

    pub(crate) fn frame_url(&self) -> Option<Rc<str>> {
        self.local("banner:frame".to_owned(), || self.frame().map(|frame| (*frame).clone()))
    }

    fn frame(&self) -> Option<Rc<RgbaImage>> {
        if let Some(frame) = self.banner_frame.borrow().as_ref() {
            return Some(Rc::clone(frame));
        }

        let frame = Rc::new(gfx::banner_frame()?);

        *self.banner_frame.borrow_mut() = Some(Rc::clone(&frame));

        Some(frame)
    }

    pub(crate) fn asset(&self, name: &str, bytes: &[u8]) -> Option<Rc<str>> {
        self.local(format!("asset:{name}"), || Some(image::load_from_memory(bytes).ok()?.to_rgba8()))
    }

    pub(crate) fn sheets(&self, vfs: &Vfs, name: &str) -> Rc<Vec<SpriteSheet>> {
        if let Some(sheets) = self.sheets.borrow().get(name) {
            return Rc::clone(sheets);
        }

        let layers = Rc::new(formats::sheets(vfs, name));
        let settled = !layers.is_empty() && layers.iter().all(|layer| layer.image_data.is_some());

        if settled {
            self.sheets.borrow_mut().insert(name.to_owned(), Rc::clone(&layers));
        }

        layers
    }
}
