pub(crate) use foundry::systems::editor::figures::talents::values::*;

#[cfg(test)]
mod tests {
    use codex::combat::get_talent;

    use super::values;

    #[test]
    fn every_published_talent_has_a_value_table() {
        for id in 1..=u8::MAX {
            let Some(ability) = get_talent(id) else {
                continue;
            };

            assert!(
                values(i32::from(id)).is_some(),
                "codex publishes talent {id} ({:?}), which no value-table arm names",
                ability.identity,
            );
        }
    }

    #[test]
    fn no_table_declares_more_than_the_four_stored_pairs() {
        for id in 1..=u8::MAX {
            if get_talent(id).is_none() {
                continue;
            }

            let listed = values(i32::from(id)).unwrap_or_default();

            assert!(
                listed.len() <= super::super::VALUES,
                "talent {id} declares {} value pairs, but a row only stores {}",
                listed.len(),
                super::super::VALUES,
            );
        }
    }

    #[test]
    fn every_pair_nyanko_publishes_is_named_here() {
        for id in 1..=u8::MAX {
            let Some(ability) = get_talent(id) else {
                continue;
            };

            let listed = values(i32::from(id)).unwrap_or_default();

            assert!(
                listed.len() >= ability.talent_values.len(),
                "codex gives talent {id} ({:?}) {} value pairs, but only {} are named here",
                ability.identity,
                ability.talent_values.len(),
                listed.len(),
            );
        }
    }
}
