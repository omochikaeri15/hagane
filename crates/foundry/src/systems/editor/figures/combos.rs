use std::path::{Path, PathBuf};
use crate::domains::cat::scanner::CatEntry;
use crate::systems::editor::figures::*;
use super::resolved::Rule;
use super::schema::{combo_form, combo_unit, Schema, COMBO_SLOTS};
use super::{Address, Draft};

pub(super) const SERIES: &str = "series";

const COMBO_ID: &str = "combo_id";

pub const EFFECT: &str = "effect_type";

pub const POWER: &str = "effect_level";

pub const ABSENT: i32 = -1;

pub const THUMB: u32 = 44;

pub const EFFECT_SPAN: u16 = 2;

pub const POWER_SPAN: u16 = 1;

pub const REMOVE_LABEL: &str = "Remove Combo";

pub const NOTHING_TO_REMOVE: &str = "This combo is not in the file yet";

const EMPTY_SLOT: &str = "+";

pub const CLEAR_SLOT: &str = "None";

pub const HUNT_PLACEHOLDER: &str = "Search unit ID or name...";

pub const NO_EFFECT: &str = "None";

pub const ANCHOR_HINT: &str = "The unit this combo was opened from always stays in it";

pub fn aimed(draft: &Draft) -> i32 {
    if draft.absent() {
        return ABSENT;
    }

    i32::try_from(draft.row()).unwrap_or(ABSENT)
}

pub fn addressed(raw: i32) -> Address {
    usize::try_from(raw).map_or(Address::Appended, Address::Line)
}

pub fn named(draft: &Draft, names: &[Option<String>]) -> String {
    if let Some(name) = spoken(names, draft.row()) {
        return name;
    }

    let read = |field: &str| draft.at(field).and_then(|index| draft.reads_at(index)).unwrap_or(RETIRED);

    keyed(read(SERIES), read(COMBO_ID))
}

pub fn vacant_label(unit: i32, form: i32) -> String {
    if unit < 0 {
        return EMPTY_SLOT.to_owned();
    }

    format!("{unit:03}-{}", form.max(0) + 1)
}

pub fn icon_of(cats: &[CatEntry], unit: i32, form: i32) -> Option<&Path> {
    let id = u32::try_from(unit).ok()?;
    let form = usize::try_from(form).ok()?;
    let cat = cats.iter().find(|cat| cat.id == id)?;

    cat.deploy_icon_paths.get(form)?.as_deref()
}

#[derive(Clone, PartialEq, Eq)]
pub struct Entry {
    pub id: u32,
    pub form: usize,
    pub path: PathBuf,
    pub hunted: String,
}

pub fn seed(fields: &mut [String], schema: &Schema, anchor: Option<(i32, i32)>) {
    let Some((unit, form)) = anchor else {
        return;
    };

    for (field, value) in [(combo_unit(0), unit), (combo_form(0), form)] {
        if let Some(slot) = schema.index_of(&field).and_then(|index| fields.get_mut(index)) {
            *slot = value.to_string();
        }
    }
}

pub fn packed(schema: &Schema, cells: &[i32]) -> Vec<(usize, i32)> {
    let mut held: Vec<(i32, i32)> = Vec::with_capacity(COMBO_SLOTS);
    let mut places: Vec<(usize, usize)> = Vec::with_capacity(COMBO_SLOTS);

    for slot in 0..COMBO_SLOTS {
        let (Some(unit), Some(form)) = (schema.index_of(&combo_unit(slot)), schema.index_of(&combo_form(slot)))
        else {
            return Vec::new();
        };

        places.push((unit, form));

        let pair = (cells.get(unit).copied().unwrap_or(ABSENT), cells.get(form).copied().unwrap_or(ABSENT));

        if pair.0 >= 0 && pair.1 >= 0 {
            held.push(pair);
        }
    }

    places
        .into_iter()
        .enumerate()
        .flat_map(|(slot, (unit, form))| {
            let pair = held.get(slot).copied().unwrap_or((ABSENT, ABSENT));

            [(unit, pair.0), (form, pair.1)]
        })
        .filter(|(index, raw)| cells.get(*index) != Some(raw))
        .collect()
}

pub(super) fn seed_key(fields: &mut [String], schema: &Schema, lines: &[String], delimiter: char) {
    let held = schema
        .index_of(SERIES)
        .and_then(|index| fields.get(index))
        .and_then(|raw| raw.trim().parse().ok())
        .unwrap_or(RETIRED);

    let (series, combo_id) = next_key(lines, delimiter, held);

    for (field, value) in [(SERIES, series), (COMBO_ID, combo_id)] {
        if let Some(slot) = schema.index_of(field).and_then(|index| fields.get_mut(index)) {
            *slot = value.to_string();
        }
    }
}

pub fn next_key(lines: &[String], delimiter: char, current: i32) -> (i32, i32) {
    let keys: Vec<(i32, i32)> = lines.iter().filter_map(|line| key_of(line, delimiter)).collect();

    let series = match current > 0 {
        true => current,
        false => keys.iter().map(|(series, _)| *series).max().unwrap_or(1).max(1),
    };

    let next = keys
        .iter()
        .filter(|(held, _)| *held == series)
        .map(|(_, id)| *id)
        .max()
        .map_or(0, |highest| highest.saturating_add(1));

    (series, next)
}

fn key_of(line: &str, delimiter: char) -> Option<(i32, i32)> {
    let mut fields = line.split(delimiter);
    let combo_id = fields.next()?.trim().parse().ok()?;
    let series = fields.next()?.trim().parse().ok()?;

    Some((series, combo_id))
}

pub fn rule(field: &str) -> Option<Rule> {
    match field {
        COMBO_ID | SERIES => Some(Rule::Plain),

        "charagroup_id" | EFFECT | POWER | "slot_1_unit_id" | "slot_1_form" | "slot_2_unit_id"
        | "slot_2_form" | "slot_3_unit_id" | "slot_3_form" | "slot_4_unit_id" | "slot_4_form"
        | "slot_5_unit_id" | "slot_5_form" => Some(Rule::Floor(ABSENT)),

        "unknown_15" => Some(Rule::Opaque),

        _ => None,
    }
}
