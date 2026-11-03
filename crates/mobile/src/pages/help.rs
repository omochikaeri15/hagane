use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, ModelRc, SharedString, StyledText, VecModel};

use crate::pages::Context;
use crate::{AppWindow, Help};

const PAGES: [(&str, &str); 4] = [
    ("General", include_str!("../../help/0-GENERAL.md")),
    ("Import", include_str!("../../help/1-IMPORT.md")),
    ("Mods", include_str!("../../help/2-MODS.md")),
    ("Sandbox", include_str!("../../help/3-SANDBOX.md")),
];

pub struct State {
    page: usize,
    bodies: Vec<StyledText>,
}

pub type Shared = Rc<RefCell<State>>;

fn adapt(markdown: &str) -> String {
    markdown
        .lines()
        .map(|line| {
            let trimmed = line.trim_start_matches('#');

            if trimmed.len() != line.len() && trimmed.starts_with(' ') {
                format!("\n\u{a0}\n\n**{}**\n", trimmed.trim())
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn styled(markdown: &str) -> StyledText {
    let adapted = adapt(markdown);

    StyledText::from_markdown(&adapted).unwrap_or_else(|_| StyledText::from_plain_text(&adapted))
}

pub fn wire(window: &AppWindow, context: &Rc<Context>) -> Shared {
    let page = context.memory.get().help_page.min(PAGES.len().saturating_sub(1));
    let state: Shared = Rc::new(RefCell::new(State { page, bodies: PAGES.iter().map(|(_, body)| styled(body)).collect() }));
    let help = window.global::<Help>();

    help.set_pages(ModelRc::new(VecModel::from(PAGES.iter().map(|(label, _)| SharedString::from(*label)).collect::<Vec<_>>())));

    {
        let (held, ctx, weak) = (Rc::clone(&state), Rc::clone(context), window.as_weak());

        help.on_pick(move |index| {
            let index = (index.max(0) as usize).min(PAGES.len().saturating_sub(1));

            held.borrow_mut().page = index;
            ctx.memory.update(|memory| memory.help_page = index);

            if let Some(ui) = weak.upgrade() {
                refresh(&ui, &held);
            }
        });
    }

    refresh(window, &state);

    state
}

fn refresh(ui: &AppWindow, state: &Shared) {
    let help = ui.global::<Help>();
    let held = state.borrow();

    help.set_page(held.page as i32);

    if let Some(body) = held.bodies.get(held.page) {
        help.set_body(body.clone());
    }
}
