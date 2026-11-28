//! Writes a few fully built characters as JSON (the app's storage format),
//! for checking the printed sheet with long feature, spell and story text.
//!
//! cargo run -p rules --example sample_characters -- crates/web/public/compendium.json > chars.json

use compendium::{Compendium, Key};
use rules::{AbilityMethod, BaseAbilities, Character};

fn make(
    comp: &Compendium,
    id: &str,
    name: &str,
    species: &str,
    background: &str,
    classes: &[(&str, u8)],
) -> Character {
    let mut ch = Character::new(id);
    ch.name = name.into();
    ch.abilities = BaseAbilities {
        method: AbilityMethod::StandardArray,
        scores: [8, 14, 13, 15, 12, 10],
        rolls: vec![],
    };
    ch.set_species(Some(Key(species.into())));
    ch.set_background(Some(Key(background.into())));
    let (first, n) = classes[0];
    ch.set_starting_class(Key(first.into()));
    for _ in 1..n {
        ch.level_up(Key(first.into()));
    }
    for (class, n) in &classes[1..] {
        for _ in 0..*n {
            ch.level_up(Key((*class).into()));
        }
    }
    for _ in 0..4 {
        rules::suggest::fill(&mut ch, comp, None);
    }
    rules::apply_starting_equipment(&mut ch, comp);
    // Wear the first armor and shield, wield the weapons.
    let mut armor = false;
    let mut shield = false;
    for it in ch.inventory.iter_mut() {
        let Some(rec) = it.item.as_ref().and_then(|k| comp.items.get(k)) else { continue };
        match rec.armor.as_ref().map(|a| a.kind) {
            Some(compendium::ArmorKind::Shield) if !shield => (it.equipped, shield) = (true, true),
            Some(k) if k != compendium::ArmorKind::Shield && !armor => (it.equipped, armor) = (true, true),
            _ if rec.weapon.is_some() => it.equipped = true,
            _ => {}
        }
    }
    let d = &mut ch.details;
    d.alignment = "Neutral Good".into();
    d.age = "142".into();
    d.height = "5'8\"".into();
    d.eyes = "Silver".into();
    d.hair = "Black, braided".into();
    d.appearance = "A tall, lean figure in travel-stained robes embroidered with tiny constellations. Ink stains on both hands.".into();
    d.personality = "Speaks in careful, measured sentences and takes notes on everything, including other people's conversations.".into();
    d.ideals = "Knowledge. The path to power and self-improvement is through knowledge.".into();
    d.bonds = "I have an ancient text that holds terrible secrets that must not fall into the wrong hands.".into();
    d.flaws = "I overlook obvious solutions in favor of complicated ones.".into();
    d.backstory = "Raised in the archive-city of Candlekeep, she left after a forbidden tome she was cataloguing vanished overnight. She has followed its trail across three kingdoms, and suspects the thief is someone she trusted.\nShe now travels with a band of mercenaries who think she is merely a scholar.".into();
    d.xp = 85000;
    ch.coins.gp = 1250;
    ch.coins.sp = 32;
    ch.coins.cp = 7;
    ch
}

fn main() {
    let path = std::env::args().nth(1).expect("path to compendium.json");
    let comp = Compendium::from_json(&std::fs::read_to_string(path).expect("read")).expect("parse");
    let chars = vec![
        make(
            &comp,
            "s1",
            "Ysolde Vantreth",
            "elf|xphb",
            "sage|xphb",
            &[("cleric|xphb", 5), ("wizard|xphb", 6)],
        ),
        make(
            &comp,
            "s2",
            "Brannoc Ironhide",
            "dwarf|xphb",
            "soldier|xphb",
            &[("fighter|xphb", 20)],
        ),
        make(
            &comp,
            "s3",
            "Mirel Ashgrove",
            "human|xphb",
            "acolyte|xphb",
            &[("wizard|xphb", 20)],
        ),
        make(
            &comp,
            "s4",
            "Vex Thornfield",
            "tiefling|xphb",
            "criminal|xphb",
            &[("barbarian|xphb", 3), ("warlock|xphb", 4)],
        ),
        make(
            &comp,
            "s5",
            "Pip Underbough",
            "halfling|xphb",
            "hermit|xphb",
            &[("monk|xphb", 8)],
        ),
        make(
            &comp,
            "s6",
            "Grask",
            "half-orc|phb",
            "outlander|phb",
            &[("ranger|phb", 6)],
        ),
    ];
    println!("{}", serde_json::to_string(&chars).expect("json"));
}
