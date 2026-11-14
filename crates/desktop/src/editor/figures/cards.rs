use iced::alignment::{Horizontal, Vertical};
use iced::widget::{button, column, container, pick_list, scrollable, text, text_input, Column, Container, Id, Row};
use iced::{Element, Length, Padding, Theme};

pub(crate) use foundry::systems::editor::figures::cards::*;

use crate::app::theme;
use crate::common::feedback::CONFIRM_LABEL;
use crate::widget::{hover_hint, smooth_scroll};

use super::resolved::{Choice, Face, Toggle};
use super::schema::Subject;
use super::{Draft, Message, GLYPH_RATIO, LABEL_HEIGHTS};

pub(super) const CARD_WIDTH: f32 = 86.0;

pub(super) const CARD_GAP: f32 = 8.0;

pub(super) const BODY_PADDING: f32 = 12.0;

const LABEL_SIZE: f32 = 12.0;

pub(super) const INPUT_SIZE: f32 = 13.0;

const CARD_PADDING: f32 = 6.0;

const SYNC_WIDTH: f32 = 172.0;

const FIELD_PADDING: f32 = 4.0;

const SEARCH_FRACTION: f32 = 2.0 / 3.0;

const CARET_RESERVE: f32 = 18.0;

const FIELD_INSET: f32 = 2.0;

pub(super) fn usable(width: f32) -> f32 {
    (width - BODY_PADDING * 2.0).max(CARD_WIDTH)
}

pub(super) fn combo_room(label: f32) -> f32 {
    label + FIELD_INSET * 2.0 + CARET_RESERVE
}

fn span(face: Face) -> usize {
    let Face::Choice(options, _) = face else {
        return 1;
    };

    let inner = CARD_WIDTH - CARD_PADDING * 2.0 - CARET_RESERVE;
    let widest = options.iter().map(|choice| choice.label().chars().count()).max().unwrap_or(0);

    if widest as f32 * INPUT_SIZE * GLYPH_RATIO <= inner { 1 } else { WIDE_SPAN }
}

pub(super) fn grid_id(subject: Subject) -> Id {
    Id::new(match subject {
        Subject::Cat => "figures-grid-cat",
        Subject::Enemy => "figures-grid-enemy",
        Subject::Buy => "figures-grid-buy",
        Subject::Curve => "figures-grid-curve",
        Subject::Talents => "figures-grid-talents",
        Subject::Costs => "figures-grid-costs",
        Subject::Combo => "figures-grid-combo",
        Subject::MapStage => "figures-grid-map-stage",
        Subject::MapDrops => "figures-grid-map-drops",
        Subject::DropChara => "figures-grid-drop-chara",
        Subject::ItemBuy => "figures-grid-item-buy",
        Subject::MapOption => "figures-grid-map-option",
        Subject::Background => "figures-grid-background",
        Subject::Upgrade => "figures-grid-upgrade",
        Subject::ComboParam => "figures-grid-combo-param",
        Subject::OrbSlots => "figures-grid-orb-slots",
        Subject::Difficulty => "figures-grid-difficulty",
        Subject::ExInvasion => "figures-grid-ex-invasion",
        Subject::CannonStatus => "figures-grid-cannon-status",
        Subject::ExpCurve => "figures-grid-exp-curve",
        Subject::OrbGrades => "figures-grid-orb-grades",
        Subject::ExLottery => "figures-grid-ex-lottery",
        Subject::Shakes => "figures-grid-shakes",
        Subject::Recipes => "figures-grid-recipes",
        Subject::Shop => "figures-grid-shop",
        Subject::ZombieHeader => "figures-grid-zombie-header",
        Subject::ZombieTier => "figures-grid-zombie-tier",
        Subject::Cpu => "figures-grid-cpu",
        Subject::OrbTraits => "figures-grid-orb-traits",
        Subject::Altar => "figures-grid-altar",
        Subject::GamatotoLimit => "figures-grid-gamatoto-limit",
        Subject::GamatotoUnlock => "figures-grid-gamatoto-unlock",
        Subject::GamatotoDrop => "figures-grid-gamatoto-drop",
        Subject::Stamps => "figures-grid-stamps",
        Subject::SlotUnlocks => "figures-grid-slot-unlocks",
        Subject::Club => "figures-grid-club",
        Subject::Missions => "figures-grid-missions",
        Subject::Conditions => "figures-grid-conditions",
        Subject::MissionUnlocks => "figures-grid-mission-unlocks",
        Subject::MissionGroups => "figures-grid-mission-groups",
        Subject::GachaOptions => "figures-grid-gacha-options",
        Subject::ChanceAnimations => "figures-grid-chance-animations",
        Subject::Castles => "figures-grid-castles",
        Subject::TipRules => "figures-grid-tip-rules",
        Subject::QuestStages => "figures-grid-quest-stages",
        Subject::SlotLottery => "figures-grid-slot-lottery",
        Subject::RealmAltar => "figures-grid-realm-altar",
        Subject::RealmDraws => "figures-grid-realm-draws",
        Subject::LabyrinthFloors => "figures-grid-labyrinth-floors",
        Subject::LabyrinthLottery => "figures-grid-labyrinth-lottery",
        Subject::MissionShortcuts => "figures-grid-mission-shortcuts",
        Subject::MissionDesignations => "figures-grid-mission-designations",
        Subject::MonthlyMissions => "figures-grid-monthly-missions",
        Subject::SoundSettings => "figures-grid-sound-settings",
        Subject::DojoChests => "figures-grid-dojo-chests",
        Subject::PopupTriggers => "figures-grid-popup-triggers",
        Subject::ComboCategories => "figures-grid-combo-categories",
        Subject::CastleGrowth => "figures-grid-castle-growth",
        Subject::DemonCastles => "figures-grid-demon-castles",
        Subject::LevelUpOffers => "figures-grid-level-up-offers",
        Subject::ReelSymbols => "figures-grid-reel-symbols",
        Subject::OrbFilterChoices => "figures-grid-orb-filter-choices",
        Subject::MapSheets => "figures-grid-map-sheets",
        Subject::MenuTextSettings => "figures-grid-menu-text-settings",
        Subject::TraitIcons => "figures-grid-trait-icons",
        Subject::AbilityIcons => "figures-grid-ability-icons",
        Subject::LineupRankGroups => "figures-grid-lineup-rank-groups",
        Subject::LineupBands => "figures-grid-lineup-bands",
        Subject::LineupRatings => "figures-grid-lineup-ratings",
        Subject::EngineerCap => "figures-grid-engineer-cap",
        Subject::EnigmaRarities => "figures-grid-enigma-rarities",
        Subject::TipSettings => "figures-grid-tip-settings",
        Subject::GuideSpotlights => "figures-grid-guide-spotlights",
        Subject::FanfareTiers => "figures-grid-fanfare-tiers",
        Subject::StageShortcuts => "figures-grid-stage-shortcuts",
        Subject::EnergyRefunds => "figures-grid-energy-refunds",
        Subject::OrbOptions => "figures-grid-orb-options",
        Subject::LineupCounters => "figures-grid-lineup-counters",
        Subject::RecommendedUpgrades => "figures-grid-recommended-upgrades",
        Subject::EnigmaTimes => "figures-grid-enigma-times",
        Subject::MapMarkers => "figures-grid-map-markers",
        Subject::DiscoveryMedals => "figures-grid-discovery-medals",
        Subject::EnigmaGroups => "figures-grid-enigma-groups",
        Subject::EnigmaDrops => "figures-grid-enigma-drops",
        Subject::EnigmaRates => "figures-grid-enigma-rates",
        Subject::EnigmaLotteries => "figures-grid-enigma-lotteries",
        Subject::LineupExclusions => "figures-grid-lineup-exclusions",
        Subject::FilterMaps => "figures-grid-filter-maps",
        Subject::ZonedInSteps => "figures-grid-zoned-in-steps",
        Subject::ZonedIn => "figures-grid-zoned-in",
        Subject::FortuneOdds => "figures-grid-fortune-odds",
        Subject::ShrineExtras => "figures-grid-shrine-extras",
        Subject::ShrineOfferings => "figures-grid-shrine-offerings",
        Subject::PrizePatterns => "figures-grid-prize-patterns",
        Subject::GamatotoLevels => "figures-grid-gamatoto-levels",
        Subject::ExpeditionAreas => "figures-grid-expedition-areas",
        Subject::EnemyGuide => "figures-grid-enemy-guide",
        Subject::UnitLimits => "figures-grid-unit-limits",
        Subject::GuideEntries => "figures-grid-guide-entries",
        Subject::MaterialMixes => "figures-grid-material-mixes",
        Subject::DevelopmentTiers => "figures-grid-development-tiers",
        Subject::DevelopmentCosts => "figures-grid-development-costs",
    })
}

pub(super) fn grid<'a>(
    draft: &'a Draft,
    width: f32,
    shown: &[usize],
    dim_from: Option<usize>,
) -> Element<'a, Message> {
    let area = scrollable(centered(lay(draft, width, shown, dim_from)))
        .id(grid_id(draft.schema().subject()))
        .on_scroll(|viewport| Message::Scrolled(viewport.absolute_offset().y))
        .width(Length::Fill)
        .height(Length::Fill);

    smooth_scroll(area).into()
}

pub(super) fn band<'a>(draft: &'a Draft, width: f32, shown: &[usize]) -> Element<'a, Message> {
    container(lay(draft, width, shown, None)).width(Length::Fill).center_x(Length::Fill).into()
}

fn centered<'a>(rows: Column<'a, Message>) -> Container<'a, Message> {
    container(rows)
        .width(Length::Fill)
        .center_x(Length::Fill)
        .padding(Padding::ZERO.left(BODY_PADDING).right(BODY_PADDING).bottom(BODY_PADDING))
}

fn lay<'a>(draft: &'a Draft, width: f32, shown: &[usize], dim_from: Option<usize>) -> Column<'a, Message> {
    let per_row = (((usable(width) + CARD_GAP) / (CARD_WIDTH + CARD_GAP)).floor() as usize).max(1);

    let mut rows = Column::new().spacing(CARD_GAP);
    let mut line = Row::new().spacing(CARD_GAP);
    let mut used = 0;

    for index in shown {
        let span = span(draft.face(*index)).min(per_row);

        if used > 0 && used + span > per_row {
            rows = rows.push(line);
            line = Row::new().spacing(CARD_GAP);
            used = 0;
        }

        line = line.push(card(draft, *index, dim_from.is_some_and(|first| *index >= first)));
        used += span;
    }

    rows.push(line)
}

fn card(draft: &Draft, index: usize, dimmed: bool) -> Element<'_, Message> {
    let schema = draft.schema();
    let face = draft.face(index);
    let opaque = face == Face::Danger;
    let slots = span(face);

    let field = match face {
        Face::Toggle(current) => toggle_field(index, current),
        Face::Choice(options, current) => choice_field(index, options, current),
        Face::Disabled(reason) => disabled_field(reason),
        Face::Number | Face::Danger => number_field(draft, index),
    };

    let caption = text(draft.label(index))
        .size(LABEL_SIZE)
        .align_x(Horizontal::Center)
        .width(Length::Fill)
        .style(move |theme: &Theme| text::Style {
            color: dimmed.then(|| theme::weak_text_color(theme)),
        });

    let hint = if opaque { Some(OPAQUE_HINT) } else { draft.note(index) };

    let caption = match hint {
        Some(text) => hover_hint(caption, text),
        None => caption.into(),
    };

    let label = container(caption)
        .height(Length::Fixed(LABEL_HEIGHTS[schema.subject().slot()]))
        .align_y(Vertical::Center);

    let style = if opaque {
        theme::card_container_danger
    } else if dimmed {
        theme::card_container_muted
    } else {
        theme::card_container
    };

    let width = CARD_WIDTH * slots as f32 + CARD_GAP * (slots - 1) as f32;

    container(column![label, field].spacing(2))
        .width(Length::Fixed(width))
        .padding(CARD_PADDING)
        .style(style)
        .into()
}

pub(super) fn number(draft: &Draft, index: usize) -> Element<'_, Message> {
    number_field(draft, index)
}

pub(super) fn options<'a, T>(
    values: Vec<T>,
    current: T,
    picked: impl Fn(T) -> Message + 'a,
) -> Element<'a, Message>
where
    T: ToString + PartialEq + Clone + 'a,
{
    pick_list(values, Some(current), picked)
        .width(Length::Fill)
        .padding(FIELD_INSET)
        .text_size(INPUT_SIZE)
        .style(theme::combo_box)
        .menu_style(theme::combo_box_menu)
        .into()
}

fn number_field(draft: &Draft, index: usize) -> Element<'_, Message> {
    let schema = draft.schema();
    let values = draft.values();
    let scaled = schema.to_display(index, schema.fallback(index), values);
    let hint = draft.rule(index).to_display(scaled, values).to_string();

    text_input(&hint, draft.input(index))
        .on_input(move |entry| Message::Changed(index, entry))
        .size(INPUT_SIZE)
        .padding(FIELD_INSET)
        .align_x(Horizontal::Center)
        .style(theme::rounded_input)
        .into()
}

fn toggle_field<'a>(index: usize, current: Toggle) -> Element<'a, Message> {
    let style: theme::ButtonStyleFn = match current {
        Toggle::Yes => theme::success_button,
        Toggle::No => theme::neutral_button,
    };

    let label = theme::centered_text(current.to_string())
        .size(INPUT_SIZE)
        .width(Length::Fill)
        .wrapping(text::Wrapping::None);

    button(label)
        .width(Length::Fill)
        .padding(FIELD_INSET)
        .style(style)
        .on_press(Message::Picked(index, current.flip().raw()))
        .into()
}

fn disabled_field<'a>(reason: &'a str) -> Element<'a, Message> {
    let label = theme::centered_text(DISABLED_LABEL)
        .size(INPUT_SIZE)
        .width(Length::Fill)
        .wrapping(text::Wrapping::None);

    let inert = button(label).width(Length::Fill).padding(FIELD_INSET).style(theme::inert_button);

    hover_hint(inert, reason)
}

fn choice_field<'a>(
    index: usize,
    options: &'static [Choice],
    current: Choice,
) -> Element<'a, Message> {
    pick_list(options, Some(current), move |choice: Choice| Message::Picked(index, choice.raw()))
        .width(Length::Fill)
        .padding(FIELD_INSET)
        .text_size(INPUT_SIZE)
        .style(theme::combo_box)
        .menu_style(theme::combo_box_menu)
        .into()
}

pub(super) fn search<'a>(query: &str, width: f32, placeholder: &'a str) -> Element<'a, Message> {
    header(search_field(query, width, placeholder, Message::SearchChanged))
}

pub(super) fn hunt<'a>(query: &str, width: f32, placeholder: &'a str) -> Element<'a, Message> {
    header(search_field(query, width, placeholder, Message::HuntChanged))
}

fn search_field<'a>(
    query: &str,
    width: f32,
    placeholder: &'a str,
    typed: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
    let field = text_input(placeholder, query)
        .on_input(typed)
        .size(INPUT_SIZE)
        .padding(FIELD_PADDING)
        .width(Length::Fixed((usable(width) * SEARCH_FRACTION).max(CARD_WIDTH)))
        .style(theme::rounded_input);

    container(field).width(Length::Fill).center_x(Length::Fill).into()
}

pub(super) fn header<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content.into())
        .padding(Padding::ZERO.top(BODY_PADDING).left(BODY_PADDING).right(BODY_PADDING).bottom(CARD_GAP))
        .into()
}

pub(super) fn comment<'a>(value: &str) -> Element<'a, Message> {
    text_input("Comment...", value)
        .on_input(Message::CommentChanged)
        .size(INPUT_SIZE)
        .padding(FIELD_PADDING)
        .width(Length::Fill)
        .style(theme::rounded_input)
        .into()
}

pub(super) fn footer<'a>(rows: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    let mut stack = Column::with_capacity(rows.len()).spacing(CARD_GAP).align_x(Horizontal::Center);

    for row in rows {
        stack = stack.push(row);
    }

    container(stack)
        .width(Length::Fill)
        .center_x(Length::Fill)
        .padding(Padding::ZERO.top(CARD_GAP).left(BODY_PADDING).right(BODY_PADDING).bottom(BODY_PADDING))
        .into()
}

pub(super) fn sync<'a>(armed: bool) -> Element<'a, Message> {
    let label = if armed { CONFIRM_LABEL } else { "Sync With \"game\"" };

    let content = theme::centered_text(label).size(INPUT_SIZE).wrapping(text::Wrapping::None);

    button(content)
        .width(Length::Fixed(SYNC_WIDTH))
        .padding([FIELD_PADDING + 1.0, 10.0])
        .style(theme::danger_button)
        .on_press(Message::Sync)
        .into()
}

pub(super) fn shell<'a>(
    top: Option<Element<'a, Message>>,
    body: Element<'a, Message>,
    bottom: Element<'a, Message>,
) -> Element<'a, Message> {
    let mut stack = Column::new().height(Length::Fill);

    if let Some(top) = top {
        stack = stack.push(top);
    }

    stack.push(body).push(bottom).into()
}
