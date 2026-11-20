use serde::{Deserialize, Serialize};
use tracing::trace;

#[derive(Default, Debug, Clone, Serialize, Deserialize, Hash)]
pub struct StatRange {
    pub min: String,
    pub max: String,
}

impl StatRange {
    pub fn is_active(&self) -> bool {
        !self.min.trim().is_empty() || !self.max.trim().is_empty()
    }

    pub(crate) fn compile(&self, offset: i64) -> CompiledStatRange {
        let min_val = if self.min.trim().is_empty() {
            i64::MIN
        } else {
            self.min.trim().parse::<i64>().map(|val| val + offset).unwrap_or_else(|_| {
                trace!("Failed to parse min filter value: {}", self.min);
                i64::MIN
            })
        };

        let max_val = if self.max.trim().is_empty() {
            i64::MAX
        } else {
            self.max.trim().parse::<i64>().map(|val| val + offset).unwrap_or_else(|_| {
                trace!("Failed to parse max filter value: {}", self.max);
                i64::MAX
            })
        };

        CompiledStatRange {
            min: min_val,
            max: max_val,
            active: self.is_active(),
        }
    }

    pub(crate) fn compile_fraction(&self) -> CompiledFractionRange {
        CompiledFractionRange {
            min: fraction_bound(&self.min, f64::NEG_INFINITY),
            max: fraction_bound(&self.max, f64::INFINITY),
            active: self.is_active(),
        }
    }
}

fn fraction_bound(text: &str, open: f64) -> f64 {
    let trimmed = text.trim().trim_end_matches('%').trim();

    if trimmed.is_empty() {
        return open;
    }

    trimmed.parse::<f64>().ok().filter(|value| value.is_finite()).unwrap_or_else(|| {
        trace!("Failed to parse fractional filter value: {}", text);
        open
    })
}

pub(crate) struct CompiledFractionRange {
    min: f64,
    max: f64,
    active: bool,
}

impl CompiledFractionRange {
    pub(crate) fn matches(&self, value: f64) -> bool {
        !self.active || (value >= self.min && value <= self.max)
    }
}

pub(crate) struct CompiledStatRange {
    pub min: i64,
    pub max: i64,
    pub active: bool,
}

impl CompiledStatRange {
    pub(crate) fn matches(&self, target_val: i64) -> bool {
        if !self.active { return true; }
        target_val >= self.min && target_val <= self.max
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn range(min: &str, max: &str) -> CompiledFractionRange {
        StatRange { min: min.to_owned(), max: max.to_owned() }.compile_fraction()
    }

    // A material shows 13.33%, so typing that, or 13%, has to find it.
    #[test]
    fn a_chance_range_takes_decimals_and_a_percent_sign() {
        assert!(range("13.33", "13.33").matches(13.33));
        assert!(range("13%", "").matches(13.33));
        assert!(!range("", "13").matches(13.33));
        assert!(range("", "").matches(0.0));
    }

    #[test]
    fn an_unreadable_bound_leaves_that_side_open() {
        assert!(range("lots", "20").matches(5.0));
        assert!(!range("lots", "20").matches(25.0));
    }
}
