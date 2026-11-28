//! Converts characters (the app's storage format: one object or an array)
//! to Foundry VTT dnd5e actors, one `<name>.foundry.json` per character.
//!
//! cargo run -p rules --example foundry_export -- crates/web/public/compendium.json chars.json out/

use compendium::Compendium;
use rules::Character;

fn main() {
    let mut args = std::env::args().skip(1);
    let comp_path = args.next().expect("path to compendium.json");
    let chars_path = args.next().expect("path to characters JSON");
    let out = std::path::PathBuf::from(args.next().unwrap_or_else(|| ".".into()));
    let comp = Compendium::from_json(&std::fs::read_to_string(comp_path).expect("read")).expect("parse");
    let text = std::fs::read_to_string(chars_path).expect("read characters");
    let chars: Vec<Character> = serde_json::from_str(&text)
        .or_else(|_| serde_json::from_str::<Character>(&text).map(|c| vec![c]))
        .expect("characters JSON");
    std::fs::create_dir_all(&out).expect("output dir");
    for ch in &chars {
        let path = out.join(rules::foundry::file_name(ch));
        std::fs::write(&path, rules::foundry::actor_json(ch, &comp)).expect("write");
        eprintln!("{}", path.display());
    }
}
