mod abilities;
mod arena;
pub(crate) mod entity;
mod glossary;
mod output;
mod scaling;
mod versus;

pub use abilities::{
    Ability, AttrUnit, AttrValue, Attribute, Identity, REGISTRY, Stored, TalentValue, get_ability,
    get_talent, targets_traits,
};
pub use arena::{knockback_push, knockback_reach, range_of, Span, KNOCKBACK_FRAMES};
pub use crate::common::Scale;
pub use crate::common::Separator;
pub use entity::{Column, Entity, EntityError, Faction};
pub use scaling::{crowned, deploy_cost, magnified};
pub use output::{Exposure, Hits, Mode, Rules, Switch, Switches};
pub use versus::{resisted, Bearing, Carrier, Foe, Roll, Shortfall, Terms};
pub use glossary::{AttributeIndex, AttributeIndexError, AttributeRow, DictionaryIndex, DictionaryIndexError, DictionaryRow, Glossary, GlossaryEntry, GlossaryError, TraitLabel, TraitLabelError, TraitLabels};
