use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::common::file::scrub;

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RoguelikeMapSettingsError {
    InvalidJson,
}

impl fmt::Display for RoguelikeMapSettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "The provided byte slice could not be parsed as valid JSON.")
    }
}

impl std::error::Error for RoguelikeMapSettingsError {}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoguelikeMapEntry {
    pub map_ids: Vec<i32>,
    pub levels: Vec<i32>,
    pub preset_file_name: String,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoguelikeEvent {
    pub data: Vec<RoguelikeMapEntry>,
    pub reset_map_id: Option<i32>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoguelikeMapSettings {
    pub events: HashMap<i32, RoguelikeEvent>,
}

impl RoguelikeMapSettings {
    pub fn parse<B: AsRef<[u8]>>(bytes: B) -> Result<Self, RoguelikeMapSettingsError> {
        let clean = scrub(bytes.as_ref());

        parse_inner(&clean)
    }

    pub fn entry(&self, map_id: i32) -> Option<&RoguelikeMapEntry> {
        self.events.values().flat_map(|event| event.data.iter()).find(|entry| entry.map_ids.contains(&map_id))
    }

    pub fn event_of(&self, map_id: i32) -> Option<i32> {
        self.events.iter().filter(|(_, event)| event.data.iter().any(|entry| entry.map_ids.contains(&map_id))).map(|(id, _)| *id).min()
    }
}

fn numbers(value: Option<&serde_json::Value>) -> Vec<i32> {
    value
        .and_then(serde_json::Value::as_array)
        .map(|cells| cells.iter().filter_map(|cell| cell.as_i64().and_then(|number| i32::try_from(number).ok())).collect())
        .unwrap_or_default()
}

fn parse_inner(json: &str) -> Result<RoguelikeMapSettings, RoguelikeMapSettingsError> {
    let root: serde_json::Value = serde_json::from_str(json).map_err(|_| RoguelikeMapSettingsError::InvalidJson)?;
    let mut events = HashMap::new();

    let Some(listed) = root.get("EventID").and_then(serde_json::Value::as_object) else {
        return Ok(RoguelikeMapSettings { events });
    };

    for (key, value) in listed {
        let Ok(id) = key.parse::<i32>() else {
            continue;
        };
        let data = value
            .get("Data")
            .and_then(serde_json::Value::as_array)
            .map(|rows| {
                rows.iter()
                    .map(|row| RoguelikeMapEntry {
                        map_ids: numbers(row.get("MapID")),
                        levels: numbers(row.get("Level")),
                        preset_file_name: row.get("PresetFileName").and_then(serde_json::Value::as_str).unwrap_or("").to_owned(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let reset_map_id = value.get("ResetMapID").and_then(serde_json::Value::as_i64).and_then(|number| i32::try_from(number).ok());

        events.insert(id, RoguelikeEvent { data, reset_map_id });
    }

    Ok(RoguelikeMapSettings { events })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{"EventID":{"0":{"Data":[{"MapID":[5,6],"Level":[0,1],"PresetFileName":"A.preset"},{"MapID":[7],"Level":[0]}],"ResetMapID":5}}}"#;

    #[test]
    fn every_listed_map_finds_its_entry() {
        let settings = RoguelikeMapSettings::parse(SAMPLE).unwrap();
        let event = &settings.events[&0];

        assert_eq!(event.reset_map_id, Some(5));
        assert_eq!(settings.entry(6).map(|entry| entry.preset_file_name.as_str()), Some("A.preset"));
        assert_eq!(settings.entry(7).map(|entry| entry.levels.as_slice()), Some(&[0][..]));
        assert!(settings.entry(8).is_none());
        assert_eq!(settings.event_of(7), Some(0));
        assert_eq!(settings.event_of(8), None);
    }

    // A file with no event table is an empty table, not an error.
    #[test]
    fn a_file_without_events_is_empty() {
        assert!(RoguelikeMapSettings::parse("{}").unwrap().events.is_empty());
        assert_eq!(RoguelikeMapSettings::parse("{"), Err(RoguelikeMapSettingsError::InvalidJson));
    }
}
