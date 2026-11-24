mod abilities;
mod arena;
pub(crate) mod entity;
mod glossary;
mod scaling;

pub use abilities::{
    Ability, AttrUnit, AttrValue, Attribute, Identity, REGISTRY, Stored, TalentValue, get_ability,
    get_talent, targets_traits,
};
pub use arena::{range_of, Span};
pub use crate::common::Scale;
pub use crate::common::Separator;
pub use entity::{Column, Entity, EntityError, Faction};
pub use scaling::{crowned, deploy_cost, magnified};
pub use glossary::{AttributeIndex, AttributeIndexError, AttributeRow, DictionaryIndex, DictionaryIndexError, DictionaryRow, Glossary, GlossaryEntry, GlossaryError, TraitLabel, TraitLabelError, TraitLabels};
