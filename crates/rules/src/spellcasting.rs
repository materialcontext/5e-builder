//! Spell slot tables and caster progression arithmetic.

use compendium::Caster;

/// Spell slots per spell level (1..=9) for a full caster of `caster_level`.
pub const FULL_CASTER_SLOTS: [[u8; 9]; 20] = [
    [2, 0, 0, 0, 0, 0, 0, 0, 0],
    [3, 0, 0, 0, 0, 0, 0, 0, 0],
    [4, 2, 0, 0, 0, 0, 0, 0, 0],
    [4, 3, 0, 0, 0, 0, 0, 0, 0],
    [4, 3, 2, 0, 0, 0, 0, 0, 0],
    [4, 3, 3, 0, 0, 0, 0, 0, 0],
    [4, 3, 3, 1, 0, 0, 0, 0, 0],
    [4, 3, 3, 2, 0, 0, 0, 0, 0],
    [4, 3, 3, 3, 1, 0, 0, 0, 0],
    [4, 3, 3, 3, 2, 0, 0, 0, 0],
    [4, 3, 3, 3, 2, 1, 0, 0, 0],
    [4, 3, 3, 3, 2, 1, 0, 0, 0],
    [4, 3, 3, 3, 2, 1, 1, 0, 0],
    [4, 3, 3, 3, 2, 1, 1, 0, 0],
    [4, 3, 3, 3, 2, 1, 1, 1, 0],
    [4, 3, 3, 3, 2, 1, 1, 1, 0],
    [4, 3, 3, 3, 2, 1, 1, 1, 1],
    [4, 3, 3, 3, 3, 1, 1, 1, 1],
    [4, 3, 3, 3, 3, 2, 1, 1, 1],
    [4, 3, 3, 3, 3, 2, 2, 1, 1],
];

fn ceil_div(a: u8, b: u8) -> u8 {
    a.div_ceil(b)
}

/// Caster level contributed by one class for a single-class character
/// (its own slot table).
pub fn single_class_caster_level(caster: Option<Caster>, level: u8, legacy: bool) -> u8 {
    match caster {
        Some(Caster::Full) => level,
        Some(Caster::Half) if legacy && level < 2 => 0,
        Some(Caster::Half) | Some(Caster::Artificer) => ceil_div(level, 2),
        Some(Caster::Third) if level < 3 => 0,
        Some(Caster::Third) => ceil_div(level, 3),
        Some(Caster::Pact) | None => 0,
    }
}

/// Caster level contributed to the shared multiclass slot table (2024 rules:
/// half casters round up, third casters round down).
pub fn multiclass_caster_level(caster: Option<Caster>, level: u8) -> u8 {
    match caster {
        Some(Caster::Full) => level,
        Some(Caster::Half) | Some(Caster::Artificer) => ceil_div(level, 2),
        Some(Caster::Third) => level / 3,
        Some(Caster::Pact) | None => 0,
    }
}

pub fn slots_for_caster_level(cl: u8) -> [u8; 9] {
    if cl == 0 {
        [0; 9]
    } else {
        FULL_CASTER_SLOTS[(cl.min(20) - 1) as usize]
    }
}

/// Warlock Pact Magic: (number of slots, slot level).
pub fn pact_slots(level: u8) -> (u8, u8) {
    let n = match level {
        0 => 0,
        1 => 1,
        2..=10 => 2,
        11..=16 => 3,
        _ => 4,
    };
    (n, ceil_div(level, 2).min(5))
}

/// Highest spell level a class can learn or prepare at a class level.
pub fn max_spell_level(caster: Option<Caster>, level: u8, legacy: bool) -> u8 {
    if caster == Some(Caster::Pact) {
        return pact_slots(level).1;
    }
    let slots = slots_for_caster_level(single_class_caster_level(caster, level, legacy));
    slots.iter().rposition(|&n| n > 0).map(|i| i as u8 + 1).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_levels() {
        assert_eq!(max_spell_level(Some(Caster::Full), 1, false), 1);
        assert_eq!(max_spell_level(Some(Caster::Full), 5, false), 3);
        assert_eq!(max_spell_level(Some(Caster::Full), 17, false), 9);
        assert_eq!(max_spell_level(Some(Caster::Half), 1, false), 1);
        assert_eq!(max_spell_level(Some(Caster::Half), 1, true), 0);
        assert_eq!(max_spell_level(Some(Caster::Half), 5, false), 2);
        assert_eq!(max_spell_level(Some(Caster::Third), 3, false), 1);
        assert_eq!(max_spell_level(Some(Caster::Third), 7, false), 2);
        assert_eq!(max_spell_level(Some(Caster::Pact), 9, false), 5);
        assert_eq!(max_spell_level(None, 9, false), 0);
    }
}
