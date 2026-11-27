//! Integration tests against the real compendium (skipped when it hasn't been generated).

use crate::*;
use compendium::{Ability, Compendium, Key, Options, Prof, Skill};
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
    for _ in 0..200 {
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
    ch.abilities = BaseAbilities { method: AbilityMethod::StandardArray, scores: [8, 14, 13, 15, 12, 10], rolls: vec![] };
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
    let mut b = BaseAbilities { method: AbilityMethod::PointBuy, scores: [15, 15, 15, 8, 8, 8], rolls: vec![] };
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
            ch.abilities = BaseAbilities { method: AbilityMethod::Manual, scores: [13; 6], rolls: vec![] };
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

#[test]
fn audit_content() {
    let Some(comp) = comp() else { return };
    let r = audit::audit(comp);
    for p in &r.problems {
        eprintln!("AUDIT {p}");
    }
    assert!(r.problems.is_empty(), "{} audit problems", r.problems.len());
}

fn choice_with<'a>(b: &'a Build, suffix: &str) -> &'a ChoiceState {
    b.choices.iter().find(|c| c.path.ends_with(suffix)).unwrap_or_else(|| panic!("no choice ending {suffix}: {:#?}", b.choices.iter().map(|c| &c.path).collect::<Vec<_>>()))
}

fn option_labels(ch: &Character, b: &Build, c: &ChoiceState, comp: &Compendium) -> Vec<String> {
    options(c, b, ch, comp).into_iter().filter(|o| o.disabled.is_none()).map(|o| o.label).collect()
}

#[test]
fn resilient_grants_a_save() {
    let Some(comp) = comp() else { return };
    let mut ch = hero("fighter|xphb", "soldier|xphb", "human|xphb");
    ch.set_starting_level(4);
    autofill(&mut ch, comp);
    let asi = build(&ch, comp).choices.iter().find(|c| c.label.starts_with("Ability Score Improvement")).unwrap().path.clone();
    ch.set_choice(&asi, vec!["resilient|xphb".into()]);
    autofill(&mut ch, comp);
    let b = build(&ch, comp);
    let saves = b.choices.iter().find(|c| matches!(c.options, Options::Saves(_))).expect("save choice");
    let pick = saves.selected[0].clone();
    assert!(b.has_prof(&build::decode_prof(&pick).unwrap()));
}

#[test]
fn divine_order_protector_grants_martial_and_heavy() {
    let Some(comp) = comp() else { return };
    let mut ch = hero("cleric|xphb", "acolyte|xphb", "human|xphb");
    let b = build(&ch, comp);
    let order = b.choices.iter().find(|c| c.label == "Divine Order").expect("divine order").path.clone();
    ch.set_choice(&order, vec!["Protector".into()]);
    let b = build(&ch, comp);
    assert!(b.has_prof(&Prof::Weapon("martial".into())));
    assert!(b.has_prof(&Prof::Armor("heavy".into())));
    ch.set_choice(&order, vec!["Thaumaturge".into()]);
    let b = build(&ch, comp);
    assert!(b.choices.iter().any(|c| c.path.starts_with(&format!("{order}/Thaumaturge")) && matches!(c.options, Options::Spells(_))));
}

#[test]
fn warlock_mystic_arcanum_and_expanded_list() {
    let Some(comp) = comp() else { return };
    let mut ch = hero("warlock|xphb", "sage|xphb", "human|xphb");
    ch.set_starting_level(11);
    let b = build(&ch, comp);
    let arcanum = b.choices.iter().find(|c| matches!(&c.options, Options::Spells(f) if f.levels == [6])).expect("6th-level arcanum");
    assert!(option_labels(&ch, &b, arcanum, comp).iter().any(|l| l == "Mass Suggestion"));

    // 2014 Fiend patron adds Burning Hands to the warlock list.
    let mut ch = hero("warlock|phb", "sage|xphb", "human|xphb");
    ch.set_starting_level(3);
    let b = build(&ch, comp);
    let sub = choice_with(&b, "/subclass").path.clone();
    let fiend = comp.subclasses_of(&Key("warlock|phb".into())).find(|s| s.short_name == "Fiend" && s.source == "PHB").unwrap().key.0.clone();
    ch.set_choice(&sub, vec![fiend]);
    let b = build(&ch, comp);
    let known = choice_with(&b, "/known");
    assert!(option_labels(&ch, &b, known, comp).iter().any(|l| l == "Burning Hands"));
}

#[test]
fn bard_magical_secrets_2024_expands_list() {
    let Some(comp) = comp() else { return };
    let mut ch = hero("bard|xphb", "sage|xphb", "human|xphb");
    ch.set_starting_level(9);
    let b = build(&ch, comp);
    assert!(!option_labels(&ch, &b, choice_with(&b, "/prepared"), comp).iter().any(|l| l == "Fireball"));
    ch.set_starting_level(10);
    let b = build(&ch, comp);
    assert!(option_labels(&ch, &b, choice_with(&b, "/prepared"), comp).iter().any(|l| l == "Fireball"));
}

#[test]
fn tasha_optional_features_are_opt_in() {
    let Some(comp) = comp() else { return };
    let mut ch = hero("rogue|phb", "criminal|phb", "human|phb");
    ch.set_starting_level(3);
    let b = build(&ch, comp);
    let opt = b.choices.iter().find(|c| c.label == "Optional feature: Steady Aim").expect("steady aim choice").path.clone();
    assert!(!b.has_feature_named("Steady Aim"));
    ch.set_choice(&opt, vec!["Use it".into()]);
    assert!(build(&ch, comp).has_feature_named("Steady Aim"));
}

#[test]
fn feats_granting_options() {
    let Some(comp) = comp() else { return };
    let mut ch = hero("wizard|xphb", "acolyte|phb", "human|xphb");
    ch.set_choice("background/origin-feat", vec![]);
    let b = build(&ch, comp);
    let origin = choice_with(&b, "background/origin-feat").path.clone();
    let _ = origin;
    // Fighting Initiate (TCE, no category) through the 2024 general feat at 4.
    ch.set_starting_level(4);
    let b = build(&ch, comp);
    let asi = b.choices.iter().find(|c| c.label.starts_with("Ability Score Improvement")).unwrap().path.clone();
    ch.set_choice(&asi, vec!["fighting initiate|tce".into()]);
    let b = build(&ch, comp);
    assert!(b.choices.iter().any(|c| c.path.starts_with(&format!("{asi}/fighting initiate|tce")) && matches!(c.options, Options::OptionalFeatures { .. })));
}

#[test]
fn wizard_scholar_expertise_list() {
    let Some(comp) = comp() else { return };
    let mut ch = hero("wizard|xphb", "sage|xphb", "human|xphb");
    ch.set_starting_level(2);
    let b = build(&ch, comp);
    let c = b.choices.iter().find(|c| matches!(c.options, Options::ExpertiseFrom(_))).expect("scholar");
    assert_eq!(c.count, 1);
}

#[test]
fn subrace_overwrites_languages() {
    let Some(comp) = comp() else { return };
    let sp = comp.species.get(&Key("elf|phb".into())).unwrap();
    let high = sp.lineages.iter().find(|l| l.name == "High").unwrap();
    assert!(high.base_override.is_some());
}

#[test]
fn starting_gold_alternative_for_2014_classes() {
    let Some(comp) = comp() else { return };
    let ch = hero("fighter|phb", "soldier|phb", "human|phb");
    let b = build(&ch, comp);
    let eq = b.choices.iter().find(|c| c.path.ends_with("equipment.or-gold")).expect("gold alternative");
    assert!(matches!(&eq.options, Options::Bundles(b) if b.len() == 2 && b[1].name.contains("Gold")));
}

#[test]
fn focus_items_resolve() {
    let Some(comp) = comp() else { return };
    for k in ["holy symbol|xphb", "druidic focus|xphb", "arcane focus|xphb"] {
        assert!(comp.items.get(&Key(k.into())).is_some(), "{k}");
    }
}

#[test]
fn invocation_prerequisites_enforced() {
    let Some(comp) = comp() else { return };
    // 2014 Agonizing Blast needs the eldritch blast cantrip.
    let agonizing = comp.optional_features.get(&Key("agonizing blast|phb".into())).unwrap();
    let p = agonizing.prereq.clone().unwrap();
    let mut ch = hero("warlock|phb", "sage|xphb", "human|xphb");
    ch.set_starting_level(2);
    let b = build(&ch, comp);
    assert!(options::prereq_problem_in(&p, &b, &ch, Some(comp)).is_some());
    let cantrips = choice_with(&b, "/cantrips").path.clone();
    ch.set_choice(&cantrips, vec!["eldritch blast|phb".into(), "minor illusion|phb".into()]);
    let b = build(&ch, comp);
    assert!(options::prereq_problem_in(&p, &b, &ch, Some(comp)).is_none());
    // 2024 invocations check warlock level, not character level.
    let lessons = comp.optional_features.get(&Key("lessons of the first ones|xphb".into())).unwrap();
    let lp = lessons.prereq.clone().unwrap();
    assert_eq!(lp.level_class.as_deref(), Some("warlock"));
}

#[test]
fn manual_inventory_edits_survive() {
    let Some(comp) = comp() else { return };
    let mut ch = hero("fighter|xphb", "soldier|xphb", "human|xphb");
    autofill(&mut ch, comp);
    apply_starting_equipment(&mut ch, comp);
    let b = build(&ch, comp);
    assert_eq!(ch.starting_signature, starting_signature(&ch, &b));
    // Player spends 3 gp and drops an item: nothing re-syncs until choices change.
    let before = ch.coins.total_cp();
    ch.coins = Coins::from_cp(before - 300);
    ch.inventory.remove(0);
    let n = ch.inventory.len();
    let b = build(&ch, comp);
    assert_eq!(ch.starting_signature, starting_signature(&ch, &b));
    // Changing the equipment choice swaps items and keeps the 3 gp spent.
    let eq = b.choices.iter().find(|c| c.step == Step::Equipment && c.path.contains("class:")).unwrap().clone();
    let other = match &eq.options { Options::Bundles(bs) => bs.iter().find(|x| !eq.selected.contains(&x.name)).unwrap().name.clone(), _ => unreachable!() };
    ch.set_choice(&eq.path, vec![other]);
    apply_starting_equipment(&mut ch, comp);
    let (_, coins) = starting_equipment(&build(&ch, comp), comp);
    let bg_coins: u32 = 0;
    let _ = (n, bg_coins);
    assert_eq!(ch.coins.total_cp() + 300, coins);
}

#[test]
fn multiclass_requirements_checked() {
    let Some(comp) = comp() else { return };
    let mut ch = hero("fighter|xphb", "soldier|xphb", "human|xphb");
    ch.abilities = BaseAbilities { method: AbilityMethod::Manual, scores: [15, 10, 14, 8, 12, 10], rolls: vec![] };
    ch.level_up(Key("wizard|xphb".into()));
    autofill(&mut ch, comp);
    let b = build(&ch, comp);
    let iss = issues(&ch, &b, comp);
    assert!(iss.iter().any(|i| i.text.contains("Wizard requires INT 13")), "{iss:?}");
}

#[test]
fn suggestions_complete_a_character_sensibly() {
    let Some(comp) = comp() else { return };
    let mut ch = hero("wizard|xphb", "sage|xphb", "elf|xphb");
    let n = suggest::fill(&mut ch, comp, None);
    assert!(n > 5);
    let b = build(&ch, comp);
    assert_eq!(b.pending().count(), 0);
    // Background bonus goes to Intelligence (the wizard's primary ability).
    assert!(b.score(Ability::Int) >= 16, "{:?}", b.scores);
    assert!(issues(&ch, &b, comp).is_empty());
}

#[test]
fn rolled_scores() {
    let mut seq = [0.99, 0.0, 0.5, 0.99].iter().cycle().copied();
    let r = roll_ability_scores(|| seq.next().unwrap());
    assert_eq!(r.len(), 6);
    assert!(r.iter().all(|&x| (3..=18).contains(&x)));
    let b = BaseAbilities { method: AbilityMethod::Rolled, scores: [r[0], r[1], r[2], r[3], r[4], r[5]], rolls: r.clone() };
    assert!(ability_problem(&b).is_none());
}

fn has_spells(grants: &[compendium::Grant]) -> bool {
    use compendium::Grant;
    grants.iter().any(|g| match g {
        Grant::Spells(_) => true,
        Grant::Choose(c) => matches!(&c.options, Options::Bundles(b) if b.iter().any(|x| has_spells(&x.grants))),
        Grant::AtLevel { grants, .. } => has_spells(grants),
        _ => false,
    })
}

/// A level-`lvl` caster whose first Ability Score Improvement takes `feat`.
fn with_feat(class: &str, lvl: u8, feat: &str, comp: &Compendium) -> Character {
    let mut ch = hero(class, "sage|xphb", "human|xphb");
    ch.abilities = BaseAbilities { method: AbilityMethod::Manual, scores: [13, 14, 14, 15, 13, 13], rolls: vec![] };
    ch.set_starting_level(lvl);
    let asi = build(&ch, comp).choices.iter().find(|c| c.label.starts_with("Ability Score Improvement")).unwrap().path.clone();
    ch.set_choice(&asi, vec![feat.into()]);
    autofill(&mut ch, comp);
    ch
}

#[test]
fn every_spell_feat_works_in_a_build() {
    let Some(comp) = comp() else { return };
    let mut failures = Vec::new();
    for f in comp.feats.iter().filter(|f| has_spells(&f.grants)) {
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| with_feat("wizard|xphb", 8, &f.key.0, comp)));
        let Ok(ch) = r else {
            failures.push(format!("{}: choices cannot be completed", f.key));
            continue;
        };
        let b = build(&ch, comp);
        let origin = format!("Feat: {}", f.name);
        let mine: Vec<_> = b.spells.iter().filter(|s| s.origin == origin).collect();
        if mine.is_empty() {
            failures.push(format!("{}: grants no spells", f.key));
        }
        for s in mine.iter().filter(|s| s.ability.is_none()) {
            failures.push(format!("{}: {} has no spellcasting ability", f.key, s.key));
        }
        for s in &mine {
            if comp.spells.get(&s.key).is_none() {
                failures.push(format!("{}: unknown spell {}", f.key, s.key));
            }
        }
    }
    assert!(failures.is_empty(), "{} problems:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn fey_touched_uses_the_raised_ability() {
    let Some(comp) = comp() else { return };
    let mut ch = with_feat("fighter|xphb", 4, "fey-touched|xphb", comp);
    let b = build(&ch, comp);
    let asi = b.choices.iter().find(|c| c.path.contains("fey-touched|xphb") && matches!(c.options, Options::Abilities { .. })).unwrap().path.clone();
    ch.set_choice(&asi, vec!["wis".into()]);
    let b = build(&ch, comp);
    let misty = b.spells.iter().find(|s| s.key.0 == "misty step|xphb").expect("misty step");
    assert_eq!(misty.ability, Some(Ability::Wis));
    let s = sheet(&ch, &b, comp);
    assert!(s.casting.iter().any(|c| c.ability == Ability::Wis));
}

#[test]
fn spell_sniper_offers_only_attack_cantrips() {
    let Some(comp) = comp() else { return };
    let mut ch = with_feat("wizard|phb", 4, "spell sniper|phb", comp);
    let b = build(&ch, comp);
    let list = b.choices.iter().find(|c| c.path.contains("spell sniper|phb") && matches!(c.options, Options::Bundles(_))).unwrap().path.clone();
    ch.set_choice(&list, vec!["Wizard Spells".into()]);
    let b = build(&ch, comp);
    let pick = b.choices.iter().find(|c| c.path.contains("spell sniper|phb") && matches!(c.options, Options::Spells(_))).unwrap();
    let opts = options(pick, &b, &ch, comp);
    assert!(!opts.is_empty());
    for o in &opts {
        let sp = comp.spells.get(&Key(o.value.clone())).unwrap();
        assert!(!sp.attack.is_empty() && sp.level == 0, "{} is not an attack cantrip", sp.name);
    }
    assert!(opts.iter().any(|o| o.label == "Fire Bolt"));
    assert!(!opts.iter().any(|o| o.label == "Prestidigitation"));
}

#[test]
fn spellcasting_feature_prerequisite() {
    let Some(comp) = comp() else { return };
    let war_caster = comp.feats.get(&Key("war caster|xphb".into())).unwrap().prereq.clone().unwrap();
    // A fighter with Magic Initiate can cast spells but has no Spellcasting feature.
    let mut fighter = hero("fighter|xphb", "acolyte|xphb", "human|xphb");
    fighter.set_starting_level(4);
    autofill(&mut fighter, comp);
    let b = build(&fighter, comp);
    assert!(!b.spells.is_empty());
    assert!(options::prereq_problem_in(&war_caster, &b, &fighter, Some(comp)).is_some());
    let mut cleric = hero("cleric|xphb", "acolyte|xphb", "human|xphb");
    cleric.set_starting_level(4);
    let b = build(&cleric, comp);
    assert!(options::prereq_problem_in(&war_caster, &b, &cleric, Some(comp)).is_none());
}

#[test]
fn ritual_caster_2024() {
    let Some(comp) = comp() else { return };
    let ch = with_feat("fighter|xphb", 5, "ritual caster|xphb", comp);
    let b = build(&ch, comp);
    let mine: Vec<_> = b.spells.iter().filter(|s| s.origin == "Feat: Ritual Caster").collect();
    assert_eq!(mine.len(), 3, "proficiency bonus at level 5");
    for s in &mine {
        let sp = comp.spells.get(&s.key).unwrap();
        assert!(sp.ritual && sp.level == 1, "{}", sp.name);
        assert!(s.ability.is_some());
    }
}

#[test]
fn graviturgy_offers_only_dunamancy() {
    let Some(comp) = comp() else { return };
    let mut ch = hero("wizard|phb", "sage|xphb", "human|xphb");
    ch.set_starting_level(2);
    let b = build(&ch, comp);
    let sub = choice_with(&b, "/subclass").path.clone();
    let grav = comp.subclasses_of(&Key("wizard|phb".into())).find(|s| s.short_name == "Graviturgy").unwrap().key.0.clone();
    ch.set_choice(&sub, vec![grav]);
    let b = build(&ch, comp);
    let pick = b.choices.iter().find(|c| c.path.contains("graviturgy") && matches!(&c.options, Options::Spells(f) if !f.sources.is_empty()));
    if let Some(pick) = pick {
        for o in options(pick, &b, &ch, comp) {
            assert_eq!(comp.spells.get(&Key(o.value.clone())).unwrap().source, "EGW", "{}", o.label);
        }
    }
}

#[test]
fn guild_and_college_backgrounds_expand_class_lists() {
    let Some(comp) = comp() else { return };
    let mut failures = Vec::new();
    for bg in comp.backgrounds.iter().filter(|b| has_spells(&b.grants)) {
        let mut ch = hero("wizard|xphb", &bg.key.0, "human|xphb");
        ch.set_starting_level(9);
        let b = build(&ch, comp);
        if b.expanded.is_empty() {
            failures.push(format!("{}: no expanded spells", bg.key));
            continue;
        }
        // Some expanded spell that isn't normally a wizard spell must become selectable.
        let offered: Vec<String> = ["/cantrips", "/spellbook", "/prepared"]
            .iter()
            .flat_map(|p| option_labels(&ch, &b, choice_with(&b, p), comp))
            .collect();
        let extra = b.expanded.iter().filter_map(|(_, e)| match e {
            build::Expansion::Spell(k) => comp.spells.iter().find(|s| s.name.to_lowercase() == k.name_part() && !s.classes.contains(&"wizard".to_string())),
            _ => None,
        }).find(|s| s.level <= 5);
        if let Some(sp) = extra {
            if !offered.contains(&sp.name) {
                failures.push(format!("{}: {} not offered to a wizard", bg.key, sp.name));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn magic_initiate_twice_with_the_same_list_is_flagged() {
    let Some(comp) = comp() else { return };
    // Acolyte gives Magic Initiate (Cleric); take Magic Initiate again with the Cleric list.
    let mut ch = with_feat("wizard|xphb", 4, "magic initiate|xphb", comp);
    ch.set_background(Some(Key("acolyte|xphb".into())));
    let b = build(&ch, comp);
    let list = b.choices.iter().find(|c| c.path.contains("/magic initiate|xphb") && matches!(c.options, Options::Bundles(_))).unwrap().path.clone();
    ch.set_choice(&list, vec!["Cleric Spells".into()]);
    autofill(&mut ch, comp);
    let b = build(&ch, comp);
    assert!(issues(&ch, &b, comp).iter().any(|i| i.text.contains("Magic Initiate is taken twice")));
    ch.set_choice(&list, vec!["Druid Spells".into()]);
    autofill(&mut ch, comp);
    let b = build(&ch, comp);
    assert!(!issues(&ch, &b, comp).iter().any(|i| i.text.contains("taken twice")));
}

#[test]
fn rune_shaper_knows_half_proficiency_runes() {
    let Some(comp) = comp() else { return };
    let ch = with_feat("fighter|xphb", 4, "rune shaper|bgg", comp);
    let b = build(&ch, comp);
    let mine: Vec<_> = b.spells.iter().filter(|s| s.origin == "Feat: Rune Shaper").collect();
    assert_eq!(mine.len(), 2, "comprehend languages + 1 rune: {mine:?}");
    assert!(mine.iter().all(|s| s.ability.is_some()));
}

#[test]
fn per_record_audit_regressions() {
    let Some(comp) = comp() else { return };
    // Strike of the Giants: the base feat offers the giant type.
    let ch = with_feat("fighter|xphb", 4, "strike of the giants|bgg", comp);
    let b = build(&ch, comp);
    assert!(b.feats.iter().any(|(k, _)| k.0.starts_with("strike of the giants; ")), "{:?}", b.feats);
    // Genie Touched grants Magic Initiate (Wizard).
    let ch = hero("wizard|xphb", "genie touched|frhof", "human|xphb");
    assert!(build(&ch, comp).feats.iter().any(|(k, _)| k.0 == "magic initiate; wizard|xphb"));
    // Monk Diamond Soul: every save.
    let mut monk = hero("monk|phb", "acolyte|phb", "human|phb");
    monk.set_starting_level(14);
    let b = build(&monk, comp);
    assert!(Ability::ALL.iter().all(|a| b.has_prof(&Prof::Save(*a))));
    // Unarmored Defense from data: barbarian DEX + CON.
    let mut barb = hero("barbarian|xphb", "soldier|xphb", "human|xphb");
    barb.abilities = BaseAbilities { method: AbilityMethod::Manual, scores: [15, 14, 16, 8, 10, 8], rolls: vec![] };
    autofill(&mut barb, comp);
    barb.inventory.iter_mut().for_each(|i| i.equipped = false);
    let b = build(&barb, comp);
    let s = sheet(&barb, &b, comp);
    assert_eq!(s.armor_class, 10 + b.modifier(Ability::Dex) + b.modifier(Ability::Con), "{}", s.ac_note);
    // Tortle natural armor 17; Warforged +1.
    let tortle = hero("wizard|xphb", "sage|xphb", "tortle|mpmm");
    let b = build(&tortle, comp);
    assert_eq!(sheet(&tortle, &b, comp).armor_class, 17);
    // Draconic sorcerer: +1 HP per sorcerer level.
    let mut sorc = hero("sorcerer|xphb", "sage|xphb", "human|xphb");
    sorc.set_starting_level(3);
    let b = build(&sorc, comp);
    let sub = choice_with(&b, "/subclass").path.clone();
    let before = sheet(&sorc, &b, comp).hp_max;
    let draconic = comp.subclasses_of(&Key("sorcerer|xphb".into())).find(|s| s.short_name == "Draconic" && s.source == "XPHB").unwrap().key.0.clone();
    sorc.set_choice(&sub, vec![draconic]);
    let b = build(&sorc, comp);
    assert_eq!(sheet(&sorc, &b, comp).hp_max, before + 3);
    // 2014 Rogue Expertise can pick thieves' tools.
    let rogue = hero("rogue|phb", "criminal|phb", "human|phb");
    let b = build(&rogue, comp);
    let exp = b.choices.iter().find(|c| c.label == "Expertise").unwrap();
    assert!(options(exp, &b, &rogue, comp).iter().any(|o| o.value == "tool:thieves' tools"));
    // Speed feats.
    let ch = with_feat("fighter|xphb", 4, "speedy|xphb", comp);
    let b = build(&ch, comp);
    assert_eq!(sheet(&ch, &b, comp).speed.walk, 40);
    // Gloom Stalker 2024 Umbral Sight stacks on existing darkvision.
    let mut gs = hero("ranger|xphb", "sage|xphb", "dwarf|xphb");
    gs.set_starting_level(3);
    let b = build(&gs, comp);
    let sub = choice_with(&b, "/subclass").path.clone();
    let gloom = comp.subclasses_of(&Key("ranger|xphb".into())).find(|s| s.short_name == "Gloom Stalker" && s.source == "XPHB").unwrap().key.0.clone();
    gs.set_choice(&sub, vec![gloom]);
    let b = build(&gs, comp);
    assert_eq!(sheet(&gs, &b, comp).darkvision, 120 + 60);
    // 2024 Ranger Roving: +10 speed, climb and swim equal to speed.
    let mut r = hero("ranger|xphb", "sage|xphb", "human|xphb");
    r.set_starting_level(6);
    let b = build(&r, comp);
    let s = sheet(&r, &b, comp);
    assert_eq!((s.speed.walk, s.speed.climb, s.speed.swim), (40, 40, 40));
}

#[test]
fn ability_increases_respect_their_own_cap() {
    let Some(comp) = comp() else { return };
    let mut ch = hero("fighter|xphb", "soldier|xphb", "human|xphb");
    for _ in 1..20 {
        ch.level_up(Key("fighter|xphb".into()));
    }
    // Put every ability increase into Dexterity, including the Epic Boon's.
    for _ in 0..30 {
        let b = build(&ch, comp);
        let Some(c) = b.pending().next().cloned() else {
            break;
        };
        if let Options::Abilities { from, .. } = &c.options {
            if from.contains(&Ability::Dex) && c.count == 1 {
                ch.set_choice(&c.path, vec!["dex".into()]);
                continue;
            }
        }
        if c.path.ends_with("/p1/pick") || c.path.contains("epic") {
            let boon = options(&c, &b, &ch, comp)
                .into_iter()
                .find(|o| o.value == "boon of combat prowess|xphb");
            if let Some(o) = boon {
                ch.set_choice(&c.path, vec![o.value]);
                continue;
            }
        }
        let mut one = ch.clone();
        autofill(&mut one, comp);
        let v = one.choices.get(&c.path).cloned().unwrap_or_default();
        ch.set_choice(&c.path, v);
    }
    let b = build(&ch, comp);
    assert!(b.has_feat_named("Boon of Combat Prowess"), "{:?}", b.feats);
    // Ordinary increases stop at 20; only the boon's +1 may go beyond.
    assert!(b.score(Ability::Dex) <= 21, "dex {}", b.score(Ability::Dex));
    assert!(b.score(Ability::Dex) >= 20, "dex {}", b.score(Ability::Dex));
}

#[test]
fn class_features_name_the_level_they_were_gained() {
    let Some(comp) = comp() else { return };
    let mut ch = hero("fighter|xphb", "soldier|xphb", "human|xphb");
    for _ in 1..10 {
        ch.level_up(Key("fighter|xphb".into()));
    }
    autofill(&mut ch, comp);
    let b = build(&ch, comp);
    let origin = |name: &str| b.features.iter().find(|f| f.feature.name == name).map(|f| f.origin.clone());
    assert_eq!(origin("Second Wind").as_deref(), Some("Fighter 1"));
    assert_eq!(origin("Action Surge").as_deref(), Some("Fighter 2"));
    assert_eq!(origin("Extra Attack").as_deref(), Some("Fighter 5"));
    // Leveled picks name the level each became available (masteries: 3 at 1, then 4, 10).
    let masteries: Vec<String> = b.features.iter().filter(|f| f.feature.name.starts_with("Weapon Mastery: ")).map(|f| f.origin.clone()).collect();
    assert_eq!(masteries, ["Fighter 1", "Fighter 1", "Fighter 1", "Fighter 4", "Fighter 10"]);
    // Subclass features keep the subclass name.
    let sub = b.features.iter().find(|f| f.origin.contains('(')).expect("subclass feature");
    assert!(sub.origin.starts_with("Fighter 3 ("), "{}", sub.origin);
}

#[test]
fn older_subclasses_follow_2024_subclass_levels() {
    let Some(comp) = comp() else { return };
    let mut ch = hero("cleric|xphb", "acolyte|xphb", "human|xphb");
    for _ in 1..3 {
        ch.level_up(Key("cleric|xphb".into()));
    }
    autofill(&mut ch, comp);
    let path = build(&ch, comp).choices.iter().find(|c| matches!(c.options, Options::Subclass { .. })).expect("subclass at 3").path.clone();
    ch.set_choice(&path, vec!["life|phb|cleric|xphb".into()]);
    autofill(&mut ch, comp);
    let b = build(&ch, comp);
    // The 2014 Life Domain's level 1 features arrive with the subclass at level 3.
    let disciple = b.features.iter().find(|f| f.feature.name == "Disciple of Life").expect("Disciple of Life");
    assert_eq!(disciple.origin, "Cleric 3 (Life)");
    assert!(b.features.iter().all(|f| !f.origin.contains("Cleric 1 (")), "no subclass feature before level 3");
}

#[test]
fn older_feats_only_where_not_reprinted() {
    let Some(comp) = comp() else { return };
    let mut ch = hero("fighter|xphb", "soldier|xphb", "human|xphb");
    for _ in 1..4 {
        ch.level_up(Key("fighter|xphb".into()));
    }
    autofill(&mut ch, comp);
    let b = build(&ch, comp);
    let asi = b.choices.iter().find(|c| c.path.ends_with(".asi") && matches!(c.options, Options::Feats { .. })).expect("level 4 feat choice");
    let opts = options(asi, &b, &ch, comp);
    let current = |k: &str| opts.iter().any(|o| o.value == k && o.older.is_none());
    let folded = |k: &str| opts.iter().any(|o| o.value == k && o.older.is_some());
    assert!(current("dungeon delver|phb"), "2014 Dungeon Delver has no 2024 version");
    assert!(folded("mobile|phb"), "2014 Mobile was reprinted as Speedy");
    assert!(current("sharpshooter|xphb") && folded("sharpshooter|phb"), "2024 reprint replaces the 2014 feat; the old one is folded");
    assert!(folded("alert|phb"), "2014 Alert gives way to the 2024 Origin feat");
}

#[test]
fn sheet_shows_only_chosen_options() {
    let Some(comp) = comp() else { return };
    let mut ch = hero("fighter|xphb", "soldier|xphb", "dragonborn|phb");
    for _ in 1..3 {
        ch.level_up(Key("fighter|xphb".into()));
    }
    autofill(&mut ch, comp);
    let path = build(&ch, comp).choices.iter().find(|c| matches!(c.options, Options::Subclass { .. })).unwrap().path.clone();
    ch.set_choice(&path, vec!["battle master|xphb|fighter|xphb".into()]);
    autofill(&mut ch, comp);
    let b = build(&ch, comp);
    let s = sheet(&ch, &b, comp);
    let names: Vec<&str> = s.features.iter().map(|f| f.feature.name.as_str()).collect();
    // The catalog of every maneuver is gone; the three chosen maneuvers remain.
    assert!(!names.contains(&"Maneuver Options"), "{names:?}");
    let chosen = b.optional_features.len();
    assert_eq!(chosen, 3, "{:?}", b.optional_features);
    for (k, _) in &b.optional_features {
        let n = &comp.optional_features.get(k).unwrap().name;
        assert!(names.contains(&n.as_str()), "{n} missing from {names:?}");
    }
    // The 2014 dragonborn's ancestry table keeps only the chosen dragon.
    let anc = s.features.iter().find(|f| f.feature.name == "Draconic Ancestry").expect("ancestry");
    let rows: usize = anc.feature.body.iter().map(|e| match e {
        compendium::Entry::Table { rows, .. } => rows.len(),
        _ => 0,
    }).sum();
    assert_eq!(rows, 1, "{:?}", anc.feature.body);
}

#[test]
fn every_feat_with_an_increase_says_so() {
    let Some(comp) = comp() else { return };
    fn raises(grants: &[compendium::Grant]) -> bool {
        grants.iter().any(|g| match g {
            compendium::Grant::Ability { amount, .. } => *amount > 0,
            compendium::Grant::Choose(c) => match &c.options {
                Options::Abilities { amounts, .. } => amounts.iter().any(|a| *a > 0),
                Options::Bundles(bs) => bs.iter().any(|b| raises(&b.grants)),
                _ => false,
            },
            compendium::Grant::AtLevel { grants, .. } => raises(grants),
            _ => false,
        })
    }
    let silent: Vec<&str> = comp
        .feats
        .iter()
        .filter(|f| f.name != "Ability Score Improvement" && raises(&f.grants))
        .filter(|f| {
            let t = compendium::Entry::plain(&f.body).to_lowercase();
            !(t.contains("ability score increase") || t.contains("score by 1") || t.contains("score by 2"))
        })
        .map(|f| f.key.0.as_str())
        .collect();
    assert!(silent.is_empty(), "feats whose text omits their ability increase: {silent:?}");
}

#[test]
fn feat_filter_tags() {
    let Some(comp) = comp() else { return };
    let f = |k: &str| crate::options::feat_facets(comp.feats.get(&Key(k.into())).unwrap(), comp);
    let sentinel = f("sentinel|xphb");
    assert!(sentinel.contains(&("Ability increase".into(), "Strength".into())));
    assert!(sentinel.contains(&("Ability increase".into(), "Dexterity".into())));
    assert!(sentinel.contains(&("Category".into(), "General".into())));
    let mi = f("magic initiate|xphb");
    assert!(mi.contains(&("Grants".into(), "Spells".into())), "{mi:?}");
    assert!(mi.contains(&("Prerequisite".into(), "None".into())), "{mi:?}");
    assert!(f("mobile|phb").contains(&("Edition".into(), "Older (2014)".into())));
}

#[test]
fn every_option_can_be_expanded() {
    let Some(comp) = comp() else { return };
    let ch0 = Character::new("x");
    for list in [crate::browse::classes(&ch0, comp), crate::browse::species(&ch0, comp), crate::browse::backgrounds(&ch0, comp)] {
        for o in &list {
            assert!(!o.body.is_empty() && !o.facets.is_empty(), "{} has no description or filters", o.label);
        }
    }
    let mut empty = Vec::new();
    for class in comp.classes.iter().filter(|c| !c.legacy && c.source == "XPHB") {
        let mut ch = hero(&class.key.0, "sage|xphb", "human|xphb");
        for _ in 1..5 {
            ch.level_up(class.key.clone());
        }
        autofill(&mut ch, comp);
        let b = build(&ch, comp);
        for c in &b.choices {
            for o in options(c, &b, &ch, comp) {
                if o.body.is_empty() {
                    empty.push(format!("{} / {}: {}", class.name, c.label, o.label));
                }
            }
        }
    }
    assert!(empty.is_empty(), "options without a description: {:?}", &empty[..empty.len().min(20)]);
}

#[test]
fn reprinted_subclasses_and_options_are_folded() {
    let Some(comp) = comp() else { return };
    let mut ch = hero("bard|xphb", "sage|xphb", "human|xphb");
    for _ in 1..3 {
        ch.level_up(Key("bard|xphb".into()));
    }
    autofill(&mut ch, comp);
    let b = build(&ch, comp);
    let c = b.choices.iter().find(|c| matches!(c.options, Options::Subclass { .. })).unwrap();
    let opts = options(c, &b, &ch, comp);
    let get = |k: &str| opts.iter().find(|o| o.value == k).unwrap_or_else(|| panic!("{k} missing"));
    assert!(get("lore|xphb|bard|xphb").older.is_none());
    assert!(get("lore|phb|bard|xphb").older.is_some(), "2014 Lore is replaced by the 2024 Lore");
    assert!(get("swords|xge|bard|xphb").older.is_none(), "Swords has no reprint");
    // Older classes are folded, not hidden.
    let classes = crate::browse::classes(&ch, comp);
    assert!(classes.iter().any(|o| o.value == "bard|phb" && o.older.is_some()));
}

#[test]
fn other_language_means_the_full_list() {
    let Some(comp) = comp() else { return };
    let ch = hero("wizard|xphb", "izzet engineer|ggr", "human|xphb");
    let b = build(&ch, comp);
    let c = b
        .choices
        .iter()
        .find(|c| c.path.starts_with("background") && matches!(&c.options, Options::Mixed(p) if p.iter().any(|x| matches!(x, Options::Languages(_)))))
        .expect("Izzet Engineer language choice");
    let opts = options(c, &b, &ch, comp);
    let labels: Vec<&str> = opts.iter().map(|o| o.label.as_str()).collect();
    assert!(!labels.contains(&"Other"), "'other' must not be a language: {labels:?}");
    assert_eq!(opts[0].group.as_deref(), Some("Listed options"));
    assert!(opts.iter().any(|o| o.label == "Elvish" && o.group.as_deref() == Some("Standard languages (common)")));
    assert!(opts.iter().any(|o| o.label == "Celestial" && o.group.as_deref() == Some("Uncommon languages")));
    // Setting languages (not Forgotten Realms) are grouped by setting, after the rest.
    assert!(opts.iter().any(|o| o.label == "Quori" && o.group.as_deref() == Some("Eberron languages")), "{:?}", opts.iter().map(|o| (&o.label, &o.group)).collect::<Vec<_>>());
    let first_setting = opts.iter().position(|o| o.group.as_deref().is_some_and(|g| g.ends_with(" languages") && !g.starts_with("Standard") && !g.starts_with("Uncommon"))).unwrap();
    assert!(opts[first_setting..].iter().all(|o| !matches!(o.group.as_deref(), Some("Standard languages (common)") | Some("Uncommon languages"))));
    let mut values: Vec<&str> = opts.iter().map(|o| o.value.as_str()).collect();
    values.sort();
    let n = values.len();
    values.dedup();
    assert_eq!(n, values.len(), "each language once");
}

#[test]
fn any_alternative_prerequisite_qualifies() {
    let Some(comp) = comp() else { return };
    let check = |scores: [u8; 6], feat: &str| {
        let mut ch = hero("fighter|xphb", "soldier|xphb", "human|xphb");
        ch.abilities = BaseAbilities { method: AbilityMethod::Manual, scores, rolls: vec![] };
        for _ in 1..4 {
            ch.level_up(Key("fighter|xphb".into()));
        }
        let b = build(&ch, comp);
        let p = comp.feats.get(&Key(feat.into())).unwrap().prereq.clone().unwrap();
        crate::options::prereq_problem_in(&p, &b, &ch, Some(comp))
    };
    // 2024 Ritual Caster: Intelligence, Wisdom or Charisma 13 (and level 4).
    assert_eq!(check([10, 10, 10, 8, 8, 14], "ritual caster|xphb"), None, "Charisma 13 qualifies");
    assert_eq!(check([10, 10, 10, 8, 14, 8], "ritual caster|xphb"), None, "Wisdom 13 qualifies");
    let none = check([14, 10, 10, 8, 8, 8], "ritual caster|xphb").expect("no qualifying score");
    assert!(none.contains("Charisma") && none.contains("Intelligence"), "names every way: {none}");
    // Athlete: Strength or Dexterity 13.
    assert_eq!(check([8, 14, 10, 8, 8, 8], "athlete|xphb"), None, "Dexterity 13 qualifies");
}
