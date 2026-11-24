use std::collections::HashMap;
use std::path::PathBuf;

use crate::Vfs;

use super::treasure::percent;
use super::{GlobalStageId, Stage, files};

pub struct Onward {
    pub target: GlobalStageId,
    pub chance: String,
    pub banner: Option<PathBuf>,
    pub name: String,
}

fn named(target: &GlobalStageId, stages: &HashMap<GlobalStageId, Stage>) -> String {
    let unnamed = format!("{:02}", target.stage);

    stages.get(target).map(|found| found.name.trim()).filter(|name| !name.is_empty() && *name != unnamed).map_or_else(|| target.code(), str::to_owned)
}

pub fn resolve(stage: &Stage, stages: &HashMap<GlobalStageId, Stage>, vfs: &Vfs) -> Vec<Onward> {
    stage
        .continuations
        .iter()
        .map(|held| {
            let target = GlobalStageId { category: held.category.clone(), map: held.map_id, stage: held.stage_id };
            let banner = files::stage_banner(vfs, &target.category, target.map, target.stage);

            Onward { chance: percent(held.chance), banner, name: named(&target, stages), target }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use codex::chapter::Category;
    use codex::chapter::stage::Continuation;

    use super::*;

    // A row is headed by the stage's own name, and by its ID when the stage is missing or was never given one.
    #[test]
    fn a_stage_without_a_name_goes_by_its_id() {
        let id = |stage: u32| GlobalStageId { category: Category::ExtraStages, map: 81, stage };
        let lead = |chance: f64, stage: u32| Continuation { chance, category: Category::ExtraStages, map_id: 81, stage_id: stage };
        let stages: HashMap<GlobalStageId, Stage> = [(id(0), Stage { name: "Ghostly Guests".to_owned(), ..Stage::default() }), (id(1), Stage { name: "01".to_owned(), ..Stage::default() })].into_iter().collect();
        let from = Stage { continuations: vec![lead(10.0, 0), lead(100.0 / 3.0, 1), lead(2.5, 2)], ..Stage::default() };
        let told: Vec<(String, String)> = resolve(&from, &stages, &Vfs::with_priority(&[])).into_iter().map(|held| (held.chance, held.name)).collect();

        assert_eq!(told, [("10%".to_owned(), "Ghostly Guests".to_owned()), ("33.33%".to_owned(), "RE081-01".to_owned()), ("2.5%".to_owned(), "RE081-02".to_owned())]);
    }
}
