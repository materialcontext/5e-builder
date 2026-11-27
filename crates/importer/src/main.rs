//! `importer <5etools data dir> <out compendium.json>`
//!
//! Reads the raw 5etools data folder and writes the normalized compendium
//! consumed by the rules engine and the web app.

mod grants;
mod inherit;
mod patches;
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

/// Loads `rel` and patches the arrays named `kinds` inside it.
fn load_patched(dir: &Path, rel: &str, kinds: &[&str], p: &mut patches::Patches) -> Value {
    let mut v = load(dir, rel);
    for k in kinds {
        if let Some(Value::Array(a)) = v.get_mut(*k) {
            p.apply(k, a);
        }
    }
    v
}

fn build(dir: &Path) -> Compendium {
    let mut patches = patches::Patches::load();
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
    patches.apply("class", &mut class_data.class);
    patches.apply("subclass", &mut class_data.subclass);
    patches.apply("classFeature", &mut class_data.class_feature);
    patches.apply("subclassFeature", &mut class_data.subclass_feature);
    let (classes, subclasses) = records::classes(&class_data);
    let spell_files = indexed(dir, "spells");
    let lookup = load(dir, "generated/gendata-spell-source-lookup.json");

    let mut species = records::species(&load_patched(dir, "races.json", &["race", "subrace"], &mut patches));
    let mut backgrounds = records::backgrounds(&load_patched(dir, "backgrounds.json", &["background"], &mut patches));
    let feats = records::feats(&load_patched(dir, "feats.json", &["feat"], &mut patches));
    let optional_features = records::optional_features(&load_patched(dir, "optionalfeatures.json", &["optionalfeature"], &mut patches));
    for u in patches.unmatched() {
        eprintln!("warn: patch matched nothing: {u}");
    }
    let mut classes = classes;
    let mut items = records::items(&load(dir, "items-base.json"), &load(dir, "items.json"));
    fix_item_refs(&mut species, &mut backgrounds, &mut classes, &mut items);

    let mut c = Compendium {
        data_version: data_version(dir),
        sources: records::sources(&load(dir, "books.json"), &load(dir, "adventures.json")),
        species: Table::new(species),
        backgrounds: Table::new(backgrounds),
        feats: Table::new(feats),
        classes: Table::new(classes),
        subclasses: Table::new(subclasses),
        optional_features: Table::new(optional_features),
        spells: Table::new(records::spells(&spell_files, &lookup)),
        items: Table::new(items),
        languages: Table::new(records::languages(&load(dir, "languages.json"))),
        rules: Table::new(records::rules(&load(dir, "skills.json"), &load(dir, "items-base.json"), &load(dir, "conditionsdiseases.json"), &load(dir, "senses.json"))),
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

/// Repoints item references whose source doesn't exist (e.g. `holy symbol|phb`
/// when only the 2024 group exists) to an item of the same name.
fn fix_item_refs(species: &mut [compendium::Species], backgrounds: &mut [compendium::Background], classes: &mut [compendium::Class], items: &mut [compendium::Item]) {
    use compendium::{Grant, ItemRef, Key, Options};
    use std::collections::HashMap;
    let keys: BTreeSet<Key> = items.iter().map(|i| i.key.clone()).collect();
    let mut by_name: HashMap<String, Key> = HashMap::new();
    for i in items.iter().filter(|i| i.rarity.is_none()) {
        let rank = |k: &Key| match k.source_part() {
            "xphb" => 0,
            "phb" => 1,
            _ => 2,
        };
        let e = by_name.entry(i.name.to_lowercase()).or_insert_with(|| i.key.clone());
        if rank(&i.key) < rank(e) {
            *e = i.key.clone();
        }
    }
    let resolve = |k: &mut Key| {
        if !keys.contains(k) {
            let name = k.name_part().to_string();
            let singular = name.trim_end_matches('s').to_string();
            if let Some(found) = by_name.get(&name).or_else(|| by_name.get(&singular)) {
                *k = found.clone();
            }
        }
    };
    fn walk(gs: &mut [Grant], f: &dyn Fn(&mut Key)) {
        for g in gs {
            match g {
                Grant::Item { item: ItemRef::Key(k), .. } => f(k),
                Grant::Choose(c) => {
                    if let Options::Bundles(b) = &mut c.options {
                        for x in b {
                            walk(&mut x.grants, f);
                        }
                    }
                }
                Grant::AtLevel { grants, .. } => walk(grants, f),
                _ => {}
            }
        }
    }
    for s in species.iter_mut() {
        walk(&mut s.grants, &resolve);
    }
    for b in backgrounds.iter_mut() {
        walk(&mut b.grants, &resolve);
    }
    for c in classes.iter_mut() {
        walk(&mut c.equipment, &resolve);
    }
    for i in items.iter_mut() {
        for (k, _) in i.contents.iter_mut() {
            resolve(k);
        }
    }
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
