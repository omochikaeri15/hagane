use std::collections::BTreeMap;
use std::mem;

use emulator::engine::{
    AppContext, CANNON_SHOT_SPACING, CannonShot, EXPLOSION_WIDTH_DEFAULTS, Entity, SLOTS_PER_FACTION, SURGE_TIMING, Vector,
    WaveRecord, WaveSprite,
};

use codex::combat::{Faction, Span};
use foundry::domains::settings::CombatLook;
use foundry::systems::animation::collision::{
    self as layout, Color, Mark, Point, Profile, Seat, Slab, BLAST_COLORS, CANNON_COLOR, LINE_RISE, SWEEP_COLOR,
};

use super::sink::{Frame, Lens, Quad};

const FACTIONS: [(i32, Faction); 2] = [(1, Faction::Enemy), (0, Faction::Cat)];
const BASE_OCCUPANT: i32 = 1;
const ATTACKING: i32 = 2;
const DESIGN_WIDTH: i32 = 0x3c0;
const STEPS_PER_PIXEL: i32 = 10;
const LAYER_RISE: i32 = 4;
const CAT_SIDE: i32 = 0;
const WAVE_RECORDS: usize = 200;
const WAVE_SPRITES: usize = 6;
const CANNON_SHOTS: usize = 15;
const ENEMY_SIDE: i32 = 1;
const WAVE_PIN: u8 = 0;
const SURGE_PIN: u8 = 1;
const COUNTER_PIN: u8 = 2;
const BLAST_PIN: u8 = 3;
const UNIT_ORDER: u8 = 0;
const EFFECT_ORDER: u8 = 1;

type Pin = (u8, i32, i32, i32);

#[derive(Default)]
pub(super) struct Overlay {
    look: Option<CombatLook>,
    lit: BTreeMap<(i32, i32, usize), i32>,
    hurt: BTreeMap<(i32, i32), i32>,
    health: BTreeMap<(i32, i32), Vital>,
    grounds: BTreeMap<(i32, i32), f32>,
    pinned: BTreeMap<Pin, f32>,
}

#[derive(Clone, Copy)]
struct Vital {
    health: i32,
    shield: i32,
    barrier: (i32, i32),
    guard: (i32, i32),
}

impl Vital {
    fn struck(self, held: Vital) -> bool {
        let flared = |now: (i32, i32), before: (i32, i32)| now.0 != 0 && (before.0 == 0 || now.1 < before.1);

        self.health < held.health
            || self.shield < held.shield
            || flared(self.barrier, held.barrier)
            || flared(self.guard, held.guard)
    }
}

struct Unit {
    profile: Profile,
    ground: Point,
    bound: Option<f32>,
    vital: Vital,
}

impl Overlay {
    pub(super) fn set(&mut self, look: Option<CombatLook>) {
        self.look = look;
    }

    pub(super) fn paint(&mut self, ctx: &AppContext, frame: &mut Frame) {
        let Some(look) = self.look else {
            self.lit.clear();
            self.hurt.clear();
            self.health.clear();
            self.grounds.clear();
            self.pinned.clear();
            return;
        };

        let Some((lens, layer)) = lens(ctx, frame) else {
            return;
        };

        let Ok(now) = ctx.i32_at(AppContext::BATTLE_FRAME_COUNTER) else {
            return;
        };

        let stroke = 1.0 / lens.matrix[0].abs().max(f32::EPSILON);
        let mut live = Vec::new();
        let mut groups: Vec<(f32, u8, Vec<Quad>)> = Vec::new();

        for (faction, side) in FACTIONS {
            for slot in 1..SLOTS_PER_FACTION {
                let Some((unit, landed)) = read(ctx, faction, side, slot) else {
                    continue;
                };

                for attack in landed {
                    self.lit.insert((faction, slot, attack), now);
                }

                live.push((faction, slot));
                self.grounds.insert((faction, slot), unit.ground.1);

                if self.health.insert((faction, slot), unit.vital).is_some_and(|held| unit.vital.struck(held)) {
                    self.hurt.insert((faction, slot), now);
                }

                let faded = |lit: &i32| layout::ramp(look.fade, now.wrapping_sub(*lit) as f32);

                let lit = |attack: usize, _: i32| {
                    self.lit.get(&(faction, slot, attack)).map_or(0.0, faded)
                };

                let seat = Seat {
                    ground: unit.ground,
                    zoom: 1.0,
                    bound: unit.bound,
                    crest: unit.ground.1 - LINE_RISE,
                    spans_bound: false,
                    hurt: self.hurt.get(&(faction, slot)).map_or(0.0, faded),
                };

                let boxes = layout::slabs(layout::arranged(&unit.profile, look, &seat, lit), stroke)
                    .into_iter()
                    .map(|slab| solid(lens, slab))
                    .collect();

                groups.push((unit.ground.1 - look.offset, UNIT_ORDER, boxes));
            }
        }

        for (top, mark) in effects(ctx, look, &self.grounds, &mut self.pinned).unwrap_or_default() {
            let boxes = layout::slabs([mark], stroke)
                .into_iter()
                .map(|slab| solid(lens, slab))
                .collect();

            groups.push((top, EFFECT_ORDER, boxes));
        }

        groups.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));

        let quads: Vec<Quad> = groups.into_iter().flat_map(|(_, _, boxes)| boxes).collect();

        let layer = layer.min(frame.quads.len());

        frame.quads.splice(layer..layer, quads);

        self.lit.retain(|(faction, slot, _), _| live.contains(&(*faction, *slot)));
        self.hurt.retain(|unit, _| live.contains(unit));
        self.health.retain(|unit, _| live.contains(unit));
    }
}

fn lens(ctx: &AppContext, frame: &Frame) -> Option<(Lens, usize)> {
    let across = ctx.f32_at(AppContext::CAMERA_OFFSET).ok()?;
    let down = ctx.f32_at(AppContext::CAMERA_OFFSET + Vector::Y).ok()?;

    let held = frame.lenses.iter().rposition(|lens| lens.camera && lens.matrix[4] == across && lens.matrix[5] == down)?;
    let layer = frame.lenses.get(held + 1).map_or(frame.quads.len(), |next| next.at);

    Some((*frame.lenses.get(held)?, layer))
}

fn solid(lens: Lens, slab: Slab) -> Quad {
    let Slab { corner, size, color } = slab;
    let premultiplied = [color[0] * color[3], color[1] * color[3], color[2] * color[3], color[3]];

    Quad {
        sheet: None,
        label: None,
        corners: [
            place(lens, corner.0, corner.1),
            place(lens, corner.0, corner.1 + size.1),
            place(lens, corner.0 + size.0, corner.1 + size.1),
            place(lens, corner.0 + size.0, corner.1),
        ],
        source: [0.0; 4],
        colors: [premultiplied; 4],
        blend: 0,
    }
}

fn place(lens: Lens, x: f32, y: f32) -> [f32; 2] {
    let local_x = x + lens.origin[0] as f32;
    let local_y = y + lens.origin[1] as f32;

    [
        lens.matrix[0] * local_x + lens.matrix[2] * local_y + lens.matrix[4],
        lens.matrix[1] * local_x + lens.matrix[3] * local_y + lens.matrix[5],
    ]
}

fn across(ctx: &AppContext) -> Option<impl Fn(i32) -> f32> {
    let left = (emulator::engine::get_drawable_width(ctx).ok()?.wrapping_sub(DESIGN_WIDTH) as f64 * 0.5) as f32;
    let camera = ctx.i32_at(AppContext::CAMERA_X).ok()?;

    Some(move |steps: i32| left + (steps.wrapping_sub(camera) / STEPS_PER_PIXEL) as f32)
}

fn effects(ctx: &AppContext, look: CombatLook, grounds: &BTreeMap<(i32, i32), f32>, pinned: &mut BTreeMap<Pin, f32>) -> Option<Vec<(f32, Mark)>> {
    let across = across(ctx)?;
    let base = |field: usize| ctx.i32_at(AppContext::entity_field(CAT_SIDE, 0, field)).ok();
    let floor = (base(Entity::POS_Y)? / STEPS_PER_PIXEL).wrapping_add(base(Entity::Z_LAYER)?.wrapping_mul(LAYER_RISE)) as f32;
    let held = mem::take(pinned);
    let mut seated = |pin: Pin, faction: i32, slot: i32| {
        let height = held.get(&pin).copied().unwrap_or_else(|| grounds.get(&(faction, slot)).copied().unwrap_or(floor));

        pinned.insert(pin, height);

        height - look.offset
    };
    let setting = |key: &[u8], fallback: i32| emulator::engine::get_setting(&ctx.settings, key, fallback).ok();

    let mut marks = Vec::new();
    let mut band = |top: f32, from: i32, to: i32, color: Color, fill: f32| {
        let (from, to) = (across(from), across(to));

        marks.push((top, Mark::Box {
            corner: (from.min(to), top),
            size: ((to - from).abs(), look.height),
            color,
            fill: fill * look.opacity,
        }));
    };

    if look.wave {
        let half = CANNON_SHOT_SPACING.wrapping_mul(5);

        for wave in 0..WAVE_RECORDS {
            let record = AppContext::WAVE_RECORDS + wave * AppContext::WAVE_RECORD_STRIDE;
            let sprites = AppContext::WAVE_SPRITES + wave * AppContext::WAVE_RECORD_STRIDE;
            let mini = ctx.u8_at(record + WaveRecord::MINI).ok()? != 0;
            let struck = ctx.i32_at(record + WaveRecord::IN_USE).ok()?;
            let owner = ctx.i32_at(record + WaveRecord::OWNER_SLOT).ok()?;
            let side = if struck == 2 { ENEMY_SIDE } else { CAT_SIDE };

            for sprite in 0..WAVE_SPRITES {
                let cell = sprites + sprite * WaveSprite::STRIDE;
                let timer = ctx.i32_at(cell + WaveSprite::TIMER).ok()?;

                if timer <= 0 {
                    continue;
                }

                let top = seated((WAVE_PIN, wave as i32, 0, 0), side, owner);
                let length = emulator::engine::get_anim_len(if mini { &ctx.mini_wave_anim } else { &ctx.wave_anim }).ok()?;
                let (first, last) = match mini {
                    true => (setting(b"battle_wave_s_frame1", 3)?, setting(b"battle_wave_s_frame2", 7)?),
                    false => (3, 7),
                };
                let played = length.wrapping_add(!timer);
                let fill = if played > first && played.wrapping_sub(1) <= last { 1.0 } else { 0.0 };
                let at = ctx.i32_at(cell + WaveSprite::POS_X).ok()?;
                let (from, to) = match struck {
                    2 => (at, at.wrapping_add(half.wrapping_mul(2))),
                    _ => (at.wrapping_sub(half), at.wrapping_add(half)),
                };

                band(top, from, to, SWEEP_COLOR, fill);
            }
        }
    }

    if look.surge {
        let half = SURGE_TIMING[0] / 2;
        let interval = SURGE_TIMING[2].max(1);

        for event in &ctx.surge_events {
            let elapsed = event.frame.wrapping_sub(SURGE_TIMING[1]);

            if elapsed < 0 || elapsed >= event.level.wrapping_mul(interval) {
                continue;
            }

            let shift = if event.faction == 1 { SURGE_TIMING[4] } else { SURGE_TIMING[4].wrapping_neg() };
            let fill = layout::ramp(interval, (elapsed % interval) as f32);

            band(
                seated((SURGE_PIN, event.faction, event.slot, event.x), event.faction, event.slot),
                event.x.wrapping_sub(half).wrapping_add(shift),
                event.x.wrapping_add(half).wrapping_add(shift),
                SWEEP_COLOR,
                fill,
            );
        }
    }

    if look.surge {
        let half = SURGE_TIMING[0] / 2;
        let summoned = setting(b"battle_demon_summon_frame", 0x1e)?;

        for event in ctx.counter_surge_events.iter().filter(|event| event.state != 1 && event.frame < summoned) {
            let Ok(owner) = emulator::engine::get_pos_x(ctx, event.faction, event.slot) else {
                continue;
            };

            let way = if event.faction == ENEMY_SIDE { 1 } else { -1 };
            let shift = SURGE_TIMING[4].wrapping_mul(way);
            let near = owner.wrapping_add(event.anchor.wrapping_mul(way)).wrapping_add(shift);
            let far = owner.wrapping_add(event.anchor.wrapping_add(event.span).wrapping_mul(way)).wrapping_add(shift);

            band(seated((COUNTER_PIN, event.faction, event.slot, event.x), event.faction, event.slot), near.min(far).wrapping_sub(half), near.max(far).wrapping_add(half), SWEEP_COLOR, 0.0);
        }
    }

    if look.explosion {
        let interval = setting(b"battle_explosion_frame4", 0xf)?.max(1);
        let start = setting(b"battle_explosion_frame1", 0xf)?;
        let burn = setting(b"battle_explosion_frame2", 0x14)?;
        let widths = [
            setting(b"battle_explosion_width1", EXPLOSION_WIDTH_DEFAULTS[0])?,
            setting(b"battle_explosion_width2", EXPLOSION_WIDTH_DEFAULTS[1])?,
            setting(b"battle_explosion_width3", EXPLOSION_WIDTH_DEFAULTS[2])?,
        ];
        let spreads = [
            0,
            widths[0].wrapping_add(widths[1]),
            widths[0].wrapping_add(widths[1].wrapping_mul(2)).wrapping_add(widths[2]),
        ];

        for event in &ctx.explosion_events {
            for ring in 0..3usize {
                let elapsed = event.frame.wrapping_sub(interval.wrapping_mul(ring as i32));
                let fill = if elapsed >= start && elapsed < start.wrapping_add(burn) { 1.0 } else { 0.0 };
                let (half, reach) = (widths[ring] / 2, spreads[ring] / 2);
                let centers = match ring {
                    0 => vec![event.x],
                    _ => vec![event.x.wrapping_sub(reach), event.x.wrapping_add(reach)],
                };

                for center in centers {
                    band(seated((BLAST_PIN, event.faction, event.slot, event.x), event.faction, event.slot), center.wrapping_sub(half), center.wrapping_add(half), BLAST_COLORS[ring], fill);
                }
            }
        }
    }

    if look.cannon {
        for side in [CAT_SIDE, ENEMY_SIDE] {
            cannon(ctx, side, &mut |from, to, fill| band(floor - look.offset, from, to, CANNON_COLOR, fill));
        }
    }

    Some(marks)
}

fn cannon(ctx: &AppContext, side: i32, band: &mut dyn FnMut(i32, i32, f32)) -> Option<()> {
    let kind = emulator::engine::get_cannon_type(ctx, side).ok()?;
    let origin = emulator::engine::cannon_shot_origin_x(ctx, side).ok()?;

    if kind == 0 || kind == 5 {
        let (after, until) = match kind {
            0 => (4, 8),
            _ => {
                let length = emulator::engine::get_anim_len(&ctx.base_anims[1]).ok()?;

                (length.wrapping_sub(8), length.wrapping_sub(4))
            }
        };

        for shot in 0..CANNON_SHOTS {
            let record = AppContext::CANNON_SHOTS
                + side as usize * AppContext::CANNON_SHOTS_FACTION_STRIDE
                + shot * AppContext::CANNON_SHOT_STRIDE;
            let timer = ctx.i32_at(record + CannonShot::TIMER).ok()?;

            if timer <= 0 {
                continue;
            }

            let fill = if timer <= until && timer.wrapping_add(1) > after { 1.0 } else { 0.0 };

            band(ctx.i32_at(record + CannonShot::POS_X).ok()?, origin, fill);
        }

        return Some(());
    }

    let state = emulator::engine::get_castle_anim_state(ctx, side).ok()?;
    let frame = emulator::engine::get_castle_anim_frame(ctx, side).ok()?;
    let at = emulator::engine::get_cannon_strike_x(ctx, side).ok()?;
    let width = emulator::engine::get_cannon_strike_width(ctx, CAT_SIDE).ok()?;

    match state {
        3 | 0xc => {
            let step = emulator::engine::get_base_level(ctx, side).ok()?.wrapping_mul(CANNON_SHOT_SPACING);
            let reach = frame.wrapping_mul(step).wrapping_mul(10) / 32;
            let limit = if side == CAT_SIDE { origin.wrapping_sub(reach) } else { origin.wrapping_add(reach) };

            band(limit, origin, 1.0);
        }
        5 if (5..0x10).contains(&frame) => {
            let left = at.wrapping_sub(width / 2);

            band(left, left.wrapping_add(width), 1.0);
        }
        0xb if (0xa..=0x14).contains(&frame) => {
            let left = at.wrapping_sub(width.wrapping_mul(8) / 10);

            band(left, left.wrapping_add(width), 1.0);
        }
        8 if frame >= 5 => band(at.wrapping_sub(width / 2), at.wrapping_add(width / 2), 1.0),
        _ => (),
    }

    Some(())
}

fn pixels(steps: i32) -> f32 {
    steps as f32 / STEPS_PER_PIXEL as f32
}

fn read(ctx: &AppContext, faction: i32, side: Faction, slot: i32) -> Option<(Unit, Vec<usize>)> {
    let occupant = emulator::engine::get_occupant(ctx, faction, slot).ok()?;

    if occupant == 0 || occupant == BASE_OCCUPANT {
        return None;
    }

    let field = |field: usize| ctx.i32_at(AppContext::entity_field(faction, slot, field)).ok();

    let across = across(ctx)?;

    let ground: Point = (
        across(field(Entity::POS_X)?),
        (field(Entity::POS_Y)? / STEPS_PER_PIXEL).wrapping_add(field(Entity::Z_LAYER)?.wrapping_mul(LAYER_RISE)) as f32,
    );

    let count = emulator::engine::get_attack_count(ctx, faction, slot).ok()?.clamp(0, 3);
    let state = emulator::engine::get_entity_state(ctx, faction, slot).ok()?;
    let frame = emulator::engine::get_entity_frame(ctx, faction, slot).ok()?;

    let mut strikes = Vec::new();
    let mut landed = Vec::new();

    for attack in 0..count {
        let foreswing = emulator::engine::get_attack_foreswing(ctx, faction, slot, attack).ok()?;

        let reach = if emulator::engine::is_attack_long_range(ctx, faction, slot, attack).ok()? {
            let anchor = pixels(emulator::engine::get_ld_anchor(ctx, faction, slot, attack).ok()?);
            let edge = anchor + pixels(emulator::engine::get_ld_span(ctx, faction, slot, attack).ok()?);

            Span { near: anchor.min(edge), far: anchor.max(edge) }
        } else {
            Span { near: 0.0, far: pixels(field(Entity::STANDING_RANGE)?) }
        };

        if state == ATTACKING && frame == foreswing {
            landed.push(attack as usize);
        }

        strikes.push((foreswing, reach));
    }

    let (near, far) = (pixels(field(Entity::HITBOX_POS)?), pixels(field(Entity::HITBOX_WIDTH)?));
    let hurtbox = Span { near: near.min(far), far: near.max(far) };
    let bound = field(Entity::BACK_BOUND).filter(|bound| *bound != 0).map(across);

    let vital = Vital {
        health: emulator::engine::get_hp(ctx, faction, slot).ok()?,
        shield: field(Entity::SHIELD_HP)?,
        barrier: (field(Entity::BARRIER_VFX_ACTIVE)?, field(Entity::BARRIER_VFX_FRAME)?),
        guard: (field(Entity::SHIELD_VFX)?, field(Entity::SHIELD_VFX_FRAME)?),
    };

    Some((Unit { profile: Profile::measured(side, hurtbox, pixels(field(Entity::STANDING_RANGE)?), strikes), ground, bound, vital }, landed))
}
