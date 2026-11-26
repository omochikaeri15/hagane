use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::fuzzy::{self, Query};
use super::grammar::{self, Kind, Request, Unit};
use super::prose::{self, Line};

pub(crate) const HELD: usize = 50;
pub(crate) const NAME_LIMIT: u16 = 32;
pub(crate) const TARGET_LIMIT: u16 = 100;
const NICKNAMES: [(&str, Kind, &str); 9] = [
    ("Sgao", Kind::Cat, "333-2"),
    ("Dmitama", Kind::Cat, "378-2"),
    ("D'ark", Kind::Cat, "441-2"),
    ("Dasli", Kind::Cat, "543-2"),
    ("Dgaru", Kind::Cat, "609-2"),
    ("Diz", Kind::Cat, "657-2"),
    ("Dphono", Kind::Cat, "705-2"),
    ("Dluna", Kind::Cat, "787-2"),
    ("Dlunos", Kind::Cat, "859-2"),
];
const ORIGINALS: [(&str, Kind, &str); 38] = [
    ("Yukimura", Kind::Cat, "71"),
    ("Keiji", Kind::Cat, "72"),
    ("Kaguya", Kind::Cat, "138"),
    ("Kenshin", Kind::Cat, "158"),
    ("Zeus", Kind::Cat, "257"),
    ("Aphrodite", Kind::Cat, "259"),
    ("Chronos", Kind::Cat, "439"),
    ("Kasli", Kind::Cat, "529"),
    ("Kanna", Kind::Cat, "607"),
    ("Satoru", Kind::Cat, "617"),
    ("Hanzo", Kind::Cat, "649"),
    ("Phono", Kind::Cat, "690"),
    ("Lunacia", Kind::Cat, "779"),
    ("Rin", Kind::Cat, "363"),
    ("Asuka", Kind::Cat, "410"),
    ("Mari", Kind::Cat, "411"),
    ("Miku", Kind::Cat, "535"),
    ("Len", Kind::Cat, "537"),
    ("Luka", Kind::Cat, "560"),
    ("Ranma", Kind::Cat, "596"),
    ("Akane", Kind::Cat, "597"),
    ("Genma", Kind::Cat, "672"),
    ("Baki", Kind::Cat, "789"),
    ("Kaioh", Kind::Cat, "790"),
    ("Tanjiro", Kind::Cat, "840"),
    ("Nezuko", Kind::Cat, "841"),
    ("Peng", Kind::Enemy, "5"),
    ("Bear", Kind::Enemy, "12"),
    ("Nyandam", Kind::Enemy, "23"),
    ("Dark Emperor", Kind::Enemy, "23"),
    ("Bun", Kind::Enemy, "24"),
    ("Bun Bun", Kind::Enemy, "24"),
    ("Jones", Kind::Enemy, "119"),
    ("Sable", Kind::Enemy, "123"),
    ("Yulala", Kind::Enemy, "380"),
    ("Wind", Kind::Enemy, "580"),
    ("Casaurian", Kind::Enemy, "604"),
    ("Luza", Kind::Enemy, "628"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Target<'a> {
    Typed(&'a str),
    Known(Kind, &'a str),
    Original(Kind, &'a str),
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Row<'a> {
    pub(crate) key: String,
    pub(crate) name: &'a str,
    pub(crate) target: Target<'a>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Own {
    name: String,
    target: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Aliases {
    own: BTreeMap<String, Own>,
    hidden: BTreeSet<String>,
    bare: bool,
}

fn joined(target: &str, tail: &str) -> String {
    if tail.is_empty() { target.to_owned() } else { format!("{target} {tail}") }
}

pub(crate) fn readable(name: &str) -> Result<(), &'static Line> {
    let key = fuzzy::normalized(name);

    if key.is_empty() {
        return Err(&prose::ALIAS_UNREADABLE);
    }

    let read: Vec<Request> = Kind::ALL.into_iter().filter_map(|kind| grammar::parse(name, kind).ok()).collect();

    if read.iter().any(|request| !matches!(request.unit, Unit::Name(_))) {
        return Err(&prose::ALIAS_IS_ID);
    }

    let whole = |request: &Request| request.whole.as_deref().is_some_and(|whole| fuzzy::normalized(whole) == key);
    let named = |request: &Request| matches!(&request.unit, Unit::Name(held) if request.tail.is_empty() && fuzzy::normalized(held) == key);

    if read.iter().any(|request| request.search.is_none() && (whole(request) || named(request))) { Ok(()) } else { Err(&prose::ALIAS_SPLIT) }
}

impl Aliases {
    fn built_in(&self) -> impl Iterator<Item = Row<'_>> + '_ {
        let nicknames = NICKNAMES.iter().map(|(name, kind, unit)| (*name, Target::Known(*kind, unit)));
        let originals = ORIGINALS.iter().map(|(name, kind, unit)| (*name, Target::Original(*kind, unit)));

        nicknames.chain(originals).filter(|_| !self.bare).map(|(name, target)| Row { key: fuzzy::normalized(name), name, target }).filter(|row| !self.hidden.contains(&row.key) && !self.own.contains_key(&row.key))
    }

    pub(crate) fn rows(&self) -> Vec<Row<'_>> {
        self.own.iter().map(|(key, own)| Row { key: key.clone(), name: &own.name, target: Target::Typed(&own.target) }).chain(self.built_in()).collect()
    }

    pub(crate) fn count(&self) -> usize {
        self.own.len() + self.built_in().count()
    }

    pub(crate) fn keyed(&self, key: &str) -> Option<Target<'_>> {
        self.own.get(key).map(|own| Target::Typed(&own.target)).or_else(|| self.built_in().find(|row| row.key == key).map(|row| row.target))
    }

    pub(crate) fn full(&self, name: &str) -> bool {
        self.own.len() >= HELD && !self.own.contains_key(&fuzzy::normalized(name))
    }

    pub(crate) fn add(&mut self, name: &str, target: &str) {
        self.own.insert(fuzzy::normalized(name), Own { name: name.to_owned(), target: target.to_owned() });
    }

    pub(crate) fn remove(&mut self, key: &str) -> Option<String> {
        if let Some(own) = self.own.remove(key) {
            return Some(own.name);
        }

        let name = self.built_in().find(|row| row.key == key).map(|row| row.name.to_owned())?;

        self.hidden.insert(key.to_owned());
        Some(name)
    }

    pub(crate) fn clear(&mut self) {
        *self = Self { bare: true, ..Self::default() };
    }

    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn cleared(&self) -> bool {
        self.bare && self.own.is_empty()
    }

    pub(crate) fn untouched(&self) -> bool {
        *self == Self::default()
    }

    fn tried<'a>(&'a self, request: &'a Request) -> impl Iterator<Item = (Target<'a>, &'a str)> + 'a {
        let name = match (&request.unit, request.search) {
            (Unit::Name(name), None) => Some(name.as_str()),
            _ => None,
        };
        let tries = [name.and(request.whole.as_deref()).map(|whole| (whole, "")), name.map(|name| (name, request.tail.as_str()))];

        tries.into_iter().flatten().filter_map(|(typed, tail)| Some((self.keyed(&fuzzy::normalized(typed))?, tail)))
    }

    pub(crate) fn reroute(&self, request: &Request, kinds: &[Kind], originals: bool) -> Option<(Option<Kind>, String)> {
        self.tried(request).find_map(|(target, tail)| match target {
            Target::Typed(target) => Some((None, joined(target, tail))),
            Target::Known(kind, unit) => kinds.contains(&kind).then(|| (Some(kind), joined(unit, tail))),
            Target::Original(kind, unit) => (originals && kinds.contains(&kind)).then(|| (Some(kind), joined(unit, tail))),
        })
    }

    pub(crate) fn original(&self, request: &Request, kind: Kind) -> Option<String> {
        self.tried(request).find_map(|(target, tail)| match target {
            Target::Original(held, unit) if held == kind => Some(joined(unit, tail)),
            _ => None,
        })
    }

    pub(crate) fn close(&self, asked: &str, closeness: u32, kinds: &[Kind]) -> Vec<Row<'_>> {
        let query = Query::new(asked);
        let fits = |target: &Target<'_>| match target {
            Target::Typed(_) => true,
            Target::Known(kind, _) | Target::Original(kind, _) => kinds.contains(kind),
        };
        let mut scored: Vec<(u32, Row<'_>)> = self.rows().into_iter().filter(|row| fits(&row.target)).filter_map(|row| Some((query.score_within(row.name, closeness)?, row))).collect();

        scored.sort_by_key(|(score, _)| Reverse(*score));
        scored.into_iter().map(|(_, row)| row).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rerouted(aliases: &Aliases, text: &str, kind: Kind) -> Option<(Option<Kind>, String)> {
        aliases.reroute(&grammar::parse(text, kind).unwrap(), &[kind], true)
    }

    // The alias swaps in for the name only, and whatever was typed after it carries over.
    #[test]
    fn an_alias_stands_in_for_the_name() {
        let mut aliases = Aliases::default();

        aliases.add("Baha", "Bahamut Cat");

        assert_eq!(rerouted(&aliases, "baha", Kind::Cat), Some((None, "Bahamut Cat".to_owned())));
        assert_eq!(rerouted(&aliases, "BAHA a walk -f gif", Kind::Cat), Some((None, "Bahamut Cat a walk -f gif".to_owned())));
        assert_eq!(rerouted(&aliases, "baha -c 2", Kind::Stage), Some((None, "Bahamut Cat -c 2".to_owned())));
        assert_eq!(rerouted(&aliases, "bahamut", Kind::Cat), None, "only the whole alias counts, never something close to it");
        assert_eq!(rerouted(&aliases, "baha -s", Kind::Cat), None, "a search lists it instead");
    }

    // A name that ends in a keyword is tried whole before the keyword is taken off it.
    #[test]
    fn an_alias_may_end_in_a_keyword() {
        let mut aliases = Aliases::default();

        aliases.add("teacher a", "Teacher A");
        aliases.add("teacher", "Teacher Bun Bun");

        assert_eq!(rerouted(&aliases, "teacher a", Kind::Enemy), Some((None, "Teacher A".to_owned())));
        assert_eq!(rerouted(&aliases, "teacher a walk", Kind::Enemy), Some((None, "Teacher Bun Bun a walk".to_owned())));
    }

    // A built-in one knows what it points at, so it only answers where that kind of thing is asked for.
    #[test]
    fn built_in_aliases_keep_to_their_kind() {
        let aliases = Aliases::default();

        assert_eq!(rerouted(&aliases, "dasli lv50", Kind::Cat), Some((Some(Kind::Cat), "543-2 lv50".to_owned())));
        assert_eq!(rerouted(&aliases, "dasli", Kind::Enemy), None);
        assert_eq!(rerouted(&aliases, "Bun Bun 400%", Kind::Enemy), Some((Some(Kind::Enemy), "24 400%".to_owned())));
        assert_eq!(rerouted(&aliases, "bun bun", Kind::Cat), None);
        assert_eq!(rerouted(&aliases, "keiji a walk", Kind::Cat), Some((Some(Kind::Cat), "72 a walk".to_owned())));

        let bare = grammar::parse("keiji a walk", Kind::Cat).unwrap();

        assert_eq!(aliases.reroute(&bare, &[Kind::Cat], false), None, "a name for the original waits for the search to say which kind it found");
        assert_eq!((aliases.original(&bare, Kind::Cat), aliases.original(&bare, Kind::Enemy)), (Some("72 a walk".to_owned()), None));
        assert_eq!(aliases.reroute(&grammar::parse("dasli", Kind::Cat).unwrap(), &[Kind::Cat], false), Some((Some(Kind::Cat), "543-2".to_owned())), "a nickname never waits");
    }

    // Removing a built-in one hides it, your own one of the same name covers it, and the two bulk buttons undo all of that.
    #[test]
    fn built_in_aliases_are_layered_under_your_own() {
        let mut aliases = Aliases::default();

        assert_eq!(aliases.count(), NICKNAMES.len() + ORIGINALS.len());
        assert_eq!(aliases.remove("dasli"), Some("Dasli".to_owned()));
        assert_eq!((aliases.count(), aliases.keyed("dasli")), (NICKNAMES.len() + ORIGINALS.len() - 1, None));

        aliases.add("DASLI", "Kasli the Bane");
        assert_eq!(aliases.keyed("dasli"), Some(Target::Typed("Kasli the Bane")));

        aliases.clear();
        assert_eq!((aliases.count(), aliases.cleared()), (0, true));

        aliases.add("mine", "Cat");
        assert_eq!(aliases.rows().len(), 1, "built-in ones stay off until they are asked back");

        aliases.reset();
        assert!(aliases.untouched());
        assert_eq!(aliases.keyed("dasli"), Some(Target::Known(Kind::Cat, "543-2")));
    }

    // An alias that could never be typed as a name is turned away when it is added.
    #[test]
    fn unreachable_aliases_are_refused() {
        assert_eq!(readable("Dasli"), Ok(()));
        assert_eq!(readable("dark kasli"), Ok(()));
        assert_eq!(readable("teacher a"), Ok(()));
        assert_eq!(readable("!!!"), Err(&prose::ALIAS_UNREADABLE));
        assert_eq!(readable("543"), Err(&prose::ALIAS_IS_ID));
        assert!(NICKNAMES.iter().chain(&ORIGINALS).all(|(name, kind, unit)| readable(name).is_ok() && matches!(grammar::parse(unit, *kind).map(|request| request.unit), Ok(Unit::Id(..)))), "every built-in one can be typed, and points at an ID");
        assert_eq!(NICKNAMES.iter().chain(&ORIGINALS).map(|(name, ..)| fuzzy::normalized(name)).collect::<BTreeSet<_>>().len(), NICKNAMES.len() + ORIGINALS.len(), "no two built-in ones share a name");
        assert_eq!(readable("25-3"), Err(&prose::ALIAS_IS_ID));
        assert_eq!(readable("kasli --search"), Err(&prose::ALIAS_SPLIT));
    }

    #[test]
    fn the_cap_only_stops_new_names() {
        let mut aliases = Aliases::default();

        for slot in 0..HELD {
            aliases.add(&format!("name{slot}"), "Cat");
        }

        assert!(aliases.full("one more"));
        assert!(!aliases.full("NAME7"), "changing one you have is always allowed");
    }
}
