pub mod json;

use std::collections::BTreeSet;
use std::fmt;

use codex::graphics::backdrop;

use crate::domains::utilities::background::Catalog;
use crate::Vfs;

use json::{Malformed, Node};

const EFFECTS_PREFIX: &str = "bg";
const REDIRECT_LIMIT: usize = 8;

const DATA: &str = "data";
const REDIRECT: &str = "id";
const NAME: &str = "name";
const FILE: &str = "file";
const SPACED: &str = "equallySpaced";
const VALUE: &str = "value";
const VALUES: &str = "values";
const MIN: &str = "min";
const MAX: &str = "max";
const GROUP: &str = "randGroup";
const BASE: &str = "base";
const ZERO: &str = "0";
const ONE: &str = "1";
const FRESH_NAME: &str = "New Emitter";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Whole,
    Real,
}

impl Kind {
    fn takes(self, text: &str) -> bool {
        match self {
            Self::Whole => json::whole(text),
            Self::Real => json::numeric(text),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Spawn,
    Sprite,
    Position,
    Scale,
    Rotation,
    Motion,
    Bounds,
}

impl Section {
    pub const ALL: [Self; 7] = [Self::Spawn, Self::Sprite, Self::Position, Self::Scale, Self::Rotation, Self::Motion, Self::Bounds];

    pub fn title(self) -> &'static str {
        match self {
            Self::Spawn => "Spawn",
            Self::Sprite => "Sprite",
            Self::Position => "Position",
            Self::Scale => "Scale",
            Self::Rotation => "Rotation",
            Self::Motion => "Motion",
            Self::Bounds => "Removal",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Field {
    pub key: &'static str,
    pub label: &'static str,
    pub kind: Kind,
    pub section: Section,
}

const fn field(key: &'static str, label: &'static str, kind: Kind, section: Section) -> Field {
    Field { key, label, kind, section }
}

pub const FIELDS: [Field; 31] = [
    field("count", "Count", Kind::Whole, Section::Spawn),
    field("wait", "Respawn Wait", Kind::Whole, Section::Spawn),
    field("lifeTime", "Life Time", Kind::Whole, Section::Spawn),
    field("model", "Model", Kind::Whole, Section::Sprite),
    field("z", "Layer", Kind::Whole, Section::Sprite),
    field("frame", "Frame", Kind::Whole, Section::Sprite),
    field("startFrame", "Start Frame", Kind::Whole, Section::Sprite),
    field("alpha", "Opacity", Kind::Real, Section::Sprite),
    field("x", "X", Kind::Real, Section::Position),
    field("startX", "Start X", Kind::Real, Section::Position),
    field("y", "Y", Kind::Real, Section::Position),
    field("startY", "Start Y", Kind::Real, Section::Position),
    field("scale", "Scale", Kind::Real, Section::Scale),
    field("startScale", "Start Scale", Kind::Real, Section::Scale),
    field("scaleX", "Scale X", Kind::Real, Section::Scale),
    field("startScaleX", "Start Scale X", Kind::Real, Section::Scale),
    field("scaleY", "Scale Y", Kind::Real, Section::Scale),
    field("startScaleY", "Start Scale Y", Kind::Real, Section::Scale),
    field("angle", "Angle", Kind::Real, Section::Rotation),
    field("angularV", "Spin", Kind::Real, Section::Rotation),
    field("v", "Speed", Kind::Real, Section::Motion),
    field("startV", "Start Speed", Kind::Real, Section::Motion),
    field("moveAngle", "Heading", Kind::Real, Section::Motion),
    field("vx", "Speed X", Kind::Real, Section::Motion),
    field("startVx", "Start Speed X", Kind::Real, Section::Motion),
    field("vy", "Speed Y", Kind::Real, Section::Motion),
    field("startVy", "Start Speed Y", Kind::Real, Section::Motion),
    field("destroyLeft", "Past Left", Kind::Real, Section::Bounds),
    field("destroyRight", "Past Right", Kind::Real, Section::Bounds),
    field("destroyTop", "Past Top", Kind::Real, Section::Bounds),
    field("destroyBottom", "Past Bottom", Kind::Real, Section::Bounds),
];

pub const BASE_NAMES: [&str; 18] = [
    "",
    "bgImage",
    "worldLeft",
    "worldRight",
    "worldTop",
    "worldBottom",
    "screenLeft",
    "screenRight",
    "screenTop",
    "screenBottom",
    "screen",
    "frontChara",
    "backChara",
    "secondToFrame",
    "percentToFloat",
    "percentToAlpha",
    "animeLength",
    "animeInterval",
];

pub const BASE_LABELS: [&str; 18] = [
    "None",
    "Image Width",
    "World Left",
    "World Right",
    "World Top",
    "World Bottom",
    "Screen Left",
    "Screen Right",
    "Screen Top",
    "Screen Bottom",
    "Screen",
    "In Front Of Units",
    "Behind Units",
    "Seconds",
    "Percent",
    "Percent Opacity",
    "Animation Length",
    "Animation Loops",
];

const SPACING_BASE: &str = "default";

pub fn base_label(name: &str) -> Option<&'static str> {
    BASE_NAMES.iter().position(|held| *held == name).map(|index| BASE_LABELS[index])
}

pub fn base_name(label: &str) -> Option<&'static str> {
    BASE_LABELS.iter().position(|held| *held == label).map(|index| BASE_NAMES[index])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    pub id: u32,
    pub redirect: Option<u32>,
    pub drawn: bool,
}

impl fmt::Display for Entry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.redirect {
            Some(target) => write!(formatter, "{:03} \u{2192} {target:03}", self.id),
            None => write!(formatter, "{:03}", self.id),
        }
    }
}

fn effects_id(name: &str) -> Option<u32> {
    let digits = name.strip_prefix(EFFECTS_PREFIX)?.split('.').next()?;
    let id = digits.parse::<u32>().ok()?;

    (backdrop::effects_name(id) == name).then_some(id)
}

pub fn catalog(vfs: &Vfs) -> Vec<Entry> {
    let drawn: BTreeSet<u32> = Catalog::scan(vfs).ids.into_iter().collect();
    let mut entries: Vec<Entry> = vfs
        .glob(EFFECTS_PREFIX)
        .iter()
        .filter_map(|name| effects_id(name))
        .filter_map(|id| {
            let document = read(vfs, id)?;

            Some(Entry { id, redirect: document.redirect_id(), drawn: drawn.contains(&id) })
        })
        .collect();

    entries.sort_unstable_by_key(|entry| entry.id);
    entries.dedup_by_key(|entry| entry.id);
    entries
}

pub fn read(vfs: &Vfs, id: u32) -> Option<Document> {
    let path = vfs.find(backdrop::effects_name(id).as_str())?;
    let bytes = vfs.read(&path).ok()?;

    Document::parse(bytes.as_ref()).ok()
}

pub fn settled(entries: &[Entry], id: u32) -> u32 {
    let mut target = id;

    for _ in 0..REDIRECT_LIMIT {
        match entries.iter().find(|entry| entry.id == target).and_then(|entry| entry.redirect) {
            Some(next) => target = next,
            None => break,
        }
    }

    target
}

pub fn hosts(entries: &[Entry], id: u32) -> Vec<u32> {
    let own: Vec<u32> = entries.iter().filter(|entry| entry.drawn && settled(entries, entry.id) == id).map(|entry| entry.id).collect();

    if !own.is_empty() {
        return own;
    }

    entries.iter().filter(|entry| entry.drawn).map(|entry| entry.id).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Off,
    Fixed,
    Ranged,
    Listed,
}

impl Shape {
    pub const ALL: [Self; 4] = [Self::Off, Self::Fixed, Self::Ranged, Self::Listed];

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Fixed => "Fixed",
            Self::Ranged => "Range",
            Self::Listed => "List",
        }
    }
}

impl fmt::Display for Shape {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Slot {
    Value,
    Values,
    Min,
    MinBase,
    Max,
    MaxBase,
    Group,
    Base,
}

impl Slot {
    pub const ALL: [Self; 8] = [Self::Value, Self::Values, Self::Min, Self::MinBase, Self::Max, Self::MaxBase, Self::Group, Self::Base];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Spot {
    First,
    Last,
    Step,
    Base,
}

impl Spot {
    pub const ALL: [Self; 4] = [Self::First, Self::Last, Self::Step, Self::Base];

    fn key(self) -> &'static str {
        match self {
            Self::First => "pos1",
            Self::Last => "pos2",
            Self::Step => VALUE,
            Self::Base => BASE,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Asset {
    Image,
    Imgcut,
    Model,
    Anime,
}

impl Asset {
    pub const ALL: [Self; 4] = [Self::Image, Self::Imgcut, Self::Model, Self::Anime];

    pub fn label(self) -> &'static str {
        match self {
            Self::Image => "Image",
            Self::Imgcut => "Imgcut",
            Self::Model => "Model",
            Self::Anime => "Animation",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Imgcut => "imgcut",
            Self::Model => "model",
            Self::Anime => "anime",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unreadable {
    Malformed(Malformed),
    Shapeless,
}

impl fmt::Display for Unreadable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(fault) => write!(formatter, "{fault}"),
            Self::Shapeless => formatter.write_str("The file holds neither an emitter list nor a redirect."),
        }
    }
}

impl std::error::Error for Unreadable {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    root: Node,
    marked: bool,
}

fn typed(held: Option<&Node>, text: &str, fresh_text: bool) -> Node {
    match held {
        Some(Node::Text(_)) => Node::text(text),
        Some(_) => Node::Number(text.to_owned()),
        None if fresh_text => Node::text(text),
        None => Node::Number(text.to_owned()),
    }
}

fn covered(name: &str, node: &Node) -> bool {
    match name {
        NAME => matches!(node, Node::Text(_)),
        FILE | SPACED => matches!(node, Node::Object(_)),
        _ => matches!(node, Node::Object(_)) && FIELDS.iter().any(|held| held.key == name),
    }
}

fn base_of(node: Option<&Node>) -> String {
    node.and_then(|held| held.get(BASE)).map_or_else(String::new, Node::shown)
}

fn rebase(node: &mut Node, name: &str) {
    if name.is_empty() {
        node.remove(BASE);
    } else {
        node.set(BASE, Node::text(name));
    }
}

fn seed_of(param: Option<&Node>) -> Node {
    let lead = param.and_then(|held| {
        held.get(VALUE)
            .or_else(|| match held.get(VALUES) {
                Some(Node::List(items)) => items.first(),
                _ => None,
            })
            .or_else(|| held.get(MIN).and_then(|bound| bound.get(VALUE)))
            .or_else(|| held.get(MAX).and_then(|bound| bound.get(VALUE)))
    });

    match lead {
        Some(node @ (Node::Number(_) | Node::Text(_))) => node.clone(),
        _ => Node::Number(ZERO.to_owned()),
    }
}

impl Document {
    pub fn parse<B: AsRef<[u8]>>(bytes: B) -> Result<Self, Unreadable> {
        Self::parse_inner(bytes.as_ref())
    }

    fn parse_inner(bytes: &[u8]) -> Result<Self, Unreadable> {
        let (marked, body) = json::marked(bytes);
        let root = Node::parse(body).map_err(Unreadable::Malformed)?;
        let shaped = root.get(REDIRECT).is_some() || matches!(root.get(DATA), Some(Node::List(_)));

        if !shaped {
            return Err(Unreadable::Shapeless);
        }

        Ok(Self { root, marked })
    }

    pub fn bytes(&self) -> Vec<u8> {
        let mut bytes = if self.marked { json::mark().to_vec() } else { Vec::new() };

        bytes.extend_from_slice(self.root.to_string().as_bytes());
        bytes
    }

    pub fn redirect(&self) -> Option<String> {
        self.root.get(REDIRECT).map(Node::shown)
    }

    pub fn redirect_id(&self) -> Option<u32> {
        self.redirect().map(|text| text.trim().parse::<i64>().map_or(0, |id| id.clamp(0, i64::from(u32::MAX)) as u32))
    }

    pub fn set_redirect(&mut self, text: &str) -> bool {
        if !json::whole(text) || self.root.get(REDIRECT).is_none() {
            return false;
        }

        let node = typed(self.root.get(REDIRECT), text, false);

        self.root.set(REDIRECT, node);

        true
    }

    pub fn adopt(&mut self, other: &Self) {
        self.root = other.root.clone();
    }

    fn emitters(&self) -> &[Node] {
        match self.root.get(DATA) {
            Some(Node::List(items)) if self.redirect().is_none() => items,
            _ => &[],
        }
    }

    fn emitters_mut(&mut self) -> Option<&mut Vec<Node>> {
        if self.redirect().is_some() {
            return None;
        }

        match self.root.get_mut(DATA) {
            Some(Node::List(items)) => Some(items),
            _ => None,
        }
    }

    fn emitter(&self, at: usize) -> Option<&Node> {
        self.emitters().get(at)
    }

    fn emitter_mut(&mut self, at: usize) -> Option<&mut Node> {
        self.emitters_mut().and_then(|items| items.get_mut(at)).filter(|node| matches!(node, Node::Object(_)))
    }

    pub fn len(&self) -> usize {
        self.emitters().len()
    }

    pub fn is_empty(&self) -> bool {
        self.emitters().is_empty()
    }

    pub fn name(&self, at: usize) -> String {
        self.emitter(at).and_then(|node| node.get(NAME)).map_or_else(String::new, Node::shown)
    }

    pub fn set_name(&mut self, at: usize, text: &str) {
        if let Some(node) = self.emitter_mut(at) {
            node.set(NAME, Node::text(text));
        }
    }

    pub fn asset(&self, at: usize, asset: Asset) -> Option<String> {
        let file = self.emitter(at)?.get(FILE)?;

        matches!(file, Node::Object(_)).then(|| file.get(asset.key()).map_or_else(String::new, Node::shown))
    }

    pub fn set_asset(&mut self, at: usize, asset: Asset, text: &str) {
        let Some(file) = self.emitter_mut(at).and_then(|node| node.get_mut(FILE)) else {
            return;
        };

        file.set(asset.key(), Node::text(text));
    }

    fn param(&self, at: usize, key: &str) -> Option<&Node> {
        self.emitter(at)?.get(key).filter(|node| matches!(node, Node::Object(_)))
    }

    pub fn shape(&self, at: usize, key: &str) -> Shape {
        let Some(param) = self.param(at, key) else {
            return Shape::Off;
        };

        if param.get(VALUE).is_some() {
            Shape::Fixed
        } else if param.get(VALUES).is_some() {
            Shape::Listed
        } else {
            Shape::Ranged
        }
    }

    pub fn reshape(&mut self, at: usize, key: &str, shape: Shape) {
        if self.shape(at, key) == shape {
            return;
        }

        let held = self.param(at, key);
        let seed = seed_of(held);
        let base = base_of(held);
        let bound = |name: &str| {
            held.and_then(|param| param.get(name))
                .filter(|node| matches!(node, Node::Object(_)))
                .cloned()
                .unwrap_or_else(|| Node::Object(vec![(VALUE.to_owned(), seed.clone())]))
        };
        let fresh = match shape {
            Shape::Off => None,
            Shape::Fixed => Some(vec![(VALUE.to_owned(), seed.clone())]),
            Shape::Listed => Some(vec![(VALUES.to_owned(), Node::List(vec![seed.clone()]))]),
            Shape::Ranged => Some(vec![(MIN.to_owned(), bound(MIN)), (MAX.to_owned(), bound(MAX))]),
        };
        let Some(emitter) = self.emitter_mut(at) else {
            return;
        };

        match fresh {
            None => emitter.remove(key),
            Some(members) => {
                let mut node = Node::Object(members);

                rebase(&mut node, &base);
                emitter.set(key, node);
            }
        }
    }

    pub fn cell(&self, at: usize, key: &str, slot: Slot) -> String {
        let param = self.param(at, key);
        let shown = |node: Option<&Node>| node.map_or_else(String::new, Node::shown);
        let bound = |name: &str| param.and_then(|held| held.get(name)).filter(|node| matches!(node, Node::Object(_)));

        match slot {
            Slot::Value => shown(param.and_then(|held| held.get(VALUE))),
            Slot::Values => match param.and_then(|held| held.get(VALUES)) {
                Some(Node::List(items)) => items.iter().map(Node::shown).collect::<Vec<_>>().join(", "),
                _ => String::new(),
            },
            Slot::Min => shown(bound(MIN).and_then(|held| held.get(VALUE))),
            Slot::Max => shown(bound(MAX).and_then(|held| held.get(VALUE))),
            Slot::MinBase => base_of(bound(MIN)),
            Slot::MaxBase => base_of(bound(MAX)),
            Slot::Group => shown(param.and_then(|held| held.get(GROUP))),
            Slot::Base => base_of(param),
        }
    }

    pub fn set_cell(&mut self, at: usize, key: &str, kind: Kind, slot: Slot, text: &str) -> bool {
        let text = text.trim();
        let shape = self.shape(at, key);
        let Some(param) = self.emitter_mut(at).and_then(|node| node.get_mut(key)) else {
            return false;
        };
        let edge = |param: &mut Node, name: &str, text: &str| {
            if text.is_empty() {
                param.remove(name);

                return true;
            }

            if !kind.takes(text) {
                return false;
            }

            if !matches!(param.get(name), Some(Node::Object(_))) {
                param.set(name, Node::Object(Vec::new()));
            }

            let Some(bound) = param.get_mut(name) else {
                return false;
            };
            let node = typed(bound.get(VALUE), text, false);

            bound.set(VALUE, node);

            true
        };

        match (slot, shape) {
            (Slot::Value, Shape::Fixed) => {
                if !kind.takes(text) {
                    return false;
                }

                let node = typed(param.get(VALUE), text, false);

                param.set(VALUE, node);

                true
            }
            (Slot::Values, Shape::Listed) => {
                let parts: Vec<&str> = text.split(',').map(str::trim).collect();

                if parts.iter().any(|part| !kind.takes(part)) {
                    return false;
                }

                let lead = match param.get(VALUES) {
                    Some(Node::List(items)) => items.first().cloned(),
                    _ => None,
                };
                let items = parts.into_iter().map(|part| typed(lead.as_ref(), part, false)).collect();

                param.set(VALUES, Node::List(items));

                true
            }
            (Slot::Min, Shape::Ranged) => edge(param, MIN, text),
            (Slot::Max, Shape::Ranged) => edge(param, MAX, text),
            (Slot::MinBase | Slot::MaxBase, Shape::Ranged) => {
                let name = if slot == Slot::MinBase { MIN } else { MAX };
                let Some(bound) = param.get_mut(name).filter(|node| matches!(node, Node::Object(_))) else {
                    return false;
                };

                rebase(bound, text);

                true
            }
            (Slot::Group, Shape::Ranged) => {
                if text.is_empty() {
                    param.remove(GROUP);

                    return true;
                }

                if !json::whole(text) {
                    return false;
                }

                let node = typed(param.get(GROUP), text, true);

                param.set(GROUP, node);

                true
            }
            (Slot::Base, Shape::Fixed | Shape::Ranged | Shape::Listed) => {
                rebase(param, text);

                true
            }
            _ => false,
        }
    }

    pub fn values(&self, at: usize, key: &str) -> Vec<String> {
        match self.param(at, key).and_then(|held| held.get(VALUES)) {
            Some(Node::List(items)) => items.iter().map(Node::shown).collect(),
            _ => Vec::new(),
        }
    }

    fn listed_mut(&mut self, at: usize, key: &str) -> Option<&mut Vec<Node>> {
        if self.shape(at, key) != Shape::Listed {
            return None;
        }

        match self.emitter_mut(at)?.get_mut(key)?.get_mut(VALUES)? {
            Node::List(items) => Some(items),
            _ => None,
        }
    }

    pub fn set_value(&mut self, at: usize, key: &str, kind: Kind, index: usize, text: &str) -> bool {
        let text = text.trim();

        if !kind.takes(text) {
            return false;
        }

        let Some(held) = self.listed_mut(at, key).and_then(|items| items.get_mut(index)) else {
            return false;
        };

        *held = typed(Some(held), text, false);

        true
    }

    pub fn add_value(&mut self, at: usize, key: &str) -> bool {
        let Some(items) = self.listed_mut(at, key) else {
            return false;
        };
        let fresh = match items.last() {
            Some(node @ (Node::Number(_) | Node::Text(_))) => node.clone(),
            _ => Node::Number(ZERO.to_owned()),
        };

        items.push(fresh);

        true
    }

    pub fn drop_value(&mut self, at: usize, key: &str, index: usize) -> bool {
        let Some(items) = self.listed_mut(at, key).filter(|items| items.len() > 1 && index < items.len()) else {
            return false;
        };

        items.remove(index);

        true
    }

    pub fn spaced(&self, at: usize) -> bool {
        self.param(at, SPACED).is_some()
    }

    pub fn set_spaced(&mut self, at: usize, on: bool) {
        if self.spaced(at) == on {
            return;
        }

        let Some(emitter) = self.emitter_mut(at) else {
            return;
        };

        if !on {
            emitter.remove(SPACED);

            return;
        }

        emitter.set(
            SPACED,
            Node::Object(vec![
                (Spot::First.key().to_owned(), Node::text(ZERO)),
                (Spot::Last.key().to_owned(), Node::text(ZERO)),
                (Spot::Step.key().to_owned(), Node::text(ONE)),
                (BASE.to_owned(), Node::text(SPACING_BASE)),
            ]),
        );
    }

    pub fn spot(&self, at: usize, spot: Spot) -> String {
        self.param(at, SPACED).and_then(|node| node.get(spot.key())).map_or_else(String::new, Node::shown)
    }

    pub fn set_spot(&mut self, at: usize, spot: Spot, text: &str) -> bool {
        let text = text.trim();
        let Some(node) = self.emitter_mut(at).and_then(|emitter| emitter.get_mut(SPACED)).filter(|node| matches!(node, Node::Object(_))) else {
            return false;
        };

        if spot == Spot::Base {
            rebase(node, text);

            return true;
        }

        if !json::whole(text) {
            return false;
        }

        let fresh = typed(node.get(spot.key()), text, true);

        node.set(spot.key(), fresh);

        true
    }

    pub fn extras(&self, at: usize) -> Vec<(String, String)> {
        let Some(Node::Object(members)) = self.emitter(at) else {
            return Vec::new();
        };

        members
            .iter()
            .map(|(name, node)| (json::plain(name), node))
            .filter(|(name, node)| !covered(name, node))
            .map(|(name, node)| (name, node.to_string()))
            .collect()
    }

    pub fn set_extra(&mut self, at: usize, key: &str, text: &str) -> bool {
        let Ok(node) = Node::parse(text.as_bytes()) else {
            return false;
        };
        let Some(emitter) = self.emitter_mut(at) else {
            return false;
        };

        emitter.set(key, node);

        true
    }

    pub fn duplicate(&mut self, at: usize) -> Option<usize> {
        let items = self.emitters_mut()?;
        let copy = items.get(at)?.clone();

        items.insert(at + 1, copy);

        Some(at + 1)
    }

    pub fn append(&mut self) -> Option<usize> {
        let items = self.emitters_mut()?;

        items.push(Node::Object(vec![
            (NAME.to_owned(), Node::text(FRESH_NAME)),
            ("count".to_owned(), Node::Object(vec![(VALUE.to_owned(), Node::Number(ONE.to_owned()))])),
        ]));

        Some(items.len() - 1)
    }

    pub fn remove(&mut self, at: usize) -> bool {
        let Some(items) = self.emitters_mut().filter(|items| at < items.len()) else {
            return false;
        };

        items.remove(at);

        true
    }

    pub fn shift(&mut self, at: usize, earlier: bool) -> Option<usize> {
        let items = self.emitters_mut()?;
        let to = if earlier { at.checked_sub(1)? } else { at + 1 };

        if at >= items.len() || to >= items.len() {
            return None;
        }

        items.swap(at, to);

        Some(to)
    }
}

#[cfg(test)]
mod tests {
    use super::{base_label, base_name, effects_id, hosts, settled, Asset, Document, Entry, Kind, Shape, Slot, Spot, Unreadable};

    const SOURCE: &str = r#"{"data":[{"file":{"image":"a.png","imgcut":"a.imgcut","model":"a.mamodel","anime":"a.maanim"},"name":"dust","count":{"value":60},"x":{"min":{"value":0,"base":"worldLeft"},"max":{"value":0,"base":"worldRight"}},"z":{"value":0,"base":"frontChara"},"startWait":{"value":50},"equallySpaced":{"pos1":"0","pos2":"18","value":"240","base":"default"}},{"name":"second","model":{"values":[0,1]}}]}"#;

    fn open() -> Document {
        Document::parse(SOURCE).unwrap()
    }

    #[test]
    fn an_untouched_document_writes_back_exactly() {
        assert_eq!(open().bytes(), SOURCE.as_bytes());
    }

    #[test]
    fn shapes_follow_the_reader_order() {
        let doc = open();

        assert_eq!(doc.shape(0, "count"), Shape::Fixed);
        assert_eq!(doc.shape(0, "x"), Shape::Ranged);
        assert_eq!(doc.shape(1, "model"), Shape::Listed);
        assert_eq!(doc.shape(0, "angle"), Shape::Off);
        assert_eq!(doc.cell(0, "x", Slot::MaxBase), "worldRight");
        assert_eq!(doc.cell(1, "model", Slot::Values), "0, 1");
    }

    #[test]
    fn a_cell_edit_changes_only_its_own_number() {
        let mut doc = open();

        assert!(doc.set_cell(0, "count", Kind::Whole, Slot::Value, "75"));
        assert!(!doc.set_cell(0, "count", Kind::Whole, Slot::Value, "7.5"));
        assert!(!doc.set_cell(0, "count", Kind::Whole, Slot::Value, "-"));

        let written = String::from_utf8(doc.bytes()).unwrap();

        assert_eq!(written, SOURCE.replace(r#""count":{"value":60}"#, r#""count":{"value":75}"#));
    }

    #[test]
    fn reshaping_keeps_the_base_and_seeds_from_what_was_there() {
        let mut doc = open();

        doc.reshape(0, "z", Shape::Ranged);
        assert_eq!(doc.cell(0, "z", Slot::Min), "0");
        assert_eq!(doc.cell(0, "z", Slot::Base), "frontChara");

        doc.reshape(0, "x", Shape::Fixed);
        assert_eq!(doc.cell(0, "x", Slot::Value), "0");

        doc.reshape(0, "angle", Shape::Fixed);
        assert_eq!(doc.cell(0, "angle", Slot::Value), "0");

        doc.reshape(0, "angle", Shape::Off);
        assert_eq!(doc.shape(0, "angle"), Shape::Off);
    }

    #[test]
    fn an_emptied_bound_is_dropped_rather_than_zeroed() {
        let mut doc = open();

        assert!(doc.set_cell(0, "x", Kind::Real, Slot::Min, ""));
        assert_eq!(doc.cell(0, "x", Slot::Min), "");
        assert_eq!(doc.cell(0, "x", Slot::MinBase), "");
        assert!(!doc.set_cell(0, "x", Kind::Real, Slot::MinBase, "worldLeft"));
        assert!(doc.set_cell(0, "x", Kind::Real, Slot::Min, "-1.5"));
        assert!(doc.set_cell(0, "x", Kind::Real, Slot::MinBase, "worldLeft"));
        assert!(doc.set_cell(0, "x", Kind::Real, Slot::Group, "2"));
        assert!(String::from_utf8(doc.bytes()).unwrap().contains(r#""randGroup":"2""#));
    }

    #[test]
    fn a_list_edits_one_entry_and_never_empties() {
        let mut doc = open();

        assert_eq!(doc.values(1, "model"), vec!["0", "1"]);
        assert!(doc.set_value(1, "model", Kind::Whole, 1, "4"));
        assert!(!doc.set_value(1, "model", Kind::Whole, 2, "4"));
        assert!(doc.add_value(1, "model"));
        assert_eq!(doc.values(1, "model"), vec!["0", "4", "4"]);
        assert!(doc.drop_value(1, "model", 0));
        assert!(doc.drop_value(1, "model", 0));
        assert!(!doc.drop_value(1, "model", 0));
        assert!(!doc.add_value(0, "count"));
    }

    #[test]
    fn spacing_cells_stay_text() {
        let mut doc = open();

        assert!(doc.set_spot(0, Spot::Last, "20"));
        assert!(!doc.set_spot(0, Spot::Last, "2.5"));
        assert!(String::from_utf8(doc.bytes()).unwrap().contains(r#""pos2":"20""#));

        doc.set_spaced(1, true);
        assert_eq!(doc.spot(1, Spot::Step), "1");
        assert_eq!(doc.spot(1, Spot::Base), doc.spot(0, Spot::Base));
        doc.set_spaced(1, false);
        assert!(!doc.spaced(1));
    }

    #[test]
    fn keys_the_engine_skips_are_kept_and_listed() {
        let mut doc = open();

        assert_eq!(doc.extras(0), vec![("startWait".to_owned(), r#"{"value":50}"#.to_owned())]);
        assert!(doc.set_extra(0, "startWait", r#"{"value":35}"#));
        assert!(!doc.set_extra(0, "startWait", "{"));
    }

    #[test]
    fn emitters_copy_move_and_leave() {
        let mut doc = open();

        assert_eq!(doc.duplicate(0), Some(1));
        assert_eq!(doc.len(), 3);
        assert_eq!(doc.name(1), "dust");
        assert_eq!(doc.shift(1, false), Some(2));
        assert_eq!(doc.name(1), "second");
        assert_eq!(doc.shift(0, true), None);
        assert!(doc.remove(2));
        assert_eq!(doc.bytes(), SOURCE.as_bytes());
        assert_eq!(doc.asset(0, Asset::Anime).as_deref(), Some("a.maanim"));
        assert_eq!(doc.asset(1, Asset::Anime), None);
    }

    #[test]
    fn redirects_settle_and_name_their_hosts() {
        let entry = |id, redirect, drawn| Entry { id, redirect, drawn };
        let entries = [entry(5, Some(1000), true), entry(6, Some(5), true), entry(7, None, true), entry(1000, None, false)];

        assert_eq!(settled(&entries, 6), 1000);
        assert_eq!(hosts(&entries, 1000), vec![5, 6]);
        assert_eq!(hosts(&entries, 7), vec![7]);
        assert_eq!(entries[0].to_string(), "005 \u{2192} 1000");
        assert_eq!(effects_id("bg005.json"), Some(5));
        assert_eq!(effects_id("bg5.json"), None);
        assert_eq!(effects_id("bg.csv"), None);
        assert_eq!(base_name("Seconds"), Some("secondToFrame"));
        assert_eq!(base_label("default"), None);
    }

    #[test]
    fn a_redirect_is_a_document_with_no_emitters() {
        let mut doc = Document::parse(r#"{"id":1000}"#).unwrap();

        assert_eq!(doc.redirect().as_deref(), Some("1000"));
        assert!(doc.is_empty());
        assert!(doc.set_redirect("12"));
        assert_eq!(doc.bytes(), br#"{"id":12}"#);

        doc.adopt(&open());
        assert_eq!(doc.len(), 2);
        assert_eq!(Document::parse("[]"), Err(Unreadable::Shapeless));
    }
}
