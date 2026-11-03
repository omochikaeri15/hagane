use dioxus::prelude::*;

#[derive(Clone, Copy)]
pub(crate) struct Fitted(pub(crate) Memo<f32>);

#[component]
pub(crate) fn FitWidth(natural: f32, children: Element) -> Element {
    let mut room = use_signal(|| 0.0f32);
    let zoom = use_memo(move || {
        let width = room();

        if width > 0.0 && natural > 0.0 { (width / natural).min(1.0) } else { 1.0 }
    });

    use_context_provider(|| Fitted(zoom));

    rsx! {
        div {
            class: "fit-width",
            onresize: move |event| {
                if let Ok(size) = event.data().get_content_box_size() {
                    room.set(size.width as f32);
                }
            },
            div { class: "fit-width-inner", style: "zoom: {zoom};", {children} }
        }
    }
}
