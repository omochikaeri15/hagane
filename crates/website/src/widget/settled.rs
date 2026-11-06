use dioxus::prelude::*;

use crate::platform::Progress;
use crate::systems::library::Library;

use super::Status;

#[component]
pub(crate) fn Settled(title: &'static str, children: Element) -> Element {
    let library = use_context::<Library>();
    let mut revealed = use_signal(|| false);

    use_effect(move || {
        let _ = library.generation();
        let busy = library.busy();

        if !busy && !library.waiting() && !*revealed.peek() {
            revealed.set(true);
        }
    });

    let shown = revealed();
    let fetching = if shown { Progress::default() } else { library.fetching() };
    let class = if shown { "settled" } else { "settled veiled" };

    rsx! {
        div { class: "{class}",
            div { class: "settled-body", {children} }
            if !shown {
                Status { title, detail: Library::detail(fetching.done, fetching.total) }
            }
        }
    }
}
