use std::collections::HashMap;

use iced::widget::{button, column};
use iced::{Element, Length, Theme};
use codex::cat::unit::LevelCurve;
use codex::files::img015;

use foundry::common::context::GlobalContext;
use codex::combat::{Faction, Foe, Identity};
use foundry::domains::cat::versus::{self, Pace};
use foundry::domains::settings::Settings;
use foundry::systems::combat::{AbilityItem, CustomIcon, RenderContext, ABILITY_Y};

use crate::app::theme;
use crate::common::{CustomAssets, SpriteSheet};
use crate::systems::combat::abilities::{self as shared, ListLayout};
use crate::widget::ability_spacer;

const VERSUS_WIDTH: f32 = 100.0;
const VERSUS_HEIGHT: f32 = 24.0;
const VERSUS_TEXT_SIZE: f32 = 12.0;

#[derive(Debug, Clone)]
pub enum Message {
    ToggleConjureExpand(u32),
    ToggleVersus,
    Pick(Identity),
}

#[derive(Clone, Copy)]
pub(super) struct SpiritContext<'a> {
    pub(super) cat_id: u32,
    pub(super) global: GlobalContext<'a>,
    pub(super) level_curve: Option<&'a LevelCurve>,
    pub(super) current_level: i32,
    pub(super) conjure_unit_id: i32,
    pub(super) foe: Option<Foe>,
}

#[derive(Default)]
pub struct State {
    pub(super) shared: shared::State,
    conjure_overrides: HashMap<u32, bool>,
    versus: bool,
    foe: Foe,
}


impl State {
    pub(super) fn clear_icons(&self) {
        self.shared.clear_icons();
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::ToggleConjureExpand(cat_id) => {
                let current = self.conjure_overrides.get(&cat_id).copied();
                self.conjure_overrides.insert(cat_id, !current.unwrap_or(false));
            }
            Message::ToggleVersus => self.versus = !self.versus,
            Message::Pick(label) => self.foe = if self.foe.has(label) { self.foe.without(label) } else { self.foe.with(label) },
        }
    }

    pub(super) fn versus(&self) -> Option<Foe> {
        self.versus.then_some(self.foe)
    }

    pub(super) fn versus_card(&self) -> Element<'_, Message> {
        let active = self.versus;

        button(theme::centered_text("Versus Mode").size(VERSUS_TEXT_SIZE))
            .width(Length::Fixed(VERSUS_WIDTH))
            .height(Length::Fixed(VERSUS_HEIGHT))
            .style(move |t: &Theme, status| theme::toggle_button(t, status, active))
            .on_press(Message::ToggleVersus)
            .into()
    }

    pub(super) fn view<'a>(
        &'a self,
        ctx: &RenderContext<'_>,
        pace: Pace,
        spirit: SpiritContext<'a>,
        sheets: &'a [SpriteSheet],
        assets: &'a CustomAssets,
        settings: &'a Settings,
    ) -> Element<'a, Message> {
        let groups = match spirit.foe {
            Some(foe) => versus::palette(ctx, foe, versus::spirit_controls(ctx), pace),
            None => versus::plain(ctx, pace),
        };
        let pick: Option<fn(Identity) -> Message> = spirit.foe.map(|_| Message::Pick as fn(Identity) -> Message);

        self.shared.view(groups, Faction::Cat, pick, sheets, assets, move |items, layout| {
            self.ability_list(items, spirit, sheets, assets, settings, layout)
        })
    }

    pub(super) fn ability_list<'a>(
        &'a self,
        items: &[AbilityItem],
        spirit: SpiritContext<'a>,
        sheets: &'a [SpriteSheet],
        assets: &'a CustomAssets,
        settings: &'a Settings,
        layout: ListLayout,
    ) -> Element<'a, Message> {
        let mut col = column![].spacing(0).width(layout.width());

        for (index, item) in items.iter().enumerate() {
            let is_conjure = item.icon_id == Some(img015::ICON_CONJURE) && item.custom_icon == CustomIcon::None;

            let item_row = if is_conjure {
                self.conjure_row(item, spirit, sheets, assets, settings)
            } else {
                self.shared.ability_row(item, sheets, assets, layout, Faction::Cat)
            };

            col = col.push(item_row);

            if is_conjure && self.conjure_expanded(spirit.cat_id, settings) {
                col = col.push(ability_spacer(ABILITY_Y));
                col = col.push(self.conjure_details(spirit, sheets, assets, settings, layout.per_row));
            }

            if index + 1 < items.len() {
                col = col.push(ability_spacer(ABILITY_Y));
            }
        }

        col.into()
    }

    pub(super) fn conjure_expanded(&self, cat_id: u32, settings: &Settings) -> bool {
        self.conjure_overrides.get(&cat_id).copied().unwrap_or(settings.cat_data.expand_spirit_details)
    }

    pub(crate) fn is_conjure_expanded(&self, cat_id: u32, settings: &Settings) -> bool {
        self.conjure_expanded(cat_id, settings)
    }

}
