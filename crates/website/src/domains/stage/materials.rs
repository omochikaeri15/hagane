use dioxus::prelude::*;

use codex::chapter::map::DropItemEntry;
use foundry::domains::stage::materials;
use foundry::domains::stage::{Map, Stage};

use crate::systems::context::Shared;
use crate::systems::images;
use crate::systems::picture::Pic;
use crate::widget::Tip;

const MAX_ICON_SIZE: u32 = 32;
const MATERIAL_BASE_ID: u32 = 10000;

pub(crate) fn has_drops(map: &Map) -> bool {
    map.drop_items.as_ref().is_some_and(DropItemEntry::has_materials)
}

pub(crate) fn view(stage: &Stage, map: &Map, crown: u8, globals: &Shared) -> Element {
    let Some(drops) = map.drop_items.as_ref().filter(|_| has_drops(map)) else {
        return rsx! {};
    };
    let base = &drops.material_drops[0..8];
    let zombie = &drops.material_drops[8..16];
    let title = materials::heading(drops, stage, crown);

    rsx! {
        div { class: "section drop-table materials-table",
            div { class: "section-title", "{title}" }
            div { class: "rule" }
            div { class: "material-chunks",
                if base.iter().any(|&count| count > 0) {
                    {chunks(base, 0, globals)}
                }
                if zombie.iter().any(|&count| count > 0) {
                    {chunks(zombie, 8, globals)}
                }
            }
        }
    }
}

fn chunks(chances: &[u32], offset: usize, globals: &Shared) -> Element {
    let pictures = images::pictures();
    let vault = globals.0.vault();
    let vfs = &vault.vfs;

    rsx! {
        for (chunk_index, chunk) in chances.chunks(4).enumerate() {
            {
                let chunk_offset = offset + chunk_index * 4;
                let resolved: Vec<_> = chunk.iter().enumerate().map(|(index, &chance)| (materials::resolve(vfs, chunk_offset + index, chance, &vault.vds.items), chance)).collect();

                rsx! {
                    div { class: "zebra-table",
                        div { class: "zebra-header zebra-cells",
                            for (drop, _) in resolved.iter() {
                                div { class: "zebra-cell small", "{drop.name}" }
                            }
                        }
                        div { class: "zebra-row-even zebra-cells",
                            for (index, (drop, _)) in resolved.iter().enumerate() {
                                div { class: "zebra-cell",
                                    {
                                        let _ = MATERIAL_BASE_ID + (chunk_offset + index) as u32;
                                        let icon = drop.image_path.as_ref().and_then(|path| pictures.boxed(path, MAX_ICON_SIZE));

                                        match icon {
                                            Some(url) => rsx! { Tip { text: drop.name.clone(), Pic { url, class: "drop-icon", alt: drop.name.clone() } } },
                                            None => rsx! { span { class: "tiny", "{drop.name}" } },
                                        }
                                    }
                                }
                            }
                        }
                        div { class: "zebra-row-odd zebra-cells",
                            for (_, chance) in resolved.iter() {
                                div { class: if *chance == 0 { "zebra-cell faint" } else { "zebra-cell" }, "{chance}%" }
                            }
                        }
                    }
                }
            }
        }
    }
}
