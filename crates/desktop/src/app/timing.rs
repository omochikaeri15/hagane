use std::sync::Mutex;
use std::time::{Duration, Instant};

use tracing::debug;

const WINDOW: Duration = Duration::from_secs(1);
const SPANS: usize = 5;

#[derive(Clone, Copy)]
pub(crate) enum Span {
    View,
    Step,
    Paint,
    Prepare,
    Raster,
}

#[derive(Default, Clone, Copy)]
struct Tally {
    count: u32,
    total: Duration,
    most: Duration,
}

impl Tally {
    fn add(&mut self, spent: Duration) {
        self.count += 1;
        self.total += spent;
        self.most = self.most.max(spent);
    }

    fn average_ms(&self) -> f32 {
        if self.count == 0 {
            return 0.0;
        }

        tenths(self.total.as_secs_f32() * 1000.0 / self.count as f32)
    }

    fn most_ms(&self) -> f32 {
        tenths(self.most.as_secs_f32() * 1000.0)
    }
}

struct Second {
    began: Instant,
    drawn: Option<Instant>,
    frames: u32,
    gaps: Tally,
    spans: [Tally; SPANS],
    ticks: u32,
    size: (u32, u32),
}

impl Second {
    fn new(now: Instant) -> Self {
        Self { began: now, drawn: None, frames: 0, gaps: Tally::default(), spans: [Tally::default(); SPANS], ticks: 0, size: (0, 0) }
    }

    fn span(&self, span: Span) -> &Tally {
        &self.spans[span as usize]
    }

    fn report(&self) {
        let view = self.span(Span::View);
        let step = self.span(Span::Step);
        let paint = self.span(Span::Paint);
        let prepare = self.span(Span::Prepare);
        let raster = self.span(Span::Raster);

        debug!(
            frames = self.frames,
            frame_ms = self.gaps.average_ms(),
            frame_max_ms = self.gaps.most_ms(),
            ticks = self.ticks,
            steps = step.count,
            step_ms = step.average_ms(),
            step_max_ms = step.most_ms(),
            views = view.count,
            view_ms = view.average_ms(),
            view_max_ms = view.most_ms(),
            paint_ms = paint.average_ms(),
            paint_max_ms = paint.most_ms(),
            prepare_ms = prepare.average_ms(),
            prepare_max_ms = prepare.most_ms(),
            rasters = raster.count,
            raster_ms = raster.average_ms(),
            raster_max_ms = raster.most_ms(),
            width = self.size.0,
            height = self.size.1,
            "Battle frame time over the last second"
        );
    }
}

static SECOND: Mutex<Option<Second>> = Mutex::new(None);

fn tenths(value: f32) -> f32 {
    (value * 10.0).round() / 10.0
}

fn with(apply: impl FnOnce(&mut Second, Instant)) {
    let Ok(mut held) = SECOND.lock() else {
        return;
    };
    let now = Instant::now();
    let second = held.get_or_insert_with(|| Second::new(now));

    if now.duration_since(second.began) >= WINDOW {
        if second.frames > 0 {
            second.report();
        }

        let drawn = second.drawn;

        *second = Second { drawn, ..Second::new(now) };
    }

    apply(second, now);
}

pub(crate) fn record(span: Span, spent: Duration) {
    with(|second, _| second.spans[span as usize].add(spent));
}

pub(crate) fn tick() {
    with(|second, _| second.ticks += 1);
}

pub(crate) fn frame(width: u32, height: u32) {
    with(|second, now| {
        if let Some(gap) = second.drawn.map(|drawn| now.duration_since(drawn)).filter(|gap| *gap < WINDOW) {
            second.gaps.add(gap);
        }

        second.drawn = Some(now);
        second.frames += 1;
        second.size = (width, height);
    });
}
