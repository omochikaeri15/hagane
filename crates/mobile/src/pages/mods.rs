use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::thread;
use std::time::Duration;

use image::imageops;
use slint::{ComponentHandle, Image, ModelRc, SharedString, Timer, TimerMode, VecModel};
use tracing::{error, info, warn};

use foundry::common::gfx;
use foundry::common::job::{JobEvent, JobOutcome};
use foundry::common::region::Region;
use foundry::domains::mods::export::{ExportType, apk, bcm, pack};
use foundry::domains::mods::import::{self as mod_import, ModImportTab, ModPackType};
use foundry::domains::mods::{self, ModDataState};

use crate::pages::Context;
use crate::{AppWindow, ModRow, Mods, Tone, bridge, delivery, engine, images, picker};

const REGIONS: [Region; 4] = [Region::En, Region::Ja, Region::Ko, Region::Tw];
const ICON_RENDER_SIZE: u32 = 64;
const CONFIRM_MS: u64 = 3000;
const ERROR_MS: u64 = 3000;
const MODS_ROOT: &str = "mods";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Which {
    Import,
    Export,
}

pub struct State {
    data: ModDataState,
    icons: HashMap<String, Option<Image>>,
    confirm_delete: bool,
    confirm_timer: Timer,
    mounting: bool,
    bcm_path: Option<PathBuf>,
    pack_path: Option<PathBuf>,
    pack_error: Option<String>,
    error_timer: Timer,
    import_running: bool,
    import_log: String,
    compression: i64,
    export_running: bool,
    export_log: String,
    metadata_path: Option<PathBuf>,
    jobs: Option<bridge::Ticket>,
    mounted: Option<bridge::Ticket>,
}

pub type Shared = Rc<RefCell<State>>;

fn job_finished(result: Result<(), String>) -> JobEvent {
    JobEvent::Finished(result.map_or_else(JobOutcome::Failed, |()| JobOutcome::Completed))
}

fn process_icon(mod_path: &Path) -> Option<image::RgbaImage> {
    let icon_path = mods::locate(mod_path, mods::ICON)?;
    let opened = image::open(&icon_path).ok()?;
    let cropped = gfx::autocrop(opened.to_rgba8());
    let (width, height) = cropped.dimensions();
    let longest = width.max(height);

    if longest == 0 {
        return None;
    }

    let scale = ICON_RENDER_SIZE as f32 / longest as f32;
    let target_w = ((width as f32 * scale).round() as u32).max(1);
    let target_h = ((height as f32 * scale).round() as u32).max(1);

    Some(imageops::resize(&cropped, target_w, target_h, imageops::FilterType::Lanczos3))
}

impl State {
    fn new() -> Self {
        let mut data = ModDataState::default();

        data.refresh_mods();

        Self {
            data,
            icons: HashMap::new(),
            confirm_delete: false,
            confirm_timer: Timer::default(),
            mounting: false,
            bcm_path: None,
            pack_path: None,
            pack_error: None,
            error_timer: Timer::default(),
            import_running: false,
            import_log: String::new(),
            compression: bcm::BCM_COMPRESSION_DEFAULT,
            export_running: false,
            export_log: String::new(),
            metadata_path: None,
            jobs: None,
            mounted: None,
        }
    }

    fn selected_index(&self) -> Option<usize> {
        self.data.selected_mod.as_ref().and_then(|id| self.data.loaded_mods.iter().position(|held| &held.folder_name == id))
    }

    fn shown(&self) -> Vec<usize> {
        let query = self.data.search_query.to_lowercase();

        self.data.loaded_mods.iter().enumerate().filter(|(_, held)| query.is_empty() || held.folder_name.to_lowercase().contains(&query)).map(|(index, _)| index).collect()
    }

    fn mark_enabled(&mut self, active: Option<&str>) {
        for entry in &mut self.data.loaded_mods {
            entry.enabled = Some(entry.folder_name.as_str()) == active;
        }
    }

    fn can_import(&self) -> bool {
        match self.data.import.tab {
            ModImportTab::New => !self.data.import.new_mod_name.trim().is_empty(),
            ModImportTab::Bcm => self.bcm_path.is_some(),
            ModImportTab::Pack => self.pack_path.is_some(),
            ModImportTab::Adb => false,
        }
    }

    fn select(&mut self, folder: String) {
        self.metadata_path = None;
        self.confirm_delete = false;
        self.data.selected_mod = Some(folder.clone());
        self.data.rename_buffer = folder;
        self.populate_export_metadata();
    }

    fn populate_export_metadata(&mut self) {
        match self.selected_index() {
            Some(index) => {
                let meta = &self.data.loaded_mods[index].metadata;

                self.data.export.app_title = meta.title.clone();
                self.data.export.package_suffix = meta.package.clone();
            }
            None => {
                self.data.export.app_title.clear();
                self.data.export.package_suffix.clear();
            }
        }
    }

    fn write_metadata(&mut self) {
        let Some(index) = self.selected_index() else {
            return;
        };
        let folder = self.data.loaded_mods[index].folder_name.clone();

        self.data.loaded_mods[index].metadata.title = folder.clone();

        let path = self.metadata_path.get_or_insert_with(|| mods::ModMetadata::destination(Path::new(MODS_ROOT).join(&folder))).clone();

        if let Err(err) = self.data.loaded_mods[index].metadata.write(&path) {
            error!(path = %path.display(), "Failed to commit metadata: {err}");
        }
    }
}

pub fn wire(window: &AppWindow, context: &Rc<Context>) -> Shared {
    let state: Shared = Rc::new(RefCell::new(State::new()));
    let mods = window.global::<Mods>();

    mods.set_regions(ModelRc::new(VecModel::from(REGIONS.iter().map(|region| SharedString::from(region.metadata().display_name)).collect::<Vec<_>>())));
    mods.set_compression_min(bcm::BCM_COMPRESSION_MIN as i32);
    mods.set_compression_max(bcm::BCM_COMPRESSION_MAX as i32);

    {
        let held = Rc::clone(&state);
        let ctx = Rc::clone(context);
        let jobs = bridge::subscribe::<(Which, JobEvent)>(move |ui, (which, event)| job_event(ui, &held, &ctx, which, event));
        let held = Rc::clone(&state);
        let ctx = Rc::clone(context);
        let mounted = bridge::subscribe::<(String, bool, Option<String>)>(move |ui, (folder, enabled, error)| mount_finished(ui, &held, &ctx, folder, enabled, error));
        let mut inner = state.borrow_mut();

        inner.jobs = Some(jobs);
        inner.mounted = Some(mounted);
    }

    {
        let active = context.engine.mounted();
        let mut inner = state.borrow_mut();

        inner.mark_enabled(active.as_deref());

        if let Some(folder) = context.memory.get().last_mod.filter(|folder| inner.data.loaded_mods.iter().any(|held| &held.folder_name == folder)) {
            inner.select(folder);
        }
    }

    let on = |state: &Shared, context: &Rc<Context>, window: &AppWindow| (Rc::clone(state), Rc::clone(context), window.as_weak());

    let (held, ctx, weak) = on(&state, context, window);
    mods.on_select(move |index| {
        let folder = {
            let state = held.borrow();

            state.shown().get(index.max(0) as usize).and_then(|at| state.data.loaded_mods.get(*at)).map(|held| held.folder_name.clone())
        };

        if let (Some(ui), Some(folder)) = (weak.upgrade(), folder) {
            held.borrow_mut().select(folder.clone());
            ctx.memory.update(|memory| memory.last_mod = Some(folder));
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    mods.on_search_changed(move |query| {
        held.borrow_mut().data.search_query = query.to_string();

        if let Some(ui) = weak.upgrade() {
            refresh_list(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    mods.on_rename(move |value| {
        if let Some(ui) = weak.upgrade() {
            rename(&ui, &held, &ctx, value.to_string());
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    mods.on_toggle(move || {
        if let Some(ui) = weak.upgrade() {
            toggle(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    mods.on_open_export(move || {
        held.borrow_mut().populate_export_metadata();

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
            ui.global::<Mods>().set_export_open(true);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    mods.on_delete(move || {
        if let Some(ui) = weak.upgrade() {
            delete(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    mods.on_metadata(move |field, value| {
        {
            let mut state = held.borrow_mut();
            let Some(index) = state.selected_index() else {
                return;
            };
            let meta = &mut state.data.loaded_mods[index].metadata;

            match field {
                0 => meta.author = value.to_string(),
                1 => meta.version = value.to_string(),
                2 => meta.package = value.to_string(),
                _ => meta.description = value.to_string(),
            }

            state.write_metadata();
        }

        if let Some(ui) = weak.upgrade() {
            refresh_list(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    mods.on_open_add(move || {
        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
            ui.global::<Mods>().set_add_open(true);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    mods.on_add_tab_set(move |index| {
        held.borrow_mut().data.import.tab = match index {
            1 => ModImportTab::Bcm,
            2 => ModImportTab::Pack,
            _ => ModImportTab::New,
        };

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    mods.on_new_name_changed(move |value| {
        held.borrow_mut().data.import.new_mod_name = value.to_string();

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    mods.on_pack_type_set(move |index| {
        {
            let mut state = held.borrow_mut();

            state.data.import.pack_type = if index == 1 { ModPackType::Pack } else { ModPackType::Apk };
            state.pack_path = None;
            state.pack_error = None;
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    mods.on_pick_bcm(move || {
        if let Some(ui) = weak.upgrade() {
            let landing = Rc::clone(&held);

            ui.global::<Mods>().set_add_open(false);
            picker::open(
                &ui,
                &ctx,
                "Select an Archive",
                &["bcm", "zip"],
                Box::new(move |ui, ctx, path| {
                    landing.borrow_mut().bcm_path = Some(path);
                    refresh(ui, &landing, ctx);
                    ui.global::<Mods>().set_add_open(true);
                }),
            );
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    mods.on_pick_pack(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let apk = held.borrow().data.import.pack_type == ModPackType::Apk;
        let (title, exts): (&str, &[&str]) = if apk { ("Select an APK", &["apk", "xapk", "apkm"]) } else { ("Select a Pack", &["pack", "list"]) };
        let landing = Rc::clone(&held);

        ui.global::<Mods>().set_add_open(false);
        picker::open(
            &ui,
            &ctx,
            title,
            exts,
            Box::new(move |ui, ctx, path| {
                apply_source(ui, &landing, ctx, path);
                ui.global::<Mods>().set_add_open(true);
            }),
        );
    });

    let (held, ctx, weak) = on(&state, context, window);
    mods.on_start_import(move || {
        if let Some(ui) = weak.upgrade() {
            start_import(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    mods.on_export_tab_set(move |index| {
        held.borrow_mut().data.export.tab = match index {
            0 => ExportType::Apk,
            1 => ExportType::Bcm,
            _ => ExportType::Pack,
        };

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, _, _) = on(&state, context, window);
    mods.on_export_title_changed(move |value| held.borrow_mut().data.export.app_title = value.to_string());

    let (held, _, _) = on(&state, context, window);
    mods.on_export_package_changed(move |value| held.borrow_mut().data.export.package_suffix = value.to_string());

    let (held, ctx, weak) = on(&state, context, window);
    mods.on_pick_app(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let landing = Rc::clone(&held);

        ui.global::<Mods>().set_export_open(false);
        picker::open(
            &ui,
            &ctx,
            "Select an APK",
            &["apk"],
            Box::new(move |ui, ctx, path| {
                landing.borrow_mut().data.export.selected_apk = Some(path);
                refresh(ui, &landing, ctx);
                ui.global::<Mods>().set_export_open(true);
            }),
        );
    });

    let (held, _, _) = on(&state, context, window);
    mods.on_region_set(move |index| held.borrow_mut().data.export.target_region = REGIONS.get(index.max(0) as usize).copied().unwrap_or(Region::En));

    let (held, _, _) = on(&state, context, window);
    mods.on_compression_changed(move |value| held.borrow_mut().compression = i64::from(value).clamp(bcm::BCM_COMPRESSION_MIN, bcm::BCM_COMPRESSION_MAX));

    let (held, _, _) = on(&state, context, window);
    mods.on_pack_name_changed(move |value| held.borrow_mut().data.export.pack_name = value.to_string());

    let (held, ctx, weak) = on(&state, context, window);
    mods.on_start_export(move || {
        if let Some(ui) = weak.upgrade() {
            start_export(&ui, &held, &ctx);
        }
    });

    refresh(window, &state, context);

    if state.borrow().data.selected_mod.is_none() {
        mods.set_list_open(true);
    }

    state
}

fn refresh_list(ui: &AppWindow, state: &Shared, _context: &Rc<Context>) {
    let mods = ui.global::<Mods>();
    let rows: Vec<ModRow> = {
        let mut held = state.borrow_mut();
        let shown = held.shown();
        let selected = held.data.selected_mod.clone();
        let mut rows = Vec::with_capacity(shown.len());

        for at in shown {
            let (name, enabled) = {
                let entry = &held.data.loaded_mods[at];

                (entry.folder_name.clone(), entry.enabled)
            };
            let icon = match held.icons.get(&name) {
                Some(icon) => icon.clone(),
                None => {
                    let icon = process_icon(&Path::new(MODS_ROOT).join(&name)).map(|image| images::to_slint(&image));

                    held.icons.insert(name.clone(), icon.clone());
                    icon
                }
            };

            rows.push(ModRow { has_icon: icon.is_some(), icon: icon.unwrap_or_default(), enabled, selected: selected.as_deref() == Some(name.as_str()), name: name.into() });
        }

        rows
    };

    mods.set_mods(ModelRc::new(VecModel::from(rows)));
}

fn refresh(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    refresh_list(ui, state, context);

    let mods = ui.global::<Mods>();
    let held = state.borrow();

    match held.selected_index() {
        Some(index) => {
            let entry = &held.data.loaded_mods[index];

            mods.set_selected(true);
            mods.set_title(held.data.rename_buffer.as_str().into());
            mods.set_enabled(entry.enabled);
            mods.set_can_delete(!entry.enabled);
            mods.set_author(entry.metadata.author.as_str().into());
            mods.set_version(entry.metadata.version.as_str().into());
            mods.set_package(entry.metadata.package.as_str().into());
            mods.set_description(entry.metadata.description.as_str().into());
        }
        None => {
            mods.set_selected(false);
            mods.set_can_delete(false);
        }
    }

    mods.set_mounting(held.mounting);
    mods.set_delete_label(if held.confirm_delete { "Continue?".into() } else { "Delete".into() });
    mods.set_add_tab(match held.data.import.tab {
        ModImportTab::New => 0,
        ModImportTab::Bcm => 1,
        _ => 2,
    });
    mods.set_new_name(held.data.import.new_mod_name.as_str().into());
    mods.set_pack_type(if held.data.import.pack_type == ModPackType::Pack { 1 } else { 0 });

    let (bcm_label, bcm_tone) = held.bcm_path.as_deref().map_or(("Select Archive".to_owned(), Tone::Neutral), |path| (picker::label(path), Tone::Success));
    let default_pack = if held.data.import.pack_type == ModPackType::Pack { "Select Pack/List" } else { "Select Source" };
    let (pack_label, pack_tone) = match &held.pack_error {
        Some(message) => (message.clone(), Tone::Danger),
        None => held.pack_path.as_deref().map_or((default_pack.to_owned(), Tone::Neutral), |path| (picker::label(path), Tone::Success)),
    };

    mods.set_bcm_label(bcm_label.into());
    mods.set_bcm_tone(bcm_tone);
    mods.set_pack_label(pack_label.into());
    mods.set_pack_tone(pack_tone);
    mods.set_import_running(held.import_running);
    mods.set_can_import(held.can_import());
    mods.set_import_log(held.import_log.as_str().into());
    let (app_label, app_tone) = held.data.export.selected_apk.as_deref().map_or(("Select App File".to_owned(), Tone::Neutral), |path| (picker::label(path), Tone::Success));

    mods.set_export_tab(match held.data.export.tab {
        ExportType::Apk => 0,
        ExportType::Bcm => 1,
        ExportType::Pack => 2,
    });
    mods.set_export_title(held.data.export.app_title.as_str().into());
    mods.set_export_package(held.data.export.package_suffix.as_str().into());
    mods.set_app_label(app_label.into());
    mods.set_app_tone(app_tone);
    mods.set_can_apply(held.data.selected_mod.is_some() && held.data.export.selected_apk.is_some());
    mods.set_region(REGIONS.iter().position(|region| *region == held.data.export.target_region).unwrap_or(0) as i32);
    mods.set_compression(held.compression as i32);
    mods.set_pack_name(held.data.export.pack_name.as_str().into());
    mods.set_export_running(held.export_running);
    mods.set_export_log(held.export_log.as_str().into());
}

fn rename(ui: &AppWindow, state: &Shared, context: &Rc<Context>, new_name: String) {
    let outcome = {
        let mut held = state.borrow_mut();
        let Some(index) = held.selected_index() else {
            return;
        };
        let old_name = held.data.loaded_mods[index].folder_name.clone();
        let new_name = new_name.trim().to_owned();

        if held.mounting || new_name.is_empty() || new_name == old_name {
            held.data.rename_buffer = old_name;
            None
        } else {
            let mods_root = Path::new(MODS_ROOT);
            let old_path = mods_root.join(&old_name);
            let new_path = mods_root.join(&new_name);
            let recasing = new_name.eq_ignore_ascii_case(&old_name);
            let clashes = !recasing && mods::taken(mods_root, &new_name);

            if clashes || !old_path.exists() || std::fs::rename(&old_path, &new_path).is_err() {
                held.data.rename_buffer = old_name;
                None
            } else {
                let was_enabled = held.data.loaded_mods[index].enabled;

                held.metadata_path = None;
                held.data.loaded_mods[index].folder_name = new_name.clone();
                held.data.selected_mod = Some(new_name.clone());
                held.data.rename_buffer = new_name.clone();
                held.data.loaded_mods[index].metadata.title = new_name.clone();
                held.icons.remove(&old_name);

                if let Err(err) = held.data.loaded_mods[index].metadata.save(&new_path) {
                    warn!("Failed to save renamed metadata: {err}");
                }

                Some((old_name, new_name, was_enabled))
            }
        }
    };

    if let Some((old_name, new_name, was_enabled)) = outcome {
        context.memory.update(|memory| memory.last_mod = Some(new_name.clone()));

        if was_enabled {
            mount(ui, state, context, Some(old_name), Some(new_name.clone()), new_name);
        }
    }

    refresh(ui, state, context);
}

fn toggle(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let plan = {
        let mut held = state.borrow_mut();

        if held.mounting {
            return;
        }

        let Some(folder) = held.data.selected_mod.clone() else {
            return;
        };

        held.confirm_delete = false;

        let enabling = !held.data.loaded_mods.iter().any(|held| held.folder_name == folder && held.enabled);
        let previous = held.data.active_mod();

        (previous, enabling.then(|| folder.clone()), folder)
    };

    mount(ui, state, context, plan.0, plan.1, plan.2);
}

fn mount(ui: &AppWindow, state: &Shared, context: &Rc<Context>, previous: Option<String>, target: Option<String>, folder: String) {
    let ticket = {
        let mut held = state.borrow_mut();
        let Some(ticket) = held.mounted else {
            return;
        };

        held.mounting = true;
        ticket
    };

    context.engine.set_mounted(target.clone());
    context.engine.shared().uncache();
    context.memory.update(|memory| memory.mounted = target.clone());
    refresh(ui, state, context);

    let weak = ui.as_weak();

    context.engine.run(move |shared| {
        let vault = shared.vault();

        if let Some(name) = previous {
            mods::disable(&vault, &name);
        }

        let enabled = target.is_some();
        let error = target.and_then(|name| engine::mount_mod(&vault, &name).err());

        bridge::post(&weak, ticket, (folder, enabled, error));
    });
}

fn mount_finished(ui: &AppWindow, state: &Shared, context: &Rc<Context>, folder: String, enabled: bool, error: Option<String>) {
    {
        let mut held = state.borrow_mut();

        held.mounting = false;

        for entry in &mut held.data.loaded_mods {
            entry.enabled = false;
        }

        match error {
            Some(err) => {
                warn!(mod_name = %folder, "Mount failed, mod left disabled: {err}");
                context.engine.set_mounted(None);
                context.memory.update(|memory| memory.mounted = None);
                context.toast(ui, "The mod could not be mounted", true);
            }
            None => {
                if enabled && let Some(entry) = held.data.loaded_mods.iter_mut().find(|held| held.folder_name == folder) {
                    entry.enabled = true;
                }
            }
        }
    }

    refresh(ui, state, context);
    context.reload_globals(ui);
}

fn delete(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let armed = state.borrow().confirm_delete;

    if !armed {
        {
            let mut held = state.borrow_mut();
            let weak = ui.as_weak();
            let landing = Rc::clone(state);
            let ctx = Rc::clone(context);

            held.confirm_delete = true;
            held.confirm_timer.start(TimerMode::SingleShot, Duration::from_millis(CONFIRM_MS), move || {
                landing.borrow_mut().confirm_delete = false;

                if let Some(ui) = weak.upgrade() {
                    refresh(&ui, &landing, &ctx);
                }
            });
        }

        refresh(ui, state, context);
        return;
    }

    let removed = {
        let mut held = state.borrow_mut();

        held.confirm_delete = false;
        held.confirm_timer.stop();

        let Some(folder) = held.data.selected_mod.clone() else {
            return;
        };

        if held.data.loaded_mods.iter().any(|held| held.folder_name == folder && held.enabled) {
            warn!(mod_name = %folder, "Refusing to delete an enabled mod; disable it first");
            return;
        }

        held.data.selected_mod = None;
        held.icons.remove(&folder);
        folder
    };
    let path = Path::new(MODS_ROOT).join(&removed);
    let landing = Rc::clone(state);
    let ctx = Rc::clone(context);

    context.memory.update(|memory| memory.last_mod = None);
    bridge::offload(
        ui,
        "mod_delete",
        move || std::fs::remove_dir_all(&path).map_err(|err| err.to_string()),
        move |ui, outcome| {
            if let Err(err) = outcome {
                warn!("Failed to delete mod folder: {err}");
            }

            landing.borrow_mut().data.refresh_mods();
            refresh(ui, &landing, &ctx);
        },
    );
    refresh(ui, state, context);
}

fn apply_source(ui: &AppWindow, state: &Shared, context: &Rc<Context>, picked: PathBuf) {
    {
        let mut held = state.borrow_mut();

        if held.data.import.pack_type == ModPackType::Apk {
            held.pack_path = Some(picked);
            held.pack_error = None;
        } else {
            let parent = picked.parent().unwrap_or(Path::new(""));
            let stem = picked.file_stem().map_or_else(String::new, |stem| stem.to_string_lossy().into_owned());
            let pack_file = parent.join(format!("{stem}.pack"));
            let list_file = parent.join(format!("{stem}.list"));
            let missing = if !pack_file.exists() {
                Some("Missing .pack!")
            } else if !list_file.exists() {
                Some("Missing .list!")
            } else {
                None
            };

            match missing {
                Some(message) => {
                    held.pack_path = None;
                    held.pack_error = Some(message.to_owned());

                    let weak = ui.as_weak();
                    let landing = Rc::clone(state);
                    let ctx = Rc::clone(context);

                    held.error_timer.start(TimerMode::SingleShot, Duration::from_millis(ERROR_MS), move || {
                        landing.borrow_mut().pack_error = None;

                        if let Some(ui) = weak.upgrade() {
                            refresh(&ui, &landing, &ctx);
                        }
                    });
                }
                None => {
                    held.pack_path = Some(pack_file);
                    held.pack_error = None;
                }
            }
        }
    }

    refresh(ui, state, context);
}

fn begin(log: &mut String, status: &str) {
    log.clear();
    log.push_str(status);
    log.push('\n');
}

fn start_import(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let Some(ticket) = state.borrow().jobs else {
        return;
    };
    let weak = ui.as_weak();
    let emit_to = move |event: JobEvent| bridge::post(&weak, ticket, (Which::Import, event));
    let enforce = context.state.settings().game_data.enforce_key_validation;
    let spawned = {
        let mut held = state.borrow_mut();

        if held.import_running || !held.can_import() {
            return;
        }

        held.import_running = true;

        match held.data.import.tab {
            ModImportTab::New => {
                let name = held.data.import.new_mod_name.trim().to_owned();

                begin(&mut held.import_log, "Initializing mod creation...");
                thread::Builder::new().name("mod_new".to_owned()).spawn({ let emit_to = emit_to.clone(); move || {
                    let result = mod_import::run_new(name, &emit_to);

                    emit_to(job_finished(result));
                }})
            }
            ModImportTab::Bcm => {
                let Some(path) = held.bcm_path.clone() else {
                    held.import_running = false;
                    return;
                };

                begin(&mut held.import_log, &format!("Extracting {}...", picker::label(&path)));
                thread::Builder::new().name("mod_bcm".to_owned()).spawn({ let emit_to = emit_to.clone(); move || {
                    let result = mod_import::run_bcm(path, enforce, &emit_to);

                    emit_to(job_finished(result));
                }})
            }
            ModImportTab::Pack | ModImportTab::Adb => {
                let Some(path) = held.pack_path.clone() else {
                    held.import_running = false;
                    return;
                };
                let pack_type = held.data.import.pack_type;

                begin(&mut held.import_log, &format!("Processing {}...", picker::label(&path)));
                thread::Builder::new().name("mod_pack".to_owned()).spawn({ let emit_to = emit_to.clone(); move || {
                    let result = mod_import::run_pack(path, pack_type, enforce, &emit_to);

                    emit_to(job_finished(result));
                }})
            }
        }
    };

    if let Err(err) = spawned {
        error!("The mod import thread could not be started: {err}");
        state.borrow_mut().import_running = false;
    }

    refresh(ui, state, context);
}

fn start_export(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let Some(ticket) = state.borrow().jobs else {
        return;
    };
    let weak = ui.as_weak();
    let emit_to = move |event: JobEvent| bridge::post(&weak, ticket, (Which::Export, event));
    let enforce = context.state.settings().game_data.enforce_key_validation;
    let spawned = {
        let mut held = state.borrow_mut();

        if held.export_running {
            return;
        }

        let Some(mod_folder) = held.data.selected_mod.clone() else {
            return;
        };

        if held.data.export.tab == ExportType::Pack && held.data.export.pack_name.is_empty() {
            held.data.export.pack_name = "DownloadLocal".to_owned();
        }

        held.export_running = true;
        held.export_log.clear();

        match held.data.export.tab {
            ExportType::Pack => {
                let pack_name = held.data.export.pack_name.clone();
                let region = held.data.export.target_region;

                thread::Builder::new().name("mod_pack_export".to_owned()).spawn({ let emit_to = emit_to.clone(); move || {
                    let result = pack::run(mod_folder, pack_name, region, enforce, &emit_to);

                    emit_to(job_finished(result));
                }})
            }
            ExportType::Apk => {
                let Some(input) = held.data.export.selected_apk.clone() else {
                    held.export_running = false;
                    return;
                };
                let app_title = held.data.export.app_title.clone();
                let suffix = held.data.export.package_suffix.clone();
                let region = held.data.export.target_region;
                let behavior = context.state.settings().mods.export_behavior.clone();

                thread::Builder::new().name("mod_apk_export".to_owned()).spawn({ let emit_to = emit_to.clone(); move || {
                    let result = apk::run(mod_folder, input, app_title, suffix, region, behavior, enforce, &emit_to);

                    emit_to(job_finished(result));
                }})
            }
            ExportType::Bcm => {
                let app_title = held.data.export.app_title.clone();
                let compression = held.compression;

                thread::Builder::new().name("mod_bcm_export".to_owned()).spawn({ let emit_to = emit_to.clone(); move || {
                    let result = bcm::run(mod_folder, app_title, compression, &emit_to);

                    emit_to(job_finished(result));
                }})
            }
        }
    };

    if let Err(err) = spawned {
        error!("The mod export thread could not be started: {err}");
        state.borrow_mut().export_running = false;
    }

    refresh(ui, state, context);
}

fn job_event(ui: &AppWindow, state: &Shared, context: &Rc<Context>, which: Which, event: JobEvent) {
    let mut exported = false;

    {
        let mut held = state.borrow_mut();
        let State { import_log, import_running, export_log, export_running, data, .. } = &mut *held;
        let (log, running) = match which {
            Which::Import => (import_log, import_running),
            Which::Export => (export_log, export_running),
        };

        match event {
            JobEvent::Log(line) => {
                log.push_str(&line);
                log.push('\n');
            }
            JobEvent::Progress { .. } => {}
            JobEvent::Finished(outcome) => {
                *running = false;

                match outcome {
                    JobOutcome::Completed => {
                        info!("The mod job completed");
                        exported = which == Which::Export;

                        if which == Which::Import {
                            if data.import.tab == ModImportTab::New {
                                data.import.new_mod_name.clear();
                            }

                            data.refresh_mods();

                            let active = context.engine.mounted();

                            for entry in &mut data.loaded_mods {
                                entry.enabled = Some(entry.folder_name.as_str()) == active.as_deref();
                            }
                        }
                    }
                    JobOutcome::Aborted => info!("The mod job was aborted"),
                    JobOutcome::Failed(message) => {
                        warn!("The mod job failed: {message}");
                        log.push_str(&format!("ERROR: {message}\n"));
                    }
                }
            }
        }
    }

    if exported {
        delivery::announce(ui, context);
    }

    refresh(ui, state, context);
}
