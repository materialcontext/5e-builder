//! "Choose for me": fills pending choices with sensible picks.
//!
//! Heuristics, in order: never pick disabled options; avoid options with a
//! warning (duplicates); prefer skills/saves that use the character's best
//! abilities; put ability bonuses into the class's primary abilities first;
//! prefer 2024 sources when the same option exists twice.

use crate::build::{build, decode_prof, Build, ChoiceState, Step};
use crate::character::Character;
use crate::options::{options, source_rank, OptionItem};
use compendium::*;

fn score_option(o: &OptionItem, c: &ChoiceState, b: &Build, primary: &[Ability], comp: &Compendium) -> i32 {
    let mut s = 0;
    if o.warning.is_some() {
        s -= 100;
    }
    // Reprinted older versions only when nothing else fits.
    if o.older.is_some() {
        s -= 200;
    }
    match &c.options {
        Options::Abilities { .. } => {
            if let Some(a) = Ability::parse(&o.value) {
                if primary.contains(&a) {
                    s += 50 - primary.iter().position(|x| *x == a).unwrap_or(0) as i32;
                }
                // Odd scores benefit most from +1.
                s += b.score(a) + if b.score(a) % 2 == 1 { 3 } else { 0 };
                // Generally useful fallbacks when no primary ability is on offer.
                s += match a {
                    Ability::Con => 6,
                    Ability::Dex => 4,
                    Ability::Wis => 2,
                    _ => 0,
                };
            }
        }
        _ => {
            if let Some(p) = decode_prof(&o.value) {
                s += match p {
                    Prof::Skill(sk) => b.modifier(sk.ability()) * 10,
                    Prof::Save(a) => b.modifier(a) * 10 + if a == Ability::Con { 15 } else { 0 },
                    _ => 0,
                };
            }
            let key = Key(o.value.clone());
            if let Some(sp) = comp.spells.get(&key) {
                s -= source_rank(&sp.source) as i32;
            } else if let Some(f) = comp.feats.get(&key) {
                s -= source_rank(&f.source) as i32 * 2;
            } else if let Some(x) = comp.subclasses.get(&key) {
                s -= source_rank(&x.source) as i32 * 2;
            }
        }
    }
    s
}

/// Fills every pending choice (optionally only those of one step).
/// Returns how many choices were filled.
pub fn fill(ch: &mut Character, comp: &Compendium, step: Option<Step>) -> usize {
    let mut filled = 0;
    let mut skip: Vec<String> = Vec::new();
    for _ in 0..300 {
        let b = build(ch, comp);
        let Some(c) = b.pending().find(|c| step.is_none_or(|s| c.step == s) && !skip.contains(&c.path)).cloned() else { break };
        let primary: Vec<Ability> = ch.levels.first().and_then(|l| comp.classes.get(&l.class)).map(|c| c.primary.clone()).unwrap_or_default();
        let mut opts: Vec<OptionItem> = options(&c, &b, ch, comp).into_iter().filter(|o| o.disabled.is_none()).collect();
        // Stable sort keeps the options' natural order for ties.
        opts.sort_by_key(|o| -score_option(o, &c, &b, &primary, comp));
        let mut picks: Vec<String> = c.selected.iter().filter(|s| !s.is_empty()).cloned().collect();
        for o in opts {
            if picks.len() >= c.count as usize {
                break;
            }
            if !picks.contains(&o.value) {
                picks.push(o.value);
            }
        }
        if picks.len() < c.count as usize {
            skip.push(c.path.clone());
            continue;
        }
        ch.set_choice(&c.path, picks);
        filled += 1;
    }
    filled
}
