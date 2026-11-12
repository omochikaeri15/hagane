use std::collections::HashMap;

use codex::cat::unit::Talent;
use codex::chapter::map::roguelike_grants;
use codex::combat::Entity;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Grants {
    pub talent: Talent,
    pub levels: HashMap<u8, u8>,
    pub extras: Vec<(i32, Vec<i32>, bool)>,
}

impl Grants {
    pub fn apply(&self, stats: &mut Entity) {
        roguelike_grants(stats, &self.extras);
    }
}
