use std::cmp::Reverse;

use super::fuzzy;
use super::grammar::{self, Kind, Request, Stats, Unit, Want};
use super::library::Library;
use super::prefs::{FORMS, Prefs};

pub(crate) struct Hit {
    pub(crate) kind: Kind,
    pub(crate) value: String,
    pub(crate) label: String,
    pub(crate) note: String,
    pub(crate) score: u32,
}

pub(crate) enum Found {
    Unread(String),
    Nothing,
    One(Kind, Request),
    Several { asked: String, searched: bool, tails: Vec<(Kind, String)>, hits: Vec<Hit> },
}

pub(crate) fn settled(request: Request, named: impl Fn(&str) -> bool) -> Request {
    match request.whole {
        Some(whole) if named(&whole) => Request { unit: Unit::Name(whole), want: Want::Stats(Stats::default()), whole: None, form: request.form, search: request.search, tail: String::new() },
        _ => request,
    }
}

pub(crate) fn names(library: &Library, kind: Kind, name: &str) -> bool {
    match kind {
        Kind::Cat => library.names_cat(name),
        Kind::Enemy => library.names_enemy(name),
    }
}

pub(crate) fn hits(library: &Library, kind: Kind, unit: &Unit, closeness: u32, prefs: &Prefs) -> Vec<Hit> {
    let cats = match (kind, unit) {
        (Kind::Cat, Unit::Id(id, form)) => library.cat(*id).map(|cat| (cat, form.unwrap_or_else(|| prefs.form_from(&cat.forms)), fuzzy::EXACT)).into_iter().collect(),
        (Kind::Cat, Unit::Name(name)) => library.search_cats(name, closeness),
        (Kind::Enemy, _) => Vec::new(),
    };
    let enemies = match (kind, unit) {
        (Kind::Enemy, Unit::Id(id, _)) => library.enemy(*id).map(|enemy| (enemy, fuzzy::EXACT)).into_iter().collect(),
        (Kind::Enemy, Unit::Name(name)) => library.search_enemies(name, closeness),
        (Kind::Cat, _) => Vec::new(),
    };
    let cats = cats.into_iter().map(|(cat, form, score)| Hit {
        kind,
        value: format!("{}-{}", cat.id, form + 1),
        label: cat.display_name(form),
        note: format!("{} • {}", cat.id_str(form), FORMS.get(form).copied().unwrap_or_default()),
        score,
    });
    let enemies = enemies.into_iter().map(|(enemy, score)| Hit { kind, value: enemy.id.to_string(), label: enemy.display_name(), note: enemy.id_str(), score });

    cats.chain(enemies).collect()
}

pub(crate) fn across(library: &Library, prefs: &Prefs, text: &str) -> Found {
    let mut refused = None;
    let mut sides: Vec<(Kind, Request, Vec<Hit>)> = Vec::new();
    let mut read = 0;

    for kind in Kind::ALL {
        match grammar::parse(text, kind) {
            Ok(request) => {
                let request = settled(request, |name| names(library, kind, name));
                let found = hits(library, kind, &request.unit, request.search.unwrap_or(fuzzy::CLOSENESS), prefs);

                read += 1;

                if !found.is_empty() {
                    sides.push((kind, request, found));
                }
            }
            Err(reason) => refused = refused.or(Some(reason)),
        }
    }

    if let (0, Some(reason)) = (read, refused) {
        return Found::Unread(reason);
    }

    let searched = sides.iter().any(|(_, request, _)| request.search.is_some());
    let top = |found: &[Hit]| found.first().map(|hit| hit.score);
    let best = sides.iter().filter_map(|(_, _, found)| top(found)).max();

    if !searched {
        sides.retain(|(_, _, found)| top(found) == best);
    }

    if !searched && sides.len() == 1 {
        return sides.pop().map_or(Found::Nothing, |(kind, request, _)| Found::One(kind, request));
    }

    let Some(asked) = sides.first().map(|(_, request, _)| match &request.unit {
        Unit::Name(name) => name.clone(),
        Unit::Id(id, _) => id.to_string(),
    }) else {
        return Found::Nothing;
    };
    let mut tails = Vec::with_capacity(sides.len());
    let mut hits = Vec::new();

    for (kind, request, found) in sides {
        tails.push((kind, request.tail));
        hits.extend(found);
    }

    if !searched {
        hits.retain(|hit| Some(hit.score) == best);
    }

    hits.sort_by_key(|hit| Reverse(hit.score));

    Found::Several { asked, searched, tails, hits }
}
