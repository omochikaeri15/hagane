use dioxus::prelude::*;

use crate::platform::Platform;

const TURN_KEY: &str = "landscape";
const TURN_ON: &str = "on";
const TURN_OFF: &str = "off";

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct Area {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
}

#[derive(Clone, Copy)]
pub(crate) struct Turn {
    wanted: Signal<bool>,
    viewport: Signal<(f64, f64)>,
}

pub(crate) fn provide(platform: &Platform) -> Turn {
    let wanted = platform.host().recall(TURN_KEY).as_deref() == Some(TURN_ON);

    use_context_provider(|| Turn { wanted: Signal::new(wanted), viewport: Signal::new((0.0, 0.0)) })
}

pub(crate) fn turn() -> Turn {
    use_context::<Turn>()
}

impl Turn {
    pub(crate) fn wanted(self) -> bool {
        (self.wanted)()
    }

    pub(crate) fn want(self, platform: &Platform, on: bool) {
        let mut wanted = self.wanted;

        platform.host().remember(TURN_KEY, if on { TURN_ON } else { TURN_OFF });
        wanted.set(on);
    }

    pub(crate) fn measure(self, width: f64, height: f64) {
        let mut viewport = self.viewport;

        if *viewport.peek() != (width, height) {
            viewport.set((width, height));
        }
    }

    pub(crate) fn frame(self) -> (bool, String) {
        let (width, height) = (self.viewport)();

        ((self.wanted)() && height > width, format!("--turn-width: {height}px; --turn-height: {width}px; --turn-shift: {width}px;"))
    }

    fn engaged(self) -> Option<f64> {
        let (width, height) = *self.viewport.peek();

        (*self.wanted.peek() && height > width).then_some(width)
    }

    pub(crate) fn point(self, x: f64, y: f64) -> (f64, f64) {
        self.engaged().map_or((x, y), |width| (y, width - x))
    }

    pub(crate) fn area(self, x: f64, y: f64, width: f64, height: f64) -> Area {
        self.engaged().map_or(Area { x, y, width, height }, |screen| Area { x: y, y: screen - x - width, width: height, height: width })
    }

    pub(crate) fn extent(self) -> (f64, f64) {
        let (width, height) = *self.viewport.peek();

        if self.engaged().is_some() { (height, width) } else { (width, height) }
    }
}
