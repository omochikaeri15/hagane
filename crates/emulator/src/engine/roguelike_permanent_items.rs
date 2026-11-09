use super::{RoguelikeMaster, roguelike_collect_groups, roguelike_resolve_groups};

pub fn roguelike_permanent_items(
    master: &RoguelikeMaster,
    event_id: i32,
    map_id: i32,
    crown: i32,
    lists: &mut Vec<Vec<(i32, i32)>>,
) -> bool {
    let permanent = master.permanent.clone();
    let Some(maps) = permanent.get(&event_id) else {
        return false;
    };
    let mut groups: Vec<i32> = Vec::new();

    roguelike_collect_groups(&mut groups, maps, map_id, crown);
    roguelike_collect_groups(&mut groups, maps, -1, crown);

    if groups.is_empty() {
        return true;
    }

    roguelike_resolve_groups(master, &groups, lists)
}
