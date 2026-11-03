use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use slint::{ComponentHandle, Timer, TimerMode};

use foundry::domains::utilities::animation as builder;

use crate::pages::Context;
use crate::picker;
use crate::viewer;
use crate::{AppWindow, Tone, Utilities};

use super::Shared;

const CONFIRM_MS: u64 = 3000;

#[derive(Default)]
pub struct State {
    png: Option<PathBuf>,
    imgcut: Option<PathBuf>,
    mamodel: Option<PathBuf>,
    anims: Vec<PathBuf>,
    confirm: bool,
    timer: Timer,
}

pub fn wire(window: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let utilities = window.global::<Utilities>();
    let on = |state: &Shared, context: &Rc<Context>, window: &AppWindow| (Rc::clone(state), Rc::clone(context), window.as_weak());

    let (held, ctx, weak) = on(state, context, window);
    utilities.on_pick_model(move || {
        if let Some(ui) = weak.upgrade() {
            let landing = Rc::clone(&held);

            picker::open(
                &ui,
                &ctx,
                "Select a MAMODEL",
                &["mamodel"],
                Box::new(move |ui, ctx, path| {
                    landing.borrow_mut().animation.mamodel = Some(path);
                    show(ui, &landing, ctx);
                    refresh(ui, &landing);
                }),
            );
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    utilities.on_add_anim(move || {
        if let Some(ui) = weak.upgrade() {
            let landing = Rc::clone(&held);

            picker::open(
                &ui,
                &ctx,
                "Select a MAANIM",
                &["maanim"],
                Box::new(move |ui, ctx, path| {
                    {
                        let mut state = landing.borrow_mut();

                        if !state.animation.anims.contains(&path) {
                            state.animation.anims.push(path);
                        }
                    }

                    show(ui, &landing, ctx);
                    refresh(ui, &landing);
                }),
            );
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    utilities.on_remove_anim(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let armed = held.borrow().animation.confirm;

        if !armed {
            held.borrow_mut().animation.confirm = true;

            let landing = Rc::clone(&held);
            let weak_inner = ui.as_weak();

            held.borrow().animation.timer.start(TimerMode::SingleShot, Duration::from_millis(CONFIRM_MS), move || {
                landing.borrow_mut().animation.confirm = false;

                if let Some(ui) = weak_inner.upgrade() {
                    refresh(&ui, &landing);
                }
            });
            refresh(&ui, &held);
            return;
        }

        {
            let mut state = held.borrow_mut();

            state.animation.confirm = false;
            state.animation.timer.stop();

            let selected = ctx.viewer.borrow().selected_file();

            match selected {
                Some(file) => state.animation.anims.retain(|path| *path != file),
                None => state.animation.anims.clear(),
            }
        }

        show(&ui, &held, &ctx);
        refresh(&ui, &held);
    });
}

pub fn show(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let (png, cut, model, anims) = {
        let held = state.borrow();
        let animation = &held.animation;

        (animation.png.clone(), animation.imgcut.clone(), animation.mamodel.clone(), animation.anims.clone())
    };
    let frames = context.state.settings().utilities.frame_count;

    match (png, cut, model) {
        (Some(png), Some(cut), Some(model)) => {
            let key = format!("utility:{}", builder::key(&png, &cut, &model, &anims, frames));

            viewer::show_set(ui, context, key, || builder::motions(&png, &cut, &model, &anims, frames));
        }
        _ => viewer::show_set(ui, context, "utility:empty".to_owned(), Default::default),
    }
}

pub fn refresh(ui: &AppWindow, state: &Shared) {
    let utilities = ui.global::<Utilities>();
    let held = state.borrow();
    let animation = &held.animation;
    let face = |picked: Option<&Path>, idle: &str| picked.map_or((idle.to_owned(), Tone::Neutral), |path| (picker::label(path), Tone::Success));
    let (png_label, png_tone) = face(animation.png.as_deref(), "Add PNG");
    let (cut_label, cut_tone) = face(animation.imgcut.as_deref(), "Add IMGCUT");
    let (model_label, model_tone) = face(animation.mamodel.as_deref(), "Add MAMODEL");
    let (remove_label, remove_tone, remove_enabled) = if animation.anims.is_empty() {
        ("Missing MAANIM".to_owned(), Tone::Neutral, false)
    } else if animation.confirm {
        ("Continue?".to_owned(), Tone::Danger, true)
    } else {
        ("Remove MAANIM".to_owned(), Tone::Danger, true)
    };

    utilities.set_anim_png_label(png_label.into());
    utilities.set_anim_png_tone(png_tone);
    utilities.set_anim_cut_label(cut_label.into());
    utilities.set_anim_cut_tone(cut_tone);
    utilities.set_anim_model_label(model_label.into());
    utilities.set_anim_model_tone(model_tone);
    utilities.set_anim_remove_label(remove_label.into());
    utilities.set_anim_remove_tone(remove_tone);
    utilities.set_anim_remove_enabled(remove_enabled);
}

pub fn pick_png(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let landing = Rc::clone(state);

    picker::open(
        ui,
        context,
        "Select a PNG",
        &["png"],
        Box::new(move |ui, ctx, path| {
            landing.borrow_mut().animation.png = Some(path);
            show(ui, &landing, ctx);
            refresh(ui, &landing);
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
            landing.borrow_mut().animation.imgcut = Some(path);
            show(ui, &landing, ctx);
            refresh(ui, &landing);
        }),
    );
}
