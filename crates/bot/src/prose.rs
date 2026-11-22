use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Prose {
    #[default]
    Human,
    Tsundere,
}

impl Prose {
    pub(crate) const ALL: [Prose; 2] = [Prose::Human, Prose::Tsundere];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Prose::Human => "Human",
            Prose::Tsundere => "Tsundere",
        }
    }

    pub(crate) fn slug(self) -> &'static str {
        match self {
            Prose::Human => "human",
            Prose::Tsundere => "tsundere",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Line {
    human: &'static str,
    tsundere: &'static str,
}

impl Line {
    const fn new(human: &'static str, tsundere: &'static str) -> Self {
        Self { human, tsundere }
    }

    pub(crate) fn said(&self, prose: Prose) -> &'static str {
        match prose {
            Prose::Human => self.human,
            Prose::Tsundere => self.tsundere,
        }
    }

    pub(crate) fn filled(&self, prose: Prose, parts: &[(&str, &str)]) -> String {
        let mut rest = self.said(prose);
        let mut text = String::with_capacity(rest.len());

        while let Some(open) = rest.find('{') {
            let tail = &rest[open + 1..];
            let part = tail.find('}').and_then(|close| parts.iter().find(|(name, _)| *name == &tail[..close]).map(|(_, value)| (close, value)));

            text.push_str(&rest[..open]);

            match part {
                Some((close, value)) => {
                    text.push_str(value);
                    rest = &tail[close + 1..];
                }
                None => {
                    text.push('{');
                    rest = tail;
                }
            }
        }

        text.push_str(rest);
        text
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Trouble {
    NoCat,
    NoEnemy,
    NoLoop,
    NoBounds,
    TooLong,
    TooMany { frames: i32, limit: i32 },
    NoStats(String),
    NoTalents(String),
    FewTalents(String, usize),
    NoTalentLike(String, String),
    Other(String),
}

impl From<String> for Trouble {
    fn from(reason: String) -> Self {
        Trouble::Other(reason)
    }
}

impl Trouble {
    pub(crate) fn told(&self, prose: Prose) -> String {
        match self {
            Trouble::NoCat => NO_CAT.said(prose).to_owned(),
            Trouble::NoEnemy => NO_ENEMY.said(prose).to_owned(),
            Trouble::NoLoop => NO_LOOP.said(prose).to_owned(),
            Trouble::NoBounds => NO_BOUNDS.said(prose).to_owned(),
            Trouble::TooLong => TOO_LONG.said(prose).to_owned(),
            Trouble::TooMany { frames, limit } => TOO_MANY.filled(prose, &[("frames", &frames.to_string()), ("limit", &limit.to_string())]),
            Trouble::NoStats(unit) => NO_STATS.filled(prose, &[("unit", unit)]),
            Trouble::NoTalents(unit) => NO_TALENTS.filled(prose, &[("unit", unit)]),
            Trouble::FewTalents(unit, count) => FEW_TALENTS.filled(prose, &[("unit", unit), ("count", &count.to_string())]),
            Trouble::NoTalentLike(unit, name) => NO_TALENT_LIKE.filled(prose, &[("unit", unit), ("name", name)]),
            Trouble::Other(reason) => COULD_NOT.filled(prose, &[("reason", reason)]),
        }
    }

    pub(crate) fn unexpected(&self) -> Option<&str> {
        match self {
            Trouble::Other(reason) => Some(reason),
            _ => None,
        }
    }
}

pub(crate) const NO_CAT: Line = Line::new("Unable to find Cat!", "Hmph. There's no cat like that, dummy! Check your spelling!");
pub(crate) const NO_ENEMY: Line = Line::new("Unable to find Enemy!", "That enemy doesn't exist! D-Don't make me look for things that aren't there!");
pub(crate) const BUSY: Line = Line::new("Sorry! But I am busy encoding at the moment; check back later maybe?", "Can't you see I'm busy encoding?! Come back later... n-not that I'll be waiting for you or anything!");
pub(crate) const WARMING: Line = Line::new("Still loading the game data, try again in a few minutes", "I'm still loading the game data, okay?! Give me a few minutes, geez!");
pub(crate) const INVITE: Line = Line::new("If you want to invite me to a server, ask my creator to both whitelist your server ID and to provide to you an invite link!", "You want me in your server? Hmph... ask my creator to both whitelist your server ID and to give you an invite link. I-It's not like I want to go or anything!");
pub(crate) const UNLISTED: Line = Line::new("If you want to enable me in a server, ask my creator to whitelist your server ID!", "I'm not lifting a finger in this server! If you want me that badly, ask my creator to whitelist your server ID!");
pub(crate) const DM_ENCODE: Line = Line::new("I only encode new animations in servers! If you want that in DMs, ask my creator to whitelist your user ID!", "I'm not encoding something new for you in private! Do it in a server, or ask my creator to whitelist your user ID, dummy!");
pub(crate) const NOT_YOURS: Line = Line::new("Only the person who asked can change this one", "Hands off! That one isn't yours, only the person who asked gets to change it!");
pub(crate) const FAILED: Line = Line::new("Something went wrong on my side", "S-Something went wrong on my side... it's not my fault, okay?!");
pub(crate) const NO_LOOP: Line = Line::new("Couldn't find loop!", "I looked everywhere and there's no loop! D-Don't look at me like that!");
pub(crate) const NO_BOUNDS: Line = Line::new("Nothing in that animation is visible!", "There's nothing to see in that animation! What did you expect me to draw, air?!");
pub(crate) const TOO_LONG: Line = Line::new("That one took too long to encode!", "That one took way too long to encode! I do have limits, you know!");
pub(crate) const OVERSIZED: Line = Line::new("That one comes out at {size} MB, over Discord's 20 MB upload limit. Try `--format avif`, which is far smaller", "That one comes out at {size} MB, and Discord only takes 20 MB! Try `--format avif`, it's far smaller... n-not that I'm helping you or anything!");
pub(crate) const COULD_NOT: Line = Line::new("I couldn't make that one: {reason}", "I couldn't make that one, okay?! {reason}");
pub(crate) const TOO_MANY: Line = Line::new("That's {frames} frames, and the most I'll make is {limit}", "{frames} frames?! The most I'll make is {limit}, so don't push your luck!");
pub(crate) const NO_CLIP: Line = Line::new("{unit} has no {clip} animation", "{unit} doesn't even have a {clip} animation! Do your homework!");
pub(crate) const NO_FORM: Line = Line::new("{unit} has no {form} form", "{unit} has no {form} form, dummy!");
pub(crate) const NO_STATS: Line = Line::new("{unit} has no stats for that form", "{unit} has no stats for that form! What do you want me to do, make them up?!");
pub(crate) const SEE_HELP: Line = Line::new("{reason}. See `{prefix}help`", "{reason}. Go read `{prefix}help` already, geez!");
pub(crate) const NO_PAGE: Line = Line::new("I don't have a command called `{name}`. See `{prefix}help`", "I don't have a command called `{name}`! Read `{prefix}help` before you ask, dummy!");
pub(crate) const SETTINGS_PROMPT: Line = Line::new("Settings are private, so they open just for whoever presses this:", "Settings are private, so they only open for whoever presses this. N-No peeking at anyone else's!");
pub(crate) const SETTINGS_TITLE: Line = Line::new("Your settings", "Your settings, I guess");
pub(crate) const SETTINGS_NOTE: Line = Line::new("Changes save as soon as you press them.", "They save the moment you press them. I-It's not like I'm remembering them for your sake!");
pub(crate) const WEBSITE: Line = Line::new("https://hagane.app/", "Here, just take it already: https://hagane.app/ ... i-it's not like I want you to visit!");
pub(crate) const NO_TALENTS: Line = Line::new("{unit} has no talents", "{unit} doesn't have any talents! What were you expecting?!");
pub(crate) const FEW_TALENTS: Line = Line::new("{unit} only has {count} talents", "{unit} only has {count} talents! Can't you count?!");
pub(crate) const NO_TALENT_LIKE: Line = Line::new("{unit} has no talent like `{name}`", "{unit} has no talent like `{name}`! Where did you even get that from?!");
pub(crate) const TALENTLESS: Line = Line::new("No talents", "No talents. None. Stop staring!");
pub(crate) const NOT_A_LEVEL: Line = Line::new("A level looks like `50`, `50+70` or `lv30`", "That's not a level! It looks like `50`, `50+70` or `lv30`, got it?!");
pub(crate) const PREFIX_SPACED: Line = Line::new("A prefix can't have spaces in it", "A prefix can't have spaces in it, dummy!");
pub(crate) const PREFIX_LONG: Line = Line::new("A prefix can be {limit} characters at most", "{limit} characters at most! I'm not memorizing a whole speech just for you!");
pub(crate) const HELP_HOME: Line = Line::new("Use `hg!`, mention me, reply to me, DM me, or use `/`.\nThe buttons below open each command, or use `hg!help <command>`.", "Use `hg!`, mention me, reply to me, DM me, or use `/`. I-It's not like I'm happy when you do!\nThe buttons below open each command, or use `hg!help <command>`. Don't make me explain it twice!");
pub(crate) const HOME_CAT: Line = Line::new("A cat's statblock or animation, by ID or name", "A cat's statblock or animation, by ID or name. Fine, I'll fetch it");
pub(crate) const HOME_ENEMY: Line = Line::new("An enemy's statblock or animation, by ID or name", "An enemy's statblock or animation, by ID or name. Don't blame me if it's scary");
pub(crate) const HOME_SETTINGS: Line = Line::new("Your language, image format, default form and prefix", "Your language, image format, default form and prefix. Have it your way, see if I care");
pub(crate) const HOME_WEBSITE: Line = Line::new("A link to the Hagane website", "A link to the Hagane website, if you really must");
pub(crate) const HOME_INVITE: Line = Line::new("How to get me into a server", "How to get me into a server... n-not that I want to be invited!");
pub(crate) const HOME_LEGAL: Line = Line::new("Licenses, and who I'm not affiliated with", "Licenses, and who I'm not affiliated with. The boring part");
pub(crate) const HOME_HELP: Line = Line::new("This list, or one command's page", "This list, or one command's page. You're already reading it, dummy");
pub(crate) const PAGE_SETTINGS: Line = Line::new("Your own defaults, changed with buttons. `/settings` opens them just for you; `hg!settings` gives a button that opens yours privately.", "Your own defaults, changed with buttons. `/settings` opens them just for you; `hg!settings` gives a button that opens yours privately. I-It's not like I care what you pick!");
pub(crate) const PAGE_WEBSITE: Line = Line::new("A link to the Hagane website.", "A link to the Hagane website. Go on, click it already!");
pub(crate) const PAGE_INVITE: Line = Line::new("How to get me into another server.", "How to get me into another server... as if I'd be lonely here!");
pub(crate) const PAGE_LEGAL: Line = Line::new("The licenses for everything I'm built from, and who I'm not affiliated with.", "The licenses for everything I'm built from, and who I'm not affiliated with. D-Don't go reading all of it!");
pub(crate) const PAGE_HELP: Line = Line::new("`hg!help` lists every command. `hg!help <command>` opens one, aliases included, like `hg!help c`. The buttons below switch between them.", "`hg!help` lists every command. `hg!help <command>` opens one, aliases included, like `hg!help c`. The buttons below switch between them, since you clearly need the help!");
pub(crate) const DO_NOTHING: Line = Line::new("Okay! I will do nothing", "Fine! I'll do nothing! I-It's not like I wanted to do something for you anyway!");

#[cfg(test)]
pub(crate) const LINES: [&Line; 45] = [
    &NO_CAT,
    &NO_ENEMY,
    &BUSY,
    &WARMING,
    &INVITE,
    &UNLISTED,
    &DM_ENCODE,
    &NOT_YOURS,
    &FAILED,
    &NO_LOOP,
    &NO_BOUNDS,
    &TOO_LONG,
    &OVERSIZED,
    &COULD_NOT,
    &TOO_MANY,
    &NO_CLIP,
    &NO_FORM,
    &NO_STATS,
    &SEE_HELP,
    &NO_PAGE,
    &SETTINGS_PROMPT,
    &SETTINGS_TITLE,
    &SETTINGS_NOTE,
    &WEBSITE,
    &NO_TALENTS,
    &FEW_TALENTS,
    &NO_TALENT_LIKE,
    &TALENTLESS,
    &NOT_A_LEVEL,
    &PREFIX_SPACED,
    &PREFIX_LONG,
    &HELP_HOME,
    &HOME_CAT,
    &HOME_ENEMY,
    &HOME_SETTINGS,
    &HOME_WEBSITE,
    &HOME_INVITE,
    &HOME_LEGAL,
    &HOME_HELP,
    &PAGE_SETTINGS,
    &PAGE_WEBSITE,
    &PAGE_INVITE,
    &PAGE_LEGAL,
    &PAGE_HELP,
    &DO_NOTHING,
];

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    fn blanks(text: &str) -> BTreeSet<&str> {
        text.split('{').skip(1).filter_map(|tail| tail.split('}').next()).collect()
    }

    // Both voices of a line have to carry the same blanks, or one of them drops a name or a number.
    #[test]
    fn both_voices_fill_the_same_blanks() {
        for line in LINES {
            assert_eq!(blanks(line.human), blanks(line.tsundere), "{}", line.human);
            assert!(!line.human.is_empty() && !line.tsundere.is_empty());
        }
    }

    // A value is put in as it is, even when it looks like a blank itself.
    #[test]
    fn blanks_are_filled_once() {
        assert_eq!(NO_PAGE.filled(Prose::Human, &[("name", "{prefix}"), ("prefix", "hg!")]), "I don't have a command called `{prefix}`. See `hg!help`");
        assert_eq!(NO_CLIP.filled(Prose::Tsundere, &[("unit", "Akaza"), ("clip", "Dig")]), "Akaza doesn't even have a Dig animation! Do your homework!");
        assert_eq!(Trouble::TooMany { frames: 9000, limit: 8400 }.told(Prose::Human), "That's 9000 frames, and the most I'll make is 8400");
        assert_eq!(Trouble::from("the rig broke".to_owned()).told(Prose::Human), "I couldn't make that one: the rig broke");
    }
}
