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
pub enum Sheet {
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

pub(super) const SHEETS: &[Sheet] = &[Sheet::Param, Sheet::Localizable];

impl Sheet {
    pub fn title(self) -> &'static str {
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

    pub fn base(self) -> &'static str {
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

    pub fn textual(self) -> bool {
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

    pub fn pitch(self) -> f32 {
        match self.textual() {
            true => STACKED_ROW,
            false => LINE_ROW,
        }
    }

    pub fn resolved(self, key: &str) -> String {
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

    pub fn limits(self, key: &str) -> Option<(i32, i32)> {
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

    pub fn field(self, line: usize, text: &str) -> Option<(Field, String)> {
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

    pub fn fields(self, lines: &[String]) -> (Vec<Field>, Vec<String>) {
        lines.iter().enumerate().filter_map(|(at, text)| self.field(at, text)).unzip()
    }
}

const LINE_ROW: f32 = 30.0;

const STACKED_ROW: f32 = 48.0;

pub(super) const TAB: char = '\t';

const COMMA: char = ',';

const WEIGHT_LABEL: &str = "Rarity Tier";

const PERCENT_FLOOR: i32 = 0;

const PIPE: char = '|';

const MISSION_LABEL: &str = "Mission";

const EFFECT_LABEL: &str = "Effect";

const MEDAL_LABEL: &str = "Medal";

const TRAIT_LABEL: &str = "Trait";

pub(super) const MESSAGE_LABEL: &str = "Message";

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

pub(super) const MIRACLE_LABEL: &str = "Miracle";

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

pub struct Field {
    pub line: usize,
    pub key: String,
    pub head: String,
    pub tail: String,
}

pub(super) fn whole(line: usize, text: &str) -> Option<(Field, String)> {
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

pub(super) fn numbered(line: usize, text: &str) -> Option<(Field, String)> {
    let content = text.split(COMMENT).next().unwrap_or_default();
    let key = content.split_whitespace().next()?;
    let value = content.split_whitespace().last().filter(|value| *value != key)?;
    let at = content.rfind(value)?;

    let head = text[..at].to_owned();
    let tail = text[at + value.len()..].to_owned();

    Some((Field { line, key: key.to_owned(), head, tail }, value.to_owned()))
}

pub(super) fn prettify(key: &str) -> String {
    key.split('_')
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();

            chars.next().map_or_else(String::new, |first| first.to_uppercase().chain(chars).collect())
        })
        .collect::<Vec<String>>()
        .join(" ")
}

const COMMENT: &str = "//";
