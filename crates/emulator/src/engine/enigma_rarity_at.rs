use super::{AppContext, EnigmaRarity};

pub fn enigma_rarity_at(ctx: &mut AppContext, rarity: i32) -> &mut EnigmaRarity {
    ctx.enigma.rarities.entry(rarity).or_default()
}
