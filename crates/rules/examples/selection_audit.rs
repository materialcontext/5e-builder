//! Builds every class × subclass (and every species) and reports how feature
//! text was cut down to the player's selections, plus any remaining
//! menu-like blocks that no choice covers.
//!
//! cargo run -p rules --example selection_audit -- crates/web/public/compendium.json

use compendium::{Compendium, Entry, Key, Options};
use rules::selections::{self, norm};
use rules::{AbilityMethod, BaseAbilities, Character};
use std::collections::BTreeSet;

fn filled(mut ch: Character, comp: &Compendium, subclass: Option<&Key>) -> Character {
    for _ in 0..6 {
        if let Some(sub) = subclass {
            let b = rules::build(&ch, comp);
            if let Some(c) = b.choices.iter().find(|c| matches!(c.options, Options::Subclass { .. }) && c.selected.is_empty()) {
                let p = c.path.clone();
                ch.set_choice(&p, vec![sub.0.clone()]);
            }
        }
        rules::suggest::fill(&mut ch, comp, None);
    }
    ch
}

/// Blocks with ≥2 entries naming something from the whole compendium.
fn menus(entries: &[Entry], universe: &BTreeSet<String>, out: &mut Vec<String>) {
    let name = |e: &Entry| match e {
        Entry::Section { name: Some(n), .. } => Some(norm(n)),
        Entry::P(t) => {
            let h = t.split_once(". ").map(|(h, _)| h).unwrap_or(t);
            (h.chars().count() <= 60).then(|| norm(h))
        }
        _ => None,
    };
    let hits: Vec<String> = entries.iter().filter_map(name).filter(|n| universe.contains(n)).collect();
    if hits.len() >= 2 {
        out.push(hits.join(", "));
    }
    for e in entries {
        match e {
            Entry::Section { body, .. } => menus(body, universe, out),
            Entry::List(items) => menus(items, universe, out),
            Entry::Table { rows, .. } => {
                let hits: Vec<String> = rows.iter().filter_map(|r| r.first()).map(|c| norm(c)).filter(|n| universe.contains(n)).collect();
                if hits.len() >= 2 {
                    out.push(format!("table: {}", hits.join(", ")));
                }
            }
            Entry::P(_) => {}
        }
    }
}

fn main() {
    let path = std::env::args().nth(1).expect("path to compendium.json");
    let comp = Compendium::from_json(&std::fs::read_to_string(path).expect("read")).expect("parse");
    let mut universe: BTreeSet<String> = BTreeSet::new();
    universe.extend(comp.optional_features.iter().map(|o| norm(&o.name)));
    universe.extend(comp.subclasses.iter().flat_map(|s| [norm(&s.name), norm(&s.short_name)]));
    for sp in comp.species.iter() {
        universe.extend(sp.lineages.iter().map(|l| norm(&l.name)));
    }

    let base = |id: &str| {
        let mut ch = Character::new(id);
        ch.abilities = BaseAbilities { method: AbilityMethod::StandardArray, scores: [15, 14, 13, 12, 10, 8], rolls: vec![] };
        ch.set_species(Some(Key("human|xphb".into())));
        ch.set_background(Some(Key("sage|xphb".into())));
        ch
    };
    let mut builds: Vec<(String, Character)> = Vec::new();
    for class in comp.classes.iter() {
        for sub in comp.subclasses_of(&class.key) {
            let mut ch = base("a");
            ch.set_starting_class(class.key.clone());
            for _ in 1..20 {
                ch.level_up(class.key.clone());
            }
            builds.push((format!("{} / {}", class.key.0, sub.key.0), filled(ch, &comp, Some(&sub.key))));
        }
    }
    for sp in comp.species.iter() {
        let mut ch = base("s");
        ch.set_species(Some(sp.key.clone()));
        ch.set_starting_class(Key("fighter|xphb".into()));
        builds.push((format!("species {}", sp.key.0), filled(ch, &comp, None)));
    }

    let (mut pruned, mut dropped, mut flagged) = (0, 0, 0);
    for (label, ch) in &builds {
        let b = rules::build(ch, &comp);
        let names = selections::names(&b, &comp);
        let after = selections::prune(b.features.clone(), &names);
        let mut lines = Vec::new();
        for f in &b.features {
            match after.iter().find(|x| x.feature.name == f.feature.name && x.origin == f.origin) {
                None => {
                    dropped += 1;
                    lines.push(format!("  dropped  {} ({})", f.feature.name, f.origin));
                }
                Some(x) if x.feature != f.feature => {
                    pruned += 1;
                    let chosen = Entry::plain(&x.feature.body).lines().filter(|l| l.starts_with("Chosen:")).collect::<Vec<_>>().join(" ");
                    lines.push(format!("  trimmed  {} ({}) {}", f.feature.name, f.origin, chosen));
                }
                _ => {}
            }
        }
        for x in &after {
            let mut m = Vec::new();
            menus(&x.feature.body, &universe, &mut m);
            for hit in m {
                flagged += 1;
                lines.push(format!("  MENU?    {}: {}", x.feature.name, hit));
            }
        }
        if !lines.is_empty() {
            println!("{label}\n{}", lines.join("\n"));
        }
    }
    eprintln!("builds {} · trimmed {pruned} · dropped {dropped} · remaining menus {flagged}", builds.len());
}
