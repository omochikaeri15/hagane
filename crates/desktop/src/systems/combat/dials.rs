use iced::alignment::Vertical;
use iced::widget::{container, pick_list, text};
use iced::{Element, Length, Padding};

use codex::combat::Entity;
use foundry::systems::combat::lens::{self, Lens, MODES, ONE_HIT};

use crate::app::theme;
use crate::widget::hover_hint;

const CARD_WIDTH: f32 = 100.0;
const CARD_HEIGHT: f32 = 24.0;
const CARD_TEXT_SIZE: f32 = 12.0;
const CARD_PADDING: Padding = Padding { top: 4.0, right: 6.0, bottom: 4.0, left: 6.0 };

#[derive(Debug, Clone)]
pub enum Message {
    Mode(&'static str),
    Hits(&'static str),
}

#[derive(Default)]
pub(crate) struct State {
    lens: Lens,
}

impl State {
    pub(crate) fn update(&mut self, message: Message) {
        match message {
            Message::Mode(label) => self.lens.mode = lens::mode_named(label),
            Message::Hits(label) => self.lens.hits = lens::hits_of(label),
        }
    }

    pub(crate) fn lens(&self, stats: &Entity) -> Lens {
        Lens { hits: lens::settled(stats, self.lens.hits), ..self.lens }
    }

    pub(crate) fn mode_card(&self) -> Element<'_, Message> {
        pick_list(MODES.map(lens::mode_label).to_vec(), Some(lens::mode_label(self.lens.mode)), Message::Mode)
            .width(Length::Fixed(CARD_WIDTH))
            .text_size(CARD_TEXT_SIZE)
            .padding(CARD_PADDING)
            .style(theme::combo_box)
            .menu_style(theme::combo_box_menu)
            .into()
    }

    pub(crate) fn hits_card(&self, stats: &Entity) -> Element<'_, Message> {
        let shown = lens::hits_label(lens::shown_hits(stats, self.lens.hits));

        if !lens::picks_hits(stats) {
            let idle = container(text(shown).size(CARD_TEXT_SIZE).style(|theme: &iced::Theme| text::Style { color: Some(theme::weak_text_color(theme)) }))
                .width(Length::Fixed(CARD_WIDTH))
                .height(Length::Fixed(CARD_HEIGHT))
                .padding(CARD_PADDING)
                .align_y(Vertical::Center)
                .style(theme::combo_box_idle);

            return hover_hint(idle, ONE_HIT);
        }

        pick_list(lens::hit_choices(stats).into_iter().map(lens::hits_label).collect::<Vec<_>>(), Some(shown), Message::Hits)
            .width(Length::Fixed(CARD_WIDTH))
            .text_size(CARD_TEXT_SIZE)
            .padding(CARD_PADDING)
            .style(theme::combo_box)
            .menu_style(theme::combo_box_menu)
            .into()
    }
}
