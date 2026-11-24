use std::collections::HashMap;
use std::path::PathBuf;

use iced::alignment::Horizontal;
use iced::font::Weight;
use iced::mouse::Interaction;
use iced::widget::{column, container, mouse_area, row};
use iced::{Alignment, Element, Font, Length, Theme};

use foundry::domains::stage::continuation;
use foundry::domains::stage::{GlobalStageId, Stage};
use foundry::{Source, Vfs};

use crate::app::theme;
use crate::common::glyphs::Ruler;
use crate::common::item_icon;
use crate::widget::section;

use super::info::{FALLBACK_NAME_SIZE, Texture, banner_element};
use super::treasure::{CELL_PADDING, COLUMN_SPACING, HEADER_TEXT_SIZE, TREASURE_TABLE_WIDTH};

const BANNER_HEIGHT: f32 = 35.0;
const HEADER_WIDTH: f32 = 40.0;
const SIDES: f32 = CELL_PADDING[1] as f32 * 2.0;
const CHANCE_WIDTH: f32 = (TREASURE_TABLE_WIDTH - SIDES - COLUMN_SPACING * 2.0) / 3.0;
const STAGE_ROOM: f32 = super::CONTENT_WIDTH - SIDES - COLUMN_SPACING - CHANCE_WIDTH;

struct Lead {
    target: GlobalStageId,
    chance: String,
    banner: Option<Texture>,
    height: f32,
    name: String,
}

#[derive(Default)]
pub struct State {
    leads: Vec<Lead>,
    stage_width: f32,
    textures: HashMap<PathBuf, Option<Texture>>,
}

fn fitted(width: u32, height: u32) -> (f32, f32) {
    let tall = BANNER_HEIGHT.min(STAGE_ROOM * height.max(1) as f32 / width.max(1) as f32);

    (width as f32 * tall / height.max(1) as f32, tall)
}

impl State {
    pub fn refresh(&mut self, stage: Option<&Stage>, stages: &HashMap<GlobalStageId, Stage>, vfs: &Vfs) {
        let resolved = stage.map_or_else(Vec::new, |stage| continuation::resolve(stage, stages, vfs));
        let textures = &mut self.textures;
        let mut ruler = Ruler::new(Font { weight: Weight::Bold, ..Font::DEFAULT }, FALLBACK_NAME_SIZE);
        let mut widest = HEADER_WIDTH;

        self.leads = resolved
            .into_iter()
            .map(|held| {
                let banner = held.banner.and_then(|path| textures.entry(path).or_insert_with_key(|path| item_icon::load_cropped(&Source::disk(path))).clone());
                let (wide, height) = banner.as_ref().map_or_else(|| (ruler.width(&held.name), BANNER_HEIGHT), |(_, width, height)| fitted(*width, *height));

                widest = widest.max(wide);

                Lead { target: held.target, chance: held.chance, banner, height, name: held.name }
            })
            .collect();
        self.stage_width = widest.ceil().min(STAGE_ROOM);
    }

    pub fn clear_icons(&mut self) {
        self.textures.clear();
    }

    pub fn view<'a>(&'a self) -> Option<Element<'a, super::Message>> {
        if self.leads.is_empty() {
            return None;
        }

        let width = SIDES + CHANCE_WIDTH + COLUMN_SPACING + self.stage_width;
        let grid = self.leads.iter().enumerate().fold(column![header_row(self.stage_width)], |grid, (index, lead)| grid.push(lead_row(index, lead, self.stage_width)));

        Some(section("Continuation", Length::Fixed(width), grid.width(Length::Fixed(width))))
    }
}

fn header_row<'a>(stage_width: f32) -> Element<'a, super::Message> {
    container(
        row![
            theme::table_cell_text("Chance", Length::Fixed(CHANCE_WIDTH)).size(HEADER_TEXT_SIZE),
            theme::table_cell_text("Stage", Length::Fixed(stage_width)).size(HEADER_TEXT_SIZE),
        ]
            .spacing(COLUMN_SPACING)
    )
        .style(theme::zebra_table_header)
        .padding(CELL_PADDING)
        .width(Length::Fill)
        .into()
}

fn lead_row<'a>(index: usize, lead: &'a Lead, stage_width: f32) -> Element<'a, super::Message> {
    let plate = mouse_area(banner_element(lead.banner.clone(), lead.height, &lead.name))
        .interaction(Interaction::Pointer)
        .on_press(super::Message::JumpToStage(lead.target.clone()));

    container(
        row![
            theme::table_cell_text(lead.chance.as_str(), Length::Fixed(CHANCE_WIDTH)),
            container(plate).width(Length::Fixed(stage_width)).align_x(Horizontal::Center),
        ]
            .spacing(COLUMN_SPACING)
            .align_y(Alignment::Center)
    )
        .style(move |theme: &Theme| theme::zebra_table_row(theme, index))
        .padding(CELL_PADDING)
        .width(Length::Fill)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    // A plate keeps its height until it would run past the table's widest, and then shrinks whole.
    #[test]
    fn a_long_plate_shrinks_to_the_room_it_has() {
        assert_eq!(fitted(300, 70), (150.0, BANNER_HEIGHT));

        let (wide, tall) = fitted(4000, 70);

        assert!((wide - STAGE_ROOM).abs() < 0.01 && tall < BANNER_HEIGHT);
        assert!((CHANCE_WIDTH * 3.0 + COLUMN_SPACING * 2.0 + SIDES - TREASURE_TABLE_WIDTH).abs() < 0.01, "the chance column is the treasure table's own");
    }
}
