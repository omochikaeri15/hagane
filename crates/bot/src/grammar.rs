use std::mem;

use super::fuzzy::CLOSENESS;
use super::render::{Clip, Format};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Cat,
    Enemy,
}

#[derive(Debug, PartialEq)]
pub(crate) enum Unit {
    Id(u32, Option<usize>),
    Name(String),
}

#[derive(Debug, PartialEq)]
pub(crate) enum Talent {
    All,
    Standard,
    Ultra,
    None,
    Show,
    Level(u32),
    Named(String, Option<u32>),
}

#[derive(Debug, Default, PartialEq)]
pub(crate) struct Stats {
    pub(crate) level: Option<String>,
    pub(crate) magnification: Option<(i32, i32)>,
    pub(crate) talents: Option<Vec<Talent>>,
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
    pub(crate) search: Option<u32>,
    pub(crate) tail: String,
}

const MAX_CULL: i32 = 100;
const MAX_PERCENT: i64 = 100;
const MAX_CLOSENESS: u32 = 100;
const LOOP_MINIMUM: i32 = 15;
const LOOP_DIFFERENCE: i32 = 30;

fn lower(token: &str) -> String {
    token.to_lowercase()
}

fn is_flag(token: &str) -> bool {
    token.starts_with("--") || token.strip_prefix('-').is_some_and(|rest| rest.chars().next().is_some_and(char::is_alphabetic))
}

const SEARCH_HINT: &str = "`--search` (`-s`) looks a name up, so give it a name instead of an ID";
const FORM_HINT: &str = "`form` (`f`) takes 1-4, n e t u, f c s u, or Normal, Evolved, True, Ultra";

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
        "--range" | "-r" | "--loop" | "-l" | "--format" | "-f" | "--cull" | "--combat" | "-c" | "--rig" | "-g" | "--world" | "-w" | "--origin" | "-o" | "--background" | "-b" | "--quality" | "-q" | "--compress" | "--compression" | "-p"
    )
}

fn stats_tell(token: &str, kind: Kind) -> bool {
    let marked_level = level_of(token).is_some() && (token.contains('+') || !token.chars().all(|glyph| glyph.is_ascii_digit()));
    let marked_percent = kind == Kind::Enemy && token.contains('%') && magnification_of(token).is_some();

    matches!(lower(token).as_str(), "--talents" | "-t" | "lv" | "lvl" | "level") || (kind == Kind::Cat && marked_level) || marked_percent
}

fn inferred(rest: &[&str], kind: Kind) -> Result<bool, String> {
    let motion = rest.first().is_some_and(|token| !is_flag(token) && clip_of(token).is_some()) || rest.iter().any(|token| motion_flag(token));
    let stats = rest.iter().any(|token| stats_tell(token, kind));

    match (motion, stats) {
        (true, true) => Err("That mixes stats and animation options; add `s` or `a` to say which".to_owned()),
        (motion, _) => Ok(motion),
    }
}

fn picks_a_form(rest: &[&str], kind: Kind) -> bool {
    kind == Kind::Cat && rest.first().is_some_and(|token| form_word(token)) && rest.get(1).is_some_and(|value| form_value(value).is_some())
}

fn bare_keyword(token: &str) -> bool {
    matches!(lower(token).as_str(), "a" | "s")
}

fn carries_on(rest: &[&str], kind: Kind) -> bool {
    match rest.split_first() {
        Some((keyword, after)) => match want_of(keyword) {
            Some(true) => motion_of(after).is_ok(),
            Some(false) => stats_of(after, kind).is_ok(),
            None => false,
        },
        None => false,
    }
}

fn want_of(token: &str) -> Option<bool> {
    match lower(token).as_str() {
        "stats" | "stat" | "s" => Some(false),
        "animation" | "anim" | "a" => Some(true),
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
        "model" | "m" => Some(Clip::Model),
        _ => None,
    }
}

fn ends_a_name(token: &str) -> bool {
    let marked_level = level_of(token).is_some() && (token.contains('+') || !token.chars().all(|glyph| glyph.is_ascii_digit()));
    let marked_percent = token.contains('%') && magnification_of(token).is_some();

    let level_word = matches!(lower(token).as_str(), "lv" | "lvl" | "level");

    want_of(token).is_some() || is_flag(token) || marked_level || marked_percent || level_word
}

fn unit_of(token: &str, kind: Kind) -> Option<Unit> {
    if let Ok(id) = token.parse::<u32>() {
        return Some(Unit::Id(id, None));
    }

    if kind == Kind::Enemy {
        return None;
    }

    let (id, form) = token.split_once(['-', '_'])?;

    Some(Unit::Id(id.parse::<u32>().ok()?, Some(form_value(form)?)))
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

fn magnification_of(token: &str) -> Option<(i32, i32)> {
    match token.split_once('/') {
        Some((hitpoints, attack)) => Some((percent_of(hitpoints)?, percent_of(attack)?)),
        None => percent_of(token).map(|percent| (percent, percent)),
    }
}

fn range_of(tokens: &[&str], at: &mut usize) -> Result<(i32, i32), String> {
    let first = tokens.get(*at).ok_or("`--range` needs two frames, like `0~30` or `0 30`")?;

    if let Some((from, to)) = first.split_once('~') {
        *at += 1;

        return match (from.parse::<i32>(), to.parse::<i32>()) {
            (Ok(from), Ok(to)) => Ok((from, to)),
            _ => Err(format!("`{first}` is not a frame range")),
        };
    }

    let second = tokens.get(*at + 1).ok_or("`--range` needs two frames, like `0~30` or `0 30`")?;

    match (first.parse::<i32>(), second.parse::<i32>()) {
        (Ok(from), Ok(to)) => {
            *at += 2;
            Ok((from, to))
        }
        _ => Err(format!("`{first} {second}` is not a frame range")),
    }
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
            "show" | "sh" => Some(Talent::Show),
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

pub(crate) fn parse(text: &str, kind: Kind) -> Result<Request, String> {
    let typed: Vec<&str> = text.split_whitespace().collect();
    let flagged = |at: usize| at > 0 && typed.get(at).is_some_and(|token| search_flag(token));
    let closeness = |at: usize| typed.get(at).and_then(|token| token.trim_end_matches('%').parse::<u32>().ok());
    let search = (1..typed.len()).find(|at| flagged(*at)).map(|at| closeness(at + 1).map_or(CLOSENESS, |typed| typed.clamp(1, MAX_CLOSENESS)));
    let tokens: Vec<&str> = typed.iter().enumerate().filter(|(at, _)| !flagged(*at) && !(flagged(at.wrapping_sub(1)) && closeness(*at).is_some())).map(|(_, token)| *token).collect();
    let mut at = 0;
    let unit = match tokens.first().and_then(|token| unit_of(token, kind)) {
        Some(unit) => {
            at = 1;
            unit
        }
        None => {
            while let Some(token) = tokens.get(at) {
                let clip_ends = clip_word(token) && motion_of(&tokens[at..]).is_ok();

                if picks_a_form(&tokens[at..], kind) || clip_ends || (ends_a_name(token) && !(bare_keyword(token) && !carries_on(&tokens[at..], kind))) {
                    break;
                }

                at += 1;
            }

            Unit::Name(tokens[..at].join(" "))
        }
    };
    let whole = match &unit {
        Unit::Name(name) if !name.is_empty() && at + 1 == tokens.len() && (bare_keyword(tokens[at]) || clip_word(tokens[at])) => Some(tokens.join(" ")),
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

    let animated = match tokens.get(at).and_then(|token| want_of(token)) {
        Some(animated) => {
            at += 1;
            animated
        }
        None => inferred(&tokens[at..], kind)?,
    };
    let rest = &tokens[at..];

    let want = if animated { Want::Motion(motion_of(rest)?) } else { Want::Stats(stats_of(rest, kind)?) };

    Ok(Request { unit, want, whole, form, search, tail })
}

fn stats_of(tokens: &[&str], kind: Kind) -> Result<Stats, String> {
    let mut stats = Stats::default();
    let mut at = 0;

    while let Some(token) = tokens.get(at) {
        at += 1;

        match lower(token).as_str() {
            "--talents" | "-t" if kind == Kind::Cat => {
                let end = tokens[at..].iter().position(|next| is_flag(next)).map_or(tokens.len(), |offset| at + offset);

                stats.talents = Some(talents_of(&tokens[at..end]));
                at = end;
            }
            _ if kind == Kind::Cat && stats.level.is_none() && level_of(token).is_some() => stats.level = level_of(token),
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
                motion.format = match tokens.get(at).map(|next| lower(next)).as_deref() {
                    Some("avif" | "a") => Some(Format::Avif),
                    Some("webp" | "w") => Some(Format::WebP),
                    Some("gif" | "g") => Some(Format::Gif),
                    _ => return Err("`--format` takes `avif` (`a`), `webp` (`w`) or `gif` (`g`)".to_owned()),
                };
                at += 1;
            }
            "--cull" => {
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
        let model = motion("25 m -f gif -b -c --cull 40").unwrap();

        assert_eq!((model.clip, model.format, model.background, model.overlays.combat, model.cull), (Clip::Model, Some(Format::Gif), true, true, Some(40)));
        assert_eq!(motion("bahamut cat a model").unwrap().clip, Clip::Model);
        assert_eq!(motion("25 model").unwrap().clip, Clip::Model);
        assert!(motion("25 model -r 0~10").unwrap_err().contains("--range"));
        assert!(motion("25 m -l").unwrap_err().contains("--loop"));
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
        assert_eq!(parse("jamiera cat s 30", Kind::Cat).map(|request| request.unit), Ok(Unit::Name("jamiera cat".to_owned())));
        assert_eq!(stats("jamiera cat s 30").level.as_deref(), Some("30"));
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
        assert_eq!(stats("25 -t show").talents, Some(vec![Talent::Show]));
        assert_eq!(stats("25 -t sh").talents, Some(vec![Talent::Show]));
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
        assert!(parse("25 a walk -l -r 50~10", Kind::Cat).is_err());
    }
}
