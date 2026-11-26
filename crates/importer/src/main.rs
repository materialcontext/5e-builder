//! `importer <5etools data dir> <out compendium.json>`
//!
//! Reads the raw 5etools data folder and writes the normalized compendium
//! consumed by the rules engine and the web app.

mod grants;
mod inherit;
mod records;
mod text;

use compendium::{Compendium, Record, Table};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::{env, fs, process};

fn load(dir: &Path, rel: &str) -> Value {
    let p = dir.join(rel);
    match fs::read_to_string(&p) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_else(|e| {
            eprintln!("error: {}: {e}", p.display());
            process::exit(1)
        }),
        Err(e) => {
            eprintln!("error: cannot read {}: {e}", p.display());
            process::exit(1)
        }
    }
}

/// Files listed in an `index.json` (`{ "SRC": "file.json" }`).
fn indexed(dir: &Path, sub: &str) -> Vec<Value> {
    let idx = load(dir, &format!("{sub}/index.json"));
    let files: BTreeSet<String> =
        idx.as_object().into_iter().flat_map(|m| m.values()).filter_map(Value::as_str).map(String::from).collect();
    files.iter().map(|f| load(dir, &format!("{sub}/{f}"))).collect()
}

fn data_version(dir: &Path) -> String {
    let changelog = load(dir, "changelog.json");
    changelog
        .as_array()
        .and_then(|a| a.last())
        .and_then(|e| e.get("ver"))
        .and_then(Value::as_str)
        .map(String::from)
        .unwrap_or_else(|| "unknown".into())
}

fn build(dir: &Path) -> Compendium {
    let mut class_data = records::ClassData::default();
    for f in indexed(dir, "class") {
        for (k, target) in [
            ("class", &mut class_data.class),
            ("subclass", &mut class_data.subclass),
            ("classFeature", &mut class_data.class_feature),
            ("subclassFeature", &mut class_data.subclass_feature),
        ] {
            if let Some(a) = f.get(k).and_then(Value::as_array) {
                target.extend(a.iter().cloned());
            }
        }
    }
    let (classes, subclasses) = records::classes(&class_data);
    let spell_files = indexed(dir, "spells");
    let lookup = load(dir, "generated/gendata-spell-source-lookup.json");

    let mut c = Compendium {
        data_version: data_version(dir),
        sources: records::sources(&load(dir, "books.json"), &load(dir, "adventures.json")),
        species: Table::new(records::species(&load(dir, "races.json"))),
        backgrounds: Table::new(records::backgrounds(&load(dir, "backgrounds.json"))),
        feats: Table::new(records::feats(&load(dir, "feats.json"))),
        classes: Table::new(classes),
        subclasses: Table::new(subclasses),
        optional_features: Table::new(records::optional_features(&load(dir, "optionalfeatures.json"))),
        spells: Table::new(records::spells(&spell_files, &lookup)),
        items: Table::new(records::items(&load(dir, "items-base.json"), &load(dir, "items.json"))),
        languages: Table::new(records::languages(&load(dir, "languages.json"))),
    };

    // Keep only sources that content actually uses, and add unknown ones.
    let mut used = BTreeSet::new();
    fn collect<T: Record>(t: &Table<T>, used: &mut BTreeSet<String>) {
        used.extend(t.iter().map(|r| r.source().to_string()));
    }
    collect(&c.species, &mut used);
    collect(&c.backgrounds, &mut used);
    collect(&c.feats, &mut used);
    collect(&c.classes, &mut used);
    collect(&c.subclasses, &mut used);
    collect(&c.optional_features, &mut used);
    collect(&c.spells, &mut used);
    collect(&c.items, &mut used);
    used.remove("");
    c.sources.retain(|s| used.contains(&s.id));
    let known: BTreeSet<String> = c.sources.iter().map(|s| s.id.clone()).collect();
    for id in used.difference(&known) {
        c.sources.push(records::extra_source(id));
    }
    c.sources.sort_by(|a, b| (a.group, &a.published, &a.name).cmp(&(b.group, &b.published, &b.name)));
    c
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: importer <5etools data dir> <out.json>");
        process::exit(2);
    }
    let dir = PathBuf::from(&args[1]);
    let out = PathBuf::from(&args[2]);
    let c = build(&dir);
    let json = serde_json::to_string(&c).expect("serialize");
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent).ok();
    }
    fs::write(&out, &json).unwrap_or_else(|e| {
        eprintln!("error: cannot write {}: {e}", out.display());
        process::exit(1)
    });
    println!(
        "compendium {} → {} ({:.1} MB): {} species, {} backgrounds, {} feats, {} classes, {} subclasses, {} optional features, {} spells, {} items, {} languages, {} sources",
        c.data_version,
        out.display(),
        json.len() as f64 / 1e6,
        c.species.len(),
        c.backgrounds.len(),
        c.feats.len(),
        c.classes.len(),
        c.subclasses.len(),
        c.optional_features.len(),
        c.spells.len(),
        c.items.len(),
        c.languages.len(),
        c.sources.len()
    );
}
