use crate::chapter::treasure::weaken;

pub fn magnified(stat: i32, percent: i32, gap: i32) -> i32 {
    let scaled = i64::from(percent).wrapping_mul(i64::from(stat)).wrapping_add(50) / 100;

    weaken(scaled, gap) as i32
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

    // Outside the Empire of Cats chapters the game halves three times the cost and drops the half.
    #[test]
    fn an_odd_cost_rounds_down_like_the_game() {
        assert_eq!(deploy_cost(50), 75);
        assert_eq!(deploy_cost(75), 112);
        assert_eq!(deploy_cost(-10), 0);
    }
}
