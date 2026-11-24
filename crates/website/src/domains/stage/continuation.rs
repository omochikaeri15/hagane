use dioxus::prelude::*;

use foundry::domains::stage::continuation;
use foundry::domains::stage::{GlobalMapId, Stage, StageDataState};

use crate::systems::context::Shared;
use crate::systems::images;
use crate::systems::picture::Pic;

use super::state;

const BANNER_HEIGHT: f32 = 35.0;
const FALLBACK_SHARE: f32 = 0.62;

pub(crate) fn view(stage: &Stage, data: &StageDataState, globals: &Shared) -> Element {
    let pictures = images::pictures();
    let mut state = state();
    let leads = continuation::resolve(stage, &data.registry.stages, &globals.0.vault().vfs);

    if leads.is_empty() {
        return rsx! {};
    }

    rsx! {
        div { class: "lead-wrap",
            div { class: "section lead-section",
                div { class: "section-title", "Continuation" }
                div { class: "rule" }
                div { class: "zebra-table",
                    div { class: "zebra-header zebra-cells",
                        div { class: "zebra-cell small chance", "Chance" }
                        div { class: "zebra-cell small lead", "Stage" }
                    }
                    for (index, lead) in leads.into_iter().enumerate() {
                        {
                            let target = lead.target.clone();
                            let known = data.registry.stages.contains_key(&target);
                            let plate = lead.banner.as_deref().and_then(|path| pictures.trimmed(path));

                            rsx! {
                                div { class: if index % 2 == 0 { "zebra-row-even zebra-cells" } else { "zebra-row-odd zebra-cells" },
                                    div { class: "zebra-cell chance", "{lead.chance}" }
                                    div { class: "zebra-cell lead",
                                        button {
                                            class: "lead-jump",
                                            disabled: !known,
                                            onclick: move |_| {
                                                state.category.set(Some(target.category.clone()));
                                                state.map.set(Some(GlobalMapId { category: target.category.clone(), map: target.map }));
                                                state.stage.set(Some(target.clone()));
                                            },
                                            match plate {
                                                Some(sized) => {
                                                    let width = sized.width as f32 * (BANNER_HEIGHT / sized.height.max(1) as f32);

                                                    rsx! { Pic { url: sized.url, class: "lead-banner", style: format!("width: {width}px; aspect-ratio: {} / {};", sized.width, sized.height.max(1)), alt: lead.name.clone() } }
                                                }
                                                None => rsx! { span { class: "lead-name bold", style: format!("font-size: {}px;", BANNER_HEIGHT * FALLBACK_SHARE), "{lead.name}" } },
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
