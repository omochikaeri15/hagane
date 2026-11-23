use std::cmp::Reverse;

use foundry::domains::cat::scanner::CatEntry;
use foundry::domains::enemy::scanner::EnemyEntry;
use foundry::domains::stage::names;

use super::fuzzy;
use super::grammar::{self, Kind, Request, Stats, Unit, Want};
use super::library::{Area, Library, Staged};
use super::prefs::{FORMS, Prefs};

pub(crate) const CROWN_GLYPH: &str = "\u{1F732}";

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
        Some(whole) if named(&whole) => Request { unit: Unit::Name(whole), want: Want::Stats(Stats::default()), whole: None, form: request.form, crowns: request.crowns, search: request.search, tail: String::new() },
        _ => request,
    }
}

pub(crate) fn names(library: &Library, kind: Kind, name: &str) -> bool {
    match kind {
        Kind::Cat => library.names_cat(name),
        Kind::Enemy => library.names_enemy(name),
        Kind::Stage => library.names_stage(name),
        Kind::Map => library.names_map(name),
    }
}

pub(crate) fn hits(library: &Library, kind: Kind, unit: &Unit, closeness: u32, prefs: &Prefs) -> Vec<Hit> {
    match (kind, unit) {
        (Kind::Cat, Unit::Id(id, form)) => library.cat(*id).map(|cat| cat_hit(cat, form.unwrap_or_else(|| prefs.form_from(&cat.forms)), fuzzy::EXACT)).into_iter().collect(),
        (Kind::Cat, Unit::Name(name)) => library.search_cats(name, closeness).into_iter().map(|(cat, form, score)| cat_hit(cat, form, score)).collect(),
        (Kind::Enemy, Unit::Id(id, _)) => library.enemy(*id).map(|enemy| enemy_hit(enemy, fuzzy::EXACT)).into_iter().collect(),
        (Kind::Enemy, Unit::Name(name)) => library.search_enemies(name, closeness).into_iter().map(|(enemy, score)| enemy_hit(enemy, score)).collect(),
        (Kind::Stage, Unit::Code(code)) => library.stage(code).map(|found| stage_hit(library, &found, fuzzy::EXACT)).into_iter().collect(),
        (Kind::Stage, Unit::Name(name)) => library.search_stages(name, closeness).into_iter().map(|(found, score)| stage_hit(library, &found, score)).collect(),
        (Kind::Map, Unit::Code(code)) => library.map(code).map(|area| map_hit(&area, fuzzy::EXACT)).into_iter().collect(),
        (Kind::Map, Unit::Name(name)) => library.search_maps(name, closeness).into_iter().map(|(area, score)| map_hit(&area, score)).collect(),
        (Kind::Cat | Kind::Enemy, Unit::Code(_)) | (Kind::Stage | Kind::Map, Unit::Id(..)) => Vec::new(),
    }
}

pub(crate) fn among(library: &Library, kinds: &[Kind], unit: &Unit, closeness: u32, prefs: &Prefs) -> Vec<Hit> {
    let mut found: Vec<Hit> = kinds.iter().flat_map(|kind| hits(library, *kind, unit, closeness, prefs)).collect();

    found.sort_by_key(|hit| Reverse(hit.score));
    found
}

fn cat_hit(cat: &CatEntry, form: usize, score: u32) -> Hit {
    Hit { kind: Kind::Cat, value: format!("{}-{}", cat.id, form + 1), label: cat.display_name(form), note: format!("{} • {}", cat.id_str(form), FORMS.get(form).copied().unwrap_or_default()), score }
}

fn enemy_hit(enemy: &EnemyEntry, score: u32) -> Hit {
    Hit { kind: Kind::Enemy, value: enemy.id.to_string(), label: enemy.display_name(), note: enemy.id_str(), score }
}

fn map_hit(area: &Area<'_>, score: u32) -> Hit {
    let crowns = format!("{}{CROWN_GLYPH}", area.map.max_crowns.max(1));
    let invasion = area.invasion().then_some("Invasion");
    let note: Vec<&str> = [Some(names::category_name(area.category())), invasion, Some(crowns.as_str())].into_iter().flatten().collect();

    Hit { kind: Kind::Map, value: area.code(), label: area.name().to_owned(), note: note.join(" • "), score }
}

pub(crate) fn stage_line<'a>(library: &'a Library, found: &Staged<'a>) -> (&'a str, Option<&'static str>) {
    let seat = found.host.map_or(found.stage, |(_, host)| host);
    let variant = match (found.zombie(), found.invasion()) {
        (true, true) => Some("Outbreak Invasion"),
        (true, false) => Some("Outbreak"),
        (false, true) => Some("Invasion"),
        (false, false) => None,
    };

    (library.stage_map(seat).map_or("", |map| map.name.as_str()), variant)
}

fn stage_hit(library: &Library, found: &Staged<'_>, score: u32) -> Hit {
    let (map, variant) = stage_line(library, found);
    let crowns = format!("{}{CROWN_GLYPH}", found.stage.max_crowns.max(1));
    let note: Vec<&str> = [Some(map).filter(|map| !map.is_empty()), variant, Some(crowns.as_str())].into_iter().flatten().collect();

    Hit { kind: Kind::Stage, value: found.code(), label: found.name().to_owned(), note: note.join(" • "), score }
}

pub(crate) fn across(library: &Library, prefs: &Prefs, text: &str) -> Found {
    let mut refused = None;
    let mut sides: Vec<(Kind, Request, Vec<Hit>)> = Vec::new();
    let mut crowned = false;

    for kind in Kind::ALL {
        match grammar::parse(text, kind) {
            Ok(request) => {
                let request = settled(request, |name| names(library, kind, name));
                let found = hits(library, kind, &request.unit, request.search.unwrap_or(fuzzy::CLOSENESS), prefs);

                crowned |= request.crowns.is_some();

                if !found.is_empty() {
                    sides.push((kind, request, found));
                }
            }
            Err(reason) => refused = refused.or(Some(reason)),
        }
    }

    if let (true, false, Some(reason)) = (sides.is_empty(), crowned, refused) {
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
        Unit::Name(name) | Unit::Code(name) => name.clone(),
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
