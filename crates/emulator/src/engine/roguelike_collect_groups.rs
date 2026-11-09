use std::collections::BTreeMap;

pub fn roguelike_collect_groups(
    groups: &mut Vec<i32>,
    maps: &BTreeMap<i32, BTreeMap<i32, Vec<i32>>>,
    map_id: i32,
    crown: i32,
) {
    let Some(crowns) = maps.get(&map_id) else {
        return;
    };
    let crowns = crowns.clone();

    if let Some(listed) = crowns.get(&crown) {
        groups.extend_from_slice(listed);
    }

    if let Some(listed) = crowns.get(&-1) {
        groups.extend_from_slice(listed);
    }
}
