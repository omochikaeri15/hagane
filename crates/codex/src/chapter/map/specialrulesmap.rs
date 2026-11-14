use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::common::file::scrub;

const WAR_FUNDS_CONTENTS: u8 = 1;
const EXPENSES_CONTENTS: u8 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SpecialRulesMapError {
    InvalidJson,
}

impl fmt::Display for SpecialRulesMapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "The provided byte slice could not be parsed as valid JSON.")
    }
}

impl std::error::Error for SpecialRulesMapError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleType {
    TrustFund(Vec<u32>),
    CooldownEquality(Vec<u32>),
    LineupLimit(Vec<u32>),
    RarityLimit(Vec<u32>),
    CheapLabor(Vec<u32>),
    CatCost(Vec<u32>),
    CatProduction(Vec<u32>),
    TotalDeployLimit(Vec<u32>),
    MoreThanOne(Vec<u32>),
    MegaCatCannon(Vec<u32>),
    UniformMotion(Vec<u32>),
    CompoundingCost(Vec<u32>),
    Unknown(u8, Vec<u32>),
}

impl RuleType {
    pub fn from_parts(id: u8, params: Vec<u32>) -> Self {
        match id {
            0 => Self::TrustFund(params),
            1 => Self::CooldownEquality(params),
            2 => Self::LineupLimit(params),
            3 => Self::RarityLimit(params),
            4 => Self::CheapLabor(params),
            5 => Self::CatCost(params),
            6 => Self::CatProduction(params),
            7 => Self::TotalDeployLimit(params),
            8 => Self::MoreThanOne(params),
            9 => Self::MegaCatCannon(params),
            10 => Self::UniformMotion(params),
            11 => Self::CompoundingCost(params),
            _ => Self::Unknown(id, params),
        }
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpecialRulesMapEntry {
    pub contents_type: u8,
    pub rules: Vec<RuleType>,
    pub name_label: String,
    pub explanation_label: String,
}

impl SpecialRulesMapEntry {
    fn has_trust_fund(&self) -> bool {
        self.rules.iter().any(|rule| matches!(rule, RuleType::TrustFund(_)))
    }

    pub fn draws_war_funds(&self) -> bool {
        self.contents_type == WAR_FUNDS_CONTENTS && self.has_trust_fund()
    }

    pub fn draws_expenses(&self) -> bool {
        self.contents_type == EXPENSES_CONTENTS && self.has_trust_fund()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpecialRulesMap {
    pub entries: HashMap<u32, SpecialRulesMapEntry>,
}

impl SpecialRulesMap {
    pub fn parse<B: AsRef<[u8]>>(bytes: B) -> Result<Self, SpecialRulesMapError> {
        let clean = scrub(bytes.as_ref());
        parse_inner(&clean)
    }
}

fn parse_inner(json_str: &str) -> Result<SpecialRulesMap, SpecialRulesMapError> {
    let val: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|_| SpecialRulesMapError::InvalidJson)?;

    let mut entries = HashMap::new();

    let Some(map_obj) = val.get("MapID").and_then(|v| v.as_object()) else {
        return Ok(SpecialRulesMap { entries });
    };

    for (id_str, data) in map_obj {
        let Ok(id) = id_str.parse::<u32>() else { continue; };

        let contents_type = data.get("ContentsType").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
        let name_label = data.get("RuleNameLabel").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let explanation_label = data.get("RuleExplanationLabel").and_then(|v| v.as_str()).unwrap_or("").to_string();

        let mut rules = Vec::new();
        if let Some(rule_obj) = data.get("RuleType").and_then(|v| v.as_object()) {
            for (r_id_str, r_data) in rule_obj {
                let Ok(r_id) = r_id_str.parse::<u8>() else { continue; };

                let params: Vec<u32> = r_data
                    .get("Parameters")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|x| x.as_u64().map(|n| n as u32)).collect())
                    .unwrap_or_default();

                rules.push(RuleType::from_parts(r_id, params));
            }
        }

        entries.insert(id, SpecialRulesMapEntry {
            contents_type,
            rules,
            name_label,
            explanation_label,
        });
    }

    Ok(SpecialRulesMap { entries })
}

#[cfg(test)]
mod tests {
    use super::*;

    // The contents type picks which saved balance a trust fund draws on, and a map without one draws on neither.
    #[test]
    fn a_saved_balance_needs_its_contents_type_and_a_trust_fund() {
        let entry = |contents_type, rules| SpecialRulesMapEntry { contents_type, rules, ..SpecialRulesMapEntry::default() };
        let fund = || vec![RuleType::TrustFund(vec![4500])];

        assert!(entry(1, fund()).draws_war_funds());
        assert!(!entry(0, fund()).draws_war_funds());
        assert!(!entry(2, fund()).draws_war_funds());
        assert!(!entry(1, vec![RuleType::CatProduction(vec![50])]).draws_war_funds());

        assert!(entry(2, fund()).draws_expenses());
        assert!(!entry(1, fund()).draws_expenses());
        assert!(!entry(3, fund()).draws_expenses());
        assert!(!entry(2, vec![RuleType::CatProduction(vec![50])]).draws_expenses());
    }
}
