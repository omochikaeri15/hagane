use std::time::{Duration, Instant};

use iced::mouse;
use iced::widget::canvas as canvas_widget;
use iced::widget::canvas::{self, Geometry, Path, Stroke};
use iced::{window, Color, Element, Length, Point, Rectangle, Renderer, Size, Theme, Vector};

use codex::combat::{Entity, Faction, Span};
use codex::graphics::animate;
use codex::graphics::rig::{Animation, Rig};

use foundry::domains::settings::{CombatLook, HitPositions};
use foundry::systems::animation::Role;

use super::canvas as viewer;
use super::data;
use super::diagnostics::Sheet;

const STRIKE_COLORS: [Color; 3] = [
    Color::from_rgb(1.0, 0.12, 0.12),
    Color::from_rgb(1.0, 0.55, 0.0),
    Color::from_rgb(1.0, 0.95, 0.1),
];
const HURTBOX_COLOR: Color = Color::from_rgb(0.1, 1.0, 0.9);
const ORIGIN_COLOR: Color = Color::from_rgb(0.72, 0.3, 1.0);
const BOUND_COLOR: Color = Color::from_rgb(1.0, 0.35, 0.75);
const RANGE_COLOR: Color = Color::from_rgb(0.75, 0.25, 0.0);
const BASE_COLOR: Color = Color::from_rgb(0.82, 0.82, 0.82);

const OUTLINE_WIDTH: f32 = 2.0;
const LINE_WIDTH: f32 = 2.0;
const FRAMES_PER_SECOND: f32 = 30.0;
pub(crate) const LINE_RISE: f32 = 200.0;
const CLOCK_DRIFT: f32 = 3.0;

#[derive(Clone)]
pub struct Profile {
    faction: Faction,
    hurtbox: Span,
    range: f32,
    strikes: Vec<(i32, Span)>,
}

impl Profile {
    pub fn of(stats: &Entity) -> Self {
        let strikes = stats
            .strike_frames()
            .into_iter()
            .enumerate()
            .map(|(attack, frame)| (frame, stats.strike_reach(attack)))
            .collect();

        Self {
            faction: stats.faction,
            hurtbox: stats.hurtbox(),
            range: stats.standing_reach(),
            strikes,
        }
    }

    pub(crate) fn measured(faction: Faction, hurtbox: Span, range: f32, strikes: Vec<(i32, Span)>) -> Self {
        Self { faction, hurtbox, range, strikes }
    }

    fn facing(&self) -> f32 {
        match self.faction {
            Faction::Cat => -1.0,
            Faction::Enemy => 1.0,
        }
    }
}

#[derive(Clone)]
pub struct Staged {
    pub profile: Profile,
    pub look: CombatLook,
    pub attacking: bool,
    pub crest: Option<f32>,
}

pub(crate) enum Mark {
    Line {
        from: Point,
        to: Point,
        color: Color,
    },
    Box {
        corner: Point,
        size: Size,
        color: Color,
        fill: f32,
    },
}

struct Scene<'a> {
    unit: &'a Rig,
    anim: Option<&'a Animation>,
    seated: i32,
    offset: Option<usize>,
    zoom: f32,
    crest: Option<f32>,
}

pub(crate) struct Seat {
    pub(crate) ground: Point,
    pub(crate) zoom: f32,
    pub(crate) bound: Option<f32>,
    pub(crate) crest: f32,
    pub(crate) spans_bound: bool,
    pub(crate) hurt: f32,
}

fn aged(period: Option<i32>, frame: f32, landed: i32) -> f32 {
    match (frame - landed as f32, period) {
        (age, Some(period)) if age < 0.0 => age + period as f32,
        (age, _) => age,
    }
}

pub(crate) fn ramp(fade: i32, age: f32) -> f32 {
    let life = fade.max(1) as f32;

    match (0.0..life).contains(&age) {
        true => 1.0 - age / life,
        false => 0.0,
    }
}

fn glow(fade: i32, period: Option<i32>, frame: f32, landed: i32) -> f32 {
    ramp(fade, aged(period, frame, landed))
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
    pub(super) fn tick(&mut self, profile: Option<&Profile>, data: &data::State, state: &viewer::State) {
        let Some(profile) = profile.filter(|_| state.is_playing && attacking(data)) else {
            self.lit = [None; 3];
            return;
        };

        let now = Instant::now();
        let period = period(data, state);

        for (attack, slot) in self.lit.iter_mut().enumerate() {
            let age = profile
                .strikes
                .get(attack)
                .map(|(landed, _)| aged(period, state.current_frame, *landed))
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

        self.lit.iter().flatten().any(|lit| ramp(fade, elapsed(now, *lit)) > 0.0)
    }
}

fn elapsed(now: Instant, lit: Instant) -> f32 {
    now.saturating_duration_since(lit).as_secs_f32() * FRAMES_PER_SECOND
}

fn rows(strikes: &[(i32, Span)], positions: HitPositions) -> Vec<(f32, f32)> {
    let count = strikes.len().min(STRIKE_COLORS.len());
    let clashing = |a: usize, b: usize| {
        strikes[a].1.near < strikes[b].1.far && strikes[b].1.near < strikes[a].1.far
    };

    let mut group: Vec<usize> = (0..count).collect();

    if positions == HitPositions::Fill {
        for _ in 0..count {
            for a in 0..count {
                for b in 0..a {
                    if clashing(a, b) {
                        let joined = group[a].min(group[b]);

                        group[a] = joined;
                        group[b] = joined;
                    }
                }
            }
        }
    }

    (0..count)
        .map(|attack| match positions {
            HitPositions::Stacked => (0.0, 1.0),
            HitPositions::Layered => (attack as f32 / count as f32, 1.0 / count as f32),
            HitPositions::Fill => {
                let peers = group.iter().filter(|held| **held == group[attack]).count();
                let above = group[..attack].iter().filter(|held| **held == group[attack]).count();

                (above as f32 / peers as f32, 1.0 / peers as f32)
            }
        })
        .collect()
}

fn laid(
    profile: &Profile,
    look: CombatLook,
    scene: &Scene<'_>,
    to_screen: impl Fn(f32, f32) -> Point,
    lit: impl Fn(usize, i32) -> f32,
) -> Vec<Mark> {
    let anchors = animate::anchors(scene.unit, scene.anim, scene.seated);
    let anchored = |row: usize| anchors.get(row).copied().flatten();

    let Some(combat) = anchored(0) else {
        return Vec::new();
    };

    let lifted = scene.offset.and_then(anchored).unwrap_or((0.0, 0.0));
    let ground = to_screen(combat.0 - lifted.0, combat.1 - lifted.1);

    let bound = anchored(profile.faction.bound_row())
        .map(|(reach, _)| ground.x - profile.facing() * profile.faction.bound_depth(reach) * scene.zoom);

    let crest = scene
        .crest
        .map_or(ground.y - LINE_RISE * scene.zoom, |crest| to_screen(0.0, crest).y);

    let seat = Seat { ground, zoom: scene.zoom, bound, crest: crest.min(ground.y), spans_bound: true, hurt: 1.0 };

    arranged(profile, look, &seat, lit)
}

pub(crate) fn arranged(
    profile: &Profile,
    look: CombatLook,
    seat: &Seat,
    lit: impl Fn(usize, i32) -> f32,
) -> Vec<Mark> {
    let ground = seat.ground;
    let bound = seat.bound;

    let ahead = |distance: f32| ground.x + profile.facing() * distance * seat.zoom;
    let depth = look.height * seat.zoom;
    let top = ground.y - look.offset * seat.zoom;
    let boxed = |from: f32, to: f32, row: f32, share: f32, color: Color, fill: f32| Mark::Box {
        corner: Point::new(from.min(to), top + row * depth),
        size: Size::new((to - from).abs(), share * depth),
        color,
        fill: fill * look.opacity,
    };
    let rising = |x: f32, color: Color| Mark::Line {
        from: Point::new(x, ground.y.max(top)),
        to: Point::new(x, seat.crest),
        color,
    };

    let (left, right) = [
        ground.x,
        ahead(-profile.hurtbox.near),
        ahead(-profile.hurtbox.far),
    ]
    .into_iter()
    .chain(bound.filter(|_| seat.spans_bound))
    .chain([ahead(profile.range)])
    .chain(
        profile
            .strikes
            .iter()
            .flat_map(|(_, reach)| [ahead(reach.near), ahead(reach.far)]),
    )
    .fold((ground.x, ground.x), |(left, right), x| {
        (left.min(x), right.max(x))
    });

    let mut marks = Vec::new();

    if look.system {
        marks.push(boxed(left, right, 0.0, 1.0, BASE_COLOR, 0.0));
    }

    if look.width {
        marks.push(boxed(
            ahead(-profile.hurtbox.near),
            ahead(-profile.hurtbox.far),
            0.0,
            1.0,
            HURTBOX_COLOR,
            seat.hurt,
        ));
    }

    marks.extend(bound.filter(|_| look.bound).map(|x| rising(x, BOUND_COLOR)));

    if look.range {
        marks.push(rising(ahead(profile.range), RANGE_COLOR));
    }

    if look.origin {
        marks.push(rising(ground.x, ORIGIN_COLOR));
    }

    let rows = rows(&profile.strikes, look.positions);

    for (attack, (((landed, reach), color), (row, share))) in
        profile.strikes.iter().zip(STRIKE_COLORS).zip(rows).enumerate().rev()
    {
        if !look.hits.get(attack).is_some_and(|shown| *shown) {
            continue;
        }

        marks.push(boxed(ahead(reach.near), ahead(reach.far), row, share, color, lit(attack, *landed)));
    }

    marks
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
    let scene = Scene {
        unit: still.unit,
        anim: still.anim,
        seated: still.frame,
        offset: still.offset,
        zoom: still.zoom,
        crest: staged.crest,
    };
    let lit = |_: usize, landed: i32| match still.attacking {
        true => glow(staged.look.fade, still.period, still.played, landed),
        false => 0.0,
    };

    let mut sheet = Sheet::over(pixels, width, height);

    for mark in laid(&staged.profile, staged.look, &scene, to_screen, lit) {
        match mark {
            Mark::Line { from, to, color } => sheet.span(from, to, color, LINE_WIDTH),
            Mark::Box {
                corner,
                size,
                color,
                fill,
            } => {
                sheet.flood(
                    corner,
                    size,
                    Color {
                        a: fill,
                        ..color
                    },
                );

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
    profile: Option<&'a Profile>,
    look: CombatLook,
    clock: Clock,
) -> Element<'a, M> {
    let period = period(data, state);

    canvas_widget(Marks {
        data,
        profile,
        frame: state.current_frame,
        pan: state.pan,
        zoom: state.zoom,
        look,
        period,
        clock,
    })
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

struct Marks<'a> {
    data: &'a data::State,
    profile: Option<&'a Profile>,
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

        (self.profile.is_some() && self.clock.live(self.look.fade)).then(canvas::Action::request_redraw)
    }

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let (Some(profile), Some(unit)) = (self.profile, self.data.held_unit.as_ref()) else {
            return Vec::new();
        };

        let scene = Scene {
            unit,
            anim: self.data.current_anim.as_deref(),
            seated: self.data.playback_frame(self.frame).floor() as i32,
            offset: self.data.offset(),
            zoom: self.zoom,
            crest: self.data.bounds().map(|reach| reach.min_y),
        };

        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let center = frame.center();
        let to_screen = |x: f32, y: f32| {
            Point::new(
                center.x + (x + self.pan.x) * self.zoom,
                center.y + (y + self.pan.y) * self.zoom,
            )
        };

        let attacking = attacking(self.data);
        let now = Instant::now();
        let lit = |attack: usize, landed: i32| match (self.clock.lit.get(attack).copied().flatten(), attacking) {
            (Some(lit), _) => ramp(self.look.fade, elapsed(now, lit)),
            (None, true) => glow(self.look.fade, self.period, self.frame, landed),
            (None, false) => 0.0,
        };

        for mark in laid(profile, self.look, &scene, to_screen, lit) {
            match mark {
                Mark::Line { from, to, color } => {
                    frame.stroke(
                        &Path::line(from, to),
                        Stroke::default().with_color(color).with_width(LINE_WIDTH),
                    );
                }
                Mark::Box {
                    corner,
                    size,
                    color,
                    fill,
                } => {
                    if fill > 0.0 {
                        frame.fill_rectangle(
                            corner,
                            size,
                            Color {
                                a: fill,
                                ..color
                            },
                        );
                    }

                    frame.stroke(
                        &Path::rectangle(corner, size),
                        Stroke::default()
                            .with_color(color)
                            .with_width(OUTLINE_WIDTH),
                    );
                }
            }
        }

        vec![frame.into_geometry()]
    }
}

pub(crate) fn rects(marks: Vec<Mark>, stroke: f32) -> Vec<(Point, Size, Color)> {
    let mut rects = Vec::new();

    for mark in marks {
        match mark {
            Mark::Line { from, to, color } => {
                let width = LINE_WIDTH * stroke;

                rects.push((
                    Point::new(from.x.min(to.x) - width / 2.0, from.y.min(to.y)),
                    Size::new((to.x - from.x).abs() + width, (to.y - from.y).abs()),
                    color,
                ));
            }
            Mark::Box { corner, size, color, fill } => {
                let width = OUTLINE_WIDTH * stroke;
                let half = width / 2.0;

                if fill > 0.0 {
                    rects.push((corner, size, Color { a: fill, ..color }));
                }

                let (left, top) = (corner.x - half, corner.y - half);
                let (wide, tall) = (size.width + width, size.height + width);

                rects.extend([
                    (Point::new(left, top), Size::new(wide, width), color),
                    (Point::new(left, top + size.height), Size::new(wide, width), color),
                    (Point::new(left, top), Size::new(width, tall), color),
                    (Point::new(left + size.width, top), Size::new(width, tall), color),
                ]);
            }
        }
    }

    rects
}

pub(super) fn attacking(data: &data::State) -> bool {
    data.current_motion()
        .is_some_and(|motion| motion.role == Some(Role::Attack))
}

#[cfg(test)]
mod tests {
    use super::*;

    // A fade that outlives the animation carries into the next pass, and along the
    // way the fill may only ever jump up on the frame its hit lands.
    #[test]
    fn a_fill_carries_across_the_loop_and_only_brightens_when_its_hit_lands() {
        let (fade, period, landed) = (30, 40, 12);
        let ticks: Vec<f32> = (0..period * 4).map(|tick| (tick % (period * 2)) as f32 / 2.0).collect();

        assert!(glow(fade, Some(period), 0.0, landed) > 0.0, "the last pass's tail is still lit");
        assert_eq!(glow(fade, None, 0.0, landed), 0.0, "and a first pass has no tail to show");

        for pair in ticks.windows(2) {
            let (before, after) = (glow(fade, Some(period), pair[0], landed), glow(fade, Some(period), pair[1], landed));

            assert!(after <= before || pair[1] == landed as f32, "rose at frame {}", pair[1]);
        }
    }

    fn span(near: f32, far: f32) -> (i32, Span) {
        (1, Span { near, far })
    }

    // Fill only splits the height between hits that actually share ground.
    #[test]
    fn fill_gives_a_lone_hit_the_whole_height_and_splits_it_between_hits_that_overlap() {
        let strikes = [span(0.0, 50.0), span(40.0, 90.0), span(200.0, 260.0)];

        assert_eq!(rows(&strikes, HitPositions::Fill), vec![(0.0, 0.5), (0.5, 0.5), (0.0, 1.0)]);
        assert_eq!(rows(&strikes, HitPositions::Stacked), vec![(0.0, 1.0); 3]);
        assert_eq!(rows(&strikes, HitPositions::Layered)[2], (2.0 / 3.0, 1.0 / 3.0));
    }

    #[test]
    fn hits_that_only_touch_at_an_edge_do_not_count_as_overlapping() {
        let strikes = [span(0.0, 50.0), span(50.0, 90.0)];

        assert_eq!(rows(&strikes, HitPositions::Fill), vec![(0.0, 1.0), (0.0, 1.0)]);
    }
}
