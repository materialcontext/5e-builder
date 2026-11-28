//! Classifies every class, subclass and optional feature, feat and species
//! trait by the action it takes (see `rules::play::activation`), and prints
//! the ones whose text names more than one kind of action with the context
//! of each phrase, for review.
//!
//! cargo run -p rules --example action_audit -- crates/web/public/compendium.json [all]

use compendium::{Compendium, Entry};
use rules::play::{activation, ActionKind};

fn main() {
    let mut args = std::env::args().skip(1);
    let comp = Compendium::from_json(&std::fs::read_to_string(args.next().expect("compendium.json")).expect("read")).expect("parse");
    let all = args.next().as_deref() == Some("all");
    let mut items: Vec<(String, String, Vec<Entry>)> = Vec::new();
    for c in comp.classes.iter() {
        for f in &c.features {
            items.push((format!("{} {}", c.name, c.source), f.name.clone(), f.body.clone()));
        }
    }
    for s in comp.subclasses.iter() {
        for f in &s.features {
            items.push((format!("{} {}", s.short_name, s.source), f.name.clone(), f.body.clone()));
        }
    }
    for o in comp.optional_features.iter() {
        items.push((format!("option {}", o.source), o.name.clone(), o.body.clone()));
    }
    for f in comp.feats.iter() {
        items.push((format!("feat {}", f.source), f.name.clone(), f.body.clone()));
    }
    for sp in comp.species.iter() {
        for f in &sp.features {
            items.push((format!("{} {}", sp.name, sp.source), f.name.clone(), f.body.clone()));
        }
    }
    let label = |k: Option<ActionKind>| match k {
        Some(ActionKind::Action) => "ACTION",
        Some(ActionKind::Bonus) => "BONUS",
        Some(ActionKind::Reaction) => "REACTION",
        None => "-",
    };
    let (mut counted, mut mixed) = (0, 0);
    for (from, name, body) in &items {
        let text = Entry::plain(body);
        let lower = text.to_lowercase();
        let a = activation(&text);
        if a.primary.is_some() {
            counted += 1;
        }
        let kinds: std::collections::BTreeSet<_> = a.found.iter().map(|f| format!("{:?}", f.1)).collect();
        if kinds.len() < 2 && !all {
            continue;
        }
        mixed += 1;
        let also: Vec<&str> = a.also.iter().map(|k| label(Some(*k))).collect();
        println!(
            "== {name} [{from}] -> {}{}{}",
            label(a.primary),
            if also.is_empty() { String::new() } else { format!(" + also {}", also.join(", ")) },
            a.command.map(|c| format!(" + command {}", label(Some(c)))).unwrap_or_default()
        );
        for (i, kind, p) in &a.found {
            let s = lower[..*i].char_indices().rev().nth(70).map(|(j, _)| j).unwrap_or(0);
            let e = (i + p.len() + 50).min(lower.len());
            let e = (e..=lower.len()).find(|&e| lower.is_char_boundary(e)).unwrap_or(lower.len());
            println!("     {:8} …{}…", label(Some(*kind)), lower[s..e].replace('\n', " "));
        }
    }
    eprintln!("{} entries, {counted} with an action type, {mixed} naming more than one kind", items.len());
}
