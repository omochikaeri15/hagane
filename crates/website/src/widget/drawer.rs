use dioxus::prelude::*;

const TOUCH: &str = "touch";
const LOCK: f64 = 12.0;
const SLANT: f64 = 1.3;
const SETTLE: f64 = 60.0;
const FLICK: f64 = 0.35;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Side {
    Left,
    Right,
}

impl Side {
    fn opening(self) -> f64 {
        match self {
            Self::Left => 1.0,
            Self::Right => -1.0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
struct Pull {
    side: Side,
    shift: f64,
}

#[derive(Clone, Copy, PartialEq, Debug)]
struct Swipe {
    pointer: i32,
    from: (f64, f64),
    side: Option<Side>,
    last: (f64, f64),
    speed: f64,
}

#[derive(Clone, Copy)]
pub(crate) struct Drawers {
    pub(crate) compact: Signal<bool>,
    pub(crate) narrow: Signal<bool>,
    pub(crate) nav: Signal<bool>,
    hosted: Signal<Option<Signal<bool>>>,
    pull: Signal<Option<Pull>>,
    swipe: Signal<Option<Swipe>>,
    claimed: Signal<Option<i32>>,
}

pub(crate) fn provide() -> Drawers {
    use_context_provider(|| Drawers {
        compact: Signal::new(false),
        narrow: Signal::new(false),
        nav: Signal::new(false),
        hosted: Signal::new(None),
        pull: Signal::new(None),
        swipe: Signal::new(None),
        claimed: Signal::new(None),
    })
}

pub(crate) fn drawers() -> Drawers {
    use_context::<Drawers>()
}

impl Drawers {
    pub(crate) fn shut(self) {
        if let Some(mut open) = *self.hosted.peek()
            && *open.peek()
        {
            open.set(false);
        }
    }

    pub(crate) fn claim(self, pointer: i32) {
        let mut claimed = self.claimed;

        claimed.set(Some(pointer));
    }

    pub(crate) fn press(self, pointer: i32, kind: &str, at: (f64, f64), now: f64) {
        let mut claimed = self.claimed;
        let mut swipe = self.swipe;
        let taken = claimed.take() == Some(pointer);

        if taken || kind != TOUCH || !*self.compact.peek() || swipe.peek().is_some() {
            return;
        }

        swipe.set(Some(Swipe { pointer, from: at, side: None, last: (at.0, now), speed: 0.0 }));
    }

    pub(crate) fn follow(self, pointer: i32, at: (f64, f64), now: f64) {
        let (mut swipe, mut pull) = (self.swipe, self.pull);
        let Some(mut held) = *swipe.peek() else {
            return;
        };

        if held.pointer != pointer {
            return;
        }

        let (dx, dy) = (at.0 - held.from.0, at.1 - held.from.1);
        let Some(side) = held.side else {
            if dy.abs() > LOCK && dy.abs() >= dx.abs() {
                swipe.set(None);

                return;
            }

            if dx.abs() < LOCK || dx.abs() < dy.abs() * SLANT {
                return;
            }

            let Some(side) = self.aim(dx) else {
                swipe.set(None);

                return;
            };

            held.side = Some(side);
            held.from = at;
            held.last = (at.0, now);
            swipe.set(Some(held));
            pull.set(Some(Pull { side, shift: 0.0 }));

            return;
        };
        let elapsed = now - held.last.1;

        if elapsed > 0.0 {
            held.speed = (at.0 - held.last.0) / elapsed;
        }

        held.last = (at.0, now);
        swipe.set(Some(held));
        pull.set(Some(Pull { side, shift: dx }));
    }

    pub(crate) fn release(self, pointer: i32) {
        let (mut swipe, mut pull) = (self.swipe, self.pull);
        let Some(held) = *swipe.peek() else {
            return;
        };

        if held.pointer != pointer {
            return;
        }

        swipe.set(None);

        let Some(Pull { side, shift }) = pull.take() else {
            return;
        };
        let target = match side {
            Side::Left => *self.hosted.peek(),
            Side::Right => Some(self.nav),
        };
        let Some(mut open) = target else {
            return;
        };
        let was = *open.peek();
        let toward = if was { -side.opening() } else { side.opening() };
        let moved = shift * toward;

        if moved >= SETTLE || (moved > 0.0 && held.speed * toward >= FLICK) {
            open.set(!was);
        }
    }

    pub(crate) fn abandon(self, pointer: i32) {
        let (mut swipe, mut pull) = (self.swipe, self.pull);

        if swipe.peek().is_some_and(|held| held.pointer == pointer) {
            swipe.set(None);
            pull.set(None);
        }
    }

    pub(crate) fn pulled(self, side: Side) -> Option<f64> {
        (self.pull)().filter(|pull| pull.side == side).map(|pull| pull.shift)
    }

    fn aim(self, dx: f64) -> Option<Side> {
        let hosted = *self.hosted.peek();
        let left_open = hosted.is_some_and(|open| *open.peek());

        match (left_open, *self.nav.peek()) {
            (true, _) => (dx < 0.0).then_some(Side::Left),
            (false, true) => (dx > 0.0).then_some(Side::Right),
            (false, false) if dx > 0.0 => hosted.map(|_| Side::Left),
            (false, false) => Some(Side::Right),
        }
    }
}

pub(crate) fn styled(base: &str, open: bool, pull: Option<f64>) -> (String, String) {
    let mut class = base.to_owned();

    if open {
        class.push_str(" open");
    }

    if pull.is_some() {
        class.push_str(" pulling");
    }

    (class, pull.map_or_else(String::new, |shift| format!("--pull: {shift}px;")))
}

#[component]
pub(crate) fn Drawer(open: Signal<bool>, children: Element) -> Element {
    let drawers = drawers();
    let mut hosted = drawers.hosted;
    let before = use_hook(|| *hosted.peek());

    use_effect(move || hosted.set(Some(open)));
    use_drop(move || {
        if *hosted.peek() == Some(open) {
            hosted.set(before.filter(|held| *held != open));
        }
    });

    let shown = open();
    let (class, style) = styled("drawer", shown, drawers.pulled(Side::Left));

    rsx! {
        if shown {
            div { class: "drawer-scrim", onclick: move |_| { let mut open = open; open.set(false); } }
        }
        div { class: "{class}", style: "{style}", {children} }
    }
}
