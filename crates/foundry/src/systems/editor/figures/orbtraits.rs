use super::resolved::Rule;

pub const NOTICE: &str = "The enemy traits an orb with this trait works against\nAn orb can cover more than one";

pub const LABELS: [&str; 13] = [
    "Red", "Floating", "Black", "Metal", "Angel", "Alien", "Zombie", "Relic", "Traitless", "Witch", "Eva Angel", "Aku",
    "No Trait",
];

pub fn rule() -> Rule {
    Rule::Flag
}
