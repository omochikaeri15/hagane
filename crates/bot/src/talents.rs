use std::collections::HashMap;

use foundry::domains::cat::scanner::CatEntry;

use super::fuzzy::Query;
use super::grammar::Talent;
use super::library::Library;
use super::prose::Trouble;

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
                let query = Query::new(name);
                let index = data
                    .groups
                    .iter()
                    .enumerate()
                    .filter_map(|(index, group)| Some((index, query.score(&library.talent_name(group.ability_id))?)))
                    .max_by_key(|(_, score)| *score)
                    .map(|(index, _)| index)
                    .ok_or_else(|| Trouble::NoTalentLike(cat.display_name(0), name.clone()))?;

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

            format!("{}{}: {}/{}", library.talent_name(group.ability_id), ultra, level, group.max_level.max(1))
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

    // Every talent at level 0 is the same as having none, so the button reads plain "Talents".
    #[test]
    fn all_zero_levels_mean_no_talents() {
        assert_eq!(encoded(&HashMap::new(), 4), None);
        assert_eq!(encoded(&HashMap::from([(0, 0), (2, 0)]), 4), None);
    }
}
