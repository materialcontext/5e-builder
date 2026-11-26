//! Content audit: every reference resolves and every choice can be satisfied.
//! Run with `cargo test -p rules audit -- --nocapture` after generating the compendium.

use crate::build::Step;
use crate::options::options;
use crate::{build, Build, Character, ChoiceState};
use compendium::*;
use std::collections::BTreeSet;

#[derive(Default)]
pub struct Report {
    pub problems: BTreeSet<String>,
}

fn walk(grants: &[Grant], owner: &str, comp: &Compendium, r: &mut Report) {
    for g in grants {
        match g {
            Grant::Feat(k) if comp.feats.get(k).is_none() => {
                r.problems.insert(format!("{owner}: unknown feat {k}"));
            }
            Grant::Item { item: ItemRef::Key(k), .. } if comp.items.get(k).is_none() => {
                r.problems.insert(format!("{owner}: unknown item {k}"));
            }
            Grant::Spells(sg) => {
                for s in &sg.spells {
                    if let SpellPick::Fixed(k) = &s.pick {
                        if comp.spells.get(k).is_none() {
                            r.problems.insert(format!("{owner}: unknown spell {k}"));
                        }
                    }
                }
            }
            Grant::Choose(c) => {
                if let Options::Bundles(b) = &c.options {
                    for x in b {
                        walk(&x.grants, owner, comp, r);
                    }
                }
                check_choice(&c.options, c.count.at(20).max(1), owner, comp, r);
            }
            Grant::AtLevel { grants, .. } => walk(grants, owner, comp, r),
            _ => {}
        }
    }
}

/// Checks that an option set offers at least `need` options with every source enabled.
fn check_choice(o: &Options, need: u8, owner: &str, comp: &Compendium, r: &mut Report) {
    let ch = Character::default();
    let b = Build { scores: [20; 6], ..Default::default() };
    let state = ChoiceState {
        path: "audit".into(),
        label: String::new(),
        origin: String::new(),
        step: Step::Class,
        count: need,
        options: o.clone(),
        selected: vec![],
        level: 20,
        class: None,
    };
    if matches!(o, Options::SkillExpertise | Options::Lineage { .. } | Options::Subclass { .. }) {
        return;
    }
    let n = options(&state, &b, &ch, comp).len();
    let need = match o {
        Options::Abilities { amounts, .. } => amounts.len(),
        _ => need as usize,
    };
    if n < need {
        r.problems.insert(format!("{owner}: choice offers {n} options, needs {need}: {o:?}").chars().take(300).collect());
    }
}

pub fn audit(comp: &Compendium) -> Report {
    let mut r = Report::default();
    for s in comp.species.iter() {
        walk(&s.grants, &format!("species {}", s.key), comp, &mut r);
        for l in &s.lineages {
            walk(&l.grants, &format!("lineage {}", l.key), comp, &mut r);
        }
    }
    for b in comp.backgrounds.iter() {
        walk(&b.grants, &format!("background {}", b.key), comp, &mut r);
    }
    for f in comp.feats.iter() {
        walk(&f.grants, &format!("feat {}", f.key), comp, &mut r);
    }
    for o in comp.optional_features.iter() {
        walk(&o.grants, &format!("optfeature {}", o.key), comp, &mut r);
    }
    for c in comp.classes.iter() {
        let owner = format!("class {}", c.key);
        walk(&c.starting, &owner, comp, &mut r);
        walk(&c.multiclass, &owner, comp, &mut r);
        walk(&c.equipment, &owner, comp, &mut r);
        walk(&c.grants, &owner, comp, &mut r);
        for f in &c.features {
            walk(&f.grants, &format!("{owner} / {}", f.name), comp, &mut r);
        }
        for p in &c.progressions {
            let o = match &p.kind {
                ProgressionKind::Feats(c) => Options::Feats { categories: c.clone() },
                ProgressionKind::OptionalFeatures(t) => Options::OptionalFeatures { types: t.clone() },
            };
            check_choice(&o, p.totals.iter().copied().max().unwrap_or(0), &format!("{owner} / {}", p.name), comp, &mut r);
        }
        if c.subclass_level().is_some() && comp.subclasses_of(&c.key).next().is_none() {
            r.problems.insert(format!("{owner}: no subclasses"));
        }
        if c.spellcasting.is_caster() && !comp.spells.iter().any(|s| s.classes.contains(&c.spellcasting.list)) {
            r.problems.insert(format!("{owner}: empty spell list {}", c.spellcasting.list));
        }
    }
    for s in comp.subclasses.iter() {
        let owner = format!("subclass {}", s.key);
        if comp.classes.get(&s.class).is_none() {
            r.problems.insert(format!("{owner}: unknown class"));
        }
        walk(&s.grants, &owner, comp, &mut r);
        for f in &s.features {
            walk(&f.grants, &format!("{owner} / {}", f.name), comp, &mut r);
        }
        for p in &s.progressions {
            let o = match &p.kind {
                ProgressionKind::Feats(c) => Options::Feats { categories: c.clone() },
                ProgressionKind::OptionalFeatures(t) => Options::OptionalFeatures { types: t.clone() },
            };
            check_choice(&o, p.totals.iter().copied().max().unwrap_or(0), &format!("{owner} / {}", p.name), comp, &mut r);
        }
    }
    for i in comp.items.iter() {
        for (k, _) in &i.contents {
            if comp.items.get(k).is_none() {
                r.problems.insert(format!("item {}: unknown pack content {k}", i.key));
            }
        }
    }
    let _ = build;
    r
}
