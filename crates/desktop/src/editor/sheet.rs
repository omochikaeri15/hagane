use std::fs;
use std::path::{Path, PathBuf};

use iced::alignment::Vertical;
use iced::widget::{button, column, container, responsive, row, scrollable, space, text, text_input, Column, Id};
use iced::{Element, Length, Padding, Size, Task};
use tracing::warn;

use codex::common;
use foundry::common::preview::{self, Stamp};
use foundry::domains::mods;
use foundry::domains::settings::EditorMode;
use foundry::Vfs;

use crate::app::theme;
use crate::common::feedback::{Slot, CONFIRM_LABEL, FAILURE_LABEL};
use crate::common::row_window::{self, RowWindow};
use crate::widget::{popup, smooth_scroll};

use super::{Mark, Reader};

const SIZE: Size = Size::new(420.0, 520.0);
const POPUP: popup::Spec = popup::Spec::new(popup::Kind::Sheet, SIZE);

const BODY_PADDING: f32 = 12.0;
const GAP: f32 = 8.0;
const LABEL_SIZE: f32 = 12.0;
const INPUT_SIZE: f32 = 13.0;
const INPUT_PADDING: f32 = 4.0;
const INPUT_WIDTH: f32 = 110.0;
const BUTTON_SIZE: f32 = 13.0;
const BUTTON_PADDING: f32 = 8.0;
const SCROLL_GAP: f32 = 12.0;
const FRAME_GAP: f32 = 3.0;
const FRAME_INSET: f32 = theme::CARD_BORDER_WIDTH + FRAME_GAP;
const LINE_ROW: f32 = 30.0;
const STACKED_ROW: f32 = 48.0;

const COMMENT: &str = "//";
const TAB: char = '\t';
const COMMA: char = ',';
const WEIGHT_LABEL: &str = "Rarity Tier";
const SYNC_LABEL: &str = "Sync With \"game\"";
const SEARCH_HINT: &str = "Search Field...";
const EMPTY_LABEL: &str = "No fields match";

const PERCENT_FLOOR: i32 = 0;
const PIPE: char = '|';
const MISSION_LABEL: &str = "Mission";
const EFFECT_LABEL: &str = "Effect";
const MEDAL_LABEL: &str = "Medal";
const TRAIT_LABEL: &str = "Trait";
const MESSAGE_LABEL: &str = "Message";
const BAND_LABEL: &str = "Strength";
const BAND_LABELS: [&str; 6] = ["Small", "Medium", "Large", "Ultimate", "Down", "Granted"];
const PERCENT_CEILING: i32 = 100;
const TREASURE_LABEL: &str = "Treasure";
const SET_LABEL: &str = "Set";
const TEXT_LABEL: &str = "Label";
const LEAD_CELL: usize = 1;
const AREA_LABEL: &str = "Area";
const EVENT_AREA_LABEL: &str = "Event Area";
const PHRASE_LABEL: &str = "Phrase";
const GOD_NAME_LABEL: &str = "Name";
const MIRACLE_LABEL: &str = "Miracle";
const WARNING_LABEL: &str = "Warning";
const PAGE_LABEL: &str = "Page";
const FILTER_LABEL: &str = "Filter";
const STORY_LABEL: &str = "Line";
const STORY_TITLE: &str = "Story";
const DAY_LABEL: &str = "Day";
const EVENT_LABEL: &str = "Event";
const EFFECT_CELL: usize = 2;
const TREASURE_LABELS: [&str; 12] =
    ["Inferior Grade", "Normal Grade", "Superior Grade", "Treasure", "From", "Activated", "Received", "Power", "Percent Sign", "Treasure From", "", "Item"];

pub(super) const TREASURE_NAMES: [(usize, &str); 5] = [
    (0, "Treasure1_0.csv"),
    (4, "Treasure1_1.csv"),
    (7, "Treasure1_2_0.csv"),
    (8, "Treasure1_2_1.csv"),
    (9, "Treasure1_2_2.csv"),
];

pub(super) const TREASURE_SETS: [(usize, &str); 5] = [
    (0, "Treasure3_0.csv"),
    (4, "Treasure3_1.csv"),
    (7, "Treasure3_2_0.csv"),
    (8, "Treasure3_2_1.csv"),
    (9, "Treasure3_2_2.csv"),
];

pub(super) const TREASURE_RETOLD: (usize, &str) = (4, "Treasure3_1_AfterFirstEncounter.csv");

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum Sheet {
    Param,
    Localizable,
    Weights,
    MissionText,
    ComboEffects,
    ComboBands,
    MedalNames,
    MedalDescriptions,
    OrbNames,
    OrbDescriptions,
    TraitNames,
    TraitDescriptions,
    RankMessages,
    TreasureNames(&'static str),
    TreasureSets(&'static str),
    TreasureLeads(&'static str),
    TreasureEffects(&'static str),
    TreasureLabels,
    GamatotoAreas,
    GamatotoEventAreas,
    GamatotoConnectors,
    GamatotoObjects,
    GamatotoActions,
    GodName,
    MiracleNames,
    Warnings,
    PageNames,
    StampMessages,
    CalendarFilters,
    Story(&'static str),
    LoginMessages,
    CalendarEvents,
    DailyEvents,
}

pub(super) const LOGIN_PREFIX: &str = "DailyLoginEventText_";
pub(super) const LOGIN_EXTENSION: &str = "tsv";

pub(super) const STORIES: [(&str, &str); 11] = [
    ("OP.csv", "Opening"),
    ("OP2.csv", "Opening 2"),
    ("OP3.csv", "Opening 3"),
    ("ED.csv", "Ending"),
    ("ED2.csv", "Ending 2"),
    ("ED3.csv", "Ending 3"),
    ("OPLegend.csv", "Legend Opening"),
    ("EDLegend.csv", "Legend Ending"),
    ("EDLegend2.csv", "Legend Ending 2"),
    ("OPDemon.csv", "Demon Opening"),
    ("EDDemon.csv", "Demon Ending"),
];

pub(crate) const SHEETS: &[Sheet] = &[Sheet::Param, Sheet::Localizable];

impl Sheet {
    pub(super) fn title(self) -> &'static str {
        match self {
            Sheet::Param => "Global Parameters",
            Sheet::Localizable => "Localized Strings",
            Sheet::Weights => "Lottery Weights",
            Sheet::MissionText => "Mission Text",
            Sheet::ComboEffects => "Combo Effect Names",
            Sheet::ComboBands => "Combo Strength Names",
            Sheet::MedalNames => "Medal Names",
            Sheet::MedalDescriptions => "Medal Descriptions",
            Sheet::OrbNames => "Orb Effect Names",
            Sheet::OrbDescriptions => "Orb Effect Descriptions",
            Sheet::TraitNames => "Orb Trait Names",
            Sheet::TraitDescriptions => "Orb Trait Descriptions",
            Sheet::RankMessages => "Rank Reward Messages",
            Sheet::TreasureNames(_) => "Treasure Names",
            Sheet::TreasureSets(_) => "Set Names",
            Sheet::TreasureLeads(_) => "Set Effect Line 1",
            Sheet::TreasureEffects(_) => "Set Effect Line 2",
            Sheet::TreasureLabels => "Treasure Labels",
            Sheet::GamatotoAreas => "Area Names",
            Sheet::GamatotoEventAreas => "Event Area Names",
            Sheet::GamatotoConnectors => "Log Connectors",
            Sheet::GamatotoObjects => "Log Objects",
            Sheet::GamatotoActions => "Log Actions",
            Sheet::GodName => "Cat God Name",
            Sheet::MiracleNames => "Miracle Names",
            Sheet::Warnings => "Warnings",
            Sheet::PageNames => "Page Names",
            Sheet::StampMessages => "Stamp Messages",
            Sheet::CalendarFilters => "Calendar Filters",
            Sheet::Story(file) => STORIES.iter().find(|(held, _)| *held == file).map_or(STORY_TITLE, |(_, title)| *title),
            Sheet::LoginMessages => "Login Messages",
            Sheet::CalendarEvents => "Calendar Events",
            Sheet::DailyEvents => "All-Day Events",
        }
    }

    pub(super) fn base(self) -> &'static str {
        match self {
            Sheet::Param => "param.tsv",
            Sheet::Localizable => "localizable.tsv",
            Sheet::Weights => "RoguelikeBonusLotteryWeight.csv",
            Sheet::MissionText => "Mission_Name.csv",
            Sheet::ComboEffects => "Nyancombo1.csv",
            Sheet::ComboBands => "Nyancombo2.csv",
            Sheet::MedalNames | Sheet::MedalDescriptions => "medalname.tsv",
            Sheet::OrbNames | Sheet::OrbDescriptions => "equipment_explonation.tsv",
            Sheet::TraitNames | Sheet::TraitDescriptions => "attribute_explonation.tsv",
            Sheet::RankMessages => "rankGiftMessage.tsv",
            Sheet::TreasureNames(file) | Sheet::TreasureSets(file) | Sheet::TreasureLeads(file) | Sheet::TreasureEffects(file) => file,
            Sheet::TreasureLabels => "Treasure2.csv",
            Sheet::GamatotoAreas => "GamatotoExpedition_Stage_name.csv",
            Sheet::GamatotoEventAreas => "GamatotoExpedition_Stage_nameEvent.csv",
            Sheet::GamatotoConnectors => "GamatotoExpedition_Log_1.csv",
            Sheet::GamatotoObjects => "GamatotoExpedition_Log_2.csv",
            Sheet::GamatotoActions => "GamatotoExpedition_Log_3.csv",
            Sheet::GodName => "GodName.csv",
            Sheet::MiracleNames => "GodItemName.csv",
            Sheet::Warnings => "Warning1.csv",
            Sheet::PageNames => "PageName.csv",
            Sheet::StampMessages => "StampMessage.csv",
            Sheet::CalendarFilters => "CalendarFilter.csv",
            Sheet::Story(file) => file,
            Sheet::LoginMessages => "DailyLoginEventText.tsv",
            Sheet::CalendarEvents => "Guerrilla_calendar.tsv",
            Sheet::DailyEvents => "All_day_event.tsv",
        }
    }

    fn textual(self) -> bool {
        match self {
            Sheet::Param | Sheet::Weights => false,
            Sheet::Localizable
            | Sheet::MissionText
            | Sheet::ComboEffects
            | Sheet::ComboBands
            | Sheet::MedalNames
            | Sheet::MedalDescriptions
            | Sheet::OrbNames
            | Sheet::OrbDescriptions
            | Sheet::TraitNames
            | Sheet::TraitDescriptions
            | Sheet::RankMessages
            | Sheet::TreasureNames(_)
            | Sheet::TreasureSets(_)
            | Sheet::TreasureLeads(_)
            | Sheet::TreasureEffects(_)
            | Sheet::TreasureLabels
            | Sheet::GamatotoAreas
            | Sheet::GamatotoEventAreas
            | Sheet::GamatotoConnectors
            | Sheet::GamatotoObjects
            | Sheet::GamatotoActions
            | Sheet::GodName
            | Sheet::MiracleNames
            | Sheet::Warnings
            | Sheet::PageNames
            | Sheet::StampMessages
            | Sheet::CalendarFilters
            | Sheet::Story(_)
            | Sheet::LoginMessages
            | Sheet::CalendarEvents
            | Sheet::DailyEvents => true,
        }
    }

    fn pitch(self) -> f32 {
        match self.textual() {
            true => STACKED_ROW,
            false => LINE_ROW,
        }
    }

    fn resolved(self, key: &str) -> String {
        match self {
            Sheet::Param => PARAM_NAMES
                .iter()
                .find(|(known, _)| *known == key)
                .map_or_else(|| prettify(key), |(_, label)| (*label).to_owned()),
            Sheet::Localizable => prettify(key),
            Sheet::Weights => format!("{WEIGHT_LABEL} {key}"),
            Sheet::MissionText => format!("{MISSION_LABEL} {key}"),
            Sheet::ComboEffects => format!("{EFFECT_LABEL} {key}"),
            Sheet::ComboBands => key.parse::<usize>().ok().and_then(|at| BAND_LABELS.get(at)).map_or_else(|| format!("{BAND_LABEL} {key}"), |band| (*band).to_owned()),
            Sheet::MedalNames | Sheet::MedalDescriptions => format!("{MEDAL_LABEL} {key}"),
            Sheet::OrbNames | Sheet::OrbDescriptions => format!("{EFFECT_LABEL} {key}"),
            Sheet::TraitNames | Sheet::TraitDescriptions => format!("{TRAIT_LABEL} {key}"),
            Sheet::RankMessages => format!("{MESSAGE_LABEL} {key}"),
            Sheet::TreasureNames(_) => format!("{TREASURE_LABEL} {key}"),
            Sheet::TreasureSets(_) | Sheet::TreasureLeads(_) | Sheet::TreasureEffects(_) => format!("{SET_LABEL} {key}"),
            Sheet::TreasureLabels => key
                .parse::<usize>()
                .ok()
                .and_then(|at| TREASURE_LABELS.get(at))
                .filter(|label| !label.is_empty())
                .map_or_else(|| format!("{TEXT_LABEL} {key}"), |label| (*label).to_owned()),
            Sheet::GamatotoAreas => counted(AREA_LABEL, key),
            Sheet::GamatotoEventAreas => counted(EVENT_AREA_LABEL, key),
            Sheet::GamatotoConnectors | Sheet::GamatotoObjects | Sheet::GamatotoActions => counted(PHRASE_LABEL, key),
            Sheet::GodName => GOD_NAME_LABEL.to_owned(),
            Sheet::MiracleNames => counted(MIRACLE_LABEL, key),
            Sheet::Warnings => counted(WARNING_LABEL, key),
            Sheet::PageNames => counted(PAGE_LABEL, key),
            Sheet::StampMessages => counted(MESSAGE_LABEL, key),
            Sheet::CalendarFilters => counted(FILTER_LABEL, key),
            Sheet::Story(_) => counted(STORY_LABEL, key),
            Sheet::LoginMessages => counted(DAY_LABEL, key),
            Sheet::CalendarEvents | Sheet::DailyEvents => counted(EVENT_LABEL, key),
        }
    }

    fn limits(self, key: &str) -> Option<(i32, i32)> {
        match self {
            Sheet::Param => (PARAM_PERCENTS.iter().any(|known| key.starts_with(known))
                && !PARAM_MULTIPLIERS.iter().any(|known| key.starts_with(known)))
            .then_some((PERCENT_FLOOR, PERCENT_CEILING)),
            Sheet::Localizable
            | Sheet::MissionText
            | Sheet::ComboEffects
            | Sheet::ComboBands
            | Sheet::MedalNames
            | Sheet::MedalDescriptions
            | Sheet::OrbNames
            | Sheet::OrbDescriptions
            | Sheet::TraitNames
            | Sheet::TraitDescriptions
            | Sheet::RankMessages
            | Sheet::TreasureNames(_)
            | Sheet::TreasureSets(_)
            | Sheet::TreasureLeads(_)
            | Sheet::TreasureEffects(_)
            | Sheet::TreasureLabels
            | Sheet::GamatotoAreas
            | Sheet::GamatotoEventAreas
            | Sheet::GamatotoConnectors
            | Sheet::GamatotoObjects
            | Sheet::GamatotoActions
            | Sheet::GodName
            | Sheet::MiracleNames
            | Sheet::Warnings
            | Sheet::PageNames
            | Sheet::StampMessages
            | Sheet::CalendarFilters
            | Sheet::Story(_)
            | Sheet::LoginMessages
            | Sheet::CalendarEvents
            | Sheet::DailyEvents => None,
            Sheet::Weights => Some((0, i32::MAX)),
        }
    }

    fn field(self, line: usize, text: &str) -> Option<(Field, String)> {
        match self {
            Sheet::Localizable => {
                let (key, value) = text.split_once(TAB)?;

                Some((Field { line, key: key.to_owned(), head: format!("{key}{TAB}"), tail: String::new() }, value.to_owned()))
            }
            Sheet::Weights => {
                let (key, value) = text.split_once(COMMA)?;

                Some((Field { line, key: key.to_owned(), head: format!("{key}{COMMA}"), tail: String::new() }, value.to_owned()))
            }
            Sheet::MissionText => {
                let digits = text.len() - text.trim_start_matches(|glyph: char| glyph.is_ascii_digit()).len();
                let key = text.get(..digits).filter(|key| !key.is_empty())?;
                let rest = text.get(digits..)?;
                let separator = rest.chars().next().filter(|glyph| [PIPE, COMMA].contains(glyph))?;
                let value = rest.get(separator.len_utf8()..)?;

                Some((Field { line, key: key.to_owned(), head: format!("{key}{separator}"), tail: String::new() }, value.to_owned()))
            }
            Sheet::ComboEffects | Sheet::ComboBands => {
                let separator = text.chars().last().filter(|glyph| [PIPE, COMMA].contains(glyph))?;
                let value = text.strip_suffix(separator)?;

                Some((Field { line, key: line.to_string(), head: String::new(), tail: separator.to_string() }, value.to_owned()))
            }
            Sheet::MedalNames | Sheet::OrbNames | Sheet::TraitNames | Sheet::RankMessages => {
                let (name, rest) = text.split_once(TAB)?;

                Some((Field { line, key: line.to_string(), head: String::new(), tail: format!("{TAB}{rest}") }, name.to_owned()))
            }
            Sheet::MedalDescriptions | Sheet::OrbDescriptions | Sheet::TraitDescriptions => {
                let (name, rest) = text.split_once(TAB)?;

                Some((Field { line, key: line.to_string(), head: format!("{name}{TAB}"), tail: String::new() }, rest.to_owned()))
            }
            Sheet::TreasureNames(_)
            | Sheet::TreasureSets(_)
            | Sheet::TreasureLabels
            | Sheet::GamatotoAreas
            | Sheet::GamatotoEventAreas
            | Sheet::GamatotoConnectors
            | Sheet::GamatotoObjects
            | Sheet::GamatotoActions
            | Sheet::GodName
            | Sheet::MiracleNames => nth_cell(line, text, 0),
            Sheet::Warnings | Sheet::PageNames | Sheet::StampMessages | Sheet::CalendarFilters => nth_cell(line, text, 0).or_else(|| whole(line, text)),
            Sheet::Story(_) => nth_cell(line, text, 0).or_else(|| spacer(line, text)),
            Sheet::LoginMessages | Sheet::CalendarEvents | Sheet::DailyEvents => lead_cell(line, text, TAB),
            Sheet::TreasureLeads(_) => nth_cell(line, text, LEAD_CELL),
            Sheet::TreasureEffects(_) => nth_cell(line, text, EFFECT_CELL),
            Sheet::Param => numbered(line, text),
        }
    }

    fn fields(self, lines: &[String]) -> (Vec<Field>, Vec<String>) {
        lines.iter().enumerate().filter_map(|(at, text)| self.field(at, text)).unzip()
    }
}

const PARAM_NAMES: &[(&str, &str)] = &[
    ("battle_slot_desc_wait", "Hold Cat Details Time"),
    ("battle_death_volcano_time", "Time Until Death Surge"),
    ("battle_super_beast_hunter", "Behemoth Slayer Attack Multiplier"),
    ("battle_super_beast_hunter_df", "Behemoth Slayer Defense Multiplier"),
    ("logo_gimmick_sec", "Superfeline Logo Roll Timer"),
    ("logo_gimmick_probability", "Superfeline Logo Chance %"),
    ("logo_gimmick_increase", "Superfeline Logo Failure Increase"),
    ("volume_percentage_s", "Small Volume %"),
    ("volume_percentage_m", "Medium Volume %"),
    ("volume_percentage_l", "Large Volume %"),
    ("summon_position", "Spirit Position Offset"),
    ("battle_super_sage_weaken", "Sage Resists Weaken %"),
    ("battle_super_sage_freeze", "Sage Resists Freeze %"),
    ("battle_super_sage_slow", "Sage Resists Slow %"),
    ("battle_super_sage_curse", "Sage Resists Curse %"),
    ("battle_super_sage_knockback", "Sage Resists Knockback %"),
    ("battle_super_sage_hunter_weaken", "Sage Slayer Resists Weaken %"),
    ("battle_super_sage_hunter_freeze", "Sage Slayer Resists Freeze %"),
    ("battle_super_sage_hunter_slow", "Sage Slayer Resists Slow %"),
    ("battle_super_sage_hunter_curse", "Sage Slayer Resists Curse %"),
    ("battle_super_sage_hunter_knockback", "Sage Slayer Resists Other %"),
    ("battle_super_sage_hunter_warp", "Sage Slayer Resists Warp %"),
    ("battle_super_sage_hunter_damage1", "Sage Slayer Attack Multiplier"),
    ("battle_super_sage_hunter_damage2", "Sage Slayer Defense Multiplier"),
    ("battle_metal_killer_hp_type", "Metal Killer Hitpoint Behavior"),
    ("story_fast_forward_speed", "Story Hold Speed"),
    ("battle_explosion_frame1", "Explosion Start Time"),
    ("battle_explosion_frame2", "Explosion Hitbox Duration"),
    ("battle_explosion_frame3", "Explosion Delay Before Deletion"),
    ("battle_explosion_frame4", "Explosion Delay Until Spike"),
    ("battle_explosion_width1", "Explosion Big Width"),
    ("battle_explosion_width2", "Explosion Medium Width"),
    ("battle_explosion_width3", "Explosion Small Width"),
    ("battle_explosion_damage1", "Explosion Big Damage Multiplier"),
    ("battle_explosion_damage2", "Explosion Medium Damage Multiplier"),
    ("battle_explosion_damage3", "Explosion Small Damage Multiplier"),
    ("battle_wave_s_st", "Mini-Wave Strength %"),
    ("battle_wave_s_inter", "Mini-Wave Interval"),
    ("battle_wave_s_frame1", "Mini-Wave First Frame"),
    ("battle_wave_s_frame2", "Mini-Wave Last Frame"),
    ("battle_af_death_volcano_distance", "Death Surge Distance"),
    ("battle_af_death_volcano_width", "Death Surge Width"),
    ("battle_sentai_recast", "Squad Recast Time"),
    ("battle_powerup_kill_enemy", "Power-Up Kills"),
    ("comeback_treasure_day", "Comeback Treasure Festival Days"),
    ("tutorial_money", "Tutorial Money"),
    ("option_vibration_power_ios", "Vibration Power (iOS)"),
    ("option_vibration_power_android", "Vibration Power (Android)"),
    ("option_vibration_time_ios", "Vibration Time (iOS)"),
    ("option_vibration_time_android", "Vibration Time (Android)"),
    ("option_vibration_sharpness_ios", "Vibration Sharpness (iOS)"),
    ("roguelike_charabonus_plusvalue", "Registered Plus Levels"),
];

const PARAM_PERCENTS: &[&str] = &["volume_percentage_", "battle_super_sage_", "logo_gimmick_probability", "battle_wave_s_st"];

const PARAM_MULTIPLIERS: &[&str] = &["battle_super_sage_hunter_damage"];

#[derive(Clone, PartialEq)]
pub(crate) struct Plan {
    sheet: Sheet,
    label: String,
    file: String,
    game: PathBuf,
    target_mod: String,
    values: EditorMode,
}

pub(super) fn plan(sheet: Sheet, label: String, file: String, game: PathBuf, target_mod: String, values: EditorMode) -> Plan {
    Plan { sheet, label, file, game, target_mod, values }
}

impl Plan {
    fn held(&self, vfs: &Vfs) -> Option<PathBuf> {
        mods::find(vfs, &self.target_mod, Path::new(&self.file))
    }

    fn source(&self, vfs: &Vfs) -> PathBuf {
        self.held(vfs).unwrap_or_else(|| self.game.clone())
    }
}

struct Field {
    line: usize,
    key: String,
    head: String,
    tail: String,
}

fn whole(line: usize, text: &str) -> Option<(Field, String)> {
    (!text.trim().is_empty()).then(|| (Field { line, key: line.to_string(), head: String::new(), tail: String::new() }, text.to_owned()))
}

fn lead_cell(line: usize, text: &str, separator: char) -> Option<(Field, String)> {
    let (value, rest) = text.split_once(separator).unwrap_or((text, ""));
    let tail = match text.len() > value.len() {
        true => format!("{separator}{rest}"),
        false => String::new(),
    };

    Some((Field { line, key: line.to_string(), head: String::new(), tail }, value.to_owned()))
}

fn spacer(line: usize, text: &str) -> Option<(Field, String)> {
    whole(line, text).or_else(|| text.trim().is_empty().then(|| (Field { line, key: line.to_string(), head: String::new(), tail: String::new() }, text.to_owned())))
}

fn counted(noun: &str, key: &str) -> String {
    key.parse::<usize>().map_or_else(|_| format!("{noun} {key}"), |at| format!("{noun} {}", at + 1))
}

fn nth_cell(line: usize, text: &str, cell: usize) -> Option<(Field, String)> {
    let separator = [PIPE, COMMA].into_iter().find(|glyph| text.contains(*glyph))?;
    let mut start = 0;

    for _ in 0..cell {
        start += text.get(start..)?.find(separator)? + separator.len_utf8();
    }

    let end = start + text.get(start..)?.find(separator)?;
    let value = text.get(start..end)?;

    Some((Field { line, key: line.to_string(), head: text.get(..start)?.to_owned(), tail: text.get(end..)?.to_owned() }, value.to_owned()))
}

fn numbered(line: usize, text: &str) -> Option<(Field, String)> {
    let content = text.split(COMMENT).next().unwrap_or_default();
    let key = content.split_whitespace().next()?;
    let value = content.split_whitespace().last().filter(|value| *value != key)?;
    let at = content.rfind(value)?;

    let head = text[..at].to_owned();
    let tail = text[at + value.len()..].to_owned();

    Some((Field { line, key: key.to_owned(), head, tail }, value.to_owned()))
}

struct Draft {
    plan: Plan,
    read_from: PathBuf,
    stamp: Stamp,
    lines: Vec<String>,
    fields: Vec<Field>,
    inputs: Vec<String>,
    labels: Vec<String>,
    failed: bool,
}

impl Draft {
    fn load(plan: Plan, vfs: &Vfs) -> Option<Draft> {
        let read_from = plan.source(vfs);

        let bytes = fs::read(&read_from)
            .inspect_err(|err| warn!(path = %read_from.display(), "Sheet editor could not read the file: {}", err))
            .ok()?;

        let stamp = preview::stamp(&read_from)?;
        let lines: Vec<String> = common::scrub(&bytes).lines().map(str::to_owned).collect();
        let (fields, inputs) = plan.sheet.fields(&lines);
        let labels = labels(&plan, &fields);

        Some(Draft { plan, read_from, stamp, lines, fields, inputs, labels, failed: false })
    }

    fn drifted(&self) -> bool {
        preview::stamp(&self.read_from) != Some(self.stamp)
    }

    fn bounds(&self, index: usize) -> Option<(i32, i32)> {
        let field = self.fields.get(index)?;

        (self.plan.values == EditorMode::Resolved).then(|| self.plan.sheet.limits(&field.key)).flatten()
    }

    fn edit(&mut self, index: usize, typed: &str) -> bool {
        let textual = self.plan.sheet.textual();

        if !textual && !typable(typed) {
            return false;
        }

        let bounds = self.bounds(index);

        let Some(slot) = self.inputs.get_mut(index) else {
            return false;
        };

        let value = match textual {
            true => typed.to_owned(),
            false => {
                let Ok(number) = typed.parse::<i32>() else {
                    *slot = typed.to_owned();

                    return false;
                };

                let number = bounds.map_or(number, |(least, most)| number.clamp(least, most));

                number.to_string()
            }
        };

        slot.clone_from(&value);

        let Some(field) = self.fields.get(index) else {
            return false;
        };

        let line = format!("{}{value}{}", field.head, field.tail);

        let Some(held) = self.lines.get_mut(field.line) else {
            return false;
        };

        if *held == line {
            return false;
        }

        *held = line;

        true
    }

    fn restore(&mut self, vfs: &Vfs) -> bool {
        let Ok(bytes) = fs::read(&self.plan.game) else {
            warn!(path = %self.plan.game.display(), "Sheet editor could not read the vanilla file");

            return false;
        };

        self.lines = common::scrub(&bytes).lines().map(str::to_owned).collect();

        let (fields, inputs) = self.plan.sheet.fields(&self.lines);

        self.labels = labels(&self.plan, &fields);
        self.fields = fields;
        self.inputs = inputs;

        self.persist(vfs)
    }

    fn destination(&self, vfs: &Vfs) -> Option<(PathBuf, Stamp)> {
        if self.read_from != self.plan.game {
            return Some((self.read_from.clone(), self.stamp));
        }

        let path = mods::ensure_as(vfs, &self.plan.target_mod, &self.plan.game, &self.plan.file)
            .inspect_err(|err| warn!(source = %self.plan.game.display(), "Sheet editor could not stage the file: {}", err))
            .ok()?;

        if let Err(err) = vfs.create((self.plan.target_mod.as_str(), path.as_path())) {
            warn!(path = %path.display(), "Sheet editor staged a file the index would not take: {}", err);
        }

        preview::stamp(&path).map(|stamp| (path, stamp))
    }

    fn persist(&mut self, vfs: &Vfs) -> bool {
        let Some((path, stamp)) = self.destination(vfs) else {
            self.failed = true;

            return false;
        };

        let mut body = self.lines.join("\n");
        body.push('\n');

        match preview::save(&path, body.as_bytes(), stamp) {
            Ok(fresh) => {
                self.read_from = path;
                self.stamp = fresh;
                self.failed = false;

                true
            }
            Err(err) => {
                warn!(path = %path.display(), "Sheet editor could not write the file: {}", err);
                self.failed = true;

                false
            }
        }
    }
}

fn labels(plan: &Plan, fields: &[Field]) -> Vec<String> {
    fields
        .iter()
        .map(|field| match plan.values {
            EditorMode::Raw => field.key.clone(),
            EditorMode::Resolved => plan.sheet.resolved(&field.key),
        })
        .collect()
}

fn typable(typed: &str) -> bool {
    let digits = typed.strip_prefix('-').unwrap_or(typed);

    digits.chars().all(|glyph| glyph.is_ascii_digit())
}

fn prettify(key: &str) -> String {
    key.split('_')
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();

            chars.next().map_or_else(String::new, |first| first.to_uppercase().chain(chars).collect())
        })
        .collect::<Vec<String>>()
        .join(" ")
}

fn list_id() -> Id {
    Id::new("sheet-list")
}

#[derive(Debug, Clone)]
pub enum Message {
    Popup(popup::Message),
    Searched(String),
    Changed(usize, String),
    Scrolled(f32),
    Sync,
    ConfirmExpired,
    FailureExpired,
}

#[derive(Default)]
pub(super) struct State {
    draft: Option<Draft>,
    frame: popup::State,
    query: String,
    shown: Vec<usize>,
    offset: f32,
    confirm: Slot<()>,
    failed: Slot<()>,
}

pub(super) fn kind() -> popup::Kind {
    POPUP.kind()
}

impl State {
    pub(super) fn begin(&mut self, plan: Plan, nudge: usize, vfs: &Vfs) {
        self.frame = popup::cascaded(nudge);
        self.query.clear();
        self.offset = 0.0;
        self.confirm.clear();
        self.failed.clear();
        self.draft = Draft::load(plan, vfs);
        self.refilter();
    }

    pub(super) fn drafting(&self) -> bool {
        self.draft.is_some()
    }

    pub(super) fn raised(&self) -> u64 {
        self.frame.raised()
    }

    pub(super) fn drifted(&self) -> bool {
        self.draft.as_ref().is_some_and(Draft::drifted)
    }

    fn refilter(&mut self) {
        let needle = self.query.trim().to_lowercase();

        self.shown = self.draft.as_ref().map_or_else(Vec::new, |draft| {
            (0..draft.labels.len())
                .filter(|index| {
                    needle.is_empty()
                        || draft.labels.get(*index).is_some_and(|label| label.to_lowercase().contains(&needle))
                        || (draft.plan.sheet.textual()
                            && draft.inputs.get(*index).is_some_and(|input| input.to_lowercase().contains(&needle)))
                })
                .collect()
        });
    }

    pub(super) fn sync(&mut self, plan: Option<Plan>, vfs: &Vfs) {
        let Some(current) = self.draft.as_ref() else {
            return;
        };

        let Some(plan) = plan else {
            self.draft = None;

            return;
        };

        let moved = current.plan.held(vfs).is_some() != (current.read_from != current.plan.game);

        if current.plan != plan || current.drifted() || moved {
            self.draft = Draft::load(plan, vfs);
            self.refilter();
        }
    }

    pub(super) fn update(&mut self, message: Message, vfs: &Vfs) -> Task<Message> {
        match message {
            Message::Popup(msg) => {
                if self.frame.update(msg, POPUP) {
                    self.draft = None;
                    self.confirm.clear();
                    self.failed.clear();
                }

                Task::none()
            }
            Message::Searched(query) => {
                self.query = query;
                self.offset = 0.0;
                self.refilter();

                iced::widget::operation::scroll_to(list_id(), scrollable::AbsoluteOffset { x: 0.0, y: 0.0 })
            }
            Message::Changed(index, typed) => {
                let Some(draft) = self.draft.as_mut() else {
                    return Task::none();
                };

                if draft.edit(index, &typed) && !draft.persist(vfs) {
                    return self.failed.set((), Message::FailureExpired);
                }

                Task::none()
            }
            Message::Scrolled(offset) => {
                self.offset = offset;

                Task::none()
            }
            Message::Sync => {
                if !self.confirm.take(&()) {
                    return self.confirm.set((), Message::ConfirmExpired);
                }

                let restored = self.draft.as_mut().is_some_and(|draft| draft.restore(vfs));

                self.refilter();

                match restored {
                    true => Task::none(),
                    false => self.failed.set((), Message::FailureExpired),
                }
            }
            Message::ConfirmExpired => {
                self.confirm.expire();

                Task::none()
            }
            Message::FailureExpired => {
                self.failed.expire();

                Task::none()
            }
        }
    }

    pub(super) fn view(&self, window: Size) -> Option<Element<'_, Message>> {
        let draft = self.draft.as_ref()?;

        Some(self.frame.view(&draft.plan.label, POPUP, window, Message::Popup, move || self.body(draft), None))
    }

    fn body<'a>(&'a self, draft: &'a Draft) -> Element<'a, Message> {
        let search = text_input(SEARCH_HINT, &self.query)
            .on_input(Message::Searched)
            .padding(INPUT_PADDING + 2.0)
            .size(INPUT_SIZE)
            .width(Length::Fill)
            .style(theme::rounded_input);

        let listing: Element<'_, Message> = match self.shown.is_empty() {
            true => container(theme::centered_text(EMPTY_LABEL).size(LABEL_SIZE).style(text::danger))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into(),
            false => responsive(move |size: Size| self.rows(draft, size)).into(),
        };

        let framed = container(listing)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(FRAME_INSET)
            .style(theme::card_container_outlined);

        let label = match (self.failed.is_set() || draft.failed, self.confirm.is_set()) {
            (true, _) => FAILURE_LABEL,
            (false, true) => CONFIRM_LABEL,
            (false, false) => SYNC_LABEL,
        };

        let sync = button(theme::button_label(label).size(BUTTON_SIZE))
            .width(Length::Fill)
            .padding(BUTTON_PADDING)
            .style(theme::danger_button)
            .on_press(Message::Sync);

        column![search, framed, sync]
            .spacing(GAP)
            .padding(BODY_PADDING)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn rows<'a>(&'a self, draft: &'a Draft, size: Size) -> Element<'a, Message> {
        let pitch = draft.plan.sheet.pitch();

        let RowWindow { range, pad_before, pad_after } =
            row_window::compute_with(self.shown.len(), size.height, self.offset, pitch, 0.0);

        let mut list = Column::with_capacity(range.len() + 2).width(Length::Fill);

        if pad_before > 0.0 {
            list = list.push(space().height(Length::Fixed(pad_before)));
        }

        for at in range {
            let Some(&index) = self.shown.get(at) else {
                continue;
            };

            list = list.push(line(draft, index, pitch));
        }

        if pad_after > 0.0 {
            list = list.push(space().height(Length::Fixed(pad_after)));
        }

        let area = scrollable(container(list).padding(Padding::default().right(SCROLL_GAP)))
            .id(list_id())
            .on_scroll(|viewport| Message::Scrolled(viewport.absolute_offset().y))
            .width(Length::Fill)
            .height(Length::Fill);

        smooth_scroll(area).into()
    }
}

fn line(draft: &Draft, index: usize, pitch: f32) -> Element<'_, Message> {
    let label = draft.labels.get(index).map_or("", String::as_str);
    let input = draft.inputs.get(index).map_or("", String::as_str);

    let field = text_input("", input)
        .on_input(move |typed| Message::Changed(index, typed))
        .padding(INPUT_PADDING)
        .size(INPUT_SIZE)
        .style(theme::rounded_input);

    let body: Element<'_, Message> = match draft.plan.sheet.textual() {
        true => column![text(label).size(LABEL_SIZE), field.width(Length::Fill)].spacing(2).into(),
        false => row![text(label).size(LABEL_SIZE).width(Length::Fill), field.width(Length::Fixed(INPUT_WIDTH))]
            .spacing(GAP)
            .align_y(Vertical::Center)
            .into(),
    };

    container(body).height(Length::Fixed(pitch)).align_y(Vertical::Center).into()
}

pub(super) fn mark(plan: &Plan, vfs: &Vfs, reader: &mut Reader) -> Mark {
    let Some(held) = plan.held(vfs) else {
        return Mark::Same;
    };

    Mark::of(reader.text(&held), reader.text(&plan.game))
}

pub(super) fn reset(plan: Plan, vfs: &Vfs) -> bool {
    Draft::load(plan, vfs).is_some_and(|mut draft| draft.restore(vfs))
}

#[cfg(test)]
mod tests {
    use super::Sheet;

    #[test]
    fn a_value_is_rewritten_without_touching_the_rest_of_its_line() {
        let (held, value) = Sheet::Param.field(3, "battle_zoom_y\t520 // camera").expect("a key and a value");

        assert_eq!(held.key, "battle_zoom_y");
        assert_eq!(value, "520");
        assert_eq!(format!("{}{}{}", held.head, 640, held.tail), "battle_zoom_y\t640 // camera");

        // a key with no value is not a field at all
        assert!(Sheet::Param.field(0, "lonely_key").is_none());
    }

    #[test]
    fn mission_text_splits_at_its_pipe() {
        let (held, value) = Sheet::MissionText.field(2, "2|Raise your User Rank by %d!<br>(more)").expect("an id and a text");

        assert_eq!((held.key.as_str(), value.as_str()), ("2", "Raise your User Rank by %d!<br>(more)"));
        assert_eq!(format!("{}{}{}", held.head, value, held.tail), "2|Raise your User Rank by %d!<br>(more)");

        // the Japanese file puts a comma where the others put a pipe
        let (held, value) = Sheet::MissionText.field(0, "0,テスト,です").expect("a comma split");
        assert_eq!((held.key.as_str(), value.as_str()), ("0", "テスト,です"));
    }

    #[test]
    fn combo_text_keeps_its_trailing_separator() {
        let (held, value) = Sheet::ComboBands.field(0, " (Sm)|").expect("a band name");

        assert_eq!((held.key.as_str(), value.as_str()), ("0", " (Sm)"));
        assert_eq!(format!("{}{}{}", held.head, "【小】", held.tail), "【小】|");
        assert!(Sheet::ComboEffects.field(3, "no separator").is_none());
    }

    #[test]
    fn a_medal_line_edits_its_name_and_description_apart() {
        let line = "Builder\tCollect all Gold Treasures";
        let (name, value) = Sheet::MedalNames.field(4, line).expect("a name");
        let (description, rest) = Sheet::MedalDescriptions.field(4, line).expect("a description");

        assert_eq!((value.as_str(), rest.as_str()), ("Builder", "Collect all Gold Treasures"));
        assert_eq!(format!("{}{}{}", name.head, "Maker", name.tail), "Maker\tCollect all Gold Treasures");
        assert_eq!(format!("{}{}{}", description.head, "Get them", description.tail), "Builder\tGet them");
    }

    #[test]
    fn a_treasure_set_line_edits_one_cell_in_place() {
        let line = "Energy Drink|Activated!|Worker Cat Efficiency increased!|";
        let (field, value) = Sheet::TreasureEffects("Treasure3_0.csv").field(0, line).expect("the effect cell");

        assert_eq!(value, "Worker Cat Efficiency increased!");
        assert_eq!(format!("{}{}{}", field.head, "Faster workers!", field.tail), "Energy Drink|Activated!|Faster workers!|");

        let (field, value) = Sheet::TreasureSets("Treasure3_0.csv").field(0, "九州 ,戦闘中に,発動中！,").expect("a comma line");
        assert_eq!(value, "九州 ");
        assert_eq!(field.tail, ",戦闘中に,発動中！,");
    }

    #[test]
    fn a_weight_line_splits_at_its_comma() {
        let (held, value) = Sheet::Weights.field(3, "3,30").expect("a tier and a weight");

        assert_eq!((held.key.as_str(), value.as_str()), ("3", "30"));
        assert_eq!(format!("{}{}{}", held.head, 45, held.tail), "3,45");
    }

    #[test]
    fn text_keeps_everything_after_its_first_tab() {
        let (held, value) = Sheet::Localizable.field(0, "help_url\thttps://example.com // not a comment").expect("a field");

        assert_eq!(value, "https://example.com // not a comment");
        assert_eq!(format!("{}{}{}", held.head, value, held.tail), "help_url\thttps://example.com // not a comment");
    }
}
