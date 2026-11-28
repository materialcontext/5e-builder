//! Lists, for every class × subclass at a given level, the limited-use
//! features and the action / bonus action / reaction lists the printed sheet
//! will show, for reviewing what the rules-text reading picks up.
//!
//! cargo run -p rules --example play_audit -- crates/web/public/compendium.json [level] [source]

use compendium::{Compendium, Key, Options};
use rules::{AbilityMethod, BaseAbilities, Character};

fn filled(mut ch: Character, comp: &Compendium, subclass: &Key) -> Character {
    for _ in 0..6 {
        let b = rules::build(&ch, comp);
        if let Some(c) = b.choices.iter().find(|c| matches!(c.options, Options::Subclass { .. }) && c.selected.is_empty()) {
            let p = c.path.clone();
            ch.set_choice(&p, vec![subclass.0.clone()]);
        }
        rules::suggest::fill(&mut ch, comp, None);
    }
    ch
}

fn main() {
    let mut args = std::env::args().skip(1);
    let comp = Compendium::from_json(&std::fs::read_to_string(args.next().expect("compendium.json")).expect("read")).expect("parse");
    let level: u8 = args.next().and_then(|l| l.parse().ok()).unwrap_or(20);
    let source = args.next().unwrap_or_else(|| "xphb".into()).to_lowercase();
    for class in comp.classes.iter().filter(|c| c.key.source_part() == source) {
        for sub in comp.subclasses.iter().filter(|s| s.class == class.key && s.reprinted.is_none()) {
            let mut ch = Character::new("audit");
            ch.abilities = BaseAbilities { method: AbilityMethod::StandardArray, scores: [15, 14, 13, 12, 10, 8], rolls: vec![] };
            ch.set_species(Some(Key("human|xphb".into())));
            ch.set_background(Some(Key("sage|xphb".into())));
            ch.set_starting_class(class.key.clone());
            for _ in 1..level {
                ch.level_up(class.key.clone());
            }
            let ch = filled(ch, &comp, &sub.key);
            let b = rules::build(&ch, &comp);
            let s = rules::sheet(&ch, &b, &comp);
            println!("== {} {level} ({})", class.name, sub.short_name);
            for u in &s.limited {
                let n = if u.unit.is_empty() { u.uses.to_string() } else { format!("{} {}", u.uses, u.unit) };
                println!("   use  {:34} {:>8}  {:12} {}", u.name, n, u.recharge, u.note);
            }
            println!("   A: {}", s.actions.action.join(", "));
            println!("   B: {}", s.actions.bonus.join(", "));
            println!("   R: {}", s.actions.reaction.join(", "));
        }
    }
}
