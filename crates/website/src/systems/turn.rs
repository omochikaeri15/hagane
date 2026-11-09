use std::fmt;

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::platform::Platform;

const ORIENTATION_KEY: &str = "orientation";
const LEGACY_KEY: &str = "landscape";
const LEGACY_ON: &str = "on";
const CLOCKWISE_ANGLE: u16 = 270;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum Orientation {
    #[default]
    Automatic,
    Portrait,
    Landscape,
}

impl Orientation {
    pub(crate) const ALL: [Self; 3] = [Self::Automatic, Self::Portrait, Self::Landscape];

    fn key(self) -> &'static str {
        match self {
            Self::Automatic => "automatic",
            Self::Portrait => "portrait",
            Self::Landscape => "landscape",
        }
    }
}

impl fmt::Display for Orientation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Automatic => "Automatic",
            Self::Portrait => "Portrait",
            Self::Landscape => "Landscape",
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Spin {
    Clockwise,
    Counter,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct Area {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
struct Viewport {
    width: f64,
    height: f64,
    angle: u16,
}

#[derive(Clone, Copy)]
pub(crate) struct Turn {
    mode: Signal<Orientation>,
    viewport: Signal<Viewport>,
    touch: Signal<bool>,
    battle: Signal<Option<Orientation>>,
}

pub(crate) fn provide(platform: &Platform) -> Turn {
    let saved = platform.host().recall(ORIENTATION_KEY).and_then(|key| Orientation::ALL.into_iter().find(|mode| mode.key() == key));
    let mode = saved.unwrap_or_else(|| if platform.host().recall(LEGACY_KEY).as_deref() == Some(LEGACY_ON) { Orientation::Landscape } else { Orientation::Automatic });

    let turn = use_context_provider(|| Turn { mode: Signal::new(mode), viewport: Signal::new(Viewport::default()), touch: Signal::new(false), battle: Signal::new(None) });
    let watcher = platform.clone();

    use_hook(move || watcher.host().screen_angle_watch(Box::new(move |angle| turn.rotated(angle))));

    turn
}

pub(crate) fn turn() -> Turn {
    use_context::<Turn>()
}

fn in_force(mode: Orientation, battle: Option<Orientation>, touch: bool) -> Orientation {
    battle.filter(|_| touch).unwrap_or(mode)
}

fn spin(mode: Orientation, viewport: Viewport) -> Option<Spin> {
    match mode {
        Orientation::Automatic => None,
        Orientation::Landscape => (viewport.height > viewport.width).then_some(Spin::Clockwise),
        Orientation::Portrait => (viewport.width > viewport.height).then_some(if viewport.angle == CLOCKWISE_ANGLE { Spin::Clockwise } else { Spin::Counter }),
    }
}

impl Turn {
    pub(crate) fn mode(self) -> Orientation {
        (self.mode)()
    }

    pub(crate) fn choose(self, platform: &Platform, mode: Orientation) {
        let mut held = self.mode;

        platform.host().remember(ORIENTATION_KEY, mode.key());
        held.set(mode);
    }

    pub(crate) fn sense(self, touch: bool) {
        let mut held = self.touch;

        if *held.peek() != touch {
            held.set(touch);
        }
    }

    pub(crate) fn hold(self, battle: Option<Orientation>) -> bool {
        let mut held = self.battle;
        let before = self.engaged().map(|(spin, _)| spin);

        if *held.peek() != battle {
            held.set(battle);
        }

        self.engaged().map(|(spin, _)| spin) != before
    }

    pub(crate) fn measure(self, width: f64, height: f64, angle: u16) {
        let mut viewport = self.viewport;
        let measured = Viewport { width, height, angle };

        if *viewport.peek() != measured {
            viewport.set(measured);
        }
    }

    fn rotated(self, angle: u16) {
        let mut viewport = self.viewport;
        let held = *viewport.peek();

        if held.angle != angle {
            viewport.set(Viewport { angle, ..held });
        }
    }

    pub(crate) fn frame(self) -> (Option<&'static str>, String) {
        let viewport = (self.viewport)();
        let (width, height) = (viewport.width, viewport.height);
        let class = spin(in_force((self.mode)(), (self.battle)(), (self.touch)()), viewport).map(|spin| match spin {
            Spin::Clockwise => "turned",
            Spin::Counter => "turned counter",
        });

        (class, format!("--turn-width: {height}px; --turn-height: {width}px; --turn-shift: {width}px; --turn-drop: {height}px;"))
    }

    pub(crate) fn turned(self) -> bool {
        self.frame().0.is_some()
    }

    fn engaged(self) -> Option<(Spin, Viewport)> {
        let viewport = *self.viewport.peek();

        spin(in_force(*self.mode.peek(), *self.battle.peek(), *self.touch.peek()), viewport).map(|spin| (spin, viewport))
    }

    pub(crate) fn point(self, x: f64, y: f64) -> (f64, f64) {
        self.engaged().map_or((x, y), |(spin, screen)| match spin {
            Spin::Clockwise => (y, screen.width - x),
            Spin::Counter => (screen.height - y, x),
        })
    }

    pub(crate) fn area(self, x: f64, y: f64, width: f64, height: f64) -> Area {
        self.engaged().map_or(Area { x, y, width, height }, |(spin, screen)| match spin {
            Spin::Clockwise => Area { x: y, y: screen.width - x - width, width: height, height: width },
            Spin::Counter => Area { x: screen.height - y - height, y: x, width: height, height: width },
        })
    }

    pub(crate) fn extent(self) -> (f64, f64) {
        let viewport = *self.viewport.peek();

        if self.engaged().is_some() { (viewport.height, viewport.width) } else { (viewport.width, viewport.height) }
    }
}
