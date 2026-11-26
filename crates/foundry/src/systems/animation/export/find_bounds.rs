use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use codex::graphics::rig::{Animation, BoundingBox, Rig, Tolerance};
use codex::graphics::tools::boundary::scan_bounds;
use tracing::{info, warn};

use crate::common::job::ProgressCounter;
use crate::systems::animation::Loop;

use super::BoundsOutcome;

const YIELD_INTERVAL: usize = 100;

pub fn search(
    unit: &Rig,
    motions: &[(&Animation, Loop, i32, i32)],
    tolerance: f32,
    offset: Option<usize>,
    progress: &ProgressCounter,
    abort_signal: &AtomicBool,
) -> BoundsOutcome {
    let shown: Vec<Vec<i32>> = motions.iter()
        .map(|(animation, looping, from, to)| {
            let (low, high) = (*from.min(to), *from.max(to));
            let high = looping.wraps(animation).map_or(high, |end| high.min(low.max(0).saturating_add(end)));
            let mut frames: Vec<i32> = (low..=high).map(|raw| looping.seat(animation, raw as f32).floor() as i32).collect();

            frames.sort_unstable();
            frames.dedup();
            frames
        })
        .collect();

    let total: usize = shown.iter().map(Vec::len).sum();
    progress.reset(total.max(1));

    info!("Starting bounds measurement over {} frames", total);

    let strictness = Tolerance::new(tolerance);

    if motions.is_empty() {
        return resting(unit, strictness, offset, progress);
    }

    let mut bounds: Option<BoundingBox> = None;

    for ((animation, _, _, _), frames) in motions.iter().zip(&shown) {
        for &frame in frames {
            if abort_signal.load(Ordering::Relaxed) {
                info!("Bounds measurement explicitly aborted by user.");
                return BoundsOutcome::Aborted;
            }

            if let Some(measured) = scan_bounds(unit, Some(animation), strictness, Some((frame, frame)), offset) {
                bounds = Some(bounds.map_or(measured, |bounds| bounds.union(&measured)));
            }

            if progress.advance().is_multiple_of(YIELD_INTERVAL) {
                thread::sleep(Duration::from_millis(1));
            }
        }
    }

    settle(bounds)
}

fn resting(unit: &Rig, strictness: Tolerance, offset: Option<usize>, progress: &ProgressCounter) -> BoundsOutcome {
    info!("No animation to sweep, measuring the rig's resting pose instead");

    let measured = scan_bounds(unit, None, strictness, Some((0, 0)), offset);
    progress.advance();

    settle(measured)
}

fn settle(bounds: Option<BoundingBox>) -> BoundsOutcome {
    bounds.map_or_else(
        || {
            warn!("No visible geometry found while measuring bounds.");
            BoundsOutcome::Empty
        },
        BoundsOutcome::Found,
    )
}
