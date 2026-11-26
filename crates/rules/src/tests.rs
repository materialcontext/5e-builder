//! Integration tests against the real compendium (skipped when it hasn't been generated).

use crate::*;
use compendium::{Ability, Compendium, Key, Skill};
use std::sync::OnceLock;

fn comp() -> Option<&'static Compendium> {
    static C: OnceLock<Option<Compendium>> = OnceLock::new();
    C.get_or_init(|| {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../web/public/compendium.json");
        std::fs::read_to_string(path).ok().map(|s| Compendium::from_json(&s).expect("valid compendium"))
    })
    .as_ref()
}

/// Resolves every pending choice by picking the first allowed options.
pub fn autofill(ch: &mut Character, comp: &Compendium) {
    for _ in 0..50 {
        let b = build(ch, comp);
        let Some(c) = b.pending().next().cloned() else { return };
        let opts = options(&c, &b, ch, comp);
        let mut picks = c.selected.clone();
        let mut allowed: Vec<_> = opts.iter().filter(|o| o.disabled.is_none()).collect();
        allowed.sort_by_key(|o| o.warning.is_some());
        for o in allowed {
            if picks.len() >= c.count as usize {
                break;
            }
            if !picks.contains(&o.value) {
                picks.push(o.value.clone());
            }
        }
        if picks.len() < c.count as usize {
            panic!("cannot satisfy {} ({} options)", c.path, opts.len());
        }
        ch.set_choice(&c.path, picks);
    }
    panic!("autofill did not converge");
}

fn hero(class: &str, background: &str, species: &str) -> Character {
    let mut ch = Character::new("t");
    ch.name = "Test".into();
    ch.abilities = BaseAbilities { method: AbilityMethod::StandardArray, scores: [8, 14, 13, 15, 12, 10] };
    ch.set_starting_class(Key(class.into()));
    ch.set_background(Some(Key(background.into())));
    ch.set_species(Some(Key(species.into())));
    ch
}

#[test]
fn level_one_wizard_2024() {
    let Some(comp) = comp() else { return };
    let mut ch = hero("wizard|xphb", "sage|xphb", "elf|xphb");
    let b = build(&ch, comp);
    let paths: Vec<&str> = b.choices.iter().map(|c| c.path.as_str()).collect();
    assert!(paths.contains(&"species/lineage"), "{paths:?}");
    assert!(paths.iter().any(|p| p.ends_with("/cantrips")));
    assert!(paths.iter().any(|p| p.ends_with("/spellbook")));

    ch.set_choice("species/lineage", vec!["elf; high elf lineage|xphb".into()]);
    autofill(&mut ch, comp);
    let b = build(&ch, comp);
    assert_eq!(b.pending().count(), 0);
    let issues = issues(&ch, &b, comp);
    assert!(issues.is_empty(), "{issues:?}");

    let s = sheet(&ch, &b, comp);
    assert_eq!(s.proficiency_bonus, 2);
    // Sage gives +2/+1 (or +1/+1/+1) among Con/Int/Wis; Int 15 base.
    assert!(s.abilities[Ability::Int.index()].score >= 16);
    assert!(s.abilities[Ability::Int.index()].save_prof);
    assert_eq!(s.hp_max, 6 + s.abilities[Ability::Con.index()].modifier);
    assert_eq!(s.slots[0], 2);
    // 3 cantrips + Elf's cantrip, 6 spellbook spells.
    assert!(s.spells.iter().filter(|x| x.level == 0).count() >= 4, "{:?}", s.spells);
    assert_eq!(s.spells.iter().filter(|x| x.level == 1 && x.status != "1/long rest").count(), 6);
    assert!(s.skills.iter().any(|x| x.skill == Skill::Arcana && x.prof != sheet::Proficiency::None));
    assert!(s.features.iter().any(|f| f.feature.name == "Arcane Recovery"));
    assert_eq!(s.darkvision, 60);
    assert_eq!(s.languages.len(), 3, "{:?}", s.languages);
    assert!(s.languages.contains(&"Common".to_string()));
}

#[test]
fn fighter_with_equipment_and_mastery() {
    let Some(comp) = comp() else { return };
    let mut ch = hero("fighter|xphb", "soldier|xphb", "human|xphb");
    ch.abilities.scores = [15, 13, 14, 8, 12, 10];
    autofill(&mut ch, comp);
    apply_starting_equipment(&mut ch, comp);
    let b = build(&ch, comp);
    let mastery = b.choices.iter().find(|c| c.path.ends_with("/mastery")).expect("mastery choice");
    assert_eq!(mastery.count, 3);
    assert!(b.feats.len() >= 2, "origin feat + human versatile + fighting style: {:?}", b.feats);
    let s = sheet(&ch, &b, comp);
    assert!(b.choices.iter().any(|c| c.path == "species/size"));
    let opts = options(mastery, &b, &ch, comp);
    assert!(opts.iter().any(|o| o.label == "Longsword"));
    assert!(!opts.iter().any(|o| o.label.contains("Rifle")));
    assert!(!ch.inventory.is_empty());
    assert!(s.armor_class >= 16, "chain mail: {} ({})", s.armor_class, s.ac_note);
    assert!(s.attacks.len() >= 2);
    assert_eq!(s.hp_max, 10 + 2);
}

#[test]
fn legacy_content_under_2024_rules() {
    let Some(comp) = comp() else { return };
    let mut ch = hero("rogue|phb", "criminal|phb", "dwarf|phb");
    let b = build(&ch, comp);
    // Legacy background gets the 2024 ASI and origin-feat choices.
    assert!(b.choices.iter().any(|c| c.path == "background/asi"));
    assert!(b.choices.iter().any(|c| c.path == "background/origin-feat"));
    autofill(&mut ch, comp);
    let b = build(&ch, comp);
    assert_eq!(b.pending().count(), 0);
    // Legacy species ASI ignored by default: Con base 13 + background only.
    let s = sheet(&ch, &b, comp);
    assert!(s.speed.walk == 25);
    assert!(s.resistances.iter().any(|r| r == "Poison"));
    // Legacy dwarf defines its own languages (Common, Dwarvish); no extra picks.
    assert!(!b.choices.iter().any(|c| c.path == "species/languages"));
    assert!(s.languages.contains(&"Dwarvish".to_string()));

    ch.settings.legacy_species_asi = true;
    let b2 = build(&ch, comp);
    assert!(!b2.choices.iter().any(|c| c.path == "background/asi"));
}

#[test]
fn multiclass_and_level_down() {
    let Some(comp) = comp() else { return };
    let mut ch = hero("cleric|xphb", "acolyte|xphb", "dwarf|xphb");
    ch.set_starting_level(3);
    ch.level_up(Key("wizard|xphb".into()));
    autofill(&mut ch, comp);
    let b = build(&ch, comp);
    assert_eq!(b.pending().count(), 0);
    let s = sheet(&ch, &b, comp);
    assert_eq!(s.level, 4);
    assert_eq!(s.hit_dice, "3d8 + 1d6");
    // Full caster 3 + full caster 1 = caster level 4.
    assert_eq!(s.slots[..2], [4, 3]);
    // Dwarven Toughness: +1 HP per level.
    let con = s.abilities[Ability::Con.index()].modifier;
    assert_eq!(s.hp_max, 8 + 5 * 2 + 4 + con * 4 + 4);

    ch.level_down();
    ch.prune_choices(comp);
    assert!(!ch.choices.keys().any(|k| k.starts_with("class:wizard|xphb")));
    assert_eq!(build(&ch, comp).pending().count(), 0);
}

#[test]
fn every_2024_class_resolves_at_levels_1_to_20() {
    let Some(comp) = comp() else { return };
    for class in comp.classes.iter().filter(|c| !c.legacy && c.source == "XPHB") {
        let mut ch = hero(&class.key.0, "sage|xphb", "human|xphb");
        ch.abilities.scores = [13, 13, 13, 13, 13, 13];
        ch.abilities.method = AbilityMethod::Manual;
        ch.set_starting_level(20);
        autofill(&mut ch, comp);
        let b = build(&ch, comp);
        assert!(b.warnings.is_empty(), "{}: {:?}", class.name, b.warnings);
        let s = sheet(&ch, &b, comp);
        assert_eq!(s.proficiency_bonus, 6, "{}", class.name);
    }
}

#[test]
fn formula_eval() {
    let scores = [10, 10, 10, 10, 16, 14];
    assert_eq!(build::eval_formula("<$level$> + <$wis_mod$>", 5, &scores), 8);
    assert_eq!(build::eval_formula("<$level$> / 2 + <$cha_mod$>", 5, &scores), 4);
}

#[test]
fn point_buy_validation() {
    let mut b = BaseAbilities { method: AbilityMethod::PointBuy, scores: [15, 15, 15, 8, 8, 8] };
    assert!(ability_problem(&b).is_none());
    b.scores[3] = 9;
    assert!(ability_problem(&b).is_some());
}


#[test]
fn every_class_and_species_resolves() {
    let Some(comp) = comp() else { return };
    let mut failures = Vec::new();
    for class in comp.classes.iter() {
        for lvl in [1u8, 5, 20] {
            let mut ch = hero(&class.key.0, "acolyte|phb", "human|phb");
            ch.abilities = BaseAbilities { method: AbilityMethod::Manual, scores: [13; 6] };
            ch.set_starting_level(lvl);
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| autofill(&mut ch, comp)));
            if r.is_err() {
                failures.push(format!("{} L{lvl}", class.key));
            }
        }
    }
    for sp in comp.species.iter() {
        let mut ch = hero("fighter|xphb", "sage|xphb", &sp.key.0);
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| autofill(&mut ch, comp))).is_err() {
            failures.push(sp.key.0.clone());
        }
    }
    for bg in comp.backgrounds.iter() {
        let mut ch = hero("fighter|xphb", &bg.key.0, "human|xphb");
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| autofill(&mut ch, comp))).is_err() {
            failures.push(bg.key.0.clone());
        }
    }
    assert!(failures.is_empty(), "{} failures: {failures:?}", failures.len());
}
