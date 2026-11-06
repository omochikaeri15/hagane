use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{Cursor, Read};
use std::rc::Rc;
use std::sync::{Arc, Mutex, PoisonError, RwLock};

use image::{ImageFormat, RgbaImage};
use js_sys::{Array, AsyncIterator, Function, Promise, Reflect, Uint8Array};
use structured_zstd::decoding::StreamingDecoder;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};
use tracing_wasm::WASMLayerConfigBuilder;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use futures_channel::oneshot;
use futures_util::future::{self, join};
use futures_util::stream::{self, FuturesUnordered, StreamExt};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{
    DedicatedWorkerGlobalScope, FileSystemDirectoryHandle, FileSystemFileHandle, FileSystemGetDirectoryOptions, FileSystemGetFileOptions,
    FileSystemReadWriteOptions, FileSystemRemoveOptions, FileSystemSyncAccessHandle, MessageEvent, ReadableStreamDefaultReader, Request as WebRequest, RequestCache, RequestInit, Response,
    XmlHttpRequest, XmlHttpRequestResponseType,
};

use codex::graphics::rig::SpriteSheet as SheetRig;
use foundry::common::formats::{self, SpriteSheet};
use foundry::common::gfx;
use foundry::common::payload::{MANIFEST, Manifest};
use foundry::domains::{cat, enemy, stage};
use foundry::{Lazy, Loader, Vault};

use crate::protocol::{self, Answer, Call, HELLO, PictureKind, Prepared, Reply, Request, RigWire};

pub(crate) const HOST: &str = "https://hagane.cloud";
const GAME: &str = "game";
const DECODED_CAP: usize = 256 * 1024 * 1024;
const PARALLEL: usize = 8;
const BACKGROUND_PARALLEL: usize = 2;
const TABLE_POOLS: [&str; 3] = ["csv", "tsv", "json"];
const HEADS: &str = "heads";
const TABLES: &str = "Loading the tables...";
const CAT_POOLS: [&str; 3] = ["roster:banners", "roster:icons", "roster:stats"];
const ENEMY_POOLS: [&str; 1] = ["roster:enemy-icons"];
const STAGE_POOLS: [&str; 2] = ["roster:items", "roster:enemy-icons"];
const STATS_POOL: &str = "roster:stats";
const STAGE_NEEDS: [&str; 1] = ["preset"];
const MAANIM: &str = ".maanim";
const PNG: &str = ".png";
const ATTACK: &str = "02";
const ENEMY_ATTACK: &str = "e02";
const ENEMY_ICON: &str = "enemy_icon_";
const CAT_FORMS: [char; 5] = ['f', 'c', 's', 'u', 'm'];
const UNIT_DIGITS: usize = 3;
const LANGUAGE_CODE: usize = 2;
const CATS: &str = "cats";
const CAT_SCAN: &str = "scan-cats.bin";
const ENEMY_SCAN: &str = "scan-enemies.bin";
const STAGE_SCAN: &str = "scan-stages.bin";
const SCANS: [&str; 3] = [CAT_SCAN, ENEMY_SCAN, STAGE_SCAN];
const ENEMIES: &str = "enemies";
const INDEX_FILE: &str = "index.bin";
const UNENCODABLE: &str = "the reply could not be encoded";
pub(crate) const NAME: &str = "data";
pub(crate) const LOGGED_NAME: &str = "data-logged";
pub(crate) const LATEST_FILE: &str = "latest-battle.bin";
pub(crate) const UPLOADED_FILE: &str = "latest-battle.bcv";
pub(crate) const UPLOADED_NAME: &str = "latest-battle.name";
pub(crate) const LATEST_FILES: &str = "latest-battle.files";
const ETAG: &str = "etag";
const DONE: &str = "done";
const VALUE: &str = "value";

type Listener = Closure<dyn FnMut(MessageEvent)>;
type Kept = (&'static str, Vec<u8>);

#[derive(Default)]
struct Decoded {
    frames: HashMap<usize, Arc<[u8]>>,
    order: VecDeque<usize>,
    bytes: usize,
}

pub(crate) struct Engine {
    pub(crate) vault: Arc<Vault>,
    pub(crate) loader: Arc<ChunkLoader>,
    cache: Option<FileSystemDirectoryHandle>,
    root: Option<FileSystemDirectoryHandle>,
}

struct ChunkRef {
    name: Box<str>,
    address: Box<str>,
}

#[derive(Serialize, Deserialize)]
struct SideRef {
    kind: Box<str>,
    address: Box<str>,
    size: u64,
    raw: u64,
    files: Vec<(Box<str>, u32, u32)>,
}

struct Catalog {
    id: Box<str>,
    chunks: Vec<ChunkRef>,
    sidecars: Vec<SideRef>,
}

struct Sidefiles {
    raw: Vec<u8>,
    index: HashMap<Box<str>, (usize, usize)>,
}

#[derive(Default)]
struct Needs {
    frames: Vec<usize>,
    sides: Vec<&'static str>,
}

#[derive(Serialize, Deserialize)]
struct Snapshot {
    etag: Box<str>,
    id: Box<str>,
    chunks: Vec<(Box<str>, Box<str>)>,
    frames: Vec<(u32, u64, u64, u64)>,
    pools: Vec<(Box<str>, Vec<u32>)>,
    names: Vec<(Box<str>, u32, u32, u32)>,
    sidecars: Vec<SideRef>,
}

#[derive(Clone, Copy)]
struct Located {
    chunk: usize,
    offset: u64,
    size: u64,
    raw: u64,
}

#[derive(Clone, Copy)]
struct Placement {
    frame: usize,
    offset: usize,
    size: usize,
}

#[derive(Default)]
pub(crate) struct Layer {
    pub(crate) files: HashMap<Box<str>, Arc<[u8]>>,
    pub(crate) withheld: HashSet<Box<str>>,
}

pub(crate) struct ChunkLoader {
    catalog: Arc<Catalog>,
    frames: Vec<Located>,
    index: HashMap<Box<str>, Placement>,
    sorted: Vec<Box<str>>,
    pools: HashMap<Box<str>, Vec<usize>>,
    heads: Option<Sidefiles>,
    sides: Mutex<HashMap<Box<str>, Arc<Sidefiles>>>,
    layer: RwLock<Arc<Layer>>,
}

fn cache_key(chunk: &str, offset: u64) -> String {
    format!("{chunk}.{offset}")
}

thread_local! {
    static SCOPE: RefCell<Option<DedicatedWorkerGlobalScope>> = const { RefCell::new(None) };
    static ENGINE: RefCell<Option<Rc<Engine>>> = const { RefCell::new(None) };
    static FAULT: RefCell<Option<String>> = const { RefCell::new(None) };
    static BACKLOG: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
    static FRAME: RefCell<Option<Rc<RgbaImage>>> = const { RefCell::new(None) };
    static SHEETS: RefCell<HashMap<Box<str>, Rc<Vec<SpriteSheet>>>> = RefCell::new(HashMap::new());
    static PRIORITY: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static HANDLES: RefCell<HashMap<usize, FileSystemSyncAccessHandle>> = RefCell::new(HashMap::new());
    static CACHED: RefCell<HashMap<usize, Box<str>>> = RefCell::new(HashMap::new());
    static CACHE_DIR: RefCell<Option<FileSystemDirectoryHandle>> = const { RefCell::new(None) };
    static QUEUE: RefCell<Vec<(usize, Vec<u8>)>> = const { RefCell::new(Vec::new()) };
    static DECODED: RefCell<Decoded> = RefCell::new(Decoded::default());
    static ONMESSAGE: RefCell<Option<Listener>> = const { RefCell::new(None) };
    static PENDING: RefCell<HashMap<usize, Vec<oneshot::Sender<()>>>> = RefCell::new(HashMap::new());
    static SIDE_PENDING: RefCell<HashMap<Box<str>, Vec<oneshot::Sender<()>>>> = RefCell::new(HashMap::new());
    static FOREGROUND: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static IDLE: RefCell<Vec<oneshot::Sender<()>>> = const { RefCell::new(Vec::new()) };
}

pub(crate) fn run(scope: DedicatedWorkerGlobalScope) {
    console_error_panic_hook::set_once();

    if scope.name() == LOGGED_NAME {
        tracing_wasm::set_as_global_default_with_config(WASMLayerConfigBuilder::new().set_report_logs_in_timings(false).build());
    }

    let listener = Listener::new(|event: MessageEvent| received(event));

    scope.set_onmessage(Some(listener.as_ref().unchecked_ref()));
    ONMESSAGE.with(|slot| *slot.borrow_mut() = Some(listener));
    SCOPE.with(|slot| *slot.borrow_mut() = Some(scope.clone()));
    post(Answer { id: HELLO, reply: Reply::Ready });

    spawn_local(boot(scope));
}

fn fail(reason: String) {
    error!("{reason}");
    FAULT.with(|slot| *slot.borrow_mut() = Some(reason));
    drain();
}

fn phase(label: &str, done: usize, total: usize) {
    post(Answer { id: HELLO, reply: Reply::Phase { label: Box::from(label), done, total } });
}

fn grouped(count: usize) -> String {
    let digits = count.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);

    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }

        out.push(digit);
    }

    out
}

fn snapshot_of(manifest: &Manifest, etag: Box<str>) -> Result<Snapshot, String> {
    if let Some(chunk) = manifest.chunks.iter().find(|chunk| !chunk.framed()) {
        return Err(format!("{} has no frame index; the payload predates ranged fetching", chunk.name));
    }

    let mut frames = Vec::new();
    let mut names: Vec<(Box<str>, u32, u32, u32)> = Vec::new();
    let mut pools: HashMap<Box<str>, Vec<u32>> = HashMap::new();

    for (at, chunk) in manifest.chunks.iter().enumerate() {
        for frame in &chunk.frames {
            let id = frames.len() as u32;

            frames.push((at as u32, frame.offset, frame.size, frame.raw));

            if let Some(pool) = &frame.pool {
                pools.entry(Box::from(pool.as_str())).or_default().push(id);
            }

            names.extend(frame.files.iter().map(|placed| (Box::from(placed.0.as_str()), id, placed.1 as u32, placed.2 as u32)));
        }
    }

    names.sort_unstable_by(|left, right| left.0.cmp(&right.0));

    let sidecars = manifest
        .sidecars
        .iter()
        .map(|sidecar| SideRef {
            kind: Box::from(sidecar.kind.as_str()),
            address: Box::from(sidecar.address.as_str()),
            size: sidecar.size,
            raw: sidecar.raw,
            files: sidecar.files.iter().map(|placed| (Box::from(placed.0.as_str()), placed.1 as u32, placed.2 as u32)).collect(),
        })
        .collect();

    Ok(Snapshot {
        etag,
        id: Box::from(manifest.id.as_str()),
        chunks: manifest.chunks.iter().map(|chunk| (Box::from(chunk.name.as_str()), Box::from(chunk.address.as_str()))).collect(),
        frames,
        pools: pools.into_iter().collect(),
        names,
        sidecars,
    })
}

async fn download_index(scope: &DedicatedWorkerGlobalScope, url: &str, known: Option<Box<str>>) -> Result<Snapshot, String> {
    phase("Downloading the file index...", 0, 0);

    let response = fetch_response(scope, "GET", url, None, Some(RequestCache::NoCache)).await?;

    if response.status() != 200 {
        return Err(format!("[{}] {url}", response.status()));
    }

    let etag = response.headers().get(ETAG).ok().flatten().map(|held| strong(&held)).or(known).unwrap_or_default();
    let bytes = body(&response, url).await?;

    phase("Reading the file index...", 0, 0);

    let manifest = Manifest::read(&bytes)?;

    info!(id = %manifest.id, chunks = manifest.chunks.len(), files = manifest.files, "File host manifest loaded");

    snapshot_of(&manifest, etag)
}

async fn boot(scope: DedicatedWorkerGlobalScope) {
    let started = now();
    let url = format!("{HOST}/{MANIFEST}");

    phase("Checking the file index...", 0, 0);

    let probe = begin_head(&scope, &url);
    let root: Option<FileSystemDirectoryHandle> = JsFuture::from(scope.navigator().storage().get_directory()).await.ok().and_then(|value| value.dyn_into().ok());
    let kept = match &root {
        Some(root) => read_root(root, INDEX_FILE).await.and_then(|bytes| protocol::decode::<Snapshot>(&bytes)),
        None => None,
    };
    let outcome = match kept {
        Some(snapshot) => {
            let etag = finish_head(probe).await;

            if etag.as_deref() == Some(&*snapshot.etag) {
                info!(ms = (now() - started) as u32, "File index reused from browser storage");
                Ok((snapshot, false))
            } else {
                download_index(&scope, &url, etag).await.map(|snapshot| (snapshot, true))
            }
        }
        None => download_index(&scope, &url, None).await.map(|snapshot| (snapshot, true)),
    };
    let (snapshot, fresh) = match outcome {
        Ok(held) => held,
        Err(reason) => return fail(format!("The file host manifest could not be loaded: {reason}")),
    };
    let stored = fresh.then(|| protocol::encode(&snapshot)).flatten();
    let catalog = Arc::new(Catalog {
        id: snapshot.id,
        chunks: snapshot.chunks.into_iter().map(|(name, address)| ChunkRef { name, address }).collect(),
        sidecars: snapshot.sidecars,
    });
    let frames: Vec<Located> = snapshot.frames.iter().map(|&(chunk, offset, size, raw)| Located { chunk: chunk as usize, offset, size, raw }).collect();
    let pools: HashMap<Box<str>, Vec<usize>> = snapshot.pools.into_iter().map(|(pool, held)| (pool, held.into_iter().map(|id| id as usize).collect())).collect();
    let mut index: HashMap<Box<str>, Placement> = HashMap::with_capacity(snapshot.names.len());
    let mut sorted: Vec<Box<str>> = Vec::with_capacity(snapshot.names.len());

    for (name, frame, offset, size) in snapshot.names {
        index.insert(name.clone(), Placement { frame: frame as usize, offset: offset as usize, size: size as usize });
        sorted.push(name);
    }

    if !sorted.is_sorted() {
        sorted.sort_unstable();
    }

    let indexed = now();

    phase("Opening browser storage...", 0, 0);

    let (opened, heads) = join(prepare(root.as_ref(), &catalog, &frames), sidecar(&scope, root.as_ref(), &catalog, HEADS, &|_| {})).await;
    let (cache, stale) = match opened {
        Ok((dir, stale)) => (Some(dir), stale),
        Err(err) => {
            warn!("The browser storage could not be opened, chunks will be fetched every visit: {:?}", err);
            (None, Vec::new())
        }
    };

    phase(&format!("Mounting {} files...", grouped(sorted.len())), 0, 0);

    let vault = Vault::with_priority(&[]);

    if let Err(err) = vault.vfs.create(Lazy { key: GAME, names: sorted.clone(), withheld: Vec::new() }) {
        return fail(format!("The game mount could not be created: {err}"));
    }

    let loader = Arc::new(ChunkLoader { catalog, frames, index, sorted, pools, heads, sides: Mutex::new(HashMap::new()), layer: RwLock::new(Arc::default()) });

    vault.vfs.install(Arc::clone(&loader) as Arc<dyn Loader>);

    let engine = Rc::new(Engine { vault: Arc::new(vault), loader, cache, root: root.clone() });

    ENGINE.with(|slot| *slot.borrow_mut() = Some(Rc::clone(&engine)));
    info!(storage_ms = (now() - indexed) as u32, total_ms = (now() - started) as u32, "Data engine ready");
    drain();
    spawn_local(warm_tables(Rc::clone(&engine)));

    if let (Some(root), Some(bytes)) = (&root, stored)
        && let Err(err) = write_root(root, INDEX_FILE, &bytes).await
    {
        warn!("The file index could not be stored in browser storage: {:?}", err);
    }

    if let Some(root) = &root {
        sweep(root, &engine.loader.catalog, stale).await;
    }
}

async fn warm_tables(engine: Rc<Engine>) {
    let tables = engine.loader.pooled(|pool| TABLE_POOLS.contains(&pool));
    let total = tables.len().max(1);

    phase(TABLES, 0, total);
    warm_frames(&engine, tables, false, |_, done, _| phase(TABLES, done.min(total), total)).await;
    phase(TABLES, total, total);
}

fn drain() {
    let backlog = BACKLOG.with(|slot| std::mem::take(&mut *slot.borrow_mut()));

    for call in backlog {
        handle(call);
    }
}

fn received(event: MessageEvent) {
    let bytes = Uint8Array::new(&event.data()).to_vec();
    let Some(call) = protocol::decode::<Call>(&bytes) else {
        warn!("A message from the page could not be decoded");
        return;
    };
    let ready = ENGINE.with(|slot| slot.borrow().is_some()) || FAULT.with(|slot| slot.borrow().is_some());

    if !ready {
        BACKLOG.with(|slot| slot.borrow_mut().push(call));
        return;
    }

    handle(call);
}

fn handle(call: Call) {
    spawn_local(serve(call));
}

async fn serve(call: Call) {
    let engine = ENGINE.with(|slot| slot.borrow().clone());
    let Some(engine) = engine else {
        let reason = FAULT.with(|slot| slot.borrow().clone()).unwrap_or_else(|| "the data engine is not running".to_owned());

        post(Answer { id: call.id, reply: Reply::Failed(reason) });
        return;
    };
    let request = match call.request {
        Request::Battle(command) => {
            crate::arena::command(&engine, command);
            post(Answer { id: call.id, reply: Reply::Warmed });

            return;
        }
        Request::Mods(command) => {
            post(Answer { id: call.id, reply: crate::mods::command(&engine, command).await });

            return;
        }
        request => request,
    };
    let call = Call { id: call.id, request };

    if let Some(reply) = recalled(&engine, &call.request).await {
        let afterwards = afterwards(&engine.loader, &call.request);

        post(Answer { id: call.id, reply });

        if !afterwards.is_empty() {
            warm_frames(&engine, afterwards, true, |_, _, _| {}).await;
        }

        return;
    }

    let needs = needs(&engine, &call.request);
    let afterwards = afterwards(&engine.loader, &call.request);
    let scan = matches!(call.request, Request::Cats(_) | Request::Enemies(_) | Request::Stages(_));
    let total = if scan { outstanding(&engine, &needs) } else { 0 };
    let done = std::cell::Cell::new(0u64);
    let id = call.id;
    let advance = |bytes: u64| {
        if total > 0 {
            done.set((done.get() + bytes).min(total));
            loading(id, done.get(), total);
        }
    };

    if total > 0 {
        loading(id, 0, total);
    }

    let sides = async {
        for kind in &needs.sides {
            let loaded = engine.loader.sides.lock().unwrap_or_else(PoisonError::into_inner).contains_key(*kind);
            let streamed = std::cell::Cell::new(0u64);

            ensure_side(&engine, kind, &|bytes| {
                streamed.set(streamed.get() + bytes);
                advance(bytes);
            })
            .await;

            if !loaded {
                advance(engine.loader.side_size(kind).saturating_sub(streamed.get()));
            }
        }
    };
    let frames = warm_frames(&engine, needs.frames, false, |frame, _, _| advance(engine.loader.frame_size(frame)));

    join(sides, frames).await;

    let (reply, kept) = answer(&engine, call.id, call.request);

    post(Answer { id: call.id, reply });
    spawn_local(persist());

    if let Some((name, bytes)) = kept {
        spawn_local(keep_scan(name, bytes));
    }

    if !afterwards.is_empty() {
        warm_frames(&engine, afterwards, true, |_, _, _| {}).await;
    }
}

fn outstanding(engine: &Engine, needs: &Needs) -> u64 {
    let loaded: HashSet<Box<str>> = engine.loader.sides.lock().unwrap_or_else(PoisonError::into_inner).keys().cloned().collect();
    let sides: u64 = needs.sides.iter().filter(|kind| !loaded.contains(**kind)).map(|kind| engine.loader.side_size(kind)).sum();
    let frames: u64 = needs.frames.iter().copied().collect::<HashSet<usize>>().into_iter().filter(|frame| !decoded_now(*frame)).map(|frame| engine.loader.frame_size(frame)).sum();

    sides + frames
}

fn needs(engine: &Engine, request: &Request) -> Needs {
    let loader = &engine.loader;

    match request {
        Request::Battle(_) | Request::Mods(_) | Request::Names => Needs::default(),
        Request::Fetch(names) | Request::Warm(names) => Needs { frames: loader.frames_of(names.iter().map(Box::as_ref)), sides: Vec::new() },
        Request::Picture { kind, name } => Needs {
            frames: match kind {
                PictureKind::Cut { sheet, .. } => loader.frames_of([name.as_ref(), sheet.as_ref()]),
                _ => loader.frames_of([name.as_ref()]),
            },
            sides: Vec::new(),
        },
        Request::Rig { png, cut, model } => Needs { frames: loader.frames_of([png.as_ref(), cut.as_ref(), model.as_ref()]), sides: Vec::new() },
        Request::Cats(config) => {
            localize(engine, &config.language_priority);

            let mut frames = loader.pooled(|pool| TABLE_POOLS.contains(&pool) || pool == STATS_POOL);

            frames.extend(foreseen(engine, cat_attack, CATS));
            Needs { frames, sides: vec![CATS] }
        }
        Request::Enemies(config) => {
            localize(engine, &config.language_priority);

            let mut frames = loader.pooled(|pool| TABLE_POOLS.contains(&pool));

            frames.extend(foreseen(engine, enemy_attack, ENEMIES));
            frames.extend(foreseen(engine, enemy_icon, HEADS));
            Needs { frames, sides: vec![ENEMIES] }
        }
        Request::Stages(config) => {
            localize(engine, &config.language_priority);

            Needs { frames: loader.pooled(|pool| TABLE_POOLS.contains(&pool) || STAGE_NEEDS.contains(&pool)), sides: Vec::new() }
        }
    }
}

fn foreseen(engine: &Engine, shape: fn(&str) -> Option<String>, kind: &str) -> Vec<usize> {
    let covered: HashSet<&str> = engine.loader.catalog.sidecars.iter().filter(|side| *side.kind == *kind).flat_map(|side| side.files.iter().map(|(name, _, _)| name.as_ref())).collect();
    let bases: HashSet<String> = engine.loader.sorted.iter().filter_map(|name| shape(name)).collect();

    bases
        .iter()
        .filter_map(|base| engine.vault.vfs.find(base))
        .filter_map(|path| path.file_name().and_then(|name| name.to_str()).map(str::to_owned))
        .filter(|name| !covered.contains(name.as_str()))
        .filter_map(|name| engine.loader.index.get(name.as_str()).map(|place| place.frame))
        .collect()
}

fn unlocalized(stem: &str) -> &str {
    match stem.rsplit_once('_') {
        Some((head, tail)) if !tail.is_empty() && tail.len() <= LANGUAGE_CODE && tail.bytes().all(|byte| byte.is_ascii_lowercase()) => head,
        _ => stem,
    }
}

fn numbered(id: &str) -> bool {
    id.len() == UNIT_DIGITS && id.bytes().all(|byte| byte.is_ascii_digit())
}

fn cat_attack(name: &str) -> Option<String> {
    let base = unlocalized(name.strip_suffix(MAANIM)?);
    let (id, form) = base.split_once('_')?;
    let shaped = numbered(id) && form.len() == 3 && form.starts_with(CAT_FORMS) && form.ends_with(ATTACK);

    shaped.then(|| format!("{base}{MAANIM}"))
}

fn enemy_attack(name: &str) -> Option<String> {
    let base = unlocalized(name.strip_suffix(MAANIM)?);
    let (id, form) = base.split_once('_')?;

    (numbered(id) && form == ENEMY_ATTACK).then(|| format!("{base}{MAANIM}"))
}

fn enemy_icon(name: &str) -> Option<String> {
    let base = unlocalized(name.strip_suffix(PNG)?);

    numbered(base.strip_prefix(ENEMY_ICON)?).then(|| format!("{base}{PNG}"))
}

fn loading(id: u32, done: u64, total: u64) {
    post(Answer { id, reply: Reply::Progress { done: done as usize, total: total as usize, scanning: false } });
}

fn afterwards(loader: &ChunkLoader, request: &Request) -> Vec<usize> {
    match request {
        Request::Cats(_) => loader.pooled(|pool| CAT_POOLS.contains(&pool)),
        Request::Enemies(_) => loader.pooled(|pool| ENEMY_POOLS.contains(&pool)),
        Request::Stages(_) => loader.pooled(|pool| STAGE_POOLS.contains(&pool)),
        _ => Vec::new(),
    }
}

async fn ensure_side(engine: &Rc<Engine>, kind: &'static str, advance: &dyn Fn(u64)) {
    if engine.loader.sides.lock().unwrap_or_else(PoisonError::into_inner).contains_key(kind) {
        return;
    }

    let waiting = SIDE_PENDING.with(|pending| match pending.borrow_mut().entry(Box::from(kind)) {
        std::collections::hash_map::Entry::Occupied(mut held) => {
            let (sender, receiver) = oneshot::channel();

            held.get_mut().push(sender);
            Some(receiver)
        }
        std::collections::hash_map::Entry::Vacant(slot) => {
            slot.insert(Vec::new());
            None
        }
    });

    if let Some(receiver) = waiting {
        let _ = receiver.await;
        return;
    }

    let scope = SCOPE.with(|slot| slot.borrow().clone());

    if let Some(scope) = scope
        && let Some(loaded) = sidecar(&scope, engine.root.as_ref(), &engine.loader.catalog, kind, advance).await
    {
        engine.loader.sides.lock().unwrap_or_else(PoisonError::into_inner).insert(Box::from(kind), Arc::new(loaded));
    }

    let waiters = SIDE_PENDING.with(|pending| pending.borrow_mut().remove(kind)).unwrap_or_default();

    for sender in waiters {
        let _ = sender.send(());
    }
}

async fn sidecar(scope: &DedicatedWorkerGlobalScope, root: Option<&FileSystemDirectoryHandle>, catalog: &Catalog, kind: &str, advance: &dyn Fn(u64)) -> Option<Sidefiles> {
    let side = catalog.sidecars.iter().find(|side| *side.kind == *kind)?;
    let started = now();
    let cached = match root {
        Some(root) => read_root(root, &side.address).await,
        None => None,
    };
    let (bytes, fresh) = match cached {
        Some(bytes) => (bytes, false),
        None => match fetch_streamed(scope, &format!("{HOST}/{}", side.address), side.size, advance).await {
            Ok(bytes) => (bytes, true),
            Err(reason) => {
                warn!("The {kind} sidecar could not be fetched, scans will read whole files: {}", reason);
                return None;
            }
        },
    };
    let raw = match inflate(&bytes, side.raw) {
        Ok(raw) if raw.len() as u64 == side.raw => raw,
        Ok(raw) => {
            warn!("The {kind} sidecar inflated to {} bytes instead of {}", raw.len(), side.raw);
            return None;
        }
        Err(reason) => {
            warn!("The {kind} sidecar could not be inflated: {}", reason);
            return None;
        }
    };
    let index = side.files.iter().map(|(name, offset, size)| (name.clone(), (*offset as usize, *size as usize))).collect();

    info!(kind, files = side.files.len(), kb = bytes.len() / 1024, fresh, ms = (now() - started) as u32, "A sidecar was loaded");

    if fresh && let Some(root) = root && let Err(err) = write_root(root, &side.address, &bytes).await {
        warn!("The {kind} sidecar could not be stored in browser storage: {:?}", err);
    }

    Some(Sidefiles { raw, index })
}

pub(crate) async fn warm_frames(engine: &Rc<Engine>, frames: Vec<usize>, background: bool, report: impl Fn(usize, usize, usize)) {
    let mut waits = Vec::new();
    let mut fresh = Vec::new();
    let mut seen = HashSet::new();

    for frame in frames {
        if !seen.insert(frame) || decoded_now(frame) || (background && on_disk(frame)) {
            continue;
        }

        PENDING.with(|pending| match pending.borrow_mut().entry(frame) {
            std::collections::hash_map::Entry::Occupied(mut waiting) => {
                let (sender, receiver) = oneshot::channel();

                waiting.get_mut().push(sender);
                waits.push((frame, receiver));
            }
            std::collections::hash_map::Entry::Vacant(slot) => {
                slot.insert(Vec::new());
                fresh.push(frame);
            }
        });
    }

    if fresh.is_empty() && waits.is_empty() {
        return;
    }

    let started = now();
    let count = fresh.len();
    let total = count + waits.len();
    let done = std::cell::Cell::new(0usize);
    let fetching = stream::iter(fresh)
        .map(|frame| async move {
            engine.loader.acquire(frame, background).await;
            notify(frame);
            frame
        })
        .buffer_unordered(if background { BACKGROUND_PARALLEL } else { PARALLEL });
    let waiting: FuturesUnordered<_> = waits
        .into_iter()
        .map(|(frame, receiver)| async move {
            let _ = receiver.await;
            frame
        })
        .collect();

    stream::select(fetching, waiting)
        .for_each(|frame| {
            done.set(done.get() + 1);
            report(frame, done.get(), total);
            future::ready(())
        })
        .await;

    if count > 0 {
        debug!(frames = count, ms = (now() - started) as u32, "Frames were warmed in parallel");
    }

    spawn_local(persist());
}

fn decoded_now(frame: usize) -> bool {
    DECODED.with(|cache| cache.borrow().frames.contains_key(&frame))
}

fn on_disk(frame: usize) -> bool {
    CACHED.with(|cached| cached.borrow().contains_key(&frame)) || HANDLES.with(|handles| handles.borrow().contains_key(&frame))
}

pub(crate) async fn breathe() {
    let Some(scope) = SCOPE.with(|slot| slot.borrow().clone()) else {
        return;
    };
    let promise = Promise::new(&mut |resolve: Function, _reject: Function| {
        if scope.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 0).is_err() {
            let _ = resolve.call0(&JsValue::NULL);
        }
    });

    let _ = JsFuture::from(promise).await;
}

async fn yield_to_foreground() {
    loop {
        if FOREGROUND.with(|count| count.get()) == 0 {
            return;
        }

        let (sender, receiver) = oneshot::channel();

        IDLE.with(|idle| idle.borrow_mut().push(sender));

        let _ = receiver.await;
    }
}

fn foreground_done() {
    let remaining = FOREGROUND.with(|count| {
        count.set(count.get().saturating_sub(1));
        count.get()
    });

    if remaining == 0 {
        for sender in IDLE.with(|idle| std::mem::take(&mut *idle.borrow_mut())) {
            let _ = sender.send(());
        }
    }
}

fn notify(frame: usize) {
    let waiting = PENDING.with(|pending| pending.borrow_mut().remove(&frame)).unwrap_or_default();

    for sender in waiting {
        let _ = sender.send(());
    }
}

fn answer(engine: &Engine, id: u32, request: Request) -> (Reply, Option<Kept>) {
    let keep = !engine.loader.layered_now();
    let kept = |name: &'static str, payload: Option<Vec<u8>>| payload.filter(|_| keep).map(|bytes| (name, bytes));

    match request {
        Request::Battle(_) | Request::Mods(_) | Request::Names => (Reply::Names { id: engine.loader.catalog.id.clone(), names: engine.loader.sorted.clone() }, None),
        Request::Fetch(names) => {
            (Reply::Files(names.into_iter().filter_map(|name| engine.loader.load(&name).map(|bytes| (name, bytes.to_vec()))).collect()), None)
        }
        Request::Picture { kind, name } => {
            let started = now();
            let prepared = picture(engine, kind, &name);

            debug!(%name, ms = (now() - started) as u32, "A picture was prepared");

            (Reply::Picture(prepared), None)
        }
        Request::Warm(names) => {
            for name in &names {
                engine.loader.load(name);
            }

            (Reply::Warmed, None)
        }
        Request::Rig { png, cut, model } => (Reply::Rig(rig(engine, &png, &cut, &model)), None),
        Request::Cats(config) => {
            localize(engine, &config.language_priority);

            let scan = cat::scanner::load(config, Arc::clone(&engine.vault), move |done, total| progress(id, done, total));

            (Reply::Cats(scan.data), kept(CAT_SCAN, scan.payload))
        }
        Request::Enemies(config) => {
            localize(engine, &config.language_priority);

            let scan = enemy::scanner::load(config, Arc::clone(&engine.vault), move |done, total| progress(id, done, total));

            (Reply::Enemies(scan.data), kept(ENEMY_SCAN, scan.payload))
        }
        Request::Stages(config) => {
            localize(engine, &config.language_priority);

            let scan = stage::scanner::load(config, Arc::clone(&engine.vault), move |done, total| progress(id, done, total));

            (Reply::Stages(Box::new(scan.data)), kept(STAGE_SCAN, scan.payload))
        }
    }
}

async fn recalled(engine: &Rc<Engine>, request: &Request) -> Option<Reply> {
    let (config, name) = match request {
        Request::Cats(config) => (config, CAT_SCAN),
        Request::Enemies(config) => (config, ENEMY_SCAN),
        Request::Stages(config) => (config, STAGE_SCAN),
        _ => return None,
    };

    if engine.loader.layered_now() {
        return None;
    }

    localize(engine, &config.language_priority);

    let dir = CACHE_DIR.with(|slot| slot.borrow().clone())?;
    let bytes = read_root(&dir, name).await?;
    let started = now();
    let key = engine.vault.key(config);
    let reply = match request {
        Request::Cats(_) => {
            warm_frames(engine, engine.loader.pooled(|pool| TABLE_POOLS.contains(&pool)), false, |_, _, _| {}).await;
            cat::scanner::restore(&bytes, &engine.vault).filter(|(hash, _)| *hash == key).map(|(_, cats)| Reply::Cats(cats))
        }
        Request::Enemies(_) => enemy::scanner::restore(&bytes).filter(|(hash, _)| *hash == key).map(|(_, enemies)| Reply::Enemies(enemies)),
        _ => stage::scanner::restore(&bytes).filter(|(hash, _)| *hash == key).map(|(_, bundle)| Reply::Stages(Box::new(bundle))),
    };

    match &reply {
        Some(_) => info!(name, ms = (now() - started) as u32, "A scan was recalled from browser storage"),
        None => debug!(name, "A kept scan no longer matches; scanning again"),
    }

    reply
}

async fn keep_scan(name: &'static str, bytes: Vec<u8>) {
    let Some(dir) = CACHE_DIR.with(|slot| slot.borrow().clone()) else {
        return;
    };

    if let Err(err) = write_root(&dir, name, &bytes).await {
        warn!("A scan could not be kept in browser storage: {:?}", err);
    }
}

fn localize(engine: &Engine, priority: &[String]) {
    let changed = PRIORITY.with(|held| {
        if *held.borrow() == priority {
            return false;
        }

        *held.borrow_mut() = priority.to_vec();
        true
    });

    if changed {
        engine.vault.priority(priority);
        SHEETS.with(|slot| slot.borrow_mut().clear());
    }
}

fn progress(id: u32, done: usize, total: usize) {
    post(Answer { id, reply: Reply::Progress { done, total, scanning: true } });
}

pub(crate) fn forget_sheets() {
    SHEETS.with(|slot| slot.borrow_mut().clear());
}

pub(crate) fn scope() -> Option<DedicatedWorkerGlobalScope> {
    SCOPE.with(|slot| slot.borrow().clone())
}

pub(crate) fn now() -> f64 {
    SCOPE.with(|slot| slot.borrow().as_ref().and_then(|scope| scope.performance()).map_or(0.0, |performance| performance.now()))
}

fn rig(engine: &Engine, png: &str, cut: &str, model: &str) -> Option<RigWire> {
    let started = now();
    let (png_bytes, cut_bytes, model_bytes) = (engine.loader.load(png)?, engine.loader.load(cut)?, engine.loader.load(model)?);
    let sheet = SheetRig::parse(&png_bytes, &cut_bytes).inspect_err(|err| warn!("The sprite sheet {png} could not be parsed: {err}")).ok()?;
    let image = sheet.image_data.as_ref()?;

    debug!(sheet = png, ms = (now() - started) as u32, "A rig was assembled");

    Some(RigWire {
        width: image.width(),
        height: image.height(),
        rgba: image.as_raw().clone(),
        version: sheet.version,
        image_name: sheet.image_name,
        cuts: sheet.cuts.into_iter().map(|cut| (cut.x, cut.y, cut.width, cut.height, cut.name)).collect(),
        opaque: sheet.opaque.into_iter().map(|held| held.map(|opaque| (opaque.x, opaque.y, opaque.width, opaque.height))).collect(),
        hull: sheet.hull,
        model: model_bytes.to_vec(),
    })
}

fn frame() -> Option<Rc<RgbaImage>> {
    FRAME.with(|slot| {
        if slot.borrow().is_none() {
            *slot.borrow_mut() = gfx::banner_frame().map(Rc::new);
        }

        slot.borrow().clone()
    })
}

fn sheets(engine: &Engine, name: &str) -> Rc<Vec<SpriteSheet>> {
    if let Some(held) = SHEETS.with(|slot| slot.borrow().get(name).cloned()) {
        return held;
    }

    let layers = Rc::new(formats::sheets(&engine.vault.vfs, name));

    if !layers.is_empty() && layers.iter().all(|layer| layer.image_data.is_some()) {
        SHEETS.with(|slot| slot.borrow_mut().insert(Box::from(name), Rc::clone(&layers)));
    }

    layers
}

fn picture(engine: &Engine, kind: PictureKind, name: &str) -> Option<Prepared> {
    let decode = |name: &str| -> Option<RgbaImage> {
        let bytes = engine.loader.load(name)?;

        image::load_from_memory(&bytes).ok().map(|image| image.to_rgba8())
    };
    let composed = match kind {
        PictureKind::CatBanner => {
            let frame = frame()?;

            gfx::cat_banner(&frame, decode(name)?)
        }
        PictureKind::EnemyBanner => {
            let frame = frame()?;

            gfx::enemy_banner(&frame, decode(name)?)
        }
        PictureKind::Trimmed => gfx::trimmed(decode(name)?)?,
        PictureKind::Boxed(canvas) => gfx::boxed(decode(name)?, canvas)?,
        PictureKind::Cut { sheet, icon } => sheets(engine, &sheet).iter().find_map(|layer| layer.crop(icon as usize))?,
    };
    let mut png = Vec::new();

    composed.write_to(&mut Cursor::new(&mut png), ImageFormat::Png).inspect_err(|err| warn!("A picture could not be encoded: {}", err)).ok()?;

    Some(Prepared { width: composed.width(), height: composed.height(), png })
}

pub(crate) fn post(answer: Answer) {
    let id = answer.id;
    let Some(bytes) = protocol::encode(&answer).or_else(|| {
        error!("A reply to the page could not be encoded");

        (id != HELLO).then(|| protocol::encode(&Answer { id, reply: Reply::Failed(UNENCODABLE.to_owned()) })).flatten()
    }) else {
        return;
    };
    let array = Uint8Array::from(&bytes[..]);
    let buffer = array.buffer();

    SCOPE.with(|slot| {
        if let Some(scope) = slot.borrow().as_ref()
            && let Err(err) = scope.post_message_with_transfer(&buffer, &Array::of1(&buffer))
        {
            error!("A reply to the page could not be posted: {:?}", err);
        }
    });
}

impl Loader for ChunkLoader {
    fn load(&self, name: &str) -> Option<Arc<[u8]>> {
        self.layered(name).unwrap_or_else(|| self.vanilla(name))
    }

    fn head(&self, name: &str, len: usize) -> Option<Vec<u8>> {
        if let Some(layered) = self.layered(name) {
            return layered.map(|bytes| bytes.get(..len).unwrap_or(&bytes).to_vec());
        }

        if let Some(heads) = &self.heads
            && let Some(&(offset, size)) = heads.index.get(name)
            && let Some(head) = heads.raw.get(offset..offset + size)
        {
            return Some(head.get(..len).unwrap_or(head).to_vec());
        }

        self.load(name).map(|bytes| bytes.get(..len).unwrap_or(&bytes).to_vec())
    }
}

impl ChunkLoader {
    fn layer(&self) -> Arc<Layer> {
        self.layer.read().map_or_else(|poisoned| Arc::clone(&poisoned.into_inner()), |held| Arc::clone(&held))
    }

    fn layered(&self, name: &str) -> Option<Option<Arc<[u8]>>> {
        let layer = self.layer();

        if layer.withheld.contains(name) {
            return Some(None);
        }

        layer.files.get(name).map(|bytes| Some(Arc::clone(bytes)))
    }

    pub(crate) fn vanilla(&self, name: &str) -> Option<Arc<[u8]>> {
        if let Some(bytes) = self.sided(name) {
            return Some(bytes);
        }

        let place = *self.index.get(name)?;
        let raw = decoded(place.frame).or_else(|| self.pull(place.frame))?;

        raw.get(place.offset..place.offset + place.size).map(Arc::from)
    }

    pub(crate) fn has_vanilla(&self, name: &str) -> bool {
        self.index.contains_key(name) || self.sides.lock().unwrap_or_else(PoisonError::into_inner).values().any(|side| side.index.contains_key(name))
    }

    pub(crate) fn vanilla_size(&self, name: &str) -> Option<usize> {
        self.index.get(name).map(|place| place.size)
    }

    pub(crate) fn modded<'a>(&self, names: impl IntoIterator<Item = &'a str>) -> Vec<(Box<str>, Arc<[u8]>)> {
        let layer = self.layer();

        names.into_iter().filter_map(|name| layer.files.get(name).map(|bytes| (Box::from(name), Arc::clone(bytes)))).collect()
    }

    pub(crate) fn relayer(&self, layer: Layer) {
        match self.layer.write() {
            Ok(mut held) => *held = Arc::new(layer),
            Err(poisoned) => *poisoned.into_inner() = Arc::new(layer),
        }
    }

    fn sided(&self, name: &str) -> Option<Arc<[u8]>> {
        let sides = self.sides.lock().unwrap_or_else(PoisonError::into_inner);

        sides.values().find_map(|side| {
            let &(offset, size) = side.index.get(name)?;

            side.raw.get(offset..offset + size).map(Arc::from)
        })
    }

    fn layered_now(&self) -> bool {
        let layer = self.layer();

        !layer.files.is_empty() || !layer.withheld.is_empty()
    }

    pub(crate) fn has(&self, name: &str) -> bool {
        self.layered(name).map_or_else(|| self.has_vanilla(name), |layered| layered.is_some())
    }

    pub(crate) fn names(&self) -> Vec<Box<str>> {
        let layer = self.layer();
        let mut names: Vec<Box<str>> = self.index.keys().filter(|name| !layer.withheld.contains(*name)).cloned().collect();

        names.extend(layer.files.keys().filter(|name| !self.index.contains_key(*name)).cloned());
        names
    }

    fn frame_size(&self, frame: usize) -> u64 {
        self.frames.get(frame).map_or(0, |located| located.size)
    }

    fn side_size(&self, kind: &str) -> u64 {
        self.catalog.sidecars.iter().find(|side| *side.kind == *kind).map_or(0, |side| side.size)
    }

    pub(crate) fn frames_of<'a>(&self, names: impl IntoIterator<Item = &'a str>) -> Vec<usize> {
        names.into_iter().filter_map(|name| self.index.get(name).map(|place| place.frame)).collect()
    }

    pub(crate) fn warmable(&self, prefixes: &[Box<str>], pools: &[Box<str>]) -> Vec<usize> {
        let mut frames: Vec<usize> = Vec::new();

        for prefix in prefixes {
            let from = self.sorted.partition_point(|name| name.as_ref() < prefix.as_ref());

            frames.extend(self.sorted[from..].iter().take_while(|name| name.starts_with(prefix.as_ref())).filter_map(|name| self.index.get(name).map(|place| place.frame)));
        }

        frames.extend(self.pooled(|pool| pools.iter().any(|wanted| wanted.as_ref() == pool)));
        frames.sort_unstable();
        frames.dedup();
        frames
    }

    fn pooled(&self, chosen: impl Fn(&str) -> bool) -> Vec<usize> {
        self.pools.iter().filter(|(pool, _)| chosen(pool)).flat_map(|(_, frames)| frames.iter().copied()).collect()
    }

    fn pull(&self, frame: usize) -> Option<Arc<[u8]>> {
        let located = *self.frames.get(frame)?;
        let chunk = self.catalog.chunks.get(located.chunk)?;
        let started = now();
        let (bytes, fresh) = match stored(frame) {
            Some(bytes) => (bytes, false),
            None => match xhr_range(&format!("{HOST}/{}", chunk.address), located.offset, located.size) {
                Ok(bytes) => (bytes, true),
                Err(reason) => {
                    error!("{} could not be fetched at {}: {}", chunk.name, located.offset, reason);
                    return None;
                }
            },
        };

        self.unpack(frame, bytes, fresh, started)
    }

    async fn acquire(&self, frame: usize, background: bool) -> Option<Arc<[u8]>> {
        if let Some(raw) = decoded(frame) {
            return Some(raw);
        }

        let located = *self.frames.get(frame)?;
        let chunk = self.catalog.chunks.get(located.chunk)?;
        let started = now();

        reopen(frame, located.size).await;

        let (bytes, fresh) = match stored(frame) {
            Some(bytes) => (bytes, false),
            None => {
                let scope = SCOPE.with(|slot| slot.borrow().clone())?;

                if background {
                    yield_to_foreground().await;
                } else {
                    FOREGROUND.with(|count| count.set(count.get() + 1));
                }

                let fetched = fetch_bytes(&scope, &format!("{HOST}/{}", chunk.address), Some((located.offset, located.size)), false).await;

                if !background {
                    foreground_done();
                }

                match fetched {
                    Ok(bytes) => (bytes, true),
                    Err(reason) => {
                        error!("{} could not be fetched at {}: {}", chunk.name, located.offset, reason);
                        return None;
                    }
                }
            }
        };

        breathe().await;

        self.unpack(frame, bytes, fresh, started)
    }

    fn unpack(&self, frame: usize, bytes: Vec<u8>, fresh: bool, started: f64) -> Option<Arc<[u8]>> {
        let located = *self.frames.get(frame)?;
        let chunk = self.catalog.chunks.get(located.chunk)?;

        if bytes.len() as u64 != located.size {
            error!("{} frame at {} arrived as {} bytes instead of {}", chunk.name, located.offset, bytes.len(), located.size);
            return None;
        }

        let fetched = now();
        let raw = match inflate(&bytes, located.raw) {
            Ok(raw) if raw.len() as u64 == located.raw => raw,
            Ok(raw) => {
                error!("{} frame at {} inflated to {} bytes instead of {}", chunk.name, located.offset, raw.len(), located.raw);
                return None;
            }
            Err(reason) => {
                error!("{} frame at {} could not be inflated: {}", chunk.name, located.offset, reason);
                return None;
            }
        };

        debug!(
            chunk = %chunk.name,
            offset = located.offset,
            fresh,
            packed_kb = bytes.len() / 1024,
            raw_kb = raw.len() / 1024,
            fetch_ms = (fetched - started) as u32,
            unpack_ms = (now() - fetched) as u32,
            "A frame was unpacked"
        );

        if fresh {
            QUEUE.with(|queue| queue.borrow_mut().push((frame, bytes)));
        }

        Some(remember(frame, raw))
    }
}

fn inflate(bytes: &[u8], expected: u64) -> Result<Vec<u8>, String> {
    let mut raw = Vec::with_capacity(usize::try_from(expected).unwrap_or(0));

    StreamingDecoder::new(bytes).read_to_end(&mut raw).map_err(|err| err.to_string())?;

    Ok(raw)
}

fn decoded(frame: usize) -> Option<Arc<[u8]>> {
    DECODED.with(|cache| {
        let mut cache = cache.borrow_mut();
        let raw = Arc::clone(cache.frames.get(&frame)?);

        cache.order.retain(|held| *held != frame);
        cache.order.push_back(frame);

        Some(raw)
    })
}

fn remember(frame: usize, raw: Vec<u8>) -> Arc<[u8]> {
    let size = raw.len();
    let raw: Arc<[u8]> = Arc::from(raw);

    DECODED.with(|cache| {
        let mut cache = cache.borrow_mut();

        while cache.bytes + size > DECODED_CAP
            && let Some(oldest) = cache.order.pop_front()
        {
            if let Some(dropped) = cache.frames.remove(&oldest) {
                cache.bytes -= dropped.len();
            }
        }

        cache.frames.insert(frame, Arc::clone(&raw));
        cache.order.push_back(frame);
        cache.bytes += size;
    });

    raw
}

async fn reopen(frame: usize, size: u64) {
    let Some(key) = CACHED.with(|cached| cached.borrow_mut().remove(&frame)) else {
        return;
    };
    let Some(dir) = CACHE_DIR.with(|slot| slot.borrow().clone()) else {
        return;
    };

    match open(&dir, key.into_string(), frame, size).await {
        Ok(Some((id, handle))) => {
            HANDLES.with(|handles| handles.borrow_mut().insert(id, handle));
        }
        Ok(None) => {}
        Err(err) => warn!("A cached frame could not be opened, it will be fetched again: {:?}", err),
    }
}

fn stored(at: usize) -> Option<Vec<u8>> {
    HANDLES.with(|handles| {
        let handles = handles.borrow();
        let handle = handles.get(&at)?;
        let size = handle.get_size().ok()? as usize;
        let mut bytes = vec![0u8; size];
        let options = FileSystemReadWriteOptions::new();

        options.set_at(0.0);

        let read = handle.read_with_u8_array_and_options(&mut bytes, &options).ok()? as usize;

        (read == size).then_some(bytes)
    })
}

async fn fetch_response(scope: &DedicatedWorkerGlobalScope, method: &str, url: &str, range: Option<(u64, u64)>, cache: Option<RequestCache>) -> Result<Response, String> {
    let init = RequestInit::new();

    init.set_method(method);

    if let Some(cache) = cache {
        init.set_cache(cache);
    }

    let request = WebRequest::new_with_str_and_init(url, &init).map_err(describe)?;

    if let Some((offset, size)) = range {
        request.headers().set("Range", &format!("bytes={}-{}", offset, offset + size.max(1) - 1)).map_err(describe)?;
    }

    JsFuture::from(scope.fetch_with_request(&request)).await.map_err(describe)?.dyn_into().map_err(|_| format!("{url} did not answer with a response"))
}

async fn body(response: &Response, url: &str) -> Result<Vec<u8>, String> {
    let buffer = JsFuture::from(response.array_buffer().map_err(describe)?).await.map_err(|err| format!("{url} body could not be read: {}", describe(err)))?;

    Ok(Uint8Array::new(&buffer).to_vec())
}

fn begin_head(scope: &DedicatedWorkerGlobalScope, url: &str) -> Option<JsFuture> {
    let init = RequestInit::new();

    init.set_method("HEAD");
    init.set_cache(RequestCache::NoStore);

    let request = WebRequest::new_with_str_and_init(url, &init).inspect_err(|err| debug!("The file index could not be probed: {}", describe(err.clone()))).ok()?;

    Some(JsFuture::from(scope.fetch_with_request(&request)))
}

async fn finish_head(probe: Option<JsFuture>) -> Option<Box<str>> {
    let response: Response = probe?.await.inspect_err(|err| debug!("The file index could not be probed: {}", describe(err.clone()))).ok()?.dyn_into().ok()?;

    (response.status() == 200).then(|| response.headers().get(ETAG).ok().flatten().map(|held| strong(&held))).flatten()
}

fn strong(etag: &str) -> Box<str> {
    Box::from(etag.strip_prefix("W/").unwrap_or(etag))
}

async fn fetch_streamed(scope: &DedicatedWorkerGlobalScope, url: &str, expected: u64, advance: &dyn Fn(u64)) -> Result<Vec<u8>, String> {
    let response = fetch_response(scope, "GET", url, None, None).await?;

    if response.status() != 200 {
        return Err(format!("[{}] {url}", response.status()));
    }

    let Some(stream) = response.body() else {
        return body(&response, url).await;
    };
    let reader: ReadableStreamDefaultReader = stream.get_reader().dyn_into().map_err(|_| format!("{url} could not be read as a stream"))?;
    let mut bytes = Vec::with_capacity(usize::try_from(expected).unwrap_or(0));

    loop {
        let chunk = JsFuture::from(reader.read()).await.map_err(|err| format!("{url} body could not be read: {}", describe(err)))?;

        if Reflect::get(&chunk, &JsValue::from_str(DONE)).map_err(describe)?.is_truthy() {
            return Ok(bytes);
        }

        let value = Uint8Array::new(&Reflect::get(&chunk, &JsValue::from_str(VALUE)).map_err(describe)?);
        let start = bytes.len();

        bytes.resize(start + value.length() as usize, 0);
        value.copy_to(&mut bytes[start..]);
        advance(u64::from(value.length()));
    }
}

pub(crate) async fn fetch_bytes(scope: &DedicatedWorkerGlobalScope, url: &str, range: Option<(u64, u64)>, revalidate: bool) -> Result<Vec<u8>, String> {
    let response = fetch_response(scope, "GET", url, range, revalidate.then_some(RequestCache::NoCache)).await?;
    let status = response.status();
    let bytes = body(&response, url).await?;

    match (range, status) {
        (None, 200) | (Some(_), 206) => Ok(bytes),
        (Some((offset, size)), 200) => bytes.get(offset as usize..(offset + size) as usize).map(<[u8]>::to_vec).ok_or_else(|| format!("{url} ignored the range and is too short")),
        (_, other) => Err(format!("[{other}] {url}")),
    }
}

fn xhr_range(url: &str, offset: u64, size: u64) -> Result<Vec<u8>, String> {
    let request = XmlHttpRequest::new().map_err(describe)?;

    request.open_with_async("GET", url, false).map_err(describe)?;
    request.set_request_header("Range", &format!("bytes={}-{}", offset, offset + size.max(1) - 1)).map_err(describe)?;
    request.set_response_type(XmlHttpRequestResponseType::Arraybuffer);
    request.send().map_err(describe)?;

    let status = request.status().map_err(describe)?;
    let response = request.response().map_err(describe)?;
    let bytes = Uint8Array::new(&response).to_vec();

    match status {
        206 => Ok(bytes),
        200 => bytes.get(offset as usize..(offset + size) as usize).map(<[u8]>::to_vec).ok_or_else(|| format!("{url} ignored the range and is too short")),
        other => Err(format!("[{other}] {url}")),
    }
}

fn describe(err: JsValue) -> String {
    err.as_string().unwrap_or_else(|| format!("{err:?}"))
}

async fn prepare(root: Option<&FileSystemDirectoryHandle>, catalog: &Catalog, frames: &[Located]) -> Result<(FileSystemDirectoryHandle, Vec<String>), JsValue> {
    let root = root.ok_or_else(|| JsValue::from_str("the origin private file system is not available"))?;
    let options = FileSystemGetDirectoryOptions::new();

    options.set_create(true);

    let dir: FileSystemDirectoryHandle = JsFuture::from(root.get_directory_handle_with_options(&catalog.id, &options)).await?.dyn_into()?;
    let known: HashMap<String, usize> = frames.iter().enumerate().map(|(id, located)| (cache_key(&catalog.chunks[located.chunk].name, located.offset), id)).collect();
    let mut cached = HashMap::new();
    let mut stale = Vec::new();

    for key in keys(&dir).await? {
        match known.get(&key) {
            Some(&id) => {
                cached.insert(id, Box::from(key.as_str()));
            }
            None if SCANS.contains(&key.as_str()) => {}
            None => stale.push(key),
        }
    }

    info!(cached = cached.len(), "Browser storage opened");
    CACHED.with(|slot| *slot.borrow_mut() = cached);
    CACHE_DIR.with(|slot| *slot.borrow_mut() = Some(dir.clone()));

    Ok((dir, stale))
}

fn kept(key: &str, catalog: &Catalog) -> bool {
    key == &*catalog.id
        || key == INDEX_FILE
        || [LATEST_FILE, LATEST_FILES, UPLOADED_FILE, UPLOADED_NAME].contains(&key)
        || crate::mods::STORED.contains(&key)
        || catalog.sidecars.iter().any(|side| *side.address == *key)
}

async fn sweep(root: &FileSystemDirectoryHandle, catalog: &Catalog, stale: Vec<String>) {
    if let Some(dir) = CACHE_DIR.with(|slot| slot.borrow().clone()) {
        for key in stale {
            if let Err(err) = JsFuture::from(dir.remove_entry(&key)).await {
                warn!("A stale cached frame could not be removed: {:?}", err);
            }
        }
    }

    let found = match keys(root).await {
        Ok(found) => found,
        Err(err) => {
            warn!("Browser storage could not be listed for cleanup: {:?}", err);
            return;
        }
    };

    for key in found.into_iter().filter(|key| !kept(key, catalog)) {
        let options = FileSystemRemoveOptions::new();

        options.set_recursive(true);

        match JsFuture::from(root.remove_entry_with_options(&key, &options)).await {
            Ok(_) => info!(payload = key, "Dropped the cache of a payload that is no longer published"),
            Err(err) => warn!("An outdated cache could not be removed: {:?}", err),
        }
    }
}

pub(crate) async fn recall(engine: &Engine, name: &str) -> Option<Vec<u8>> {
    read_root(engine.root.as_ref()?, name).await
}

pub(crate) async fn keep(engine: &Engine, name: &str, bytes: &[u8]) -> Result<(), String> {
    let root = engine.root.as_ref().ok_or("the origin private file system is not available")?;

    write_root(root, name, bytes).await.map_err(|err| format!("{err:?}"))
}

pub(crate) async fn discard(engine: &Engine, name: &str) -> Result<(), String> {
    let root = engine.root.as_ref().ok_or("the origin private file system is not available")?;

    if JsFuture::from(root.get_file_handle(name)).await.is_err() {
        return Ok(());
    }

    JsFuture::from(root.remove_entry(name)).await.map(|_| ()).map_err(describe)
}

async fn read_root(dir: &FileSystemDirectoryHandle, name: &str) -> Option<Vec<u8>> {
    let file: FileSystemFileHandle = JsFuture::from(dir.get_file_handle(name)).await.ok()?.dyn_into().ok()?;
    let snapshot: web_sys::File = JsFuture::from(file.get_file()).await.ok()?.dyn_into().ok()?;
    let buffer = JsFuture::from(snapshot.array_buffer()).await.ok()?;

    Some(Uint8Array::new(&buffer).to_vec())
}

async fn write_root(dir: &FileSystemDirectoryHandle, name: &str, bytes: &[u8]) -> Result<(), JsValue> {
    let options = FileSystemGetFileOptions::new();

    options.set_create(true);

    let file: FileSystemFileHandle = JsFuture::from(dir.get_file_handle_with_options(name, &options)).await?.dyn_into()?;
    let handle: FileSystemSyncAccessHandle = JsFuture::from(file.create_sync_access_handle()).await?.dyn_into()?;
    let written = handle.truncate_with_u32(0).and_then(|_| handle.write_with_u8_array(bytes)).and_then(|_| handle.flush());

    handle.close();
    written
}

async fn open(dir: &FileSystemDirectoryHandle, key: String, id: usize, size: u64) -> Result<Option<(usize, FileSystemSyncAccessHandle)>, JsValue> {
    let file: FileSystemFileHandle = JsFuture::from(dir.get_file_handle(&key)).await?.dyn_into()?;
    let handle: FileSystemSyncAccessHandle = JsFuture::from(file.create_sync_access_handle()).await?.dyn_into()?;

    if handle.get_size()? as u64 == size {
        return Ok(Some((id, handle)));
    }

    handle.close();
    JsFuture::from(dir.remove_entry(&key)).await?;

    Ok(None)
}

async fn keys(dir: &FileSystemDirectoryHandle) -> Result<Vec<String>, JsValue> {
    let iterator: AsyncIterator = dir.keys();
    let mut found = Vec::new();

    loop {
        let next = JsFuture::from(iterator.next()?).await?;

        if Reflect::get(&next, &JsValue::from_str("done"))?.is_truthy() {
            return Ok(found);
        }

        if let Some(key) = Reflect::get(&next, &JsValue::from_str("value"))?.as_string() {
            found.push(key);
        }
    }
}

async fn persist() {
    let engine = ENGINE.with(|slot| slot.borrow().clone());
    let Some(dir) = engine.as_ref().and_then(|engine| engine.cache.clone()) else {
        QUEUE.with(|queue| queue.borrow_mut().clear());
        return;
    };
    let Some(engine) = engine else {
        return;
    };

    loop {
        let Some((frame, bytes)) = QUEUE.with(|queue| queue.borrow_mut().pop()) else {
            return;
        };
        let Some(located) = engine.loader.frames.get(frame) else {
            continue;
        };
        let Some(chunk) = engine.loader.catalog.chunks.get(located.chunk) else {
            continue;
        };

        if HANDLES.with(|handles| handles.borrow().contains_key(&frame)) || CACHED.with(|cached| cached.borrow().contains_key(&frame)) {
            continue;
        }

        if let Err(err) = store(&dir, frame, &cache_key(&chunk.name, located.offset), &bytes).await {
            warn!("{} could not be cached in browser storage: {:?}", chunk.name, err);
        }
    }
}

async fn store(dir: &FileSystemDirectoryHandle, at: usize, name: &str, bytes: &[u8]) -> Result<(), JsValue> {
    let options = FileSystemGetFileOptions::new();

    options.set_create(true);

    let file: FileSystemFileHandle = JsFuture::from(dir.get_file_handle_with_options(name, &options)).await?.dyn_into()?;
    let handle: FileSystemSyncAccessHandle = JsFuture::from(file.create_sync_access_handle()).await?.dyn_into()?;

    handle.truncate_with_u32(0)?;
    handle.write_with_u8_array(bytes)?;
    handle.flush()?;
    HANDLES.with(|handles| handles.borrow_mut().insert(at, handle));

    Ok(())
}
