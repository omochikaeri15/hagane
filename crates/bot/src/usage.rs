use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use super::whitelist;

pub(crate) const ALERT_CHANNEL: u64 = 1544357159259742398;
const WINDOW: Duration = Duration::from_secs(30 * 60);
const HOG: Duration = Duration::from_secs(15 * 60);
const REPEAT: Duration = Duration::from_secs(30 * 60);
const UNLISTED_SERVER: &str = "Unlisted server";
const UNLISTED_USER: &str = "Unlisted user";
const MINUTE: u64 = 60;
const HOUR: u64 = 60 * MINUTE;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Asker {
    Server(u64),
    User(u64),
}

impl Asker {
    pub(crate) fn id(self) -> u64 {
        match self {
            Asker::Server(id) | Asker::User(id) => id,
        }
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Asker::Server(id) => whitelist::server(id).unwrap_or(UNLISTED_SERVER),
            Asker::User(id) => whitelist::user(id).unwrap_or(UNLISTED_USER),
        }
    }
}

pub(crate) struct Tally {
    pub(crate) sessions: usize,
    pub(crate) busy: Duration,
    pub(crate) alert: Option<String>,
}

struct Spree {
    began: Instant,
    sessions: usize,
    warned: Instant,
}

#[derive(Default)]
struct Record {
    recent: VecDeque<(Instant, Duration)>,
    spree: Option<Spree>,
}

#[derive(Default)]
pub(crate) struct Ledger {
    records: HashMap<Asker, Record>,
}

impl Ledger {
    pub(crate) fn note(&mut self, asker: Asker, started: Instant, ended: Instant) -> Tally {
        let record = self.records.entry(asker).or_default();

        record.recent.push_back((started, ended.saturating_duration_since(started)));

        while record.recent.front().is_some_and(|(start, _)| ended.saturating_duration_since(*start) > WINDOW) {
            record.recent.pop_front();
        }

        let sessions = record.recent.len();
        let busy: Duration = record.recent.iter().map(|(_, held)| *held).sum();

        if busy < HOG {
            record.spree = None;
            return Tally { sessions, busy, alert: None };
        }

        let fresh = record.spree.is_none();
        let began = record.recent.front().map_or(started, |(start, _)| *start);
        let spree = record.spree.get_or_insert(Spree { began, sessions: 0, warned: ended });

        spree.sessions = if fresh { sessions } else { spree.sessions + 1 };

        let due = fresh || ended.saturating_duration_since(spree.warned) >= REPEAT;

        if due {
            spree.warned = ended;
        }

        let alert = due.then(|| format!("**ABUSE ALERT:** {} ({}) has requested {} encoding sessions within {}!", asker.name(), asker.id(), spree.sessions, span(ended.saturating_duration_since(spree.began))));

        Tally { sessions, busy, alert }
    }
}

fn span(length: Duration) -> String {
    let seconds = length.as_secs();
    let minutes = (seconds + MINUTE / 2) / MINUTE;

    if seconds < MINUTE {
        format!("{seconds}s")
    } else if minutes < HOUR / MINUTE {
        format!("{minutes}m")
    } else {
        format!("{}h", (seconds + HOUR / 2) / HOUR)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIKI: Asker = Asker::Server(348741070990868481);
    const LONG: Duration = Duration::from_secs(3 * 60);

    fn back_to_back(ledger: &mut Ledger, asker: Asker, from: Instant, count: u32) -> Vec<Option<String>> {
        (0..count).map(|at| ledger.note(asker, from + LONG * at, from + LONG * (at + 1)).alert).collect()
    }

    // Fifteen minutes on the encoder inside half an hour is the line, and a spree that keeps going is reported again every half hour.
    #[test]
    fn a_server_that_holds_the_encoder_is_reported_once_per_half_hour() {
        let mut ledger = Ledger::default();
        let alerts = back_to_back(&mut ledger, WIKI, Instant::now(), 20);
        let raised: Vec<(usize, &str)> = alerts.iter().enumerate().filter_map(|(at, alert)| alert.as_deref().map(|text| (at, text))).collect();

        assert_eq!(
            raised,
            [
                (4, "**ABUSE ALERT:** The Battle Cats Wiki Official Discord (348741070990868481) has requested 5 encoding sessions within 15m!"),
                (14, "**ABUSE ALERT:** The Battle Cats Wiki Official Discord (348741070990868481) has requested 15 encoding sessions within 45m!"),
            ]
        );
    }

    // Plenty of short encodes spread out is ordinary use, and one server's time never counts against another.
    #[test]
    fn ordinary_use_stays_quiet() {
        let mut ledger = Ledger::default();
        let from = Instant::now();
        let minute = Duration::from_secs(60);
        let short = Duration::from_secs(10);
        let busy = (0..120).map(|at| ledger.note(WIKI, from + minute * at, from + minute * at + short)).last().map(|tally| (tally.sessions, tally.busy, tally.alert));

        assert_eq!(busy, Some((30, short * 30, None)));
        assert!(back_to_back(&mut ledger, Asker::User(1360862647578394704), from, 4).iter().all(Option::is_none));
        assert!(back_to_back(&mut ledger, Asker::Server(1456204440951455786), from, 4).iter().all(Option::is_none));
    }

    // A server that goes quiet starts from nothing the next time.
    #[test]
    fn a_spree_ends_when_the_encoder_is_left_alone() {
        let mut ledger = Ledger::default();
        let from = Instant::now();
        let first = back_to_back(&mut ledger, WIKI, from, 5);
        let later = back_to_back(&mut ledger, WIKI, from + Duration::from_secs(2 * 60 * 60), 5);

        assert!(first.last().is_some_and(Option::is_some));
        assert_eq!(later.last().and_then(|alert| alert.as_deref()), Some("**ABUSE ALERT:** The Battle Cats Wiki Official Discord (348741070990868481) has requested 5 encoding sessions within 15m!"));
    }

    #[test]
    fn spans_read_in_one_unit() {
        assert_eq!(span(Duration::from_secs(45)), "45s");
        assert_eq!(span(Duration::from_secs(15 * 60)), "15m");
        assert_eq!(span(Duration::from_secs(59 * 60 + 40)), "1h");
        assert_eq!(span(Duration::from_secs(3 * 60 * 60)), "3h");
    }
}
