use std::borrow::Cow;
use std::mem;

use codex::chapter::Category;
use codex::combat::{Entity, Foe, Hits, Identity, Mode as Gauge, Switch, Switches};
use foundry::systems::combat::lens::Lens;

use super::fuzzy::CLOSENESS;
use super::prefs::Mode;
use super::render::{Clip, Format};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Cat,
    Enemy,
    Stage,
    Map,
}

impl Kind {
    pub(crate) const ALL: [Kind; 4] = [Kind::Cat, Kind::Enemy, Kind::Stage, Kind::Map];
    pub(crate) const PLACES: [Kind; 2] = [Kind::Stage, Kind::Map];

    pub(crate) fn tag(self) -> &'static str {
        match self {
            Kind::Cat => "c",
            Kind::Enemy => "e",
            Kind::Stage => "s",
            Kind::Map => "m",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Kind::Cat => "Cat",
            Kind::Enemy => "Enemy",
            Kind::Stage => "Stage",
            Kind::Map => "Map",
        }
    }
}

#[derive(Debug, PartialEq)]
pub(crate) enum Unit {
    Id(u32, Option<usize>),
    Name(String),
    Code(String),
}

#[derive(Debug, PartialEq)]
pub(crate) enum Talent {
    All,
    Standard,
    Ultra,
    None,
    Level(u32),
    Named(String, Option<u32>),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Page {
    #[default]
    Stats,
    Talents,
    Details,
}

impl Page {
    pub(crate) fn tag(self) -> &'static str {
        match self {
            Page::Stats => "s",
            Page::Talents => "t",
            Page::Details => "d",
        }
    }

    pub(crate) fn tagged(tag: &str) -> Option<Self> {
        [Page::Stats, Page::Talents, Page::Details].into_iter().find(|page| page.tag() == tag)
    }
}

enum Asked {
    Page(Page),
    Motion,
}

#[derive(Debug, Default, PartialEq)]
pub(crate) struct Stats {
    pub(crate) page: Page,
    pub(crate) mode: Option<Mode>,
    pub(crate) level: Option<String>,
    pub(crate) magnification: Option<(i32, i32)>,
    pub(crate) talents: Option<Vec<Talent>>,
    pub(crate) versus: Option<Vec<String>>,
    pub(crate) gauge: Option<Gauge>,
    pub(crate) hits: Option<Hits>,
    pub(crate) switches: Switches,
    pub(crate) orbs: [Option<String>; 2],
}

pub(crate) enum Lookup<'a> {
    Named(&'a str),
    Close(&'a str),
    Numbered(u32),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Versus {
    pub(crate) foe: Foe,
    pub(crate) aimed: bool,
}

impl Versus {
    pub(crate) fn is_empty(self) -> bool {
        self.foe.is_empty() && !self.aimed
    }

    pub(crate) fn against(self, stats: &Entity) -> Foe {
        if self.aimed { self.foe.union(stats.answered().without(Identity::TraitMetal)) } else { self.foe }
    }

    pub(crate) fn bits(self) -> u32 {
        u32::from(self.foe.bits()) | if self.aimed { AIMED } else { 0 }
    }

    pub(crate) fn from_bits(bits: u32) -> Self {
        Self { foe: Foe::from_bits(u16::try_from(bits & u32::from(u16::MAX)).unwrap_or_default()), aimed: bits & AIMED != 0 }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Overlays {
    pub(crate) combat: bool,
    pub(crate) rig: bool,
    pub(crate) world: bool,
    pub(crate) origin: bool,
}

impl Overlays {
    pub(crate) fn any(self) -> bool {
        self.combat || self.rig || self.world || self.origin
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Looping {
    pub(crate) minimum: i32,
    pub(crate) maximum: Option<i32>,
    pub(crate) difference: i32,
}

impl Default for Looping {
    fn default() -> Self {
        Self { minimum: LOOP_MINIMUM, maximum: None, difference: LOOP_DIFFERENCE }
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct Motion {
    pub(crate) clip: Clip,
    pub(crate) range: Option<(i32, i32)>,
    pub(crate) format: Option<Format>,
    pub(crate) cull: Option<i32>,
    pub(crate) overlays: Overlays,
    pub(crate) looping: Option<Looping>,
    pub(crate) background: bool,
    pub(crate) quality: Option<u32>,
    pub(crate) compression: Option<u32>,
}

#[derive(Debug, PartialEq)]
pub(crate) enum Tune {
    Track(u32, String),
    Place(Box<Request>, bool),
}

#[derive(Debug, PartialEq)]
pub(crate) enum Backing {
    Id(u32, String),
    Place(Box<Request>),
}

#[derive(Debug, PartialEq)]
pub(crate) struct Scenery {
    pub(crate) subject: Backing,
    pub(crate) seed: Option<u32>,
    pub(crate) range: Option<(i32, i32)>,
    pub(crate) offset: Option<(i32, i32)>,
    pub(crate) size: Option<(u32, u32)>,
    pub(crate) format: Option<Format>,
    pub(crate) quality: Option<u32>,
    pub(crate) compression: Option<u32>,
}

#[derive(Debug, PartialEq)]
pub(crate) enum Want {
    Stats(Stats),
    Motion(Motion),
}

#[derive(Debug, PartialEq)]
pub(crate) struct Request {
    pub(crate) unit: Unit,
    pub(crate) want: Want,
    pub(crate) whole: Option<String>,
    pub(crate) form: Option<usize>,
    pub(crate) crowns: Option<u8>,
    pub(crate) search: Option<u32>,
    pub(crate) tail: String,
}

impl Request {
    pub(crate) fn mode(&self) -> Option<Mode> {
        match &self.want {
            Want::Stats(stats) => stats.mode,
            Want::Motion(_) => None,
        }
    }
}

const MODE_HINT: &str = "`--dormant` and `--interactive` go with a cat, an enemy, a stage or a map, so leave them out here";
const MAX_CULL: i32 = 100;
const MAX_PERCENT: i64 = 100;
const MAX_CLOSENESS: u32 = 100;
const MAX_CROWNS: u8 = 4;
const GRAFT: &str = "i";
const CROWN_HINT: &str = "`--crowns` (`-c`) takes a number from 1 to 4";
const LOOP_MINIMUM: i32 = 15;
const LOOP_DIFFERENCE: i32 = 30;
const DASHES: [char; 2] = ['—', '–'];

fn lower(token: &str) -> String {
    token.to_lowercase()
}

fn spelled(text: &str) -> Cow<'_, str> {
    if !text.contains(DASHES) {
        return Cow::Borrowed(text);
    }

    let mut spelled = String::with_capacity(text.len() + 2);
    let mut chars = text.chars().peekable();
    let mut before = None;

    while let Some(glyph) = chars.next() {
        if DASHES.contains(&glyph) && before.is_none_or(char::is_whitespace) && chars.peek().is_some_and(|next| next.is_alphabetic()) {
            spelled.push_str("--");
        } else {
            spelled.push(glyph);
        }

        before = Some(glyph);
    }

    Cow::Owned(spelled)
}

fn is_flag(token: &str) -> bool {
    token.starts_with("--") || token.strip_prefix('-').is_some_and(|rest| rest.chars().next().is_some_and(char::is_alphabetic))
}

const TRACK_HINT: &str = "Give me a track's ID, like `5`, or the name of a stage or a map";
const BOSS_HINT: &str = "`--boss` (`-b`) goes with a stage or a map, not a track's ID";
const SCENERY_HINT: &str = "Give me a background's ID first, like `12`, or the name of a stage or a map";
const OFFSET_HINT: &str = "`--offset` takes two whole numbers, like `100 50` or `-200,0`";
const SIZE_HINT: &str = "`--dimensions` takes a width and a height, like `800x600`, `800×600` or `800 600`";
const PAIR_MARKS: [char; 5] = ['x', 'X', '×', ',', '*'];
const SEARCH_HINT: &str = "`--search` (`-s`) looks a name up, so give it a name instead of an ID";
const ENEMY_MARK: &str = "e";
const VERSUS_HINT: &str = "`--versus` (`-v`) takes trait names like `red metal`, an enemy's name, `targets`, `traits`, `subtraits` or `all`";
const EVERY: &str = "all";
pub(crate) const TARGETS: &str = "targets";
const OWN: [&str; 3] = [TARGETS, "target", "targeted"];
const PLAIN: [&str; 2] = ["traits", "trait"];
const SUBTLE: [&str; 2] = ["subtraits", "subtrait"];
const SUBTRAITS: [Identity; 5] = [Identity::TraitWitch, Identity::TraitEva, Identity::TraitColossus, Identity::TraitBehemoth, Identity::TraitSage];
const AIMED: u32 = 1 << 16;
const NO_TRAITS: &str = "none";
const TRAITS: [(Identity, &[&str]); 15] = [
    (Identity::TraitRed, &["red"]),
    (Identity::TraitFloating, &["floating", "float"]),
    (Identity::TraitDark, &["black", "dark"]),
    (Identity::TraitMetal, &["metal"]),
    (Identity::TraitAngel, &["angel"]),
    (Identity::TraitAlien, &["alien"]),
    (Identity::TraitZombie, &["zombie"]),
    (Identity::TraitRelic, &["relic"]),
    (Identity::TraitAku, &["aku"]),
    (Identity::TraitTraitless, &["traitless", "white"]),
    (Identity::TraitWitch, &["witch"]),
    (Identity::TraitEva, &["eva"]),
    (Identity::TraitColossus, &["colossus", "baron"]),
    (Identity::TraitBehemoth, &["behemoth"]),
    (Identity::TraitSage, &["sage"]),
];
const SWITCH_HINT: &str = "`--on` and `--off` take crit, savage, wave, surge, explosion, strengthen, kills, weaken, orb 1 or orb 2";
const ORB_HINT: &str = "`--orb1` (`-1`) and `--orb2` (`-2`) take a grade, a trait and an effect, like `-1 S Red Attack Up`";
const GAUGE_HINT: &str = "`--mode` (`-m`) takes base, avg or max, or b, a or m";
const HITS_HINT: &str = "`--hits` (`-h`) takes 1, 2, 3, avg or all";
const FORM_HINT: &str = "`form` (`f`) takes 1-4, n e t u, f c s u, or Normal, Evolved, True, Ultra";

fn orb_slot(token: &str) -> Option<usize> {
    match lower(token).as_str() {
        "--orb" | "--orb1" | "-1" => Some(0),
        "--orb2" | "-2" => Some(1),
        _ => None,
    }
}

fn breaks(token: &str, kind: Kind) -> bool {
    is_flag(token) || (kind == Kind::Cat && orb_slot(token).is_some())
}

fn versus_flag(token: &str) -> bool {
    matches!(lower(token).as_str(), "--versus" | "-v")
}

fn lens_flag(token: &str) -> bool {
    matches!(lower(token).as_str(), "--mode" | "-m" | "--hits" | "-h" | "--on" | "--off")
}

const SWITCH_FILLER: [&str; 5] = ["mini", "blow", "hit", "attack", "up"];

pub(crate) fn switch_of(word: &str) -> Option<Switch> {
    match lower(word).trim_end_matches('s') {
        "crit" | "critical" => Some(Switch::Critical),
        "savage" => Some(Switch::Savage),
        "wave" | "miniwave" => Some(Switch::Wave),
        "surge" | "minisurge" => Some(Switch::Surge),
        "explosion" | "blast" => Some(Switch::Explosion),
        "strengthen" | "strengthened" => Some(Switch::Strengthen),
        "kill" => Some(Switch::Kills),
        "weaken" | "weakened" => Some(Switch::Weaken),
        "orb1" => Some(Switch::Orb1),
        "orb2" => Some(Switch::Orb2),
        _ => None,
    }
}

fn switched(switches: Switches, words: &[&str], lit: bool) -> Result<Switches, String> {
    let mut held = switches;
    let mut words = words.iter().map(|word| lower(word)).filter(|word| !SWITCH_FILLER.contains(&word.as_str())).peekable();

    while let Some(word) = words.next() {
        let word = match (word.as_str(), words.peek().map(String::as_str)) {
            ("orb", Some(slot @ ("1" | "2"))) => {
                let joined = format!("orb{slot}");

                words.next();
                joined
            }
            _ => word,
        };

        held = held.set(switch_of(&word).ok_or_else(|| SWITCH_HINT.to_owned())?, lit);
    }

    Ok(held)
}

pub(crate) fn gauge_of(word: &str) -> Option<Gauge> {
    match lower(word).as_str() {
        "base" | "b" => Some(Gauge::Base),
        "avg" | "average" | "a" => Some(Gauge::Average),
        "max" | "maximum" | "m" => Some(Gauge::Maximum),
        _ => None,
    }
}

pub(crate) fn hits_of(word: &str) -> Option<Hits> {
    match lower(word).as_str() {
        "1" | "1st" | "first" => Some(Hits::Only(0)),
        "2" | "2nd" | "second" => Some(Hits::Only(1)),
        "3" | "3rd" | "third" => Some(Hits::Only(2)),
        "avg" | "average" => Some(Hits::Standing),
        "all" => Some(Hits::All),
        _ => None,
    }
}

pub(crate) const SWITCH_WORDS: [(Switch, &str); 10] = [
    (Switch::Critical, "crit"),
    (Switch::Savage, "savage"),
    (Switch::Wave, "wave"),
    (Switch::Surge, "surge"),
    (Switch::Explosion, "explosion"),
    (Switch::Strengthen, "strengthen"),
    (Switch::Kills, "kills"),
    (Switch::Weaken, "weaken"),
    (Switch::Orb1, "orb1"),
    (Switch::Orb2, "orb2"),
];

pub(crate) fn switch_word(switch: Switch) -> &'static str {
    SWITCH_WORDS.iter().find(|(held, _)| *held == switch).map_or("", |(_, word)| word)
}

pub(crate) fn hits_word(hits: Hits) -> &'static str {
    match hits {
        Hits::Only(0) => "1",
        Hits::Only(1) => "2",
        Hits::Only(_) => "3",
        Hits::Standing => "avg",
        Hits::All => "all",
    }
}

pub(crate) fn gauge_word(mode: Gauge) -> &'static str {
    match mode {
        Gauge::Base => "base",
        Gauge::Average => "avg",
        Gauge::Maximum => "max",
    }
}

pub(crate) fn lens_tag(lens: Lens) -> String {
    let gauge = match lens.mode {
        Gauge::Base => 'b',
        Gauge::Average => 'a',
        Gauge::Maximum => 'x',
    };
    let hits = match lens.hits {
        Hits::Only(0) => '1',
        Hits::Only(1) => '2',
        Hits::Only(_) => '3',
        Hits::Standing => 's',
        Hits::All => 'l',
    };

    match lens.switches.bits() {
        (0, 0) => format!("{gauge}{hits}"),
        (on, off) => format!("{gauge}{hits}{on:x}.{off:x}"),
    }
}

pub(crate) fn lens_tagged(tag: &str) -> Option<Lens> {
    let mut chars = tag.chars();
    let mode = match chars.next()? {
        'b' => Gauge::Base,
        'a' => Gauge::Average,
        'x' => Gauge::Maximum,
        _ => return None,
    };
    let hits = match chars.next()? {
        '1' => Hits::Only(0),
        '2' => Hits::Only(1),
        '3' => Hits::Only(2),
        's' => Hits::Standing,
        'l' => Hits::All,
        _ => return None,
    };
    let rest = chars.as_str();
    let switches = if rest.is_empty() {
        Switches::default()
    } else {
        let (on, off) = rest.split_once('.')?;

        Switches::from_bits(u16::from_str_radix(on, 16).ok()?, u16::from_str_radix(off, 16).ok()?)
    };

    Some(Lens { mode, hits, switches, switching: false })
}

pub(crate) fn trait_of(word: &str) -> Option<Identity> {
    let named = |test: &dyn Fn(&str) -> bool| -> Vec<Identity> { TRAITS.iter().filter(|(_, names)| names.iter().any(|name| test(name))).map(|(label, _)| *label).collect() };
    let exact = named(&|name| name == word);
    let begun = named(&|name| name.starts_with(word));

    match (exact.as_slice(), begun.as_slice()) {
        ([label], _) | ([], [label]) => Some(*label),
        _ => None,
    }
}

fn group(subtle: Option<bool>) -> Foe {
    let kept = |label: &Identity| *label != Identity::TraitMetal && subtle.is_none_or(|subtle| SUBTRAITS.contains(label) == subtle);

    Foe::LABELS.into_iter().filter(kept).fold(Foe::default(), Foe::with)
}

fn read_word(versus: Versus, word: &str) -> Option<Versus> {
    let joined = |foe: Foe| Some(Versus { foe: versus.foe.union(foe), ..versus });

    match word {
        NO_TRAITS => Some(Versus::default()),
        EVERY => joined(group(None)),
        _ if PLAIN.contains(&word) => joined(group(Some(false))),
        _ if SUBTLE.contains(&word) => joined(group(Some(true))),
        _ if OWN.contains(&word) => Some(Versus { aimed: true, ..versus }),
        _ => trait_of(word).and_then(|label| joined(Foe::default().with(label))),
    }
}

fn settled(versus: Versus, pending: &mut Vec<&str>, enemy: &dyn Fn(Lookup<'_>) -> Option<Foe>) -> Result<Versus, String> {
    if pending.is_empty() {
        return Ok(versus);
    }

    let name = pending.join(" ");

    pending.clear();
    enemy(Lookup::Close(&name)).map(|foe| Versus { foe: versus.foe.union(foe), ..versus }).ok_or(name)
}

pub(crate) fn versus_of(tokens: &[&str], enemy: &dyn Fn(Lookup<'_>) -> Option<Foe>) -> Result<Versus, String> {
    let words: Vec<String> = tokens.iter().flat_map(|token| token.split(',')).map(lower).filter(|word| !word.is_empty()).collect();
    let mut versus = Versus::default();
    let mut pending: Vec<&str> = Vec::new();
    let mut at = 0;

    while let Some(word) = words.get(at).map(String::as_str) {
        let spanned = (at + 2..=words.len()).rev().find_map(|end| enemy(Lookup::Named(&words[at..end].join(" "))).map(|foe| (Some(foe), end - at)));
        let single = || enemy_id(word).and_then(|id| enemy(Lookup::Numbered(id))).or_else(|| enemy(Lookup::Named(word)));
        let (foe, step) = match spanned {
            Some(found) => found,
            None if read_word(Versus::default(), word).is_some() => (None, 1),
            None => match single() {
                Some(foe) => (Some(foe), 1),
                None => {
                    pending.push(word);
                    at += 1;
                    continue;
                }
            },
        };

        versus = settled(versus, &mut pending, enemy)?;
        versus = match foe {
            Some(foe) => Versus { foe: versus.foe.union(foe), ..versus },
            None => read_word(versus, word).unwrap_or(versus),
        };
        at += step;
    }

    settled(versus, &mut pending, enemy)
}

pub(crate) fn trait_word(label: Identity) -> &'static str {
    TRAITS.iter().find(|(held, _)| *held == label).and_then(|(_, names)| names.first().copied()).unwrap_or_default()
}

pub(crate) fn versus_picked(values: &[String]) -> Versus {
    values.iter().fold(Versus::default(), |versus, value| read_word(versus, value).unwrap_or(versus))
}

fn search_flag(token: &str) -> bool {
    matches!(lower(token).as_str(), "--search" | "-s")
}

fn form_word(token: &str) -> bool {
    matches!(lower(token).as_str(), "form" | "f")
}

fn form_value(token: &str) -> Option<usize> {
    match lower(token).as_str() {
        "1" | "f" | "n" | "normal" => Some(0),
        "2" | "c" | "e" | "evolved" => Some(1),
        "3" | "s" | "t" | "true" => Some(2),
        "4" | "u" | "ultra" => Some(3),
        _ => None,
    }
}

fn clip_word(token: &str) -> bool {
    !is_flag(token) && token.chars().count() > 1 && clip_of(token).is_some()
}

fn motion_flag(token: &str) -> bool {
    matches!(
        lower(token).as_str(),
        "--range" | "-r" | "--loop" | "-l" | "--format" | "-f" | "--cull" | "-u" | "--combat" | "-c" | "--rig" | "-g" | "--world" | "-w" | "--origin" | "-o" | "--background" | "-b" | "--quality" | "-q" | "--compress" | "--compression" | "-p"
    )
}

fn marked_level(token: &str) -> bool {
    level_of(token).is_some() && (token.contains('+') || !token.chars().all(|glyph| glyph.is_ascii_digit()))
}

fn plain_number(token: &str) -> bool {
    !token.is_empty() && token.chars().all(|glyph| glyph.is_ascii_digit())
}

fn level_word(token: &str) -> bool {
    matches!(lower(token).as_str(), "lv" | "lvl" | "l" | "level")
}

fn stats_tell(token: &str, kind: Kind) -> bool {
    let marked_level = marked_level(token);
    let marked_percent = kind == Kind::Enemy && token.contains('%') && magnification_of(token).is_some();

    matches!(lower(token).as_str(), "--talents" | "-t" | "lv" | "lvl" | "level") || (kind == Kind::Cat && orb_slot(token).is_some()) || mode_of(token).is_some() || lens_flag(token) || (kind == Kind::Cat && versus_flag(token)) || (kind == Kind::Cat && marked_level) || marked_percent
}

fn mode_of(token: &str) -> Option<Mode> {
    match lower(token).as_str() {
        "--dormant" | "-d" => Some(Mode::Dormant),
        "--interactive" | "-i" => Some(Mode::Interactive),
        _ => None,
    }
}

fn inferred(rest: &[&str], kind: Kind) -> Result<bool, String> {
    let motion = rest.first().is_some_and(|token| !is_flag(token) && clip_of(token).is_some()) || rest.iter().any(|token| motion_flag(token));
    let stats = rest.iter().any(|token| stats_tell(token, kind) && !(motion && orb_slot(token).is_some()));

    match (motion, stats) {
        (true, true) => Err("That mixes stats and animation options; add `s` or `a` to say which".to_owned()),
        (motion, _) => Ok(motion),
    }
}

fn picks_a_form(rest: &[&str], kind: Kind) -> bool {
    kind == Kind::Cat && rest.first().is_some_and(|token| form_word(token)) && rest.get(1).is_some_and(|value| form_value(value).is_some())
}

fn bare_keyword(token: &str, kind: Kind) -> bool {
    matches!(lower(token).as_str(), "a" | "s") || matches!(want_of(token, kind), Some(Asked::Page(Page::Talents | Page::Details)))
}

fn carries_on(rest: &[&str], kind: Kind) -> bool {
    match rest.split_first() {
        Some((keyword, after)) => match want_of(keyword, kind) {
            Some(Asked::Motion) => motion_of(after).is_ok(),
            Some(Asked::Page(Page::Talents)) => talented(after).is_ok(),
            Some(Asked::Page(_)) => {
                let marked: Vec<&str> = after.iter().copied().filter(|token| kind != Kind::Cat || !plain_number(token)).collect();

                stats_of(&marked, kind).is_ok()
            }
            None => false,
        },
        None => false,
    }
}

fn want_of(token: &str, kind: Kind) -> Option<Asked> {
    match lower(token).as_str() {
        "stats" | "stat" | "s" => Some(Asked::Page(Page::Stats)),
        "talents" | "talent" | "t" if kind == Kind::Cat => Some(Asked::Page(Page::Talents)),
        "details" | "detail" | "description" | "d" => Some(Asked::Page(Page::Details)),
        "animation" | "anim" | "a" => Some(Asked::Motion),
        _ => None,
    }
}

pub(crate) fn clip_of(token: &str) -> Option<Clip> {
    match lower(token).as_str() {
        "attack" | "atk" | "a" => Some(Clip::Attack),
        "idle" | "i" => Some(Clip::Idle),
        "knockback" | "kb" | "k" => Some(Clip::Knockback),
        "walk" | "w" => Some(Clip::Walk),
        "showcase" | "sh" | "sc" => Some(Clip::Showcase),
        "burrow" | "bur" | "b" => Some(Clip::Burrow),
        "surface" | "su" | "sf" | "rise" | "r" => Some(Clip::Surface),
        "dig" | "d" => Some(Clip::Dig),
        "spirit" | "conjure" | "sp" | "c" => Some(Clip::Spirit),
        "soul" | "s" => Some(Clip::Soul),
        "model" | "m" => Some(Clip::Model),
        _ => None,
    }
}

fn ends_a_name(token: &str, kind: Kind) -> bool {
    let marked_level = marked_level(token);
    let marked_percent = token.contains('%') && magnification_of(token).is_some();

    let level_word = matches!(lower(token).as_str(), "lv" | "lvl" | "level");

    want_of(token, kind).is_some() || breaks(token, kind) || marked_level || marked_percent || level_word
}

fn enemy_id(token: &str) -> Option<u32> {
    let (id, mark) = token.split_once(['-', '_'])?;

    (lower(mark) == ENEMY_MARK).then(|| id.parse::<u32>().ok()).flatten()
}

pub(crate) fn enemy_number(text: &str) -> Option<u32> {
    text.parse().ok().or_else(|| enemy_id(text))
}

pub(crate) fn names_an_enemy(text: &str) -> bool {
    text.split_whitespace().next().is_some_and(|token| enemy_id(token).is_some())
}

fn unit_of(token: &str, kind: Kind) -> Option<Unit> {
    if let Ok(id) = token.parse::<u32>() {
        return Some(Unit::Id(id, None));
    }

    match (kind, enemy_id(token)) {
        (Kind::Enemy, id) => id.map(|id| Unit::Id(id, None)),
        (_, Some(_)) => None,
        (_, None) => {
            let (id, form) = token.split_once(['-', '_'])?;

            Some(Unit::Id(id.parse::<u32>().ok()?, Some(form_value(form)?)))
        }
    }
}

pub(crate) fn level_of(token: &str) -> Option<String> {
    let lowered = lower(token);
    let digits = ["level", "lvl", "lv", "l"].iter().find_map(|prefix| lowered.strip_prefix(prefix)).unwrap_or(&lowered);
    let valid = digits.split('+').all(|term| !term.is_empty() && term.chars().all(|glyph| glyph.is_ascii_digit()));

    valid.then(|| digits.to_owned())
}

fn percent_of(token: &str) -> Option<i32> {
    token.trim_end_matches('%').parse::<i32>().ok().filter(|percent| *percent > 0)
}

pub(crate) fn magnification_of(token: &str) -> Option<(i32, i32)> {
    match token.split_once('/') {
        Some((hitpoints, attack)) => Some((percent_of(hitpoints)?, percent_of(attack)?)),
        None => percent_of(token).map(|percent| (percent, percent)),
    }
}

fn range_of(tokens: &[&str], at: &mut usize) -> Result<(i32, i32), String> {
    let first = tokens.get(*at).ok_or("`--range` needs a frame or two, like `12`, `0~30` or `0 30`")?;

    if let Some((from, to)) = first.split_once('~') {
        *at += 1;

        return match (from.parse::<i32>(), to.parse::<i32>()) {
            (Ok(from), Ok(to)) => Ok((from, to)),
            _ => Err(format!("`{first}` is not a frame range")),
        };
    }

    let from = first.parse::<i32>().map_err(|_| format!("`{first}` is not a frame range"))?;
    let to = tokens.get(*at + 1).and_then(|second| second.parse::<i32>().ok());

    *at += 1 + usize::from(to.is_some());

    Ok((from, to.unwrap_or(from)))
}

pub(crate) fn talents_of(tokens: &[&str]) -> Vec<Talent> {
    let mut talents = Vec::new();
    let mut name: Vec<String> = Vec::new();

    for token in tokens {
        let preset = match lower(token).as_str() {
            "all" | "a" => Some(Talent::All),
            "standard" | "s" => Some(Talent::Standard),
            "ultra" | "u" => Some(Talent::Ultra),
            "none" | "n" => Some(Talent::None),
            _ => None,
        };

        match (token.parse::<u32>(), preset) {
            (Ok(level), _) if name.is_empty() => talents.push(Talent::Level(level)),
            (Ok(level), _) => talents.push(Talent::Named(mem::take(&mut name).join(" "), Some(level))),
            (Err(_), Some(preset)) if name.is_empty() => talents.push(preset),
            _ => name.push(token.replace(['-', '_'], " ")),
        }
    }

    if !name.is_empty() {
        talents.push(Talent::Named(name.join(" "), None));
    }

    talents
}

fn sifted(text: &str) -> (Vec<&str>, Option<u32>) {
    let typed: Vec<&str> = text.split_whitespace().collect();
    let flagged = |at: usize| at > 0 && typed.get(at).is_some_and(|token| search_flag(token));
    let closeness = |at: usize| typed.get(at).and_then(|token| token.trim_end_matches('%').parse::<u32>().ok());
    let search = (1..typed.len()).find(|at| flagged(*at)).map(|at| closeness(at + 1).map_or(CLOSENESS, |typed| typed.clamp(1, MAX_CLOSENESS)));
    let tokens = typed.iter().enumerate().filter(|(at, _)| !flagged(*at) && !(flagged(at.wrapping_sub(1)) && closeness(*at).is_some())).map(|(_, token)| *token).collect();

    (tokens, search)
}

pub(crate) fn parse(text: &str, kind: Kind) -> Result<Request, String> {
    let text = spelled(text);
    let (tokens, search) = sifted(&text);

    if matches!(kind, Kind::Stage | Kind::Map) {
        return placed(&tokens, search, &[kind]);
    }

    if let Some(token) = tokens.first().filter(|token| kind == Kind::Cat && enemy_id(token).is_some()) {
        return Err(format!("`{token}` is an enemy's ID, so ask `enemy` for it. A cat's Evolved form is `-2` or `-c`"));
    }

    let mut at = 0;
    let unit = match tokens.first().and_then(|token| unit_of(token, kind)) {
        Some(unit) => {
            at = 1;
            unit
        }
        None => {
            while let Some(token) = tokens.get(at) {
                let clip_ends = clip_word(token) && motion_of(&tokens[at..]).is_ok();

                if picks_a_form(&tokens[at..], kind) || clip_ends || (ends_a_name(token, kind) && !(bare_keyword(token, kind) && !carries_on(&tokens[at..], kind))) {
                    break;
                }

                at += 1;
            }

            Unit::Name(tokens[..at].join(" "))
        }
    };
    let whole = match &unit {
        Unit::Name(name) if !name.is_empty() && at + 1 == tokens.len() && (bare_keyword(tokens[at], kind) || clip_word(tokens[at])) => Some(tokens.join(" ")),
        _ => None,
    };

    if matches!(&unit, Unit::Name(name) if name.is_empty()) {
        return Err("Name a unit first, by ID or by name".to_owned());
    }

    if search.is_some() && matches!(unit, Unit::Id(..)) {
        return Err(SEARCH_HINT.to_owned());
    }

    let tail = tokens[at..].join(" ");

    let mut form = None;

    if kind == Kind::Cat && tokens.get(at).is_some_and(|token| form_word(token)) {
        form = Some(tokens.get(at + 1).and_then(|value| form_value(value)).ok_or(FORM_HINT)?);
        at += 2;
    }

    let asked = match tokens.get(at).and_then(|token| want_of(token, kind)) {
        Some(asked) => {
            at += 1;
            asked
        }
        None if inferred(&tokens[at..], kind)? => Asked::Motion,
        None => Asked::Page(Page::Stats),
    };
    let rest = &tokens[at..];

    let want = match asked {
        Asked::Motion => Want::Motion(motion_of(rest)?),
        Asked::Page(Page::Talents) => Want::Stats(Stats { page: Page::Talents, ..talented(rest)? }),
        Asked::Page(page) => Want::Stats(Stats { page, ..stats_of(rest, kind)? }),
    };

    Ok(Request { unit, want, whole, form, crowns: None, search, tail })
}

fn crown_flag(token: &str) -> bool {
    matches!(lower(token).as_str(), "--crowns" | "--crown" | "-c")
}

pub(crate) fn code_of(named: &[&str], kind: Kind) -> Option<String> {
    let joined = named.join(" ");
    let mut parts = joined.split(|glyph: char| glyph.is_whitespace() || matches!(glyph, '-' | '_')).filter(|part| !part.is_empty());
    let lead = parts.next()?;
    let prefix: String = lead.chars().take_while(char::is_ascii_alphabetic).collect();
    let glued = lead.get(prefix.len()..).filter(|rest| !rest.is_empty());
    let numbers: Vec<&str> = glued.into_iter().chain(parts).collect();
    let digits = |text: &str| !text.is_empty() && text.chars().all(|glyph| glyph.is_ascii_digit());
    let category = Category::from_prefix(&prefix);

    if prefix.is_empty() || matches!(category, Category::Unknown(_)) {
        return None;
    }

    match (kind, numbers.as_slice()) {
        (Kind::Map, [map]) => {
            let (map, graft) = map.strip_suffix(['i', 'I']).map_or((*map, ""), |bare| (bare, GRAFT));

            digits(map).then(|| Some(format!("{}{:03}{graft}", category.map_prefix(), map.parse::<u32>().ok()?))).flatten()
        }
        (Kind::Stage, [map, stage]) => {
            let (stage, graft) = stage.strip_suffix(['i', 'I']).map_or((*stage, ""), |bare| (bare, GRAFT));

            (digits(map) && digits(stage)).then(|| Some(format!("{}{:03}-{:02}{graft}", category.map_prefix(), map.parse::<u32>().ok()?, stage.parse::<u32>().ok()?))).flatten()
        }
        _ => None,
    }
}

pub(crate) fn place(text: &str) -> Result<Request, String> {
    let text = spelled(text);
    let (tokens, search) = sifted(&text);

    placed(&tokens, search, &Kind::PLACES)
}

fn placed(tokens: &[&str], search: Option<u32>, kinds: &[Kind]) -> Result<Request, String> {
    let flag = tokens.iter().position(|token| crown_flag(token) || mode_of(token).is_some());
    let named = &tokens[..flag.unwrap_or(tokens.len())];
    let mut crowns = None;
    let mut mode = None;
    let mut at = named.len();

    while let Some(token) = tokens.get(at) {
        at += 1;

        if crown_flag(token) {
            crowns = Some(tokens.get(at).and_then(|next| next.parse::<u8>().ok()).filter(|crowns| (1..=MAX_CROWNS).contains(crowns)).ok_or(CROWN_HINT)?);
            at += 1;
        } else if mode_of(token).is_some() {
            mode = mode_of(token);
        } else {
            return Err(format!("I don't understand `{token}`"));
        }
    }

    let unit = match (named, kinds.iter().find_map(|kind| code_of(named, *kind))) {
        ([], _) => return Err(match kinds {
            [Kind::Map] => "Name a map first",
            [Kind::Stage] => "Name a stage first",
            _ => "Name a stage or a map first",
        }
        .to_owned()),
        (_, Some(code)) => Unit::Code(code),
        (_, None) => Unit::Name(named.join(" ")),
    };

    if search.is_some() && matches!(unit, Unit::Code(_)) {
        return Err(SEARCH_HINT.to_owned());
    }

    Ok(Request { unit, want: Want::Stats(Stats { mode, ..Stats::default() }), whole: None, form: None, crowns, search, tail: tokens[named.len()..].join(" ") })
}

fn talented(tokens: &[&str]) -> Result<Stats, String> {
    let (typed, flags) = tokens.split_at(tokens.iter().position(|token| breaks(token, Kind::Cat)).unwrap_or(tokens.len()));
    let mut stats = stats_of(flags, Kind::Cat)?;
    let mut words = Vec::new();
    let mut level = None;
    let mut at = 0;

    while let Some(token) = typed.get(at) {
        at += 1;

        if marked_level(token) {
            level = level_of(token);
        } else if level_word(token) && typed.get(at).is_some_and(|next| level_of(next).is_some()) {
            level = typed.get(at).and_then(|next| level_of(next));
            at += 1;
        } else {
            words.push(*token);
        }
    }

    if !words.is_empty() {
        let flagged = stats.talents.take().unwrap_or_default();

        stats.talents = Some(talents_of(&words).into_iter().chain(flagged).collect());
    }

    stats.level = stats.level.or(level);

    Ok(stats)
}

fn stats_of(tokens: &[&str], kind: Kind) -> Result<Stats, String> {
    let mut stats = Stats::default();
    let mut at = 0;

    while let Some(token) = tokens.get(at) {
        at += 1;

        match lower(token).as_str() {
            "--talents" | "-t" if kind == Kind::Cat => {
                let end = tokens[at..].iter().position(|next| breaks(next, kind)).map_or(tokens.len(), |offset| at + offset);

                stats.talents = Some(talents_of(&tokens[at..end]));
                at = end;
            }
            flag if kind == Kind::Cat && orb_slot(flag).is_some() => {
                let end = tokens[at..].iter().position(|next| breaks(next, kind) || marked_level(next) || level_word(next)).map_or(tokens.len(), |offset| at + offset);
                let slot = stats.orbs.get_mut(orb_slot(flag).unwrap_or_default()).ok_or(ORB_HINT)?;

                if at == end {
                    return Err(ORB_HINT.to_owned());
                }

                *slot = Some(tokens[at..end].join(" "));
                at = end;
            }
            "--versus" | "-v" if kind == Kind::Cat => {
                let end = tokens[at..].iter().position(|next| breaks(next, kind)).map_or(tokens.len(), |offset| at + offset);

                if at == end {
                    return Err(VERSUS_HINT.to_owned());
                }

                stats.versus = Some(tokens[at..end].iter().map(|token| (*token).to_owned()).collect());
                at = end;
            }
            "--on" | "--off" => {
                let end = tokens[at..].iter().position(|next| breaks(next, kind)).map_or(tokens.len(), |offset| at + offset);

                if at == end {
                    return Err(SWITCH_HINT.to_owned());
                }

                stats.switches = switched(stats.switches, &tokens[at..end], lower(token) == "--on")?;
                at = end;
            }
            "--mode" | "-m" => {
                let word = tokens.get(at).ok_or(GAUGE_HINT)?;

                stats.gauge = Some(gauge_of(word).ok_or(GAUGE_HINT)?);
                at += 1;
            }
            "--hits" | "-h" => {
                let word = tokens.get(at).ok_or(HITS_HINT)?;

                stats.hits = Some(hits_of(word).ok_or(HITS_HINT)?);
                at += 1;
            }
            _ if mode_of(token).is_some() => stats.mode = mode_of(token),
            _ if kind == Kind::Cat && stats.level.is_none() && marked_level(token) => stats.level = level_of(token),
            _ if kind == Kind::Cat && plain_number(token) => return Err(format!("`{token}` needs a mark to be a level, like `lv{token}`")),
            "lv" | "lvl" | "l" | "level" if kind == Kind::Cat => {
                let level = tokens.get(at).and_then(|next| level_of(next)).ok_or_else(|| format!("`{token}` needs a level after it"))?;

                stats.level = Some(level);
                at += 1;
            }
            _ if kind == Kind::Enemy && magnification_of(token).is_some() => stats.magnification = magnification_of(token),
            _ => return Err(format!("I don't understand `{token}`")),
        }
    }

    Ok(stats)
}

fn percent_flag(value: Option<&&str>, flag: &str) -> Result<u32, String> {
    let parsed = value.and_then(|value| value.trim_end_matches('%').parse::<i64>().ok()).ok_or_else(|| format!("`{flag}` takes a number from 0 to 100"))?;

    Ok(u32::try_from(parsed.clamp(0, MAX_PERCENT)).unwrap_or(0))
}

fn format_of(value: Option<&&str>) -> Result<Format, String> {
    match value.map(|value| lower(value)).as_deref() {
        Some("avif" | "a") => Ok(Format::Avif),
        Some("webp" | "w") => Ok(Format::WebP),
        Some("gif" | "g") => Ok(Format::Gif),
        _ => Err("`--format` takes `avif` (`a`), `webp` (`w`) or `gif` (`g`)".to_owned()),
    }
}

fn numbered(token: &str, lead: &str, extension: &str) -> Option<u32> {
    let lowered = lower(token);
    let bare = lowered.strip_suffix(extension).unwrap_or(&lowered);
    let digits = bare.strip_prefix(lead).unwrap_or(bare);

    (!digits.is_empty() && digits.chars().all(|glyph| glyph.is_ascii_digit())).then(|| digits.parse().ok()).flatten()
}

fn pair_of<T: std::str::FromStr>(tokens: &[&str], at: &mut usize) -> Option<(T, T)> {
    let first = tokens.get(*at)?;

    if let Some((across, down)) = first.split_once(PAIR_MARKS) {
        *at += 1;

        return Some((across.parse().ok()?, down.parse().ok()?));
    }

    let pair = (first.parse().ok()?, tokens.get(*at + 1)?.parse().ok()?);

    *at += 2;
    Some(pair)
}

pub(crate) fn tune(text: &str) -> Result<Tune, String> {
    let text = spelled(text);
    let typed: Vec<&str> = text.split_whitespace().collect();
    let boss = typed.iter().any(|token| boss_flag(token));
    let tokens: Vec<&str> = typed.into_iter().filter(|token| !boss_flag(token)).collect();

    if let [only] = tokens.as_slice()
        && let Some(id) = numbered(only, "snd", ".ogg")
    {
        return if boss { Err(BOSS_HINT.to_owned()) } else { Ok(Tune::Track(id, (*only).to_owned())) };
    }

    if tokens.is_empty() {
        return Err(TRACK_HINT.to_owned());
    }

    let request = place(&tokens.join(" "))?;

    if request.mode().is_some() {
        return Err(MODE_HINT.to_owned());
    }

    match request.crowns {
        Some(_) => Err("A stage plays the same music at every crown, so leave `--crowns` out".to_owned()),
        None => Ok(Tune::Place(Box::new(request), boss)),
    }
}

fn boss_flag(token: &str) -> bool {
    matches!(lower(token).as_str(), "--boss" | "-b")
}

pub(crate) fn scenery(text: &str) -> Result<Scenery, String> {
    let text = spelled(text);
    let (sifted, search) = sifted(&text);
    let (named, tokens) = sifted.split_at(sifted.iter().position(|token| is_flag(token)).unwrap_or(sifted.len()));
    let subject = match (named, named.first().and_then(|token| numbered(token, "bg", ".png"))) {
        ([], _) => return Err(SCENERY_HINT.to_owned()),
        ([_], Some(_)) if search.is_some() => return Err(SEARCH_HINT.to_owned()),
        ([only], Some(id)) => Backing::Id(id, (*only).to_owned()),
        _ => Backing::Place(Box::new(Request { tail: tokens.join(" "), ..placed(named, search, &Kind::PLACES)? })),
    };
    let mut scenery = Scenery { subject, seed: None, range: None, offset: None, size: None, format: None, quality: None, compression: None };
    let mut at = 0;

    while let Some(token) = tokens.get(at) {
        at += 1;

        match lower(token).as_str() {
            "--seed" | "-e" => {
                scenery.seed = Some(tokens.get(at).and_then(|next| next.parse::<u32>().ok()).ok_or("`--seed` takes a whole number, 0 or higher")?);
                at += 1;
            }
            "--range" | "-r" => scenery.range = Some(range_of(tokens, &mut at)?),
            "--offset" | "-o" => scenery.offset = Some(pair_of(tokens, &mut at).ok_or(OFFSET_HINT)?),
            "--dimensions" | "--dimension" | "-d" => scenery.size = Some(pair_of(tokens, &mut at).ok_or(SIZE_HINT)?),
            "--format" | "-f" => {
                scenery.format = Some(format_of(tokens.get(at))?);
                at += 1;
            }
            "--quality" | "-q" => {
                scenery.quality = Some(percent_flag(tokens.get(at), "--quality")?);
                at += 1;
            }
            "--compress" | "--compression" | "-p" => {
                scenery.compression = Some(percent_flag(tokens.get(at), "--compress")?);
                at += 1;
            }
            _ => return Err(format!("I don't understand `{token}`")),
        }
    }

    Ok(scenery)
}

fn motion_of(tokens: &[&str]) -> Result<Motion, String> {
    let mut motion = Motion { clip: Clip::Attack, range: None, format: None, cull: None, overlays: Overlays::default(), looping: None, background: false, quality: None, compression: None };
    let mut at = 0;

    if let Some(clip) = tokens.first().filter(|token| !is_flag(token)).and_then(|token| clip_of(token)) {
        motion.clip = clip;
        at = 1;
    }

    while let Some(token) = tokens.get(at) {
        at += 1;

        match lower(token).as_str() {
            "--range" | "-r" => motion.range = Some(range_of(tokens, &mut at)?),
            "--format" | "-f" => {
                motion.format = Some(format_of(tokens.get(at))?);
                at += 1;
            }
            "--cull" | "-u" => {
                let cull = tokens.get(at).and_then(|next| next.trim_end_matches('%').parse::<i32>().ok()).filter(|cull| (0..=MAX_CULL).contains(cull));

                motion.cull = Some(cull.ok_or("`--cull` takes a number from 0 to 100")?);
                at += 1;
            }
            "--background" | "-b" => motion.background = true,
            "--quality" | "-q" => {
                motion.quality = Some(percent_flag(tokens.get(at), "--quality")?);
                at += 1;
            }
            "--compress" | "--compression" | "-p" => {
                motion.compression = Some(percent_flag(tokens.get(at), "--compress")?);
                at += 1;
            }
            "--combat" | "-c" => motion.overlays.combat = true,
            "--rig" | "-g" => motion.overlays.rig = true,
            "--world" | "-w" => motion.overlays.world = true,
            "--origin" | "-o" => motion.overlays.origin = true,
            "--loop" | "-l" => {
                let mut looping = Looping::default();

                while let Some(key) = tokens.get(at).map(|next| lower(next)) {
                    let slot = match key.as_str() {
                        "minimum" | "min" => &mut looping.minimum,
                        "difference" | "diff" => &mut looping.difference,
                        "maximum" | "max" => {
                            let value = tokens.get(at + 1).and_then(|next| next.parse::<i32>().ok()).filter(|value| *value > 0).ok_or("`max` needs a frame count after it")?;

                            looping.maximum = Some(value);
                            at += 2;
                            continue;
                        }
                        _ => break,
                    };

                    *slot = tokens.get(at + 1).and_then(|next| next.parse::<i32>().ok()).filter(|value| *value >= 0).ok_or_else(|| format!("`{key}` needs a number after it"))?;
                    at += 2;
                }

                motion.looping = Some(looping);
            }
            _ => return Err(format!("I don't understand `{token}`")),
        }
    }

    if motion.range.is_some() && motion.clip == Clip::Showcase {
        return Err("`--range` doesn't work on a showcase".to_owned());
    }

    if motion.range.is_some() && motion.clip == Clip::Model {
        return Err("`--range` doesn't work on a model; it is a single picture with no frames to pick from".to_owned());
    }

    if motion.looping.is_some() && motion.clip == Clip::Model {
        return Err("`--loop` doesn't work on a model; it is a single picture with nothing to loop".to_owned());
    }

    if motion.looping.is_some() && !matches!(motion.clip, Clip::Walk | Clip::Idle) {
        return Err("`--loop` only works on walk and idle".to_owned());
    }

    if let (Some(_), Some((from, to))) = (motion.looping, motion.range)
        && (from < 0 || to < from)
    {
        return Err("With `--loop`, `--range` is the window to search, from a lower frame to a higher one, starting at 0 or later".to_owned());
    }

    Ok(motion)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Each orb flag takes its own words, a level after them still counts, and -1 or -2 is never a number in cat stats.
    #[test]
    fn each_orb_flag_takes_its_own_words() {
        let Want::Stats(stats) = parse("bahamut -1 S Red Attack Up -2 cash back lv50", Kind::Cat).unwrap().want else { panic!("not stats") };

        assert_eq!(stats.orbs, [Some("S Red Attack Up".to_owned()), Some("cash back".to_owned())]);
        assert_eq!(stats.level.as_deref(), Some("50"));

        let Want::Stats(alias) = parse("25 --orb dodge", Kind::Cat).unwrap().want else { panic!("not stats") };

        assert_eq!(alias.orbs, [Some("dodge".to_owned()), None]);
        assert!(parse("25 -2", Kind::Cat).is_err());
        assert!(matches!(parse("25 attack -r -1 30", Kind::Cat).unwrap().want, Want::Motion(_)), "a negative frame stays a frame");
    }

    // The switches ride in a button with the hits and the mode, and a lens with none set keeps the short tag.
    #[test]
    fn switches_travel_with_the_lens() {
        let lens = Lens { mode: Gauge::Maximum, hits: Hits::All, switches: Switches::default().set(Switch::Strengthen, true).set(Switch::Surge, false), switching: false };

        assert_eq!(lens_tagged(&lens_tag(lens)), Some(lens));
        assert_eq!((lens_tag(Lens::default()).as_str(), lens_tagged("as")), ("as", Some(Lens::default())));
        assert_eq!(SWITCH_WORDS.iter().map(|(switch, word)| switch_of(word) == Some(*switch)).collect::<Vec<_>>(), [true; 10], "every word reads back as its switch");

        let stats = stats_of(&["--on", "kills", "--off", "crits", "-m", "base"], Kind::Cat).unwrap();

        assert_eq!((stats.switches.held(Switch::Kills), stats.switches.held(Switch::Critical), stats.gauge), (Some(true), Some(false), Some(Gauge::Base)));
        assert_eq!(["b", "A", "m"].map(gauge_of), [Some(Gauge::Base), Some(Gauge::Average), Some(Gauge::Maximum)], "each mode has a one-letter form");
        assert!(stats_of(&["--on", "toxic"], Kind::Enemy).is_err());
    }

    // Phones turn a typed double dash into an em or en dash, so a word that starts with one reads as a flag.
    #[test]
    fn a_long_dash_reads_as_a_double_dash() {
        assert_eq!(parse("25 a walk —format gif —loop", Kind::Cat).unwrap(), parse("25 a walk --format gif --loop", Kind::Cat).unwrap());
        assert_eq!(parse("25 —mode b —on kills", Kind::Cat).unwrap(), parse("25 --mode b --on kills", Kind::Cat).unwrap());
        assert_eq!(parse("cyclops —search", Kind::Enemy).unwrap(), parse("cyclops --search", Kind::Enemy).unwrap());
        assert_eq!(place("r 1 2 —crowns 2").unwrap(), place("r 1 2 --crowns 2").unwrap());
        assert_eq!(tune("korea —boss").unwrap(), tune("korea --boss").unwrap());
        assert_eq!(scenery("12 —seed 7 —format webp").unwrap(), scenery("12 --seed 7 --format webp").unwrap());
        assert_eq!(parse("25 a walk –format gif –loop", Kind::Cat).unwrap(), parse("25 a walk --format gif --loop", Kind::Cat).unwrap());
        assert_eq!(spelled("a—b — c —1 —x a–b – –2 –y"), "a—b — c —1 --x a–b – –2 --y", "only a dash that starts a word and runs into letters");
    }

    // The search flag can sit anywhere after the name, and what is left is what gets asked of the pick.
    #[test]
    fn search_keeps_the_rest_of_the_request() {
        let read = parse("cyclops --search a walk -f gif", Kind::Enemy).unwrap();
        let late = parse("bahamut cat form normal lv30 -s", Kind::Cat).unwrap();

        assert_eq!((read.unit, read.search, read.tail.as_str()), (Unit::Name("cyclops".to_owned()), Some(CLOSENESS), "a walk -f gif"));
        assert!(matches!(read.want, Want::Motion(_)));
        assert_eq!((late.unit, late.search, late.tail.as_str(), late.form), (Unit::Name("bahamut cat".to_owned()), Some(CLOSENESS), "form normal lv30", Some(0)));
        assert_eq!(parse("bahamut cat", Kind::Cat).unwrap().search, None);

        let loose = parse("cyclops 3d -s 70 a walk", Kind::Enemy).unwrap();

        assert_eq!((loose.unit, loose.search, loose.tail.as_str()), (Unit::Name("cyclops 3d".to_owned()), Some(70), "a walk"), "a number right after the flag is how close a name must be");
        assert_eq!(parse("cyclops --search 500%", Kind::Enemy).unwrap().search, Some(100));
        assert_eq!(parse("cyclops --search 0", Kind::Enemy).unwrap().search, Some(1));
        assert_eq!(parse("25 --search", Kind::Cat), Err(SEARCH_HINT.to_owned()));
        assert_eq!(parse("25-3 a walk", Kind::Cat).unwrap().tail, "a walk", "a pick is replayed as an ID with the same tail");
    }

    // A model is a single picture, so it takes the picture flags and refuses the ones about time.
    #[test]
    fn a_model_takes_no_frames_or_loop() {
        let motion = |text: &str| match parse(text, Kind::Cat).map(|request| request.want) {
            Ok(Want::Motion(motion)) => Ok(motion),
            Ok(Want::Stats(_)) => Err("read as stats".to_owned()),
            Err(reason) => Err(reason),
        };
        let model = motion("25 m -f gif -b -c -u 40").unwrap();

        assert_eq!((model.clip, model.format, model.background, model.overlays.combat, model.cull), (Clip::Model, Some(Format::Gif), true, true, Some(40)));
        assert_eq!(motion("25 --cull 40").unwrap().cull, Some(40), "the short flag alone is enough to mean an animation");
        assert_eq!(motion("25 -u 15").unwrap().cull, Some(15));
        assert_eq!(motion("bahamut cat a model").unwrap().clip, Clip::Model);
        assert_eq!(motion("25 model").unwrap().clip, Clip::Model);
        assert!(motion("25 model -r 0~10").unwrap_err().contains("--range"));
        assert!(motion("25 m -l").unwrap_err().contains("--loop"));
    }

    // Attack is the default clip, so naming it or leaving it out is one request, and one cached file.
    #[test]
    fn a_default_clip_reads_the_same_named_or_not() {
        let short = parse("gigapult a -c", Kind::Cat).unwrap();
        let long = parse("gigapult a a -c", Kind::Cat).unwrap();

        assert_eq!((&short.unit, &short.want), (&long.unit, &long.want));
        assert!(matches!(short.want, Want::Motion(Motion { clip: Clip::Attack, overlays: Overlays { combat: true, rig: false, world: false, origin: false }, .. })));
    }

    // Audio and backgrounds only go by ID, written bare or the way the game names the file.
    #[test]
    fn tracks_and_backgrounds_go_by_id() {
        let track = |text: &str| match tune(text) {
            Ok(Tune::Track(id, _)) => Some(id),
            _ => None,
        };
        let themed = |text: &str| match tune(text) {
            Ok(Tune::Place(request, boss)) => Some((request.unit, boss, request.search)),
            _ => None,
        };

        assert_eq!((track("5"), track("005"), track("snd005"), track("SND005.ogg"), track("009.ogg")), (Some(5), Some(5), Some(5), Some(5), Some(9)));
        assert!(tune("").is_err() && tune("5 -b").is_err() && tune("-b").is_err());
        assert_eq!(themed("earthshaker"), Some((Unit::Name("earthshaker".to_owned()), false, None)));
        assert_eq!(themed("sleeping lion --boss"), Some((Unit::Name("sleeping lion".to_owned()), true, None)));
        assert_eq!(themed("-b moon -s 70"), Some((Unit::Name("moon".to_owned()), true, Some(70))));
        assert_eq!(themed("5 6"), Some((Unit::Name("5 6".to_owned()), false, None)), "more than one word is a name");
        assert_eq!(themed("N000-07 -b"), Some((Unit::Code("N000-07".to_owned()), true, None)));
        assert!(tune("earthshaker -c 2").is_err(), "the music does not change with the crowns");

        let plain = scenery("12").unwrap();
        let full = scenery("bg012 -e 7 -r 30~90 -f gif -q 90 -p 20").unwrap();

        assert_eq!(plain, Scenery { subject: Backing::Id(12, "12".to_owned()), seed: None, range: None, offset: None, size: None, format: None, quality: None, compression: None });
        assert_eq!(full, Scenery { subject: Backing::Id(12, "bg012".to_owned()), seed: Some(7), range: Some((30, 90)), offset: None, size: None, format: Some(Format::Gif), quality: Some(90), compression: Some(20) });

        let staged = |text: &str| match scenery(text).map(|scenery| (scenery.subject, scenery.format)) {
            Ok((Backing::Place(request), format)) => Some((request.unit, request.search, request.tail, format)),
            _ => None,
        };

        assert_eq!(staged("sleeping lion -f gif -d 800x600"), Some((Unit::Name("sleeping lion".to_owned()), None, "-f gif -d 800x600".to_owned(), Some(Format::Gif))));
        assert_eq!(staged("moon -s 70 -e 7"), Some((Unit::Name("moon".to_owned()), Some(70), "-e 7".to_owned(), None)));
        assert_eq!(staged("1 + 2 = Tor"), Some((Unit::Name("1 + 2 = Tor".to_owned()), None, String::new(), None)));
        assert_eq!(staged("N 0 7 -f webp").map(|read| read.0), Some(Unit::Code("N000-07".to_owned())));
        assert!(scenery("12 -s").is_err() && scenery("moon -c 2").is_err());

        let framed = |text: &str| scenery(text).map(|scenery| (scenery.offset, scenery.size));

        assert_eq!(framed("12 -o 100 -50 -d 800 600"), Ok((Some((100, -50)), Some((800, 600)))));
        assert_eq!(framed("12 --dimensions 800x600 --offset -200,0"), Ok((Some((-200, 0)), Some((800, 600)))));
        assert_eq!(framed("12 -d 800×600 -f gif"), Ok((None, Some((800, 600)))));
        assert!(framed("12 -d 800").is_err() && framed("12 -d -800 600").is_err() && framed("12 -o 100").is_err() && framed("12 -d wide").is_err());
        assert_eq!(scenery("12 --seed 7 --range 30 90").unwrap().range, Some((30, 90)));
        assert!(scenery("").is_err() && scenery("-e 7").is_err() && scenery("12 -e").is_err() && scenery("12 -e -3").is_err());
    }

    // A stage goes by its name, with the crowns after it, or by the code a pick list hands back.
    #[test]
    fn a_stage_takes_a_name_and_crowns() {
        let read = |text: &str| parse(text, Kind::Stage).map(|request| (request.unit, request.crowns, request.search, request.tail));

        assert_eq!(read("earthshaker"), Ok((Unit::Name("earthshaker".to_owned()), None, None, String::new())));
        assert_eq!(read("return of terror -c 3"), Ok((Unit::Name("return of terror".to_owned()), Some(3), None, "-c 3".to_owned())));
        assert_eq!(read("sleeping lion --crowns 4 -s 70"), Ok((Unit::Name("sleeping lion".to_owned()), Some(4), Some(70), "--crowns 4".to_owned())));
        assert_eq!(read("N000-07 -c 2"), Ok((Unit::Code("N000-07".to_owned()), Some(2), None, "-c 2".to_owned())));
        assert_eq!(read("Space009-11").map(|read| read.0), Ok(Unit::Code("Space009-11".to_owned())));
        assert!(read("").is_err() && read("-c 2").is_err() && read("lion -c").is_err() && read("lion -c 5").is_err() && read("lion -c 2 extra").is_err());
        assert!(read("N000-07 -s").is_err());
        // A stage's ID is its prefix, map and stage, spaced or joined any way, padded or not, and a file's prefix finds its category.
        let code = |text: &str| read(text).ok().and_then(|read| match read.0 {
            Unit::Code(code) => Some(code),
            _ => None,
        });

        for typed in ["R 001 02", "R 1 2", "r-1-2", "R_001_02", "R001-02", "r1 2", "RR 1 2", "rr001_02"] {
            assert_eq!(code(typed).as_deref(), Some("R001-02"), "{typed}");
        }

        assert_eq!((code("RN 0 7"), code("EX070_09"), code("space 9 47i"), code("ec 0 47"), code("RNA 35 5")), (Some("N000-07".to_owned()), Some("RE070-09".to_owned()), Some("Space009-47i".to_owned()), Some("EC000-47".to_owned()), Some("NA035-05".to_owned())));
        assert_eq!(read("R 1 2 -c 3").map(|read| (read.0, read.1)), Ok((Unit::Code("R001-02".to_owned()), Some(3))));
        // A map's ID is the same without the stage, and a stage's ID is never mistaken for one.
        let map = |text: &str| match parse(text, Kind::Map).map(|request| (request.unit, request.crowns)) {
            Ok((Unit::Code(code), crowns)) => Some((code, crowns)),
            _ => None,
        };

        for typed in ["R 100", "R-100", "r_100", "R100", "RR 100"] {
            assert_eq!(map(typed), Some(("R100".to_owned(), None)), "{typed}");
        }

        assert_eq!((map("RN 0 -c 3"), map("space 9"), map("EX 26")), (Some(("N000".to_owned(), Some(3))), Some(("Space009".to_owned(), None)), Some(("RE026".to_owned(), None))));
        assert_eq!((map("space 9i"), map("Z009I"), map("space i")), (Some(("Space009i".to_owned(), None)), Some(("Z009i".to_owned(), None)), None), "a trailing i is the invaded map");
        assert!(map("R 1 2").is_none() && code("R 100").is_none() && map("Tor 1").is_none() && map("100").is_none());
        assert_eq!(parse("the legend begins -c 2", Kind::Map).map(|request| (request.unit, request.crowns, request.tail)), Ok((Unit::Name("the legend begins".to_owned()), Some(2), "-c 2".to_owned())));
        assert!(code("25-3").is_none() && code("N000").is_none() && code("li'l-cat").is_none() && code("Tor 1 2").is_none() && code("1 + 2 = Tor").is_none() && code("N 1 2 3").is_none() && code("Space009-i").is_none());
    }

    fn stats(text: &str) -> Stats {
        match parse(text, Kind::Cat).map(|request| request.want) {
            Ok(Want::Stats(stats)) => stats,
            other => panic!("{text} gave {other:?}"),
        }
    }

    fn motion(text: &str) -> Motion {
        match parse(text, Kind::Cat).map(|request| request.want) {
            Ok(Want::Motion(motion)) => motion,
            other => panic!("{text} gave {other:?}"),
        }
    }

    #[test]
    fn ids_take_a_form_by_number_or_letter() {
        assert_eq!(parse("25-3", Kind::Cat).map(|request| request.unit), Ok(Unit::Id(25, Some(2))));
        assert_eq!(parse("25_u", Kind::Cat).map(|request| request.unit), Ok(Unit::Id(25, Some(3))));
        assert_eq!(parse("25 s", Kind::Cat).map(|request| request.unit), Ok(Unit::Id(25, None)));
        assert_eq!(parse("763", Kind::Enemy).map(|request| request.unit), Ok(Unit::Id(763, None)));

        // An ID ending in e is an enemy's, the way its statblock writes it, and never a cat's Evolved form.
        for typed in ["763-e", "763_e", "763-E", "0763_E"] {
            assert_eq!(parse(typed, Kind::Enemy).map(|request| request.unit), Ok(Unit::Id(763, None)), "{typed}");
            assert!(parse(typed, Kind::Cat).is_err_and(|reason| reason.contains("enemy's ID")), "{typed}");
            assert!(names_an_enemy(typed) && names_an_enemy(&format!("{typed} a attack")));
        }

        assert_eq!(parse("763-e 400%", Kind::Enemy).map(|request| request.want), Ok(Want::Stats(Stats { magnification: Some((400, 400)), ..Stats::default() })));
        assert_eq!((parse("25-c", Kind::Cat).map(|request| request.unit), parse("25_2", Kind::Cat).map(|request| request.unit)), (Ok(Unit::Id(25, Some(1))), Ok(Unit::Id(25, Some(1)))));
        assert_eq!(parse("25 form e", Kind::Cat).map(|request| (request.unit, request.form)), Ok((Unit::Id(25, None), Some(1))));
        assert!(!names_an_enemy("763") && !names_an_enemy("25-3") && !names_an_enemy("e-763") && !names_an_enemy("elder-e"));
        assert!(parse("763-3", Kind::Enemy).map(|request| request.unit) != Ok(Unit::Id(763, None)));
    }

    // The game writes IDs padded to three digits, so people copy them that way.
    #[test]
    fn zero_padded_ids_read_as_numbers() {
        assert_eq!(parse("002", Kind::Cat).map(|request| request.unit), Ok(Unit::Id(2, None)));
        assert_eq!(parse("002-3", Kind::Cat).map(|request| request.unit), Ok(Unit::Id(2, Some(2))));
        assert_eq!(parse("002_s", Kind::Cat).map(|request| request.unit), Ok(Unit::Id(2, Some(2))));
        assert_eq!(parse("002", Kind::Enemy).map(|request| request.unit), Ok(Unit::Id(2, None)));
    }

    #[test]
    fn a_name_runs_until_what_is_wanted() {
        assert_eq!(parse("jamiera cat s lv30", Kind::Cat).map(|request| request.unit), Ok(Unit::Name("jamiera cat".to_owned())));
        assert_eq!(stats("jamiera cat s lv30").level.as_deref(), Some("30"));
        assert_eq!(parse("jamiera cat s 30", Kind::Cat).map(|request| request.unit), Err("`30` needs a mark to be a level, like `lv30`".to_owned()), "the name still ends at s, so the number gets its hint");
        assert_eq!(motion("crazed bahamut a attack").clip, Clip::Attack);
        assert_eq!(motion("25 attack").clip, Clip::Attack);
    }

    // A word that is also a clip name stays part of a name; only s, stats, a and anim end one.
    #[test]
    fn clip_words_inside_a_name_stay_in_it() {
        assert_eq!(parse("dig attack cat", Kind::Cat).map(|request| request.unit), Ok(Unit::Name("dig attack cat".to_owned())));
        assert_eq!(parse("dig attack cat a walk", Kind::Cat).map(|request| request.unit), Ok(Unit::Name("dig attack cat".to_owned())));
    }

    #[test]
    fn every_level_spelling_is_read() {
        for text in ["25 lv50+70", "25 s lvl50+70", "25 s l50+70", "25 s level50+70", "25 s 50+70", "25 s lv 50+70"] {
            assert_eq!(stats(text).level.as_deref(), Some("50+70"), "{text}");
        }
    }

    #[test]
    fn talents_take_presets_numbers_and_names() {
        assert_eq!(stats("25 s -t all").talents, Some(vec![Talent::All]));
        assert_eq!(stats("25 s -t none").talents, Some(vec![Talent::None]));
        assert_eq!(stats("25 s -t a n").talents, Some(vec![Talent::All, Talent::None]));
        assert_eq!(stats("25 s --talents 10 10 5").talents, Some(vec![Talent::Level(10), Talent::Level(10), Talent::Level(5)]));
        assert_eq!(stats("25 s -t u Attack-Buff 5").talents, Some(vec![Talent::Ultra, Talent::Named("Attack Buff".to_owned(), Some(5))]));
        assert_eq!(
            stats("25 s -t all Attack Buff 5 Resist Weaken 3 Toxic").talents,
            Some(vec![Talent::All, Talent::Named("Attack Buff".to_owned(), Some(5)), Talent::Named("Resist Weaken".to_owned(), Some(3)), Talent::Named("Toxic".to_owned(), None)])
        );
    }

    // Negative frames are why the range uses a tilde and not a dash.
    #[test]
    fn ranges_read_negatives() {
        assert_eq!(motion("25 a attack -r -10~40").range, Some((-10, 40)));
        assert_eq!(motion("25 a kb --range -5 5").range, Some((-5, 5)));
        assert!(parse("25 a sh -r 0~10", Kind::Cat).is_err());
    }

    // One number is that frame alone, whatever comes after it.
    #[test]
    fn a_lone_range_number_is_a_single_frame() {
        assert_eq!(motion("25 a attack -r 12").range, Some((12, 12)));
        assert_eq!(motion("25 a walk --range -4 -f webp").range, Some((-4, -4)));
        assert_eq!((motion("25 -r 7 -b").range, motion("25 -r 7 -b").background), (Some((7, 7)), true));
        assert_eq!(scenery("12 -r 40 -f gif").unwrap().range, Some((40, 40)));
        assert!(parse("25 a walk -l -r 30", Kind::Cat).is_ok(), "a loop search still reads it, as a window of one frame");
        assert!(parse("25 a attack -r", Kind::Cat).is_err() && parse("25 a attack -r soon", Kind::Cat).is_err());
    }

    fn known(lookup: Lookup<'_>) -> Option<Foe> {
        let carrying = |labels: &[Identity]| Some(labels.iter().fold(Foe::default(), |foe, label| foe.with(*label)));

        match lookup {
            Lookup::Named("teacher bear") | Lookup::Numbered(12) => carrying(&[Identity::TraitRed, Identity::TraitWitch]),
            Lookup::Named("red cyclone") => carrying(&[Identity::TraitFloating]),
            Lookup::Close("cyclon") => carrying(&[Identity::TraitAngel]),
            _ => None,
        }
    }

    fn picked(text: &str) -> Result<Versus, String> {
        versus_of(&text.split_whitespace().collect::<Vec<_>>(), &known)
    }

    fn labels(text: &str) -> Result<Vec<Identity>, String> {
        picked(text).map(|held| held.foe.labels().collect())
    }

    // Traits are read by name or by the start of one, and Metal is never part of a group word.
    #[test]
    fn versus_reads_trait_names() {
        assert_eq!(labels("red Metal"), Ok(vec![Identity::TraitRed, Identity::TraitMetal]));
        assert_eq!(labels("flo, zom,dark white baron"), Ok(vec![Identity::TraitFloating, Identity::TraitDark, Identity::TraitZombie, Identity::TraitTraitless, Identity::TraitColossus]));
        assert_eq!(labels("all").map(|held| (held.len(), held.contains(&Identity::TraitMetal))), Ok((Foe::LABELS.len() - 1, false)));
        assert_eq!(labels("all metal").map(|held| held.len()), Ok(Foe::LABELS.len()));
        assert_eq!(labels("traits").map(|held| (held.len(), held.contains(&Identity::TraitTraitless), held.contains(&Identity::TraitWitch))), Ok((9, true, false)));
        assert_eq!(labels("subtrait"), Ok(vec![Identity::TraitWitch, Identity::TraitEva, Identity::TraitColossus, Identity::TraitBehemoth, Identity::TraitSage]));
        assert_eq!(labels("traits subtraits"), labels("all"), "the two halves make all");
        assert_eq!(labels("red none"), Ok(Vec::new()));
        assert_eq!(labels(""), Ok(Vec::new()));
        assert_eq!(picked("a"), Err("a".to_owned()), "angel, alien and aku all start that way");

        let every = Versus { foe: Foe::from_bits(u16::MAX), aimed: true };
        let offered: Vec<String> = [TARGETS].into_iter().chain(Foe::LABELS.into_iter().map(trait_word)).map(str::to_owned).collect();

        assert_eq!(versus_picked(&offered), every, "every trait the panel offers is read back");
        assert_eq!(versus_picked(&offered[3..5]), Versus { foe: Foe::default().with(Identity::TraitDark).with(Identity::TraitMetal), aimed: false });
        assert_eq!(Versus::from_bits(every.bits()), every);
    }

    // An enemy brings all of its traits. A whole name wins over a trait word inside it, and words
    // that are neither are looked up together as one close name.
    #[test]
    fn versus_reads_enemies() {
        assert_eq!(labels("Teacher Bear metal"), Ok(vec![Identity::TraitRed, Identity::TraitMetal, Identity::TraitWitch]));
        assert_eq!(labels("red cyclone"), Ok(vec![Identity::TraitFloating]), "not Red and then something called cyclone");
        assert_eq!(labels("cyclon red"), Ok(vec![Identity::TraitRed, Identity::TraitAngel]));
        assert_eq!(labels("12-e"), Ok(vec![Identity::TraitRed, Identity::TraitWitch]));
        assert_eq!(picked("red purple haze"), Err("purple haze".to_owned()));
        assert_eq!(picked("purple red"), Err("purple".to_owned()));
    }

    // "targets" stands for whatever the cat has something for, worked out from the stats being drawn, never Metal.
    #[test]
    fn targets_follow_the_cat() {
        let chosen = picked("Targets red").unwrap();
        let cat = Entity { trait_dark: 1, trait_metal: 1, witch_killer: 1, ..Entity::default() };

        assert_eq!(chosen, Versus { foe: Foe::default().with(Identity::TraitRed), aimed: true });
        assert_eq!(chosen.against(&cat).labels().collect::<Vec<_>>(), [Identity::TraitRed, Identity::TraitDark, Identity::TraitWitch]);
        assert_eq!(picked("target").map(|held| held.against(&Entity::default()).is_empty()), Ok(true), "a cat with nothing to target is drawn as it is");
        assert_eq!(picked("targets none"), Ok(Versus::default()));
        assert!(!picked("targeted").unwrap().is_empty());
        assert_eq!(versus_picked(&[TARGETS.to_owned(), "red".to_owned()]), chosen);
    }

    // The flag says stats, so it never turns a request into an animation, and it takes everything up to the next flag;
    // what the words mean waits for the game data.
    #[test]
    fn versus_is_a_stats_flag() {
        let versus = |text: &str| parse(text, Kind::Cat).map(|request| match request.want {
            Want::Stats(stats) => stats.versus,
            Want::Motion(_) => None,
        });

        assert_eq!(versus("bahamut cat -v red metal"), Ok(Some(vec!["red".to_owned(), "metal".to_owned()])));
        assert_eq!(versus("25 lv50 --versus teacher bear -i"), Ok(Some(vec!["teacher".to_owned(), "bear".to_owned()])));
        assert_eq!(versus("25"), Ok(None));
        assert!(versus("25 -v").is_err_and(|reason| reason == VERSUS_HINT));
        assert!(parse("akaza -v red", Kind::Enemy).is_err(), "enemies have no traits to be measured against");
    }

    #[test]
    fn clip_aliases_and_flags() {
        let read = motion("25 anim c -f webp --cull 40 -c -g -w -o");

        assert_eq!(read.clip, Clip::Spirit);
        assert_eq!(read.format, Some(Format::WebP));
        assert_eq!(read.cull, Some(40));
        assert_eq!(read.overlays, Overlays { combat: true, rig: true, world: true, origin: true });
        assert!(motion("25 a -b -f gif").background);
        assert_eq!((motion("25 a -q 90 -p 40").quality, motion("25 a -q 90 -p 40").compression), (Some(90), Some(40)));
        assert_eq!(motion("25 a --quality 150 --compress -5").quality, Some(100), "clamped like the app's field");
        assert_eq!(motion("25 a -p -5").compression, Some(0));
        assert!(parse("25 a -m 5", Kind::Cat).is_err(), "-m is not a flag");
        assert_eq!(motion("25 -q 50").quality, Some(50), "a quality flag means animation");
        assert_eq!(motion("25 a --background --format GIF").format, Some(Format::Gif));
        assert_eq!([motion("25 -f a").format, motion("25 -f w").format, motion("25 -f G").format], [Some(Format::Avif), Some(Format::WebP), Some(Format::Gif)]);
        assert_eq!(motion("bahamut cat -f a").format, Some(Format::Avif), "the a after -f is the format, not the animation keyword");
        assert_eq!(motion("25 a").clip, Clip::Attack);
        assert_eq!(motion("25 a r").clip, Clip::Surface);
    }

    // From an audit of every name in every language: "a" is also an ordinary word.
    #[test]
    fn a_word_that_doubles_as_a_keyword_stays_in_a_name() {
        let name = |text: &str, kind: Kind| parse(text, kind).map(|request| request.unit);

        assert_eq!(name("Cats in a Box", Kind::Cat), Ok(Unit::Name("Cats in a Box".to_owned())));
        assert_eq!(name("A Gift of Cats", Kind::Cat), Ok(Unit::Name("A Gift of Cats".to_owned())));
        assert_eq!(name("Valchiria oscura a cavallo", Kind::Cat), Ok(Unit::Name("Valchiria oscura a cavallo".to_owned())));
        assert_eq!(name("teacher a walk", Kind::Enemy), Ok(Unit::Name("teacher".to_owned())));
        assert_eq!(parse("teacher a walk", Kind::Enemy).map(|request| request.whole), Ok(None));
    }

    // "Teacher A" is a real enemy, so a trailing "a" keeps both readings for the lookup to settle.
    #[test]
    fn a_trailing_keyword_keeps_the_whole_name_too() {
        let read = parse("Teacher A", Kind::Enemy).unwrap();

        assert_eq!(read.unit, Unit::Name("Teacher".to_owned()));
        assert_eq!(read.whole.as_deref(), Some("Teacher A"));
    }

    #[test]
    fn a_form_follows_a_name_or_an_id() {
        let read = |text: &str| parse(text, Kind::Cat).map(|request| (request.unit, request.form));

        assert_eq!(read("bahamut cat form normal"), Ok((Unit::Name("bahamut cat".to_owned()), Some(0))));
        assert_eq!(read("bahamut cat form 2"), Ok((Unit::Name("bahamut cat".to_owned()), Some(1))));
        assert_eq!(read("bahamut cat Form s"), Ok((Unit::Name("bahamut cat".to_owned()), Some(2))));
        assert_eq!(read("bahamut cat form ULTRA"), Ok((Unit::Name("bahamut cat".to_owned()), Some(3))));
        assert_eq!(read("25 form 1"), Ok((Unit::Id(25, None), Some(0))));
        assert_eq!(stats("bahamut cat form 1 s lv50").level.as_deref(), Some("50"));
        assert_eq!(motion("bahamut cat form u a walk").clip, Clip::Walk);
        assert!(parse("25 form 9", Kind::Cat).is_err());
        assert_eq!(read("bahamut cat f normal"), Ok((Unit::Name("bahamut cat".to_owned()), Some(0))));
        assert_eq!(read("bahamut cat F u"), Ok((Unit::Name("bahamut cat".to_owned()), Some(3))));
        assert_eq!(read("bahamut cat f n"), Ok((Unit::Name("bahamut cat".to_owned()), Some(0))));
        assert_eq!(read("bahamut cat form e"), Ok((Unit::Name("bahamut cat".to_owned()), Some(1))));
        assert_eq!(read("bahamut cat form T"), Ok((Unit::Name("bahamut cat".to_owned()), Some(2))));
        assert_eq!(read("25-t"), Ok((Unit::Id(25, Some(2)), None)));
        assert_eq!(read("25_n"), Ok((Unit::Id(25, Some(0)), None)));
        assert_eq!(stats("bahamut cat f 1 s lv50").level.as_deref(), Some("50"));
    }

    #[test]
    fn stats_or_animation_is_worked_out_from_the_options() {
        assert_eq!(motion("25 walk -l").clip, Clip::Walk);
        assert_eq!(motion("25 i").clip, Clip::Idle);
        assert_eq!(motion("25 -r 0~30").range, Some((0, 30)));
        assert_eq!(motion("jamiera cat -c -w").overlays, Overlays { combat: true, world: true, ..Overlays::default() });
        assert_eq!(motion("crazed bahamut cat showcase -f webp").clip, Clip::Showcase);
        assert_eq!(stats("25 -t all").talents, Some(vec![Talent::All]));
        assert_eq!(stats("jamiera cat 50+70").level.as_deref(), Some("50+70"));
        assert!(matches!(parse("akaza 400%", Kind::Enemy).map(|request| request.want), Ok(Want::Stats(_))));
        assert!(parse("25 -t all -r 0~5", Kind::Cat).is_err());
        assert!(matches!(parse("25 s", Kind::Cat).map(|request| request.want), Ok(Want::Stats(_))));
        assert_eq!(motion("25 a").clip, Clip::Attack);
    }

    // A clip word inside a name only ends it when real animation options follow.
    #[test]
    fn a_page_word_opens_its_page() {
        assert_eq!(stats("25 t").page, Page::Talents);
        assert_eq!(stats("25 talents lv30").page, Page::Talents);
        assert_eq!(stats("25 d").page, Page::Details, "d alone is the details page, not the dig animation");
        assert_eq!(stats("25 details").page, Page::Details);
        assert_eq!(stats("25 s").page, Page::Stats);
        assert_eq!(stats("25 lv30").page, Page::Stats);
        assert!(matches!(parse("akaza d", Kind::Enemy).map(|request| request.want), Ok(Want::Stats(Stats { page: Page::Details, .. }))));
        assert!(matches!(parse("25 a d", Kind::Cat).map(|request| request.want), Ok(Want::Motion(Motion { clip: Clip::Dig, .. }))));
        assert!(matches!(parse("25 dig", Kind::Cat).map(|request| request.want), Ok(Want::Motion(Motion { clip: Clip::Dig, .. }))));
        assert!(matches!(parse("25 a s", Kind::Cat).map(|request| request.want), Ok(Want::Motion(Motion { clip: Clip::Soul, .. }))));
        assert!(matches!(parse("25 soul", Kind::Cat).map(|request| request.want), Ok(Want::Motion(Motion { clip: Clip::Soul, .. }))));
    }

    // Either flag beats the Mode setting for one request, and the last one typed wins.
    #[test]
    fn a_mode_flag_overrides_the_setting() {
        assert_eq!(stats("25").mode, None);
        assert_eq!(stats("25 -i").mode, Some(Mode::Interactive));
        assert_eq!(stats("25 t --interactive -t all").mode, Some(Mode::Interactive));
        assert_eq!(stats("25 lv30 -d").mode, Some(Mode::Dormant));
        assert_eq!(stats("25 -i --dormant").mode, Some(Mode::Dormant));
        assert!(matches!(parse("akaza -i 400%", Kind::Enemy).map(|request| request.want), Ok(Want::Stats(Stats { mode: Some(Mode::Interactive), magnification: Some((400, 400)), .. }))));
        assert_eq!(parse("bahamut cat -i", Kind::Cat).map(|request| request.unit), Ok(Unit::Name("bahamut cat".to_owned())));
        assert!(parse("25 a walk -i", Kind::Cat).is_err(), "the flags belong to pictures, not animations");
    }

    // A plain number could be part of a name, a talent's level or the cat's, so the cat's always carries a mark.
    #[test]
    fn a_level_needs_its_mark() {
        let refused = |text: &str| parse(text, Kind::Cat).map(|request| request.unit);

        assert_eq!(refused("25 45"), Err("`45` needs a mark to be a level, like `lv45`".to_owned()));
        assert_eq!(refused("25 s 45"), Err("`45` needs a mark to be a level, like `lv45`".to_owned()));
        assert_eq!(refused("bahamut 45"), Ok(Unit::Name("bahamut 45".to_owned())), "after a name it is still part of the name");

        for text in ["25 lv45", "25 l45", "25 level 45", "25 lv 45", "bahamut lv45", "bahamut s l45"] {
            assert_eq!(stats(text).level.as_deref(), Some("45"), "{text}");
        }

        assert_eq!(stats("25 50+70").level.as_deref(), Some("50+70"), "a plus level is marked by its plus");
    }

    // After the page word everything is talents, so only a marked level still means the cat's own.
    #[test]
    fn the_talents_page_reads_talents_without_the_flag() {
        assert_eq!(stats("25 t all").talents, Some(vec![Talent::All]));
        assert_eq!(stats("25 t 10 10 5").talents, Some(vec![Talent::Level(10), Talent::Level(10), Talent::Level(5)]));
        assert_eq!(stats("25 talents attack buff 5 none").talents, Some(vec![Talent::Named("attack buff".to_owned(), Some(5)), Talent::None]));

        let leveled = stats("25 t 10 lv30 5");

        assert_eq!((leveled.level.as_deref(), leveled.talents), (Some("30"), Some(vec![Talent::Level(10), Talent::Level(5)])));
        assert_eq!(stats("25 t 50+70").level.as_deref(), Some("50+70"));
        assert_eq!(stats("25 t level 30 all").level.as_deref(), Some("30"));
        assert_eq!((stats("25 t 30").level, stats("25 t 30").talents), (None, Some(vec![Talent::Level(30)])), "a plain number is a talent's level here");
        assert_eq!(stats("25 lv30").level.as_deref(), Some("30"));

        let mixed = stats("25 t 10 -i -t ultra");

        assert_eq!((mixed.mode, mixed.talents), (Some(Mode::Interactive), Some(vec![Talent::Level(10), Talent::Ultra])));
        assert_eq!(parse("bahamut cat t all", Kind::Cat).map(|request| request.unit), Ok(Unit::Name("bahamut cat".to_owned())));
    }

    #[test]
    fn stages_and_maps_take_a_mode_flag_too() {
        let read = |text: &str, kind: Kind| parse(text, kind).map(|request| {
            let mode = request.mode();

            (request.unit, request.crowns, mode, request.tail)
        });

        assert_eq!(read("earthshaker -i", Kind::Stage), Ok((Unit::Name("earthshaker".to_owned()), None, Some(Mode::Interactive), "-i".to_owned())));
        assert_eq!(read("earthshaker -c 4 --dormant", Kind::Stage), Ok((Unit::Name("earthshaker".to_owned()), Some(4), Some(Mode::Dormant), "-c 4 --dormant".to_owned())));
        assert_eq!(read("R 1 2 -i -c 2", Kind::Stage), Ok((Unit::Code("R001-02".to_owned()), Some(2), Some(Mode::Interactive), "-i -c 2".to_owned())));
        assert_eq!(read("the legend begins -i", Kind::Map).map(|read| read.2), Ok(Some(Mode::Interactive)));
        assert_eq!(read("earthshaker", Kind::Stage).map(|read| read.2), Ok(None));
        assert!(read("earthshaker -i nonsense", Kind::Stage).is_err());
        assert!(tune("earthshaker -i").is_err(), "music is not a block");
    }

    // The page and the flag are two things: one picks what to look at, the other what to apply.
    #[test]
    fn the_talents_page_and_the_talents_flag_work_apart_and_together() {
        let both = stats("25 t -t all");

        assert_eq!((both.page, both.talents), (Page::Talents, Some(vec![Talent::All])));
        assert_eq!((stats("25 -t all").page, stats("25 -t all").talents), (Page::Stats, Some(vec![Talent::All])));
        assert_eq!((stats("25 d -t 10 5").page, stats("25 d -t 10 5").talents), (Page::Details, Some(vec![Talent::Level(10), Talent::Level(5)])));
        assert_eq!(stats("25 t").talents, None);
    }

    #[test]
    fn a_page_word_only_ends_a_name_when_the_rest_reads() {
        let named = |text: &str, kind: Kind| parse(text, kind).map(|request| (request.unit, request.whole));

        assert_eq!(named("bahamut cat t", Kind::Cat), Ok((Unit::Name("bahamut cat".to_owned()), Some("bahamut cat t".to_owned()))));
        assert_eq!(named("vitamin d cat", Kind::Cat), Ok((Unit::Name("vitamin d cat".to_owned()), None)));
        assert_eq!(named("mr t", Kind::Enemy), Ok((Unit::Name("mr t".to_owned()), None)), "enemies have no talents page, so t is part of the name");
        assert_eq!(named("the details man", Kind::Enemy), Ok((Unit::Name("the details man".to_owned()), None)));
    }

    #[test]
    fn a_clip_word_ends_a_name_only_when_it_carries_on() {
        assert_eq!(parse("jamiera cat walk", Kind::Cat).map(|request| (request.unit, request.whole)), Ok((Unit::Name("jamiera cat".to_owned()), Some("jamiera cat walk".to_owned()))));
        assert_eq!(parse("dig attack cat", Kind::Cat).map(|request| request.unit), Ok(Unit::Name("dig attack cat".to_owned())));
    }

    #[test]
    fn enemies_take_a_magnification() {
        let read = parse("763 s 400%", Kind::Enemy).map(|request| request.want);

        assert_eq!(read, Ok(Want::Stats(Stats { magnification: Some((400, 400)), ..Stats::default() })));
    }

    // Something that can't be part of a name ends it, so "akaza 400%" needs no `s`; a bare
    // number could be part of a name, so it stays in it.
    #[test]
    fn a_marked_level_or_percent_ends_a_name() {
        let magnified = |text: &str| parse(text, Kind::Enemy).map(|request| (request.unit, request.want));

        assert_eq!(magnified("akaza 400%"), Ok((Unit::Name("akaza".to_owned()), Want::Stats(Stats { magnification: Some((400, 400)), ..Stats::default() }))));
        assert_eq!(magnified("akaza s 400"), Ok((Unit::Name("akaza".to_owned()), Want::Stats(Stats { magnification: Some((400, 400)), ..Stats::default() }))));
        assert_eq!(magnified("763 400"), Ok((Unit::Id(763, None), Want::Stats(Stats { magnification: Some((400, 400)), ..Stats::default() }))));
        assert_eq!(magnified("akaza 400%/200%").map(|(_, want)| want), Ok(Want::Stats(Stats { magnification: Some((400, 200)), ..Stats::default() })));
        assert_eq!(parse("jamiera cat lv50", Kind::Cat).map(|request| request.unit), Ok(Unit::Name("jamiera cat".to_owned())));
        assert_eq!(parse("jamiera cat 50+70", Kind::Cat).map(|request| request.unit), Ok(Unit::Name("jamiera cat".to_owned())));
        assert_eq!(parse("cat 2000", Kind::Cat).map(|request| request.unit), Ok(Unit::Name("cat 2000".to_owned())));
        assert_eq!(stats("jamiera cat lv 50").level.as_deref(), Some("50"));
    }

    #[test]
    fn loop_takes_its_settings_and_a_window() {
        assert_eq!(motion("25 a walk -l").looping, Some(Looping::default()));
        assert_eq!(motion("25 a idle --loop min 20 max 200 diff 10").looping, Some(Looping { minimum: 20, maximum: Some(200), difference: 10 }));

        let windowed = motion("25 a w -l min 10 -r 30~120");

        assert_eq!((windowed.looping.map(|held| held.minimum), windowed.range), (Some(10), Some((30, 120))));
        assert!(parse("25 a attack -l", Kind::Cat).is_err());
        assert!(parse("25 a knockback -l", Kind::Cat).is_err());
        assert!(parse("25 a walk -l -r 50~10", Kind::Cat).is_err());
    }
}
