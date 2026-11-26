use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use iced::alignment::Vertical;
use iced::widget::image::Handle;
use iced::widget::{column, container, image as iced_image, pick_list, row, text, Space};
use iced::{Element, Length, Task};

use foundry::common::formats::SpriteSheet as CoreSpriteSheet;
use foundry::systems::combat::{Mark, NameBook};
use foundry::systems::statblock::mark_ring;
use foundry::domains::cat::orbs::{self as orb, effect_names, Allowance, Kind, GRADES, LAYERS, PLAIN_FACE};
use foundry::Vault;
use image::{imageops, RgbaImage};
use codex::cat::unit::Equipment;

use crate::app::theme;
use crate::common::{ensure_sheet_loaded, SpriteSheet};

const ROW_SPACING: f32 = 8.0;
const LABEL_WIDTH: f32 = 44.0;
const PICK_WIDTH: f32 = 230.0;
const PICK_TEXT: f32 = 12.0;
const PREVIEW_SIZE: f32 = 56.0;
const POPUP_PADDING: f32 = 10.0;
const OVERSAMPLE: f32 = 2.0;
const BEST_GRADE: usize = 4;
const IDLE_PADDING: f32 = 5.0;
const DARKENED: f32 = 0.4;


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    Attribute,
    Effect,
    Grade,
}

impl Layer {
    const ALL: [Self; 3] = [Self::Attribute, Self::Effect, Self::Grade];

    fn sheet(self) -> &'static str {
        LAYERS[self as usize]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    content: i32,
    label: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Aim {
    attribute: i32,
    label: String,
}

impl std::fmt::Display for Aim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

impl std::fmt::Display for Choice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grade(usize);

impl std::fmt::Display for Grade {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(GRADES.get(self.0).copied().unwrap_or("?"))
    }
}

#[derive(Clone)]
pub enum Message {
    Sheet(Layer, usize, Option<CoreSpriteSheet>),
    Effect(Choice),
    Aim(Aim),
    Grade(Grade),
    Clear,
}

impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sheet(layer, index, _) => write!(f, "Sheet({layer:?}, {index})"),
            Self::Effect(choice) => write!(f, "Effect({choice})"),
            Self::Aim(aim) => write!(f, "Aim({aim})"),
            Self::Grade(grade) => write!(f, "Grade({grade})"),
            Self::Clear => write!(f, "Clear"),
        }
    }
}

pub enum Outcome {
    Keep,
    Equip(Option<u32>),
}

#[derive(Default)]
pub struct State {
    orbs: Arc<Vec<Equipment>>,
    kinds: Vec<Kind>,
    choices: Vec<Choice>,
    sheets: [Vec<SpriteSheet>; 3],
    drawn: RefCell<HashMap<(u32, u32, bool, bool), Handle>>,
}

impl State {
    pub fn load(&mut self, vault: &Vault) -> Task<Message> {
        let vfs = &vault.vfs;
        let effects = effect_names(vfs);

        self.orbs = vault.vds.cats.orbs(vfs);
        self.kinds = orb::kinds(&self.orbs);
        self.choices = orb::effects(&self.kinds)
            .into_iter()
            .map(|content| Choice { content, label: label(&effects, content) })
            .collect();

        self.drawn.borrow_mut().clear();

        Task::batch(Layer::ALL.into_iter().enumerate().map(|(slot, layer)| {
            for sheet in &mut self.sheets[slot] {
                sheet.mark_stale();
            }

            ensure_sheet_loaded(&mut self.sheets[slot], vfs, layer.sheet(), false)
                .map(move |(index, sheet)| Message::Sheet(layer, index, sheet))
        }))
    }

    pub fn update(&mut self, message: Message, held: Option<u32>, allowance: &Allowance, names: &NameBook) -> Outcome {
        match message {
            Message::Sheet(layer, index, sheet) => {
                let slot = layer as usize;

                if let Some(target) = self.sheets[slot].get_mut(index) {
                    target.apply(sheet);
                    self.drawn.borrow_mut().clear();
                }

                Outcome::Keep
            }
            Message::Effect(choice) => {
                let grade = held.and_then(|orb| orb::grade(&self.orbs, orb)).unwrap_or(BEST_GRADE);
                let aimed = held.and_then(|orb| orb::kind(&self.orbs, orb)).and_then(|kind| kind.attribute);
                let offered = self.aims(choice.content, allowance, names);
                let attribute = offered.iter().find(|aim| Some(aim.attribute) == aimed).or_else(|| offered.first()).map(|aim| aim.attribute);

                Outcome::Equip(orb::pick(&self.orbs, Kind { content: choice.content, attribute }, grade))
            }
            Message::Aim(aim) => {
                let Some(kind) = held.and_then(|orb| orb::kind(&self.orbs, orb)) else {
                    return Outcome::Keep;
                };
                let grade = held.and_then(|orb| orb::grade(&self.orbs, orb)).unwrap_or(BEST_GRADE);

                Outcome::Equip(orb::pick(&self.orbs, Kind { attribute: Some(aim.attribute), ..kind }, grade))
            }
            Message::Grade(grade) => {
                let Some(kind) = held.and_then(|orb| orb::kind(&self.orbs, orb)) else {
                    return Outcome::Keep;
                };

                Outcome::Equip(orb::pick(&self.orbs, kind, grade.0))
            }
            Message::Clear => Outcome::Equip(None),
        }
    }

    fn aims(&self, content: i32, allowance: &Allowance, names: &NameBook) -> Vec<Aim> {
        orb::attributes(&self.kinds, content)
            .into_iter()
            .filter(|attribute| allowance.permits(Kind { content, attribute: Some(*attribute) }))
            .map(|attribute| Self::aim(attribute, names))
            .collect()
    }

    fn aim(attribute: i32, names: &NameBook) -> Aim {
        let label = usize::try_from(attribute)
            .ok()
            .and_then(|index| names.orb_trait(index))
            .map_or_else(|| format!("Trait {attribute}"), str::to_owned);

        Aim { attribute, label }
    }

    fn cut(&self, layer: Layer, index: i32) -> Option<RgbaImage> {
        let index = usize::try_from(index).ok()?;

        self.sheets[layer as usize].iter().find_map(|sheet| sheet.core.crop(index))
    }

    fn drawn(&self, orb: u32, edge: u32, dim: bool, lit: bool) -> Option<Handle> {
        if let Some(held) = self.drawn.borrow().get(&(orb, edge, dim, lit)) {
            return Some(held.clone());
        }

        let held = self.orbs.get(orb as usize)?;
        let mut face = self.cut(Layer::Attribute, held.attribute.unwrap_or(PLAIN_FACE))?;

        for (layer, index) in [(Layer::Effect, held.content), (Layer::Grade, held.grade_id)] {
            if let Some(over) = index.and_then(|index| self.cut(layer, index)) {
                let (across, down) = (face.width().saturating_sub(over.width()) / 2, face.height().saturating_sub(over.height()) / 2);

                imageops::overlay(&mut face, &over, i64::from(across), i64::from(down));
            }
        }

        let mut scaled = imageops::resize(&face, edge, edge, imageops::FilterType::Lanczos3);

        if dim {
            for pixel in scaled.pixels_mut() {
                for channel in &mut pixel.0[..3] {
                    *channel = (f32::from(*channel) * DARKENED) as u8;
                }
            }
        }

        if lit && let Some(ring) = mark_ring(Mark::Hit, edge, edge) {
            imageops::overlay(&mut scaled, &ring, 0, 0);
        }

        let handle = Handle::from_rgba(edge, edge, scaled.into_raw());

        self.drawn.borrow_mut().insert((orb, edge, dim, lit), handle.clone());

        Some(handle)
    }

    pub fn catalog(&self) -> &[Equipment] {
        &self.orbs
    }

    pub fn picture<'a, M: 'a>(&self, orb: u32, size: f32) -> Element<'a, M> {
        self.shaded(orb, size, false, false)
    }

    pub fn shaded<'a, M: 'a>(&self, orb: u32, size: f32, dim: bool, lit: bool) -> Element<'a, M> {
        let edge = ((size * OVERSAMPLE).round() as u32).max(1);

        self.drawn(orb, edge, dim, lit).map_or_else(
            || Space::new().width(Length::Fixed(size)).height(Length::Fixed(size)).into(),
            |handle| iced_image(handle).width(Length::Fixed(size)).height(Length::Fixed(size)).into(),
        )
    }

    pub fn view<'a>(&'a self, held: Option<u32>, allowance: &Allowance, names: &NameBook) -> Element<'a, Message> {
        let kind = held.and_then(|orb| orb::kind(&self.orbs, orb));
        let offered: Vec<Choice> = self
            .choices
            .iter()
            .filter(|choice| orb::attributes(&self.kinds, choice.content).is_empty() || !self.aims(choice.content, allowance, names).is_empty())
            .cloned()
            .collect();
        let chosen = kind.and_then(|kind| offered.iter().find(|choice| choice.content == kind.content)).cloned();
        let aims = kind.map_or_else(Vec::new, |kind| self.aims(kind.content, allowance, names));
        let aim = kind.and_then(|kind| kind.attribute).map(|attribute| Self::aim(attribute, names));
        let trait_pick: Element<'_, Message> = if aims.is_empty() {
            container(text("None").size(PICK_TEXT).style(|theme: &iced::Theme| text::Style { color: Some(theme::weak_text_color(theme)) }))
                .padding(IDLE_PADDING)
                .width(Length::Fixed(PICK_WIDTH))
                .style(theme::combo_box_idle)
                .into()
        } else {
            pick_list(aims, aim, Message::Aim)
                .placeholder("None")
                .width(Length::Fixed(PICK_WIDTH))
                .text_size(PICK_TEXT)
                .style(theme::combo_box)
                .menu_style(theme::combo_box_menu)
                .into()
        };
        let grades: Vec<Grade> = kind.map_or_else(Vec::new, |kind| {
            let mut offered: Vec<Grade> = orb::grades(&self.orbs, kind).into_iter().map(|(grade, _)| Grade(grade)).collect();

            offered.sort_by_key(|grade| grade.0);
            offered
        });
        let grade = held.and_then(|orb| orb::grade(&self.orbs, orb)).map(Grade);

        let preview: Element<'_, Message> = held.map_or_else(
            || Space::new().width(Length::Fixed(PREVIEW_SIZE)).height(Length::Fixed(PREVIEW_SIZE)).into(),
            |orb| self.picture(orb, PREVIEW_SIZE),
        );

        column![
            preview,
            row![
                text("Type").size(PICK_TEXT).width(Length::Fixed(LABEL_WIDTH)),
                pick_list(offered, chosen, Message::Effect)
                    .placeholder("None")
                    .width(Length::Fixed(PICK_WIDTH))
                    .text_size(PICK_TEXT)
                    .style(theme::combo_box)
                    .menu_style(theme::combo_box_menu),
            ]
                .spacing(ROW_SPACING)
                .align_y(Vertical::Center),
            row![
                text("Trait").size(PICK_TEXT).width(Length::Fixed(LABEL_WIDTH)),
                trait_pick,
            ]
                .spacing(ROW_SPACING)
                .align_y(Vertical::Center),
            row![
                text("Grade").size(PICK_TEXT).width(Length::Fixed(LABEL_WIDTH)),
                pick_list(grades, grade, Message::Grade)
                    .placeholder("None")
                    .width(Length::Fixed(PICK_WIDTH))
                    .text_size(PICK_TEXT)
                    .style(theme::combo_box)
                    .menu_style(theme::combo_box_menu),
            ]
                .spacing(ROW_SPACING)
                .align_y(Vertical::Center),
            theme::sized_button("None", theme::POPUP_ACTION_BUTTON_WIDTH, theme::danger_button)
                .on_press_maybe(held.map(|_| Message::Clear)),
        ]
            .spacing(ROW_SPACING)
            .align_x(iced::Alignment::Center)
            .width(Length::Fill)
            .padding(POPUP_PADDING)
            .into()
    }
}

fn label(effects: &[String], content: i32) -> String {
    usize::try_from(content)
        .ok()
        .and_then(|index| effects.get(index))
        .filter(|name| !name.is_empty())
        .map_or_else(|| format!("Effect {content}"), Clone::clone)
}
