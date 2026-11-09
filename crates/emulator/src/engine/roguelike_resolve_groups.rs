use super::RoguelikeMaster;

pub fn roguelike_resolve_groups(
    master: &RoguelikeMaster,
    groups: &[i32],
    lists: &mut Vec<Vec<(i32, i32)>>,
) -> bool {
    for group_id in groups {
        let Some(group) = master.lottery_groups.get(group_id) else {
            return false;
        };

        match group.target {
            0 => {
                let mut items: Vec<(i32, i32)> = Vec::new();

                for id in &group.target_ids {
                    if !master.charas.contains_key(id) {
                        return false;
                    }

                    items.push((0, *id));
                }

                lists.push(items);
            }
            1 => {
                let mut items: Vec<(i32, i32)> = Vec::new();

                for id in &group.target_ids {
                    let Some(buff) = master.buffs.get(id) else {
                        return false;
                    };

                    if buff.limit <= 0 {
                        continue;
                    }

                    items.push((1, *id));
                }

                lists.push(items);
            }
            _ => {}
        }
    }

    true
}
