

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Percent,
    Multiplier,
    Frames,
    Count,
    Range,
    Money,
    Inverted,
}

#[derive(Clone, Copy)]
pub struct Value {
    pub label: &'static str,
    pub unit: Unit,
    pub fixed: bool,
}

impl Value {
    pub const fn new(label: &'static str, unit: Unit) -> Value {
        Value { label, unit, fixed: false }
    }

    pub const fn fixed(mut self) -> Value {
        self.fixed = true;

        self
    }

    pub fn label(self) -> &'static str {
        self.label
    }

    pub fn unit(self) -> Unit {
        self.unit
    }

    pub(super) fn is_fixed(self) -> bool {
        self.fixed
    }
}

pub(super) const CHANCE: Value = Value::new("Chance", Unit::Percent);

const DURATION: Value = Value::new("Duration", Unit::Frames);

pub(super) const LEVEL: Value = Value::new("Level", Unit::Count).fixed();

const RESIST: Value = Value::new("Resistance", Unit::Percent);

const BOOST: Value = Value::new("Boost", Unit::Multiplier);

const ANCHOR: Value = Value::new("Spawn Anchor", Unit::Range).fixed();

const SPAN: Value = Value::new("Spawn Span", Unit::Range).fixed();

const SURGE: &[Value] = &const { [CHANCE, LEVEL, ANCHOR, SPAN] };

const WAVE: &[Value] = &const { [CHANCE, LEVEL] };

const CHANCE_ONLY: &[Value] = &const { [CHANCE] };

const RESIST_ONLY: &[Value] = &const { [RESIST] };

const AILMENT: &[Value] = &const { [CHANCE, DURATION] };

pub fn values(ability: i32) -> Option<&'static [Value]> {
    let listed: &'static [Value] = match ability {
        1 => &const { [CHANCE, DURATION, Value::new("Reduced To", Unit::Inverted).fixed()] },
        2 | 3 | 60 => AILMENT,
        8 | 11 | 13 | 15 | 58 => CHANCE_ONLY,
        9 => &const { [CHANCE, DURATION, Value::new("Min Distance", Unit::Range).fixed(), Value::new("Max Distance", Unit::Range).fixed()] },
        10 => &const { [Value::new("Health Threshold", Unit::Inverted), BOOST] },
        17 | 62 => WAVE,
        18..=22 | 30 | 52 | 54 => RESIST_ONLY,
        25 => &const { [Value::new("Reduction", Unit::Money)] },
        26 => &const { [Value::new("Reduction", Unit::Frames)] },
        27 | 28 => &const { [Value::new("Increase", Unit::Count)] },
        31 | 32 | 70 | 71 => &const { [Value::new("Increase", Unit::Percent)] },
        50 => &const { [CHANCE, BOOST] },
        51 => AILMENT,
        56 | 65 => SURGE,
        61 | 72 => &const { [Value::new("Reduction", Unit::Percent)] },
        64 => &const {
            [Value::new("Dodge Chance", Unit::Percent), Value::new("Dodge Duration", Unit::Frames)]
        },
        67 => &const { [CHANCE, ANCHOR, SPAN] },
        73 => &const { [Value::new("Combo Row", Unit::Count).fixed()] },
        4..=7 | 12 | 14 | 16 | 23 | 24 | 29 | 33..=49 | 53 | 55 | 57 | 59 | 63 | 66 | 68 | 69 => &[],
        _ => return None,
    };

    Some(listed)
}

pub fn value(ability: i32, pair: usize) -> Option<Value> {
    values(ability)?.get(pair).copied()
}
