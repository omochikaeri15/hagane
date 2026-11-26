use std::cell::RefCell;
use std::collections::HashMap;

use iced::widget::image::Handle;
use iced::widget::{button, column, container, image as iced_image, responsive, row, scrollable, tooltip, Stack};
use iced::{Alignment, Element, Length, Size};

use codex::combat::{Faction, Identity};
use foundry::systems::combat::registry::{get_fallback_by_icon, AbilityIcon};
use foundry::systems::combat::{AbilityGroups, AbilityItem, CustomIcon, Mark, ABILITY_X, ABILITY_Y, TRAIT_Y};
use foundry::systems::statblock::mark_ring;

use crate::common::ability_icon;
use crate::editor;
use crate::common::{CustomAssets, SpriteSheet};
use crate::widget::text_with_superscript;
use crate::widget::{ability_spacer, fallback_icon, icons_per_row, smooth_scroll, ICON_SIZE};

pub(crate) const DESCRIPTION_TEXT_SIZE: f32 = 13.0;

const MISSING_ICON_ID: usize = 9999;
const DIMMED: f32 = 0.4;
const RING_SIDE: u32 = (ICON_SIZE * 2.0) as u32;

#[derive(Clone, Copy)]
pub(crate) struct ListLayout {
    pub(crate) per_row: usize,
    pub(crate) fill: bool,
}

impl ListLayout {
    pub(crate) fn width(&self) -> Length {
        if self.fill { Length::Fill } else { Length::Shrink }
    }
}

#[derive(Default)]
pub(crate) struct State {
    icons: ability_icon::Cache,
    rings: RefCell<HashMap<Mark, Handle>>,
}

impl State {
    pub(crate) fn clear_icons(&self) {
        self.icons.clear();
    }

    pub(crate) fn view<'a, Message: Clone + 'a>(
        &'a self,
        groups: AbilityGroups,
        faction: Faction,
        pick: Option<fn(Identity) -> Message>,
        sheets: &'a [SpriteSheet],
        assets: &'a CustomAssets,
        body: impl Fn(&[AbilityItem], ListLayout) -> Element<'a, Message> + 'a,
    ) -> Element<'a, Message> {
        let (grp_trait, grp_hl1, grp_hl2, grp_b1, grp_b2, grp_footer) = groups;

        responsive(move |size: Size| {
            let per_row = icons_per_row(size.width, ABILITY_X);

            let mut col = column![].spacing(0).width(Length::Fill);
            let mut previous_content = false;
            let mut last_was_trait = false;

            if !grp_trait.is_empty() {
                col = col.push(self.icon_row(&grp_trait, sheets, assets, per_row, faction, pick));
                previous_content = true;
                last_was_trait = true;
            }

            for headline in [&grp_hl1, &grp_hl2] {
                if headline.is_empty() { continue; }

                if previous_content {
                    col = col.push(ability_spacer(if last_was_trait { TRAIT_Y } else { ABILITY_Y }));
                    last_was_trait = false;
                }

                col = col.push(self.icon_row(headline, sheets, assets, per_row, faction, None));
                previous_content = true;
            }

            if !grp_b1.is_empty() || !grp_b2.is_empty() {
                if previous_content {
                    col = col.push(ability_spacer(if last_was_trait { TRAIT_Y } else { ABILITY_Y }));
                    last_was_trait = false;
                }

                let layout = ListLayout { per_row, fill: true };
                col = col.push(body(&grp_b1, layout));

                if !grp_b1.is_empty() && !grp_b2.is_empty() {
                    col = col.push(ability_spacer(ABILITY_Y));
                }

                col = col.push(body(&grp_b2, layout));
                previous_content = true;
            }

            if !grp_footer.is_empty() {
                if previous_content {
                    col = col.push(ability_spacer(if last_was_trait { TRAIT_Y } else { ABILITY_Y }));
                }
                col = col.push(self.icon_row(&grp_footer, sheets, assets, per_row, faction, None));
            }

            smooth_scroll(scrollable(col).height(Length::Fill).width(Length::Fill)).into()
        }).into()
    }

    pub(crate) fn icon_row<'a, Message: Clone + 'a>(
        &self,
        items: &[AbilityItem],
        sheets: &[SpriteSheet],
        assets: &CustomAssets,
        per_row: usize,
        faction: Faction,
        pick: Option<fn(Identity) -> Message>,
    ) -> Element<'a, Message> {
        let mut col = column![].spacing(ABILITY_Y);

        for chunk in items.chunks(per_row) {
            let mut wrapped_row = row![].spacing(ABILITY_X).align_y(Alignment::Center);

            for item in chunk {
                let icon = self.icon_element(item, sheets, assets);
                let icon = match pick {
                    Some(pick) => button(icon).padding(0).style(button::text).on_press(pick(item.identity)).into(),
                    None => icon,
                };
                let shown = tooltip(
                    icon,
                    container(text_with_superscript(&item.text, DESCRIPTION_TEXT_SIZE)).padding(6).style(container::bordered_box),
                    tooltip::Position::Top,
                );

                wrapped_row = wrapped_row.push(editor::target(shown, editor::Target::Ability(item.identity, faction)));
            }

            col = col.push(wrapped_row);
        }

        col.into()
    }

    pub(crate) fn ability_row<'a, Message: 'a>(
        &self,
        item: &AbilityItem,
        sheets: &[SpriteSheet],
        assets: &CustomAssets,
        layout: ListLayout,
        faction: Faction,
    ) -> Element<'a, Message> {
        let icon = self.icon_element(item, sheets, assets);
        let description = container(text_with_superscript(&item.text, DESCRIPTION_TEXT_SIZE)).width(layout.width());

        editor::target(
            row![icon, description].spacing(8).align_y(Alignment::Center).width(layout.width()),
            editor::Target::Ability(item.identity, faction),
        )
    }

    pub(crate) fn ability_list<'a, Message: 'a>(
        &self,
        items: &[AbilityItem],
        sheets: &[SpriteSheet],
        assets: &CustomAssets,
        layout: ListLayout,
        faction: Faction,
    ) -> Element<'a, Message> {
        let mut col = column![].spacing(0).width(layout.width());

        for (index, item) in items.iter().enumerate() {
            col = col.push(self.ability_row(item, sheets, assets, layout, faction));

            if index + 1 < items.len() {
                col = col.push(ability_spacer(ABILITY_Y));
            }
        }

        col.into()
    }

    fn ring(&self, mark: Mark) -> Option<Handle> {
        if let Some(held) = self.rings.borrow().get(&mark) {
            return Some(held.clone());
        }

        let drawn = mark_ring(mark, RING_SIDE, RING_SIDE)?;
        let handle = Handle::from_rgba(drawn.width(), drawn.height(), drawn.into_raw());

        self.rings.borrow_mut().insert(mark, handle.clone());
        Some(handle)
    }

    pub(crate) fn icon_element<'a, Message: 'a>(
        &self,
        item: &AbilityItem,
        sheets: &[SpriteSheet],
        assets: &CustomAssets,
    ) -> Element<'a, Message> {
        let custom = (item.custom_icon != CustomIcon::None).then(|| assets.get_icon_texture(item.custom_icon)).flatten();
        let standard = || item.icon_id.and_then(|icon_id| self.icons.handle(icon_id, sheets)).map(|handle| [Some(handle), item.border_id.and_then(|border_id| self.icons.handle(border_id, sheets))]);
        let layers: Option<Vec<Handle>> = custom.map(|handle| vec![handle]).or_else(|| standard().map(|held| held.into_iter().flatten().collect()));

        if let Some(mut layers) = layers {
            let alpha = if item.dimmed { DIMMED } else { 1.0 };

            layers.extend(self.ring(item.mark));

            return match layers.as_slice() {
                [only] => sized_image(only.clone(), alpha),
                _ => Stack::with_children(layers.into_iter().map(|handle| sized_image(handle, alpha))).width(Length::Fixed(ICON_SIZE)).height(Length::Fixed(ICON_SIZE)).into(),
            };
        }

        let icon = if item.custom_icon != CustomIcon::None {
            AbilityIcon::Custom(item.custom_icon)
        } else {
            AbilityIcon::Standard(item.icon_id.unwrap_or(MISSING_ICON_ID))
        };

        fallback_icon(get_fallback_by_icon(icon))
    }

    pub(crate) fn raw_icon<'a, Message: 'a>(&self, icon_id: usize, sheets: &[SpriteSheet]) -> Element<'a, Message> {
        self.icons.handle(icon_id, sheets).map_or_else(
            || fallback_icon(get_fallback_by_icon(AbilityIcon::Standard(icon_id))),
            |handle| sized_image(handle, 1.0),
        )
    }
}

fn sized_image<'a, Message: 'a>(handle: Handle, alpha: f32) -> Element<'a, Message> {
    iced_image(handle).width(Length::Fixed(ICON_SIZE)).height(Length::Fixed(ICON_SIZE)).opacity(alpha).into()
}
