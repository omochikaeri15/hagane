use std::time::{Duration, Instant};

use iced::mouse;
use iced::widget::canvas as canvas_widget;
use iced::widget::canvas::{self, Geometry, Path, Stroke};
use iced::{window, Color, Element, Length, Point, Rectangle, Renderer, Size, Theme, Vector};

use codex::graphics::rig::{Animation, Rig};

use foundry::domains::settings::CombatLook;
use foundry::systems::animation::collision::{self, Combatant, Mark, Pose, FRAMES_PER_SECOND, LINE_WIDTH, OUTLINE_WIDTH};

use super::canvas as viewer;
use super::data;
use super::diagnostics::Sheet;

const CLOCK_DRIFT: f32 = 3.0;

#[derive(Clone)]
pub struct Staged {
    pub combatant: Combatant,
    pub look: CombatLook,
    pub crest: Option<f32>,
}

fn shade(color: collision::Color) -> Color {
    Color::from_rgba(color[0], color[1], color[2], color[3])
}

fn place(point: collision::Point) -> Point {
    Point::new(point.0, point.1)
}

fn period(data: &data::State, state: &viewer::State) -> Option<i32> {
    state
        .loop_end
        .or_else(|| data.loop_bound().map(|end| end.max(0) as f32))
        .map(|end| (end - state.loop_start.unwrap_or(0.0)).floor() as i32 + 1)
}

#[derive(Clone, Copy, Default)]
pub struct Clock {
    lit: [Option<Instant>; 3],
}

impl Clock {
    pub(super) fn tick(&mut self, combatant: Option<&Combatant>, data: &data::State, state: &viewer::State) {
        let Some(combatant) = combatant.filter(|held| state.is_playing && held.attacking && data.settled()) else {
            self.lit = [None; 3];
            return;
        };

        let now = Instant::now();
        let period = period(data, state);

        for (attack, slot) in self.lit.iter_mut().enumerate() {
            let age = combatant
                .profile
                .strikes()
                .get(attack)
                .map(|(landed, _)| collision::aged(period, state.current_frame, *landed))
                .filter(|age| *age >= 0.0);

            let Some(age) = age else {
                *slot = None;
                continue;
            };

            let drifted = slot.is_none_or(|lit| (elapsed(now, lit) - age).abs() > CLOCK_DRIFT);

            if drifted {
                *slot = now.checked_sub(Duration::from_secs_f32(age / FRAMES_PER_SECOND));
            }
        }
    }

    fn live(&self, fade: i32) -> bool {
        let now = Instant::now();

        self.lit.iter().flatten().any(|lit| collision::ramp(fade, elapsed(now, *lit)) > 0.0)
    }
}

fn elapsed(now: Instant, lit: Instant) -> f32 {
    now.saturating_duration_since(lit).as_secs_f32() * FRAMES_PER_SECOND
}

pub(super) struct Still<'a> {
    pub(super) unit: &'a Rig,
    pub(super) anim: Option<&'a Animation>,
    pub(super) frame: i32,
    pub(super) played: f32,
    pub(super) period: Option<i32>,
    pub(super) offset: Option<usize>,
    pub(super) zoom: f32,
    pub(super) attacking: bool,
}

pub(super) fn paint(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    still: &Still<'_>,
    to_screen: impl Fn(f32, f32) -> Point,
    staged: &Staged,
) {
    let pose = Pose {
        rig: still.unit,
        anim: still.anim,
        frame: still.frame,
        offset: still.offset,
        zoom: still.zoom,
        crest: staged.crest,
    };
    let lit = |_: usize, landed: i32| match still.attacking {
        true => collision::glow(staged.look.fade, still.period, still.played, landed),
        false => 0.0,
    };
    let seated = |x: f32, y: f32| {
        let point = to_screen(x, y);

        (point.x, point.y)
    };

    let mut sheet = Sheet::over(pixels, width, height);

    for mark in collision::posed(&staged.combatant.profile, staged.look, &pose, seated, lit) {
        match mark {
            Mark::Line { from, to, color } => sheet.span(place(from), place(to), shade(color), LINE_WIDTH),
            Mark::Box { corner, size, color, fill } => {
                let (corner, size, color) = (place(corner), Size::new(size.0, size.1), shade(color));

                sheet.flood(corner, size, Color { a: fill, ..color });

                let far = Point::new(corner.x + size.width, corner.y + size.height);

                for (from, to) in [
                    (corner, Point::new(far.x, corner.y)),
                    (Point::new(far.x, corner.y), far),
                    (far, Point::new(corner.x, far.y)),
                    (Point::new(corner.x, far.y), corner),
                ] {
                    sheet.span(from, to, color, OUTLINE_WIDTH);
                }
            }
        }
    }
}

pub fn view<'a, M: 'a>(
    data: &'a data::State,
    state: &'a viewer::State,
    combatant: Option<&'a Combatant>,
    look: CombatLook,
    clock: Clock,
) -> Element<'a, M> {
    let period = period(data, state);

    canvas_widget(Marks { data, combatant, frame: state.current_frame, pan: state.pan, zoom: state.zoom, look, period, clock })
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

struct Marks<'a> {
    data: &'a data::State,
    combatant: Option<&'a Combatant>,
    frame: f32,
    pan: Vector,
    zoom: f32,
    look: CombatLook,
    period: Option<i32>,
    clock: Clock,
}

impl<M> canvas::Program<M> for Marks<'_> {
    type State = ();

    fn update(
        &self,
        _state: &mut (),
        event: &canvas::Event,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Option<canvas::Action<M>> {
        let canvas::Event::Window(window::Event::RedrawRequested(_)) = event else {
            return None;
        };

        (self.combatant.is_some() && self.clock.live(self.look.fade)).then(canvas::Action::request_redraw)
    }

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let (Some(combatant), Some(unit)) = (self.combatant, self.data.held_unit.as_ref()) else {
            return Vec::new();
        };

        let pose = Pose {
            rig: unit,
            anim: self.data.current_anim.as_deref(),
            frame: self.data.playback_frame(self.frame).floor() as i32,
            offset: self.data.offset(),
            zoom: self.zoom,
            crest: self.data.bounds().map(|reach| reach.min_y),
        };

        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let center = frame.center();
        let to_screen = |x: f32, y: f32| {
            (center.x + (x + self.pan.x) * self.zoom, center.y + (y + self.pan.y) * self.zoom)
        };

        let attacking = combatant.attacking && self.data.settled();
        let now = Instant::now();
        let lit = |attack: usize, landed: i32| {
            if !attacking {
                return 0.0;
            }

            let age = collision::aged(self.period, self.frame, landed);

            match self.clock.lit.get(attack).copied().flatten() {
                Some(lit) if (elapsed(now, lit) - age).abs() <= CLOCK_DRIFT => collision::ramp(self.look.fade, elapsed(now, lit)),
                _ => collision::glow(self.look.fade, self.period, self.frame, landed),
            }
        };

        for mark in collision::posed(&combatant.profile, self.look, &pose, to_screen, lit) {
            match mark {
                Mark::Line { from, to, color } => {
                    frame.stroke(
                        &Path::line(place(from), place(to)),
                        Stroke::default().with_color(shade(color)).with_width(LINE_WIDTH),
                    );
                }
                Mark::Box { corner, size, color, fill } => {
                    let (corner, size, color) = (place(corner), Size::new(size.0, size.1), shade(color));

                    if fill > 0.0 {
                        frame.fill_rectangle(corner, size, Color { a: fill, ..color });
                    }

                    frame.stroke(
                        &Path::rectangle(corner, size),
                        Stroke::default().with_color(color).with_width(OUTLINE_WIDTH),
                    );
                }
            }
        }

        vec![frame.into_geometry()]
    }
}
