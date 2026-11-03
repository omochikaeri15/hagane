use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use image::{Rgba, RgbaImage};
use slint::{ComponentHandle, Image, ModelRc, SharedString, Timer, TimerMode, VecModel};
use tracing::{error, info};

use foundry::domains::utilities::imgcut::{self, Sheet};

use crate::images;
use crate::pages::Context;
use crate::picker;
use crate::{AppWindow, Tone, Utilities, bridge};

use super::Shared;

const EMPTY_LABEL: &str = "Add a PNG and an IMGCUT to begin";
const LOADING_LABEL: &str = "Reading sprite sheet...";
const SHEET_LABEL: &str = "Spritesheet";
const EXPORT_IDLE: &str = "Export Image";
const EXPORT_BUSY: &str = "Exporting...";
const EXPORT_DONE: &str = "Exported!";
const EXPORT_FAILED: &str = "Failed!";
const FEEDBACK_MS: u64 = 2000;
const OUTLINE: Rgba<u8> = Rgba([255, 64, 64, 255]);

pub struct Loaded {
    sheet: Sheet,
    full: RgbaImage,
    labels: Vec<String>,
}

#[derive(Default)]
pub struct State {
    png: Option<PathBuf>,
    imgcut: Option<PathBuf>,
    loaded: Option<Arc<Loaded>>,
    choice: usize,
    canvas: Option<Image>,
    reading: bool,
    exporting: bool,
    feedback: Option<bool>,
    notice: Option<String>,
    timer: Timer,
}

fn outlined(sheet: &Sheet, mut image: RgbaImage) -> RgbaImage {
    let (width, height) = image.dimensions();

    for cut in &sheet.cuts {
        let right = (cut.x + cut.width).min(width.saturating_sub(1));
        let bottom = (cut.y + cut.height).min(height.saturating_sub(1));

        for x in cut.x.min(width.saturating_sub(1))..=right {
            if cut.y < height {
                image.put_pixel(x, cut.y, OUTLINE);
            }

            if bottom < height {
                image.put_pixel(x, bottom, OUTLINE);
            }
        }

        for y in cut.y.min(height.saturating_sub(1))..=bottom {
            if cut.x < width {
                image.put_pixel(cut.x, y, OUTLINE);
            }

            if right < width {
                image.put_pixel(right, y, OUTLINE);
            }
        }
    }

    image
}

fn assemble(png: &Path, cut: &Path) -> Result<Arc<Loaded>, String> {
    let sheet = imgcut::load(png, cut)?;
    let bytes = std::fs::read(png).map_err(|error| format!("Could not read {}: {}", png.display(), error))?;
    let full = image::load_from_memory(&bytes).map_err(|error| format!("Could not decode {}: {}", png.display(), error))?.to_rgba8();
    let labels: Vec<String> = std::iter::once(SHEET_LABEL.to_owned()).chain(sheet.cuts.iter().enumerate().map(|(index, cut)| cut.label(index))).collect();
    let full = outlined(&sheet, full);

    Ok(Arc::new(Loaded { sheet, full, labels }))
}

pub fn wire(window: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let utilities = window.global::<Utilities>();
    let on = |state: &Shared, context: &Rc<Context>, window: &AppWindow| (Rc::clone(state), Rc::clone(context), window.as_weak());

    let (held, _, weak) = on(state, context, window);
    utilities.on_choice_set(move |index| {
        held.borrow_mut().imgcut.choice = index.max(0) as usize;
        select(&held);

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held);
        }
    });

    let (held, _, weak) = on(state, context, window);
    utilities.on_export_cut(move || {
        if let Some(ui) = weak.upgrade() {
            export(&ui, &held);
        }
    });
}

fn read(ui: &AppWindow, state: &Shared, _context: &Rc<Context>) {
    let (png, cut) = {
        let held = state.borrow();

        (held.imgcut.png.clone(), held.imgcut.imgcut.clone())
    };
    let (Some(png), Some(cut)) = (png, cut) else {
        refresh(ui, state);
        return;
    };

    {
        let mut held = state.borrow_mut();

        held.imgcut.reading = true;
        held.imgcut.notice = None;
    }

    refresh(ui, state);

    let landing = Rc::clone(state);

    bridge::offload(ui, "imgcut", move || assemble(&png, &cut), move |ui, outcome| {
        {
            let mut held = landing.borrow_mut();

            held.imgcut.reading = false;

            match outcome {
                Ok(loaded) => {
                    held.imgcut.choice = 0;
                    held.imgcut.notice = None;
                    held.imgcut.loaded = Some(loaded);
                }
                Err(problem) => {
                    held.imgcut.loaded = None;
                    held.imgcut.canvas = None;
                    held.imgcut.notice = Some(problem);
                }
            }
        }

        select(&landing);
        refresh(ui, &landing);
    });
}

fn select(state: &Shared) {
    let mut held = state.borrow_mut();
    let Some(loaded) = held.imgcut.loaded.clone() else {
        held.imgcut.canvas = None;
        return;
    };

    held.imgcut.canvas = match held.imgcut.choice {
        0 => Some(images::to_slint(&loaded.full)),
        index => match loaded.sheet.crop(index - 1) {
            Some(cropped) => Some(images::to_slint(&cropped)),
            None => {
                held.imgcut.notice = Some("That cut has no pixels to show".to_owned());
                None
            }
        },
    };
}

fn export(ui: &AppWindow, state: &Shared) {
    let (loaded, choice) = {
        let held = state.borrow();

        if held.imgcut.exporting {
            return;
        }

        (held.imgcut.loaded.clone(), held.imgcut.choice)
    };
    let Some(loaded) = loaded else {
        return;
    };

    state.borrow_mut().imgcut.exporting = true;
    refresh(ui, state);

    let landing = Rc::clone(state);

    bridge::offload(
        ui,
        "imgcut_export",
        move || match choice {
            0 => imgcut::export_all(&loaded.sheet),
            index => imgcut::export_cut(&loaded.sheet, index - 1),
        },
        move |ui, outcome| {
            match &outcome {
                Ok(path) => info!(path = %path.display(), "Imgcut exported"),
                Err(reason) => error!("The imgcut export failed: {reason}"),
            }

            {
                let mut held = landing.borrow_mut();

                held.imgcut.exporting = false;
                held.imgcut.feedback = Some(outcome.is_ok());

                let weak = ui.as_weak();
                let settle = Rc::clone(&landing);

                held.imgcut.timer.start(TimerMode::SingleShot, Duration::from_millis(FEEDBACK_MS), move || {
                    settle.borrow_mut().imgcut.feedback = None;

                    if let Some(ui) = weak.upgrade() {
                        refresh(&ui, &settle);
                    }
                });
            }

            refresh(ui, &landing);
        },
    );
}

pub fn refresh(ui: &AppWindow, state: &Shared) {
    let utilities = ui.global::<Utilities>();
    let held = state.borrow();
    let cut = &held.imgcut;
    let face = |picked: Option<&Path>, idle: &str| picked.map_or((idle.to_owned(), Tone::Neutral), |path| (picker::label(path), Tone::Success));
    let (png_label, png_tone) = face(cut.png.as_deref(), "Add PNG");
    let (cut_label, cut_tone) = face(cut.imgcut.as_deref(), "Add IMGCUT");
    let labels: Vec<SharedString> = cut.loaded.as_ref().map(|loaded| loaded.labels.iter().map(|label| SharedString::from(label.as_str())).collect()).unwrap_or_default();
    let (export_label, export_tone) = match (cut.exporting, cut.feedback) {
        (true, _) => (EXPORT_BUSY, Tone::StatusWarning),
        (false, Some(true)) => (EXPORT_DONE, Tone::StatusSuccess),
        (false, Some(false)) => (EXPORT_FAILED, Tone::StatusDanger),
        (false, None) if cut.loaded.is_some() => (EXPORT_IDLE, Tone::Primary),
        (false, None) => (EXPORT_IDLE, Tone::Neutral),
    };
    let notice = cut.notice.clone().unwrap_or_else(|| if cut.reading { LOADING_LABEL.to_owned() } else { EMPTY_LABEL.to_owned() });

    utilities.set_cut_png_label(png_label.into());
    utilities.set_cut_png_tone(png_tone);
    utilities.set_cut_cut_label(cut_label.into());
    utilities.set_cut_cut_tone(cut_tone);
    utilities.set_cut_choices(ModelRc::new(VecModel::from(labels)));
    utilities.set_cut_choice(cut.choice as i32);
    utilities.set_cut_export_label(export_label.into());
    utilities.set_cut_export_tone(export_tone);
    utilities.set_cut_export_enabled(cut.loaded.is_some() && !cut.exporting);
    utilities.set_cut_has_canvas(cut.canvas.is_some());
    utilities.set_cut_canvas(cut.canvas.clone().unwrap_or_default());
    utilities.set_cut_notice(notice.into());
}

pub fn pick_png(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let landing = Rc::clone(state);

    picker::open(
        ui,
        context,
        "Select a PNG",
        &["png"],
        Box::new(move |ui, ctx, path| {
            {
                let mut state = landing.borrow_mut();

                if state.imgcut.imgcut.is_none() {
                    state.imgcut.imgcut = imgcut::partner(&path, "imgcut");
                }

                state.imgcut.png = Some(path);
            }

            read(ui, &landing, ctx);
        }),
    );
}

pub fn pick_cut(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let landing = Rc::clone(state);

    picker::open(
        ui,
        context,
        "Select an IMGCUT",
        &["imgcut"],
        Box::new(move |ui, ctx, path| {
            {
                let mut state = landing.borrow_mut();

                if state.imgcut.png.is_none() {
                    state.imgcut.png = imgcut::partner(&path, "png");
                }

                state.imgcut.imgcut = Some(path);
            }

            read(ui, &landing, ctx);
        }),
    );
}
