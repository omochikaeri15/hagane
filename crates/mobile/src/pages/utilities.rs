mod animation;
mod backdrop;
mod clone;
mod imgcut;

use std::cell::RefCell;
use std::rc::Rc;

use slint::ComponentHandle;

use crate::pages::Context;
use crate::{AppWindow, Utilities, viewer};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Imgcut,
    Animation,
    Background,
    Clone,
}

impl Tool {
    fn from_index(index: i32) -> Self {
        match index {
            1 => Self::Animation,
            2 => Self::Background,
            3 => Self::Clone,
            _ => Self::Imgcut,
        }
    }

    fn index(self) -> i32 {
        match self {
            Self::Imgcut => 0,
            Self::Animation => 1,
            Self::Background => 2,
            Self::Clone => 3,
        }
    }
}

pub struct State {
    tool: Tool,
    imgcut: imgcut::State,
    animation: animation::State,
    backdrop: backdrop::State,
    clone: clone::State,
}

pub type Shared = Rc<RefCell<State>>;

pub fn wire(window: &AppWindow, context: &Rc<Context>) -> Shared {
    let remembered = context.memory.get();
    let state: Shared = Rc::new(RefCell::new(State {
        tool: Tool::from_index(i32::from(remembered.utilities_tool)),
        imgcut: imgcut::State::default(),
        animation: animation::State::default(),
        backdrop: backdrop::State::new(&remembered),
        clone: clone::State::default(),
    }));
    let utilities = window.global::<Utilities>();

    {
        let (held, ctx, weak) = (Rc::clone(&state), Rc::clone(context), window.as_weak());

        utilities.on_tool_set(move |index| {
            let tool = Tool::from_index(index);

            held.borrow_mut().tool = tool;
            ctx.memory.update(|memory| memory.utilities_tool = index.clamp(0, 3) as u8);

            if let Some(ui) = weak.upgrade() {
                entered(&ui, &held, &ctx);
            }
        });
    }

    {
        let (held, ctx, weak) = (Rc::clone(&state), Rc::clone(context), window.as_weak());

        utilities.on_pick_png(move |which| {
            if let Some(ui) = weak.upgrade() {
                match which {
                    0 => imgcut::pick_png(&ui, &held, &ctx),
                    1 => animation::pick_png(&ui, &held, &ctx),
                    2 => backdrop::pick_png(&ui, &held, &ctx),
                    _ => {}
                }
            }
        });
    }

    {
        let (held, ctx, weak) = (Rc::clone(&state), Rc::clone(context), window.as_weak());

        utilities.on_pick_cut(move |which| {
            if let Some(ui) = weak.upgrade() {
                match which {
                    0 => imgcut::pick_cut(&ui, &held, &ctx),
                    1 => animation::pick_cut(&ui, &held, &ctx),
                    2 => backdrop::pick_cut(&ui, &held, &ctx),
                    _ => {}
                }
            }
        });
    }

    imgcut::wire(window, &state, context);
    animation::wire(window, &state, context);
    backdrop::wire(window, &state, context);
    clone::wire(window, &state, context);
    refresh(window, &state, context);

    state
}

pub fn entered(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let tool = state.borrow().tool;

    match tool {
        Tool::Animation => animation::show(ui, state, context),
        Tool::Background => {
            viewer::hide(context);
            backdrop::show(ui, state, context);
        }
        Tool::Imgcut | Tool::Clone => {
            viewer::hide(context);
            backdrop::hide(state);
        }
    }

    if tool != Tool::Background {
        backdrop::hide(state);
    }

    refresh(ui, state, context);
}

pub fn left(state: &Shared, context: &Rc<Context>) {
    viewer::hide(context);
    backdrop::hide(state);
}

pub fn invalidate(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    backdrop::invalidate(ui, state, context);
}

fn refresh(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    ui.global::<Utilities>().set_tool(state.borrow().tool.index());
    imgcut::refresh(ui, state);
    animation::refresh(ui, state);
    backdrop::refresh(ui, state, context);
    clone::refresh(ui, state);
}
