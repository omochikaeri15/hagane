use std::cmp::Reverse;
use std::collections::HashMap;

use codex::cat::unit::TalentGroup;
use foundry::domains::cat::scanner::CatEntry;

use super::fuzzy::Query;
use super::grammar::Talent;
use super::headers;
use super::library::Library;
use super::prefs::ENGLISH;
use super::prose::Trouble;

fn literal(text: &str) -> String {
    text.chars().filter(|glyph| !glyph.is_whitespace() && !matches!(glyph, '-' | '_')).flat_map(char::to_lowercase).collect()
}

fn known(group: &TalentGroup, english: bool, held: impl FnOnce() -> String) -> Vec<String> {
    match headers::name(group) {
        Some(header) if english => vec![header.to_owned(), held()],
        Some(header) => vec![held(), header.to_owned()],
        None => vec![held()],
    }
}

fn names(library: &Library, group: &TalentGroup) -> Vec<String> {
    known(group, library.language == ENGLISH, || library.talent_name(group.ability_id))
}

fn named(groups: &[TalentGroup], typed: &str, names: impl Fn(&TalentGroup) -> Vec<String>) -> Option<usize> {
    let query = Query::new(typed);
    let wanted = literal(typed);

    groups
        .iter()
        .enumerate()
        .filter_map(|(index, group)| {
            let known = names(group);
            let score = known.iter().filter_map(|known| query.score(known)).max()?;
            let written = known.iter().any(|known| literal(known) == wanted);

            Some((index, (score, written, Reverse(index))))
        })
        .max_by_key(|(_, rank)| *rank)
        .map(|(index, _)| index)
}

pub(crate) fn levels(library: &Library, cat: &CatEntry, words: &[Talent]) -> Result<HashMap<u8, u8>, Trouble> {
    let Some(data) = cat.talent_data.as_ref() else {
        return Err(Trouble::NoTalents(cat.display_name(0)));
    };
    let ceiling = |index: usize| data.groups.get(index).map_or(0, |group| group.max_level.max(1));
    let clamp = |index: usize, level: u32| u8::try_from(level).unwrap_or(u8::MAX).min(ceiling(index));
    let mut levels: HashMap<u8, u8> = HashMap::new();
    let mut cursor = 0;

    for word in words {
        match word {
            Talent::All => levels.extend((0..data.groups.len()).map(|index| (index as u8, ceiling(index)))),
            Talent::Standard => levels.extend(data.groups.iter().enumerate().filter(|(_, group)| !group.is_ultra()).map(|(index, _)| (index as u8, ceiling(index)))),
            Talent::Ultra => levels.extend(data.groups.iter().enumerate().filter(|(_, group)| group.is_ultra()).map(|(index, _)| (index as u8, ceiling(index)))),
            Talent::None => {
                levels.clear();
                cursor = 0;
            }
            Talent::Level(level) => {
                if cursor >= data.groups.len() {
                    return Err(Trouble::FewTalents(cat.display_name(0), data.groups.len()));
                }

                levels.insert(cursor as u8, clamp(cursor, *level));
                cursor += 1;
            }
            Talent::Named(name, level) => {
                let index = named(&data.groups, name, |group| names(library, group)).ok_or_else(|| Trouble::NoTalentLike(cat.display_name(0), name.clone()))?;

                levels.insert(index as u8, level.map_or_else(|| ceiling(index), |level| clamp(index, level)));
            }
        }
    }

    Ok(levels)
}

pub(crate) fn encoded(levels: &HashMap<u8, u8>, count: usize) -> Option<String> {
    levels.values().any(|level| *level > 0).then(|| (0..count).map(|index| levels.get(&(index as u8)).copied().unwrap_or(0).to_string()).collect::<Vec<_>>().join("."))
}

pub(crate) fn decoded(text: &str) -> Option<HashMap<u8, u8>> {
    text.split('.').enumerate().map(|(index, level)| Some((u8::try_from(index).ok()?, level.parse::<u8>().ok()?))).collect()
}

pub(crate) fn listed(library: &Library, cat: &CatEntry, levels: Option<&HashMap<u8, u8>>) -> String {
    let Some(data) = cat.talent_data.as_ref() else {
        return String::new();
    };

    data.groups
        .iter()
        .enumerate()
        .map(|(index, group)| {
            let level = levels.and_then(|levels| levels.get(&(index as u8))).copied().unwrap_or(0);
            let ultra = if group.is_ultra() { " (Ultra)" } else { "" };

            format!("{}{}: {}/{}", names(library, group).first().map_or("", String::as_str), ultra, level, group.max_level.max(1))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_survive_a_round_trip() {
        let levels = HashMap::from([(0, 10), (2, 5)]);
        let text = encoded(&levels, 4).unwrap();

        assert_eq!(text, "10.0.5.0");
        assert_eq!(decoded(&text), Some(HashMap::from([(0, 10), (1, 0), (2, 5), (3, 0)])));
    }

    fn group(ability_id: u8, name_id: i16) -> TalentGroup {
        TalentGroup { ability_id, name_id, ..TalentGroup::default() }
    }

    fn held(group: &TalentGroup) -> String {
        match group.ability_id {
            3 => "Slow",
            10 => "Strengthen",
            31 => "Attack Buff",
            32 => "Health Buff",
            34 => "Floating",
            71 => "Health Buff +",
            73 => "Cat Combo",
            _ => "Other",
        }
        .to_owned()
    }

    // The talents page shows the game's own name pictures, so that name comes first, and ours still finds it.
    #[test]
    fn a_talent_goes_by_the_name_on_its_picture() {
        let groups = [group(10, -1), group(34, -1), group(32, -1), group(31, -1), group(71, -1), group(3, 300), group(73, -1)];
        let pick = |typed: &str| named(&groups, typed, |group| known(group, true, || held(group)));

        assert_eq!(pick("attack up"), Some(0), "Attack Up is the picture over Strengthen, not Attack Buff");
        assert_eq!(pick("target floating"), Some(1));
        assert_eq!(pick("floating"), Some(1), "a close spelling still finds it");
        assert_eq!(pick("hp buff"), Some(2));
        assert_eq!(pick("hp buff+"), Some(4));
        assert_eq!(pick("attack buff"), Some(3));
        assert_eq!(pick("slow relic"), Some(5));
        assert_eq!(pick("strengthen"), Some(0), "people look for Strengthen by that name, whatever its picture says");
        assert_eq!(pick("health buff"), Some(2));
        assert_eq!(pick("health buff +"), Some(4));
        assert_eq!(pick("warp blocker"), None);
        assert_eq!(pick("cat combo"), Some(6), "a talent with no name picture keeps our name");
    }

    // Other languages keep the game's own word for a talent beside the English picture name.
    #[test]
    fn another_language_keeps_its_own_word() {
        let floating = group(34, -1);

        assert_eq!(known(&floating, false, || "\u{6d6e}\u{3044}\u{3066}\u{308b}\u{6575}".to_owned()), ["\u{6d6e}\u{3044}\u{3066}\u{308b}\u{6575}", "Target Floating"]);
        assert_eq!(known(&floating, true, || "Floating".to_owned()), ["Target Floating", "Floating"]);
    }

    // Every talent at level 0 is the same as having none, so the button reads plain "Talents".
    #[test]
    fn all_zero_levels_mean_no_talents() {
        assert_eq!(encoded(&HashMap::new(), 4), None);
        assert_eq!(encoded(&HashMap::from([(0, 0), (2, 0)]), 4), None);
    }
}
