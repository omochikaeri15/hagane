use super::RoguelikeMaster;

pub fn roguelike_bonus_is_permanent(master: &RoguelikeMaster, event_id: i32, kind: i32, id: i32) -> bool {
    let permanent = master.permanent.clone();
    let Some(maps) = permanent.get(&event_id) else {
        return false;
    };

    for levels in maps.values() {
        for groups in levels.values() {
            for group_id in groups {
                let Some(group) = master.lottery_groups.get(group_id) else {
                    return false;
                };
                let ids = group.target_ids.clone();

                if group.target != kind {
                    continue;
                }

                if ids.contains(&id) {
                    return true;
                }
            }
        }
    }

    false
}
