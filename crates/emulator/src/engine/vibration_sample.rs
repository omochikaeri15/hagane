use crate::Fault;

use super::{AppContext, vibrate};

pub fn vibration_sample(ctx: &mut AppContext) -> Result<(), Fault> {
    let power = ctx.battle_event_latch.power;
    let time = ctx.battle_event_latch.sample_seconds;
    let tail = ctx.battle_event_latch.tail;

    vibrate(ctx, power, time, tail)
}
