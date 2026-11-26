use std::collections::HashMap;
use std::fmt;

use crate::common::file;

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ParamError {
    EmptyFile,
}

impl fmt::Display for ParamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParamError::EmptyFile => write!(f, "The provided file bytes contained no valid param data."),
        }
    }
}

impl std::error::Error for ParamError {}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Param {
    pub unknown_1: Option<i32>,
    pub unknown_2: Option<i32>,
    pub unknown_3: Option<i32>,
    pub unknown_4: Option<i32>,
    pub unknown_5: Option<i32>,
    pub unknown_6: Option<i32>,
    pub unknown_7: Option<i32>,
    pub unknown_8: Option<i32>,
    pub unknown_9: Option<i32>,
    pub mini_wave_interval: Option<i32>,
    pub mini_wave_first_frame: Option<i32>,
    pub mini_wave_last_frame: Option<i32>,
    pub unknown_13: Option<i32>,
    pub unknown_14: Option<i32>,
    pub unknown_15: Option<i32>,
    pub unknown_16: Option<i32>,
    pub unknown_17: Option<i32>,
    pub hold_cat_details_time: Option<i32>,
    pub time_until_death_surge: Option<i32>,
    pub behemoth_slayer_attack_multiplier: Option<i32>,
    pub behemoth_slayer_defense_multiplier: Option<i32>,
    pub unknown_22: Option<i32>,
    pub logo_superfeline_roll_timer: Option<i32>,
    pub logo_superfeline_chance_success: Option<i32>,
    pub logo_superfeline_failure_increase: Option<i32>,
    pub unknown_26: Option<i32>,
    pub unknown_27: Option<i32>,
    pub unknown_28: Option<i32>,
    pub unknown_29: Option<i32>,
    pub unknown_30: Option<i32>,
    pub unknown_31: Option<i32>,
    pub unknown_32: Option<i32>,
    pub unknown_33: Option<i32>,
    pub volume_setting_percent_small: Option<i32>,
    pub volume_setting_percent_medium: Option<i32>,
    pub volume_setting_percent_large: Option<i32>,
    pub spirit_position_offset: Option<i32>,
    pub sage_type_resist_weaken: Option<i32>,
    pub sage_type_resist_freeze: Option<i32>,
    pub sage_type_resist_slow: Option<i32>,
    pub sage_type_resist_curse: Option<i32>,
    pub sage_type_resist_knockback: Option<i32>,
    pub sage_slayer_resist_weaken: Option<i32>,
    pub sage_slayer_resist_freeze: Option<i32>,
    pub sage_slayer_resist_slow: Option<i32>,
    pub sage_slayer_resist_curse: Option<i32>,
    pub sage_slayer_resist_other: Option<i32>,
    pub sage_slayer_resist_warp: Option<i32>,
    pub sage_slayer_attack_multiplier: Option<i32>,
    pub sage_slayer_defense_multiplier: Option<i32>,
    pub metal_killer_hitpoint_behavior: Option<i32>,
    pub unknown_52: Option<i32>,
    pub unknown_53: Option<i32>,
    pub story_sequence_hold_speed: Option<i32>,
    pub explosion_start_time: Option<i32>,
    pub explosion_hitbox_duration: Option<i32>,
    pub explosion_delay_before_deletion: Option<i32>,
    pub explosion_delay_until_spike: Option<i32>,
    pub explosion_big_width: Option<i32>,
    pub explosion_medium_width: Option<i32>,
    pub explosion_small_width: Option<i32>,
    pub explosion_big_damage_multiplier: Option<i32>,
    pub explosion_medium_damage_multiplier: Option<i32>,
    pub explosion_small_damage_multiplier: Option<i32>,
    pub unknown_65: Option<i32>,
    pub unknown_66: Option<i32>,
    pub unknown_67: Option<i32>,
    pub unknown_68: Option<i32>,
    pub unknown_69: Option<i32>,
    pub unknown_70: Option<i32>,
    pub unknown_71: Option<i32>,
    pub unknown_72: Option<i32>,
    pub unknown_73: Option<i32>,
    pub unknown_74: Option<i32>,
    pub unknown_75: Option<i32>,
    pub unknown_76: Option<i32>,
    pub unknown_77: Option<i32>,
    pub powerup_kills: Option<i32>,
    pub comeback_treasure_festival_days: Option<i32>,
    pub unknown_80: Option<i32>,
    pub unknown_81: Option<i32>,
    pub tutorial_money: Option<i32>,
    pub tutorial_duration_0: Option<i32>,
    pub tutorial_duration_1: Option<i32>,
    pub tutorial_duration_2: Option<i32>,
    pub tutorial_duration_3: Option<i32>,
    pub vibration_power_ios: Option<i32>,
    pub vibration_power_android: Option<i32>,
    pub vibration_time_ios: Option<i32>,
    pub vibration_time_android: Option<i32>,
    pub vibration_sharpness_ios: Option<i32>,
    pub registered_plus_levels: Option<i32>,
    pub registered_point_rate_0: Option<i32>,
    pub registered_point_rate_1: Option<i32>,
    pub registered_point_rate_2: Option<i32>,
    pub registered_point_rate_3: Option<i32>,
    pub registered_point_rate_4: Option<i32>,
    pub registered_point_rate_5: Option<i32>,
    pub rest: Vec<Option<i32>>,
}

impl Param {
    pub fn parse<B: AsRef<[u8]>>(bytes: B) -> Result<Self, ParamError> {
        parse_inner(bytes.as_ref())
    }
}

const NAMED_ROWS: usize = 98;
const SAGE_RESIST: i32 = 50;
const SAGE_SLAYER_WARP_RESIST: i32 = 70;
const SLAYER_ATTACK: i32 = 1600;
const SLAYER_DEFENSE: i32 = 700;
const MINI_WAVE_INTERVAL: i32 = 4;
const MINI_WAVE_FIRST_FRAME: i32 = 3;
const MINI_WAVE_LAST_FRAME: i32 = 7;
const EXPLOSION_START: i32 = 15;
const EXPLOSION_WINDOW: i32 = 20;
const EXPLOSION_LINGER: i32 = 10;
const EXPLOSION_SPREAD: i32 = 15;
const EXPLOSION_BIG_WIDTH: i32 = 800;
const EXPLOSION_MEDIUM_WIDTH: i32 = 500;
const EXPLOSION_SMALL_WIDTH: i32 = 400;
const POWERUP_KILLS: i32 = 15;
const REGISTERED_PLUS: i32 = 5;

macro_rules! settled {
    ($($field:ident: $fallback:expr),* $(,)?) => {
        impl Param {
            $(pub fn $field(&self) -> i32 { self.$field.unwrap_or($fallback) })*
        }
    };
}

settled! {
    behemoth_slayer_attack_multiplier: SLAYER_ATTACK,
    behemoth_slayer_defense_multiplier: SLAYER_DEFENSE,
    sage_type_resist_weaken: SAGE_RESIST,
    sage_type_resist_freeze: SAGE_RESIST,
    sage_type_resist_slow: SAGE_RESIST,
    sage_type_resist_curse: SAGE_RESIST,
    sage_type_resist_knockback: SAGE_RESIST,
    sage_slayer_resist_weaken: SAGE_RESIST,
    sage_slayer_resist_freeze: SAGE_RESIST,
    sage_slayer_resist_slow: SAGE_RESIST,
    sage_slayer_resist_curse: SAGE_RESIST,
    sage_slayer_resist_other: SAGE_RESIST,
    sage_slayer_resist_warp: SAGE_SLAYER_WARP_RESIST,
    sage_slayer_attack_multiplier: SLAYER_ATTACK,
    sage_slayer_defense_multiplier: SLAYER_DEFENSE,
    mini_wave_interval: MINI_WAVE_INTERVAL,
    mini_wave_first_frame: MINI_WAVE_FIRST_FRAME,
    mini_wave_last_frame: MINI_WAVE_LAST_FRAME,
    explosion_start_time: EXPLOSION_START,
    explosion_hitbox_duration: EXPLOSION_WINDOW,
    explosion_delay_before_deletion: EXPLOSION_LINGER,
    explosion_delay_until_spike: EXPLOSION_SPREAD,
    explosion_big_width: EXPLOSION_BIG_WIDTH,
    explosion_medium_width: EXPLOSION_MEDIUM_WIDTH,
    explosion_small_width: EXPLOSION_SMALL_WIDTH,
    powerup_kills: POWERUP_KILLS,
    registered_plus_levels: REGISTERED_PLUS,
}

fn number(text: &str) -> Option<i32> {
    let text = text.trim_start();
    let unsigned = text.strip_prefix(['+', '-']).unwrap_or(text);
    let digits = unsigned.bytes().take_while(u8::is_ascii_digit).count();

    if digits == 0 {
        return None;
    }

    text[..text.len() - unsigned.len() + digits].parse().ok()
}

fn parse_inner(bytes: &[u8]) -> Result<Param, ParamError> {
    let file_content = file::scrub(bytes);

    let mut map = HashMap::with_capacity(150);
    let mut raw_ordered_values = Vec::with_capacity(150);

    for line in file_content.lines() {
        let clean_line = line.split("//").next().unwrap_or("").trim();

        if clean_line.is_empty() {
            continue;
        }

        let mut parts = clean_line.split_whitespace();
        let Some(key) = parts.next() else { continue; };
        let Some(val_str) = parts.last() else { continue; };

        let val = number(val_str);

        map.insert(key, val);
        raw_ordered_values.push(val);
    }

    if map.is_empty() {
        return Err(ParamError::EmptyFile);
    }

    let get_val = |target_key: &str| -> Option<i32> {
        map.get(target_key).copied().flatten()
    };

    let mut rest = Vec::new();
    if raw_ordered_values.len() > NAMED_ROWS {
        rest.extend_from_slice(&raw_ordered_values[NAMED_ROWS..]);
    }

    Ok(Param {
        unknown_1: get_val("slot_font_size"),
        unknown_2: get_val("slot_bg_y"),
        unknown_3: get_val("slot_name_margin_top"),
        unknown_4: get_val("slot_name_margin_left"),
        unknown_5: get_val("slot_page_y_offset"),
        unknown_6: get_val("slot_font_size2"),
        unknown_7: get_val("slot_name_margin_top2"),
        unknown_8: get_val("slot_name_margin_left2"),
        unknown_9: get_val("battle_wave_s_st"),
        mini_wave_interval: get_val("battle_wave_s_inter"),
        mini_wave_first_frame: get_val("battle_wave_s_frame1"),
        mini_wave_last_frame: get_val("battle_wave_s_frame2"),
        unknown_13: get_val("battle_slot_2lines_line"),
        unknown_14: get_val("battle_slot_2lines_y"),
        unknown_15: get_val("battle_zoom_y"),
        unknown_16: get_val("battle_zoom_slot_y"),
        unknown_17: get_val("battle_zoom_castle_y"),
        hold_cat_details_time: get_val("battle_slot_desc_wait"),
        time_until_death_surge: get_val("battle_death_volcano_time"),
        behemoth_slayer_attack_multiplier: get_val("battle_super_beast_hunter"),
        behemoth_slayer_defense_multiplier: get_val("battle_super_beast_hunter_df"),
        unknown_22: get_val("boost_speed"),
        logo_superfeline_roll_timer: get_val("logo_gimmick_sec"),
        logo_superfeline_chance_success: get_val("logo_gimmick_probability"),
        logo_superfeline_failure_increase: get_val("logo_gimmick_increase"),
        unknown_26: get_val("battle_demon_summon_frame"),
        unknown_27: get_val("battle_demon_summon_se_frame"),
        unknown_28: get_val("battle_castle_x0"),
        unknown_29: get_val("battle_castle_y0"),
        unknown_30: get_val("battle_castle_x1"),
        unknown_31: get_val("battle_castle_y1"),
        unknown_32: get_val("battle_castle_x2"),
        unknown_33: get_val("battle_castle_y2"),
        volume_setting_percent_small: get_val("volume_percentage_s"),
        volume_setting_percent_medium: get_val("volume_percentage_m"),
        volume_setting_percent_large: get_val("volume_percentage_l"),
        spirit_position_offset: get_val("summon_position"),
        sage_type_resist_weaken: get_val("battle_super_sage_weaken"),
        sage_type_resist_freeze: get_val("battle_super_sage_freeze"),
        sage_type_resist_slow: get_val("battle_super_sage_slow"),
        sage_type_resist_curse: get_val("battle_super_sage_curse"),
        sage_type_resist_knockback: get_val("battle_super_sage_knockback"),
        sage_slayer_resist_weaken: get_val("battle_super_sage_hunter_weaken"),
        sage_slayer_resist_freeze: get_val("battle_super_sage_hunter_freeze"),
        sage_slayer_resist_slow: get_val("battle_super_sage_hunter_slow"),
        sage_slayer_resist_curse: get_val("battle_super_sage_hunter_curse"),
        sage_slayer_resist_other: get_val("battle_super_sage_hunter_knockback"),
        sage_slayer_resist_warp: get_val("battle_super_sage_hunter_warp"),
        sage_slayer_attack_multiplier: get_val("battle_super_sage_hunter_damage1"),
        sage_slayer_defense_multiplier: get_val("battle_super_sage_hunter_damage2"),
        metal_killer_hitpoint_behavior: get_val("battle_metal_killer_hp_type"),
        unknown_52: get_val("catbase_banner1"),
        unknown_53: get_val("catbase_banner2"),
        story_sequence_hold_speed: get_val("story_fast_forward_speed"),
        explosion_start_time: get_val("battle_explosion_frame1"),
        explosion_hitbox_duration: get_val("battle_explosion_frame2"),
        explosion_delay_before_deletion: get_val("battle_explosion_frame3"),
        explosion_delay_until_spike: get_val("battle_explosion_frame4"),
        explosion_big_width: get_val("battle_explosion_width1"),
        explosion_medium_width: get_val("battle_explosion_width2"),
        explosion_small_width: get_val("battle_explosion_width3"),
        explosion_big_damage_multiplier: get_val("battle_explosion_damage1"),
        explosion_medium_damage_multiplier: get_val("battle_explosion_damage2"),
        explosion_small_damage_multiplier: get_val("battle_explosion_damage3"),
        unknown_65: get_val("battle_af_death_volcano_distance"),
        unknown_66: get_val("battle_af_death_volcano_width"),
        unknown_67: get_val("battle_af_condition_production_0"),
        unknown_68: get_val("battle_af_condition_production_1"),
        unknown_69: get_val("battle_af_condition_production_2"),
        unknown_70: get_val("battle_af_condition_production_3"),
        unknown_71: get_val("battle_af_condition_production_4"),
        unknown_72: get_val("battle_af_condition_production_5"),
        unknown_73: get_val("battle_af_condition_production_type"),
        unknown_74: get_val("autoset_level_param"),
        unknown_75: get_val("autoset_must_param"),
        unknown_76: get_val("autoset_cut_score_rate"),
        unknown_77: get_val("autoset_non_target_offset"),
        powerup_kills: get_val("battle_powerup_kill_enemy"),
        comeback_treasure_festival_days: get_val("comeback_treasure_day"),
        unknown_80: get_val("update_version_notification_interval"),
        unknown_81: get_val("battle_sentai_recast"),
        tutorial_money: get_val("tutorial_money"),
        tutorial_duration_0: get_val("tutorial_duration0"),
        tutorial_duration_1: get_val("tutorial_duration1"),
        tutorial_duration_2: get_val("tutorial_duration2"),
        tutorial_duration_3: get_val("tutorial_duration3"),
        vibration_power_ios: get_val("option_vibration_power_ios"),
        vibration_power_android: get_val("option_vibration_power_android"),
        vibration_time_ios: get_val("option_vibration_time_ios"),
        vibration_time_android: get_val("option_vibration_time_android"),
        vibration_sharpness_ios: get_val("option_vibration_sharpness_ios"),
        registered_plus_levels: get_val("roguelike_charabonus_plusvalue"),
        registered_point_rate_0: get_val("roguelike_charabonus_pointvalue0"),
        registered_point_rate_1: get_val("roguelike_charabonus_pointvalue1"),
        registered_point_rate_2: get_val("roguelike_charabonus_pointvalue2"),
        registered_point_rate_3: get_val("roguelike_charabonus_pointvalue3"),
        registered_point_rate_4: get_val("roguelike_charabonus_pointvalue4"),
        registered_point_rate_5: get_val("roguelike_charabonus_pointvalue5"),
        rest,
    })
}

#[cfg(test)]
mod tests {
    use super::Param;

    // The game falls back to its own number for a key the file leaves out, and reads a value
    // the way stoi does, so trailing junk after the digits is ignored.
    #[test]
    fn a_missing_key_takes_the_games_own_number() {
        let param = Param::parse("battle_super_sage_freeze\t70\nbattle_explosion_frame1\t11abc\nbattle_wave_s_inter\tx\n").unwrap();

        assert_eq!((param.sage_type_resist_freeze, param.sage_type_resist_freeze()), (Some(70), 70));
        assert_eq!((param.sage_type_resist_slow, param.sage_type_resist_slow()), (None, 50));
        assert_eq!(param.explosion_start_time(), 11);
        assert_eq!((param.mini_wave_interval, param.mini_wave_interval()), (None, 4), "a value that is no number is left as none");
        assert_eq!(Param::default().sage_slayer_resist_warp(), 70);
    }
}
