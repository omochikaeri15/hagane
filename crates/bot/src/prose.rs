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
    NoBackground,
    NoStage,
    StageTooBig,
    NoCrown(String, u8),
    LockedCrown(String, u8),
    Still(u32, &'static str),
    BadFrames(i32),
    BadSize { side: u32, megapixels: u32 },
    TooLong,
    TooBig,
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
            Trouble::NoBackground => NO_BACKGROUND.said(prose).to_owned(),
            Trouble::NoStage => NO_STAGE.said(prose).to_owned(),
            Trouble::StageTooBig => STAGE_TOO_BIG.said(prose).to_owned(),
            Trouble::NoCrown(stage, count) => NO_CROWN.filled(prose, &[("stage", stage), ("count", &count.to_string())]),
            Trouble::LockedCrown(stage, crown) => LOCKED_CROWN.filled(prose, &[("stage", stage), ("crown", &crown.to_string())]),
            Trouble::Still(id, flag) => STILL.filled(prose, &[("id", &id.to_string()), ("flag", flag)]),
            Trouble::BadFrames(last) => BAD_FRAMES.filled(prose, &[("last", &last.to_string())]),
            Trouble::BadSize { side, megapixels } => BAD_SIZE.filled(prose, &[("side", &side.to_string()), ("megapixels", &megapixels.to_string())]),
            Trouble::TooLong => TOO_LONG.said(prose).to_owned(),
            Trouble::TooBig => OVER_LIMIT.said(prose).to_owned(),
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

pub(crate) const NO_CAT: Line = Line::new("Unable to find Cat!", "Hmph. There's no cat like that, baka! Check your spelling!");
pub(crate) const NO_ENEMY: Line = Line::new("Unable to find Enemy!", "That enemy doesn't exist! D-Don't make me look for things that aren't there!");
pub(crate) const NO_UNIT: Line = Line::new("`{word}` isn't a command I know, and I couldn't find a cat, enemy, stage or map like that either. See `{prefix}help`", "`{word}` isn't a command, and there's no cat, enemy, stage or map like that either, dummy! Go read `{prefix}help` already!");
pub(crate) const NO_BACKGROUND: Line = Line::new("Unable to find Background!", "There's no background with that ID, dummy! Count again!");
pub(crate) const NO_AUDIO: Line = Line::new("Unable to find Audio!", "There's no track with that ID! W-What were you expecting to hear?!");
pub(crate) const STILL: Line = Line::new("Background {id} doesn't move, so `{flag}` has nothing to change", "Background {id} doesn't even move, so `{flag}` does nothing! Pay attention!");
pub(crate) const BAD_FRAMES: Line = Line::new("A background's `--range` goes from a lower frame to a higher one, from 0 up to {last}", "A background's `--range` goes from a lower frame to a higher one, from 0 up to {last}! Is that so hard?!");
pub(crate) const BAD_SIZE: Line = Line::new("A background can be from 2 to {side} on a side, and up to {megapixels} megapixels in all", "A background can be from 2 to {side} on a side, and up to {megapixels} megapixels in all! I'm not painting you a mural!");
pub(crate) const NO_STAGE: Line = Line::new("Unable to find Stage!", "There's no stage like that, dummy! Did you make it up?!");
pub(crate) const STAGE_TOO_BIG: Line = Line::new("That stage is too big to export", "That stage is way too big to export! What do you take me for, a printing press?!");
pub(crate) const NO_CROWN: Line = Line::new("{stage} only goes up to {count} \u{1F732}", "{stage} only goes up to {count} \u{1F732}, dummy! Count them yourself!");
pub(crate) const LOCKED_CROWN: Line = Line::new("{stage} isn't open at {crown} \u{1F732}", "{stage} isn't even open at {crown} \u{1F732}! Pick one that is!");
pub(crate) const NO_BOSS_THEME: Line = Line::new("{stage} has no boss theme of its own", "{stage} doesn't even have a boss theme of its own! Listen to the normal one!");
pub(crate) const NO_PLACE: Line = Line::new("Unable to find Stage or Map!", "There's no stage or map like that, dummy! Did you make it up?!");
pub(crate) const NO_MAP: Line = Line::new("Unable to find Map!", "There's no map like that, dummy! Are you lost already?!");
pub(crate) const EMPTY_MAP: Line = Line::new("{map} has no stages", "{map} doesn't even have any stages! What did you expect to find?!");
pub(crate) const MAP_CHOOSE: Line = Line::new("Choose a stage for **{map}**", "Choose a stage for **{map}** already! I'm not picking for you!");
pub(crate) const HOME_MAP: Line = Line::new("A map by name, to pick one of its stages", "A map by name, to pick one of its stages. Try not to get lost");
pub(crate) const PAGE_MAP: Line = Line::new("Finds a map and lists its stages for you to pick from. The stage you pick comes the way `hg!stage` sends it.", "Finds a map and lists its stages for you to pick from. The stage you pick comes the way `hg!stage` sends it. I-It's not like I drew the map for you!");
pub(crate) const HOME_STAGE: Line = Line::new("A stage's details by name, as a picture", "A stage's details by name, as a picture. Don't blame me when you lose it anyway");
pub(crate) const PAGE_STAGE: Line = Line::new("The stage's page as a picture: its information, drops and every enemy it sends. A long one comes as several pictures, with the enemies after the first.", "The stage's page as a picture: its information, drops and every enemy it sends. A long one comes as several pictures, with the enemies after the first. I-It's not like I counted them all for you!");
pub(crate) const BUSY: Line = Line::new("Sorry! But I am busy encoding at the moment; check back later maybe?", "Can't you see I'm busy encoding?! Come back later... n-not that I'll be waiting for you or anything!");
pub(crate) const WARMING: Line = Line::new("Still loading the game data, try again in a few minutes", "I'm still loading the game data, okay?! Give me a few minutes, geez!");
pub(crate) const INVITE: Line = Line::new("If you want to invite me to a server, ask my creator to both whitelist your server ID and to provide to you an invite link!", "You want me in your server? Hmph... ask my creator to both whitelist your server ID and to give you an invite link. I-It's not like I want to go or anything!");
pub(crate) const UNLISTED: Line = Line::new("If you want to enable me in a server, ask my creator to whitelist your server ID!", "I'm not lifting a finger in this server! If you want me that badly, ask my creator to whitelist your server ID!");
pub(crate) const DM_ENCODE: Line = Line::new("I only encode new animations in servers I'm a member of! If you want that here, ask my creator to whitelist your user ID!", "I'm not encoding something new for you here! Do it in a server I'm actually in, or ask my creator to whitelist your user ID, dummy!");
pub(crate) const NOT_YOURS: Line = Line::new("Only the person who asked can change this one", "Hands off! That one isn't yours, only the person who asked gets to change it!");
pub(crate) const FAILED: Line = Line::new("Something went wrong on my side", "S-Something went wrong on my side... it's not my fault, okay?!");
pub(crate) const NO_LOOP: Line = Line::new("Couldn't find loop!", "I looked everywhere and there's no loop! D-Don't look at me like that!");
pub(crate) const NO_BOUNDS: Line = Line::new("Nothing in that animation is visible!", "There's nothing to see in that animation! What did you expect me to draw, air?!");
pub(crate) const TOO_LONG: Line = Line::new("That one took too long to encode!", "That one took way too long to encode! I do have limits, you know!");
pub(crate) const OVERSIZED: Line = Line::new("That one comes out at {size} MB, over Discord's 20 MB upload limit. Try `--format avif`, which is far smaller", "That one comes out at {size} MB, and Discord only takes 20 MB! Try `--format avif`, it's far smaller... n-not that I'm helping you or anything!");
pub(crate) const OVER_LIMIT: Line = Line::new("That one went past Discord's 20 MB upload limit, so I stopped encoding it. Try `--format avif`, which is far smaller", "That one blew past Discord's 20 MB limit, so I stopped! Try `--format avif`, it's far smaller... n-not that I'm helping you or anything!");
pub(crate) const SEARCH_PICK: Line = Line::new("Here's everything close to `{name}`. Pick one:", "You don't even know its name?! Fine, here's everything close to `{name}`. Pick one already!");
pub(crate) const EITHER_PICK: Line = Line::new("`{name}` could be more than one thing. Pick one:", "`{name}` could be more than one thing, so which is it?! Pick one already!");
pub(crate) const RESOLVE_PANEL: Line = Line::new("How the statblock above is worked out. Only you can see this", "How the statblock above is worked out. Only you can see this, so don't go bragging!");
pub(crate) const PANEL_GONE: Line = Line::new("That panel is too old to change the statblock. Press {button} again", "That panel went stale! Press {button} again, dummy!");
pub(crate) const VERSUS_PANEL: Line = Line::new("What the statblock above is up against. Only you can see this", "What the statblock above is up against. Only you can see this, so don't get any ideas!");
pub(crate) const VERSUS_FOUND: Line = Line::new("Against **{enemy}**, with its traits picked below", "Against **{enemy}**?! Fine, its traits are picked below!");
pub(crate) const LIST_GONE: Line = Line::new("That list is too old to turn. Ask again for a fresh one", "That list is ancient history! Ask again if you want a fresh one, dummy!");
pub(crate) const SEARCH_ABORT: Line = Line::new("Okay! I've dropped that request", "Fine, forget it then! I-It's not like I wanted to look it up for you anyway!");
pub(crate) const SEARCH_LONG: Line = Line::new("That request is too long to search with. Find the unit first, then ask again with its ID", "That's way too much to search with! Find the unit first, then ask again with its ID, dummy!");
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
pub(crate) const NOT_A_LEVEL: Line = Line::new("A level looks like `50`, `50+70` or `lv30`", "That's not a level! It looks like `50`, `50+70` or `lv30`, got it?!");
pub(crate) const NOT_A_PERCENT: Line = Line::new("A magnification looks like `400%` or `400%/200%`", "That's not a magnification, baka! It looks like `400%` or `400%/200%`, got it?!");
pub(crate) const PREFIX_SPACED: Line = Line::new("A prefix can't have spaces in it", "A prefix can't have spaces in it, dummy!");
pub(crate) const PREFIX_LONG: Line = Line::new("A prefix can be {limit} characters at most", "{limit} characters at most! I'm not memorizing a whole speech just for you!");
pub(crate) const HELP_HOME: Line = Line::new("Use `hg!`, mention me, reply to me, DM me, or use `/`.\nLeave out the command and I look through cats, enemies, stages and maps: `hg!bahamut`, `hg!cyclone a walk`.\nThe list below opens each command, or use `hg!help <command>`.", "Use `hg!`, mention me, reply to me, DM me, or use `/`. I-It's not like I'm happy when you do!\nLeave out the command and I'll look through cats, enemies, stages and maps, like `hg!bahamut` or `hg!cyclone a walk`. Honestly, how lazy can you get?!\nThe list below opens each command, or use `hg!help <command>`. Don't make me explain it twice!");
pub(crate) const HOME_CAT: Line = Line::new("A cat's statblock or animation, by ID or name", "A cat's statblock or animation, by ID or name. Fine, I'll fetch it");
pub(crate) const HOME_ENEMY: Line = Line::new("An enemy's statblock or animation, by ID or name", "An enemy's statblock or animation, by ID or name. Don't blame me if it's scary");
pub(crate) const HOME_BACKGROUND: Line = Line::new("A background by ID, or a stage's by its name, as a picture or an animation if it moves", "A background by ID, or a stage's by its name, as a picture or an animation if it moves. It's just scenery, don't get excited");
pub(crate) const HOME_AUDIO: Line = Line::new("A music track or sound effect by ID, or a stage's theme by its name", "A music track or sound effect by ID, or a stage's theme by its name. D-Don't play it too loud!");
pub(crate) const PAGE_BACKGROUND: Line = Line::new("The background with that ID, as the game shows it on a 960 by 640 screen. One that moves comes as an animation, and one that doesn't comes as a single picture.", "The background with that ID, as the game shows it on a 960 by 640 screen. One that moves comes as an animation, and one that doesn't comes as a single picture. D-Don't just sit there staring at it!");
pub(crate) const PAGE_AUDIO: Line = Line::new("The music track or sound effect with that ID, or the theme of the stage with that name. A map's name lists its stages to pick one from.", "The music track or sound effect with that ID, or the theme of the stage with that name. A map's name lists its stages to pick one from. I-It's not like I was humming along!");
pub(crate) const HOME_SETTINGS: Line = Line::new("Your language, image format, default form, prefix and aliases", "Your language, image format, default form, prefix and aliases. Have it your way, see if I care");
pub(crate) const HOME_WEBSITE: Line = Line::new("A link to the Hagane website", "A link to the Hagane website, if you really must");
pub(crate) const HOME_INVITE: Line = Line::new("How to get me into a server", "How to get me into a server... n-not that I want to be invited!");
pub(crate) const HOME_LEGAL: Line = Line::new("Licenses, and who I'm not affiliated with", "Licenses, and who I'm not affiliated with. The boring part");
pub(crate) const HOME_HELP: Line = Line::new("This list, or one command's page", "This list, or one command's page. You're already reading it, dummy");
pub(crate) const PAGE_SETTINGS: Line = Line::new("Your own defaults, changed with buttons. `/settings` opens them just for you; `hg!settings` gives a button that opens yours privately.", "Your own defaults, changed with buttons. `/settings` opens them just for you; `hg!settings` gives a button that opens yours privately. I-It's not like I care what you pick!");
pub(crate) const PAGE_WEBSITE: Line = Line::new("A link to the Hagane website.", "A link to the Hagane website. Go on, click it already!");
pub(crate) const PAGE_INVITE: Line = Line::new("How to get me into another server.", "How to get me into another server... as if I'd be lonely here!");
pub(crate) const PAGE_LEGAL: Line = Line::new("The licenses for everything I'm built from, and who I'm not affiliated with.", "The licenses for everything I'm built from, and who I'm not affiliated with. D-Don't go reading all of it!");
pub(crate) const PAGE_HELP: Line = Line::new("`hg!help` lists every command. `hg!help <command>` opens one, aliases included, like `hg!help c`. The list below switches between them.", "`hg!help` lists every command. `hg!help <command>` opens one, aliases included, like `hg!help c`. The list below switches between them, since you clearly need the help!");
pub(crate) const PICK_LAPSED: Line = Line::new("Nothing was picked for {minutes} minutes, so I've dropped that request", "{minutes} whole minutes and you still couldn't pick?! Fine, I've dropped it... n-not that I was waiting on you or anything!");
pub(crate) const DO_NOTHING: Line = Line::new("Okay! I will do nothing", "Fine! I'll do nothing! I-It's not like I wanted to do something for you anyway!");
pub(crate) const NOT_A_TRAIT: Line = Line::new("`{name}` isn't a trait or an enemy I know. Traits are red, floating, black, metal, angel, alien, zombie, relic, aku, traitless, witch, eva, colossus, behemoth and sage, or `targets`, `traits`, `subtraits` and `all`", "`{name}` isn't a trait or an enemy, dummy! Traits are red, floating, black, metal, angel, alien, zombie, relic, aku, traitless, witch, eva, colossus, behemoth and sage, or `targets`, `traits`, `subtraits` and `all`!");
pub(crate) const ALIAS_TITLE: Line = Line::new("Your aliases", "Your aliases, I guess");
pub(crate) const ALIAS_NONE: Line = Line::new("You have no aliases", "You don't have a single alias! Add one first, dummy!");
pub(crate) const ALIAS_UNREADABLE: Line = Line::new("An alias needs at least one letter or number", "An alias needs at least one letter or number, dummy!");
pub(crate) const ALIAS_IS_ID: Line = Line::new("`{alias}` reads as an ID, so it can't be an alias", "`{alias}` is an ID, dummy! It can't be an alias too!");
pub(crate) const ALIAS_SPLIT: Line = Line::new("`{alias}` has a word or a flag in it that I read as something else, so it can't be an alias", "`{alias}` has a word or a flag in it that already means something else! Pick another alias, geez!");
pub(crate) const ALIAS_FULL: Line = Line::new("You can keep {limit} aliases at most. Remove one first", "{limit} aliases is the limit! Remove one first, I'm not your notebook!");
pub(crate) const ALIAS_NOWHERE: Line = Line::new("`{target}` doesn't find anything, so I didn't save `{alias}`", "`{target}` doesn't find anything, so I'm not saving `{alias}`! Check your spelling!");
pub(crate) const ALIAS_UNREAD: Line = Line::new("I couldn't read `{target}`, so I didn't save `{alias}`: {reason}", "I can't even read `{target}`, so I'm not saving `{alias}`! {reason}");
pub(crate) const ALIAS_ADDED: Line = Line::new("`{alias}` now opens {unit}", "Fine, `{alias}` opens {unit} now. I-It's not like I'll remember it for your sake!");
pub(crate) const ALIAS_ADDED_LIST: Line = Line::new("`{alias}` now searches `{target}`, which gives a list to pick from", "`{alias}` searches `{target}` now, and that gives a whole list to pick from. Don't blame me for it!");
pub(crate) const ALIAS_SHADOWED: Line = Line::new("`{prefix}{word}` is a command, so this alias only works after one, like `{prefix}cat {alias}`", "`{prefix}{word}` is a command, dummy, so this alias only works after one, like `{prefix}cat {alias}`!");
pub(crate) const ALIAS_REMOVE_PROMPT: Line = Line::new("Pick an alias to remove it", "Pick the alias you want gone already!");
pub(crate) const ALIAS_REMOVED: Line = Line::new("Removed `{alias}`", "`{alias}` is gone. Happy now?!");
pub(crate) const ALIAS_GONE: Line = Line::new("That alias isn't there anymore", "That alias is already gone, dummy!");
pub(crate) const ALIAS_CLEAR_ASK: Line = Line::new("Remove every alias, the built-in ones included?", "Remove every alias, even the built-in ones?! Are you sure about that?!");
pub(crate) const ALIAS_RESET_ASK: Line = Line::new("Remove your own aliases and bring the built-in ones back?", "Throw out your own aliases and bring the built-in ones back?! Make up your mind!");
pub(crate) const ALIAS_CLEARED: Line = Line::new("Every alias is removed", "They're all gone! Every last one!");
pub(crate) const ALIAS_RESET: Line = Line::new("Your aliases are back to the built-in ones", "Your aliases are back to the built-in ones. D-Don't mess them up again!");

#[cfg(test)]
pub(crate) const LINES: [&Line; 98] = [
    &NOT_A_TRAIT,
    &ALIAS_TITLE,
    &ALIAS_NONE,
    &ALIAS_UNREADABLE,
    &ALIAS_IS_ID,
    &ALIAS_SPLIT,
    &ALIAS_FULL,
    &ALIAS_NOWHERE,
    &ALIAS_UNREAD,
    &ALIAS_ADDED,
    &ALIAS_ADDED_LIST,
    &ALIAS_SHADOWED,
    &ALIAS_REMOVE_PROMPT,
    &ALIAS_REMOVED,
    &ALIAS_GONE,
    &ALIAS_CLEAR_ASK,
    &ALIAS_RESET_ASK,
    &ALIAS_CLEARED,
    &ALIAS_RESET,
    &PICK_LAPSED,
    &LIST_GONE,
    &RESOLVE_PANEL,
    &PANEL_GONE,
    &VERSUS_PANEL,
    &VERSUS_FOUND,
    &NO_PLACE,
    &NO_MAP,
    &EMPTY_MAP,
    &MAP_CHOOSE,
    &HOME_MAP,
    &PAGE_MAP,
    &NO_BOSS_THEME,
    &NO_CROWN,
    &LOCKED_CROWN,
    &NO_STAGE,
    &STAGE_TOO_BIG,
    &HOME_STAGE,
    &PAGE_STAGE,
    &BAD_SIZE,
    &HOME_BACKGROUND,
    &HOME_AUDIO,
    &PAGE_BACKGROUND,
    &PAGE_AUDIO,
    &NO_BACKGROUND,
    &NO_AUDIO,
    &STILL,
    &BAD_FRAMES,
    &NO_CAT,
    &NO_ENEMY,
    &NO_UNIT,
    &EITHER_PICK,
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
    &OVER_LIMIT,
    &SEARCH_PICK,
    &SEARCH_LONG,
    &SEARCH_ABORT,
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
    &NOT_A_LEVEL,
    &NOT_A_PERCENT,
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
