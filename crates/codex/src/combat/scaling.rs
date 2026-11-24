use crate::chapter::treasure::weaken;

const WHOLE: i32 = 100;

pub fn crowned(stat: i32, percent: i32, crown: i32, gap: i32) -> i32 {
    let scaled = i64::from(percent).wrapping_mul(i64::from(stat)).wrapping_add(50) / 100;
    let scaled = scaled.wrapping_mul(i64::from(crown)) / 100;

    weaken(scaled, gap) as i32
}

pub fn magnified(stat: i32, percent: i32, gap: i32) -> i32 {
    crowned(stat, percent, WHOLE, gap)
}

pub fn deploy_cost(eoc1_cost: i32) -> i32 {
    (i64::from(eoc1_cost.max(0)) * 3 / 2) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    // The engine rounds the magnification half up in integers, then adds the alien gap on top.
    // The old float path landed on 999 here.
    #[test]
    fn a_magnified_enemy_matches_the_engine() {
        assert_eq!(magnified(333, 150, 0), 500);
        assert_eq!(magnified(333, 150, 100), 1000);
        assert_eq!(magnified(50, 29, 0), 15);
        assert_eq!(magnified(1000, 100, 0), 1000);
    }

    // A crown is a second step: the magnified stat is rounded first, then the crown's share is taken and the rest dropped.
    // So 75% at a 150% crown is worth 112.5%, which one merged whole percent can never say.
    #[test]
    fn a_crown_is_applied_after_the_magnification() {
        assert_eq!(crowned(1_000_000, 75, 150, 0), 1_125_000);
        assert_eq!(crowned(335, 75, 150, 0), 376, "251 after the first step, then 376.5 with the half dropped");
        assert_eq!(crowned(333, 150, 100, 100), magnified(333, 150, 100));
        assert_eq!(crowned(200, 1, 130, 0), 2);
    }

    // Outside the Empire of Cats chapters the game halves three times the cost and drops the half.
    #[test]
    fn an_odd_cost_rounds_down_like_the_game() {
        assert_eq!(deploy_cost(50), 75);
        assert_eq!(deploy_cost(75), 112);
        assert_eq!(deploy_cost(-10), 0);
    }
}
