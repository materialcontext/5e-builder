//! Raw 5etools records → compendium records.

use crate::grants::{self as g, alternatives, choose, name_part, ref_key};
use crate::inherit;
use crate::text::{self, strip, Refs};
use compendium::*;
use serde_json::{Map, Value};
use std::collections::{BTreeSet, HashMap};

type Obj = Map<String, Value>;

fn s<'a>(o: &'a Obj, k: &str) -> &'a str {
    o.get(k).and_then(Value::as_str).unwrap_or("")
}

fn legacy(o: &Obj) -> bool {
    s(o, "edition") != "one"
}

fn arr<'a>(v: &'a Value, k: &str) -> Vec<&'a Obj> {
    v.get(k).and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_object).collect()).unwrap_or_default()
}

fn ident(o: &Obj, fields: &[&str]) -> String {
    fields.iter().map(|f| s(o, f).to_lowercase()).collect::<Vec<_>>().join("|")
}

fn u16_of(v: Option<&Value>) -> Option<u16> {
    v.and_then(Value::as_u64).map(|x| x as u16)
}

/// Proficiencies, feats, resistances and spells shared by most record types.
pub fn common_grants(o: &Obj, id: &str) -> Vec<Grant> {
    let mut out = Vec::new();
    out.extend(alternatives(o.get("skillProficiencies"), &format!("{id}.skills"), "Skill proficiency", g::skills));
    out.extend(alternatives(o.get("languageProficiencies"), &format!("{id}.languages"), "Languages", g::languages));
    out.extend(alternatives(o.get("toolProficiencies"), &format!("{id}.tools"), "Tool proficiency", g::tools));
    out.extend(alternatives(o.get("weaponProficiencies"), &format!("{id}.weapons"), "Weapon proficiency", g::weapons));
    out.extend(alternatives(o.get("armorProficiencies"), &format!("{id}.armor"), "Armor proficiency", g::armor));
    out.extend(alternatives(o.get("skillToolLanguageProficiencies"), &format!("{id}.stl"), "Proficiency", g::skill_tool_language));
    out.extend(alternatives(o.get("expertise"), &format!("{id}.expertise"), "Expertise", g::expertise));
    out.extend(alternatives(o.get("feats"), &format!("{id}.feats"), "Feat", g::feats));
    out.extend(g::damage_list(o.get("resist"), &format!("{id}.resist"), Grant::Resist));
    out.extend(g::damage_list(o.get("immune"), &format!("{id}.immune"), Grant::Immune));
    out.extend(g::damage_list(o.get("conditionImmune"), &format!("{id}.cimmune"), Grant::CondImmune));
    out.extend(g::additional_spells(o.get("additionalSpells"), &format!("{id}.spells")));
    out
}

fn ability_grants(o: &Obj, id: &str) -> Vec<Grant> {
    alternatives(o.get("ability"), id, "Ability score increase", g::ability)
}

fn speed(v: Option<&Value>) -> Option<Speed> {
    match v? {
        Value::Number(n) => Some(Speed { walk: n.as_u64().unwrap_or(30) as u16, ..Default::default() }),
        Value::Object(o) => {
            let walk = u16_of(o.get("walk")).unwrap_or(30);
            let other = |k: &str| match o.get(k) {
                Some(Value::Bool(true)) => walk,
                Some(Value::Number(n)) => n.as_u64().unwrap_or(0) as u16,
                Some(Value::Object(x)) => u16_of(x.get("number")).unwrap_or(0),
                _ => 0,
            };
            Some(Speed { walk, fly: other("fly"), swim: other("swim"), climb: other("climb"), burrow: other("burrow") })
        }
        _ => None,
    }
}

/// Legacy species entries that only describe numbers already on the sheet.
fn trait_features(v: Option<&Value>, refs: &Refs, fallback: &str) -> Vec<Feature> {
    const DESCRIPTIVE: &[&str] = &["Age", "Alignment", "Size", "Speed", "Languages", "Language", "Creature Type"];
    text::features(v, refs, fallback).into_iter().filter(|f| !DESCRIPTIVE.contains(&f.name.as_str())).collect()
}

fn toughness(features: &[Feature]) -> Vec<Grant> {
    if features.iter().any(|f| f.name == "Dwarven Toughness") {
        vec![Grant::HpPerLevel(1)]
    } else {
        vec![]
    }
}

// ---------------------------------------------------------------- species

pub fn species(data: &Value) -> Vec<Species> {
    let refs = Refs::default();
    let race_list: Vec<Value> = data.get("race").and_then(Value::as_array).cloned().unwrap_or_default();
    let sub_list: Vec<Value> = data.get("subrace").and_then(Value::as_array).cloned().unwrap_or_default();
    let races = inherit::resolve_copies(race_list, |o| ident(o, &["name", "source"]));
    let subs = inherit::resolve_copies(sub_list, |o| ident(o, &["name", "source", "raceName", "raceSource"]));

    let mut by_race: HashMap<String, Vec<&Obj>> = HashMap::new();
    for sub in subs.iter().filter_map(Value::as_object) {
        let k = format!("{}|{}", s(sub, "raceName").to_lowercase(), s(sub, "raceSource").to_lowercase());
        by_race.entry(k).or_default().push(sub);
    }

    let mut out = Vec::new();
    for race in races.iter().filter_map(Value::as_object) {
        let name = s(race, "name").to_string();
        let source = s(race, "source").to_string();
        let key = Key::new(&name, &source);
        let features = trait_features(race.get("entries"), &refs, "Traits");
        let mut grants = common_grants(race, "species");
        grants.extend(toughness(&features));

        let mut lineages: Vec<Lineage> = Vec::new();
        let race_k = format!("{}|{}", name.to_lowercase(), source.to_lowercase());
        for sub in by_race.get(&race_k).into_iter().flatten() {
            let sub_name = match s(sub, "name") {
                "" => "Standard".to_string(),
                n => n.to_string(),
            };
            let sub_source = match s(sub, "source") {
                "" => source.clone(),
                x => x.to_string(),
            };
            let feats = trait_features(sub.get("entries"), &refs, &sub_name);
            let mut lg = common_grants(sub, "lineage");
            lg.extend(toughness(&feats));
            lineages.push(Lineage {
                key: Key::new(&format!("{sub_name} ({name})"), &sub_source),
                name: sub_name,
                source: sub_source,
                replaces_base: false,
                speed: speed(sub.get("speed")),
                darkvision: u16_of(sub.get("darkvision")),
                grants: lg,
                legacy_asi: ability_grants(sub, "lineage.asi"),
                features: feats,
            });
        }
        for v in inherit::versions(&Value::Object(race.clone())) {
            let Some(vo) = v.as_object() else { continue };
            let vname = s(vo, "name").to_string();
            let short = vname
                .strip_prefix(&format!("{name}; "))
                .or_else(|| vname.strip_prefix(&format!("{name} (")).and_then(|x| x.strip_suffix(')')))
                .unwrap_or(&vname)
                .to_string();
            let feats = trait_features(vo.get("entries"), &refs, "Traits");
            let mut lg = common_grants(vo, "species");
            lg.extend(toughness(&feats));
            lineages.push(Lineage {
                key: Key::new(&vname, s(vo, "source")),
                name: short,
                source: s(vo, "source").to_string(),
                replaces_base: true,
                speed: speed(vo.get("speed")),
                darkvision: u16_of(vo.get("darkvision")),
                grants: lg,
                legacy_asi: ability_grants(vo, "species.asi"),
                features: feats,
            });
        }

        out.push(Species {
            key,
            legacy: legacy(race),
            sizes: race
                .get("size")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_str).filter_map(Size::parse).collect())
                .unwrap_or_else(|| vec![Size::Medium]),
            speed: speed(race.get("speed")).unwrap_or(Speed { walk: 30, ..Default::default() }),
            creature_type: race
                .get("creatureTypes")
                .and_then(Value::as_array)
                .and_then(|a| a.first())
                .and_then(Value::as_str)
                .map(title_case)
                .unwrap_or_else(|| "Humanoid".into()),
            darkvision: u16_of(race.get("darkvision")),
            grants,
            legacy_asi: ability_grants(race, "species.asi"),
            lineages,
            features,
            name,
            source,
        });
    }
    out
}

// ---------------------------------------------------------------- backgrounds

pub fn backgrounds(data: &Value) -> Vec<Background> {
    let refs = Refs::default();
    let list: Vec<Value> = data.get("background").and_then(Value::as_array).cloned().unwrap_or_default();
    inherit::resolve_copies(list, |o| ident(o, &["name", "source"]))
        .iter()
        .filter_map(Value::as_object)
        .map(|o| {
            let mut grants = common_grants(o, "background");
            grants.extend(ability_grants(o, "background.asi"));
            grants.extend(g::equipment(o.get("startingEquipment"), "background.equipment"));
            let features = text::features(o.get("entries"), &refs, "Background")
                .into_iter()
                .filter(|f| f.name.starts_with("Feature"))
                .map(|f| Feature { name: f.name.trim_start_matches("Feature:").trim().to_string(), body: f.body })
                .collect();
            Background {
                key: Key::new(s(o, "name"), s(o, "source")),
                name: s(o, "name").to_string(),
                source: s(o, "source").to_string(),
                legacy: legacy(o),
                has_asi: o.contains_key("ability"),
                has_feat: o.contains_key("feats"),
                grants,
                features,
            }
        })
        .collect()
}

// ---------------------------------------------------------------- feats

fn feat_record(o: &Obj, version_of: Option<Key>) -> Feat {
    let name = s(o, "name").to_string();
    let mut grants = common_grants(o, "feat");
    grants.extend(ability_grants(o, "feat.asi"));
    if name == "Tough" {
        grants.push(Grant::HpPerLevel(2));
    }
    Feat {
        key: Key::new(&name, s(o, "source")),
        source: s(o, "source").to_string(),
        category: o.get("category").and_then(Value::as_str).map(String::from),
        prereq: g::prereq(o.get("prerequisite")),
        repeatable: o.get("repeatable").and_then(Value::as_bool).unwrap_or(false),
        version_of,
        grants,
        body: text::entries(o.get("entries").unwrap_or(&Value::Null), &Refs::default()),
        name,
    }
}

pub fn feats(data: &Value) -> Vec<Feat> {
    let list: Vec<Value> = data.get("feat").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut out = Vec::new();
    for v in inherit::resolve_copies(list, |o| ident(o, &["name", "source"])) {
        let Some(o) = v.as_object() else { continue };
        let base = feat_record(o, None);
        for ver in inherit::versions(&v) {
            if let Some(vo) = ver.as_object() {
                out.push(feat_record(vo, Some(base.key.clone())));
            }
        }
        out.push(base);
    }
    out
}

// ---------------------------------------------------------------- optional features

pub fn optional_features(data: &Value) -> Vec<OptionalFeature> {
    let list: Vec<Value> = data.get("optionalfeature").and_then(Value::as_array).cloned().unwrap_or_default();
    inherit::resolve_copies(list, |o| ident(o, &["name", "source"]))
        .iter()
        .filter_map(Value::as_object)
        .map(|o| {
            let mut grants = common_grants(o, "opt");
            grants.extend(ability_grants(o, "opt.asi"));
            OptionalFeature {
                key: Key::new(s(o, "name"), s(o, "source")),
                name: s(o, "name").to_string(),
                source: s(o, "source").to_string(),
                types: o
                    .get("featureType")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(Value::as_str).map(String::from).collect())
                    .unwrap_or_default(),
                prereq: g::prereq(o.get("prerequisite")),
                grants,
                body: text::entries(o.get("entries").unwrap_or(&Value::Null), &Refs::default()),
            }
        })
        .collect()
}

// ---------------------------------------------------------------- classes

/// All class files merged: classes, subclasses and their features.
#[derive(Default)]
pub struct ClassData {
    pub class: Vec<Value>,
    pub subclass: Vec<Value>,
    pub class_feature: Vec<Value>,
    pub subclass_feature: Vec<Value>,
}

fn class_feature_uid(o: &Obj) -> Vec<String> {
    let base = format!("{}|{}|{}|{}", s(o, "name"), s(o, "className"), s(o, "classSource"), o.get("level").and_then(Value::as_u64).unwrap_or(0));
    let mut v = vec![format!("{base}|{}", s(o, "source")).to_lowercase()];
    if s(o, "source") == s(o, "classSource") {
        v.push(base.to_lowercase());
    }
    v
}

fn subclass_feature_uid(o: &Obj) -> Vec<String> {
    let base = format!(
        "{}|{}|{}|{}|{}|{}",
        s(o, "name"),
        s(o, "className"),
        s(o, "classSource"),
        s(o, "subclassShortName"),
        s(o, "subclassSource"),
        o.get("level").and_then(Value::as_u64).unwrap_or(0)
    );
    let mut v = vec![format!("{base}|{}", s(o, "source")).to_lowercase()];
    if s(o, "source") == s(o, "subclassSource") {
        v.push(base.to_lowercase());
    }
    v
}

/// Normalizes a feature uid so that omitted sources resolve (`a|b||3` → `a|b|phb|3`).
fn normalize_uid(uid: &str, subclass: bool) -> String {
    let mut parts: Vec<String> = uid.split('|').map(|p| p.trim().to_lowercase()).collect();
    if parts.len() > 2 && parts[2].is_empty() {
        parts[2] = "phb".into();
    }
    if subclass && parts.len() > 4 && parts[4].is_empty() {
        parts[4] = "phb".into();
    }
    let n = if subclass { 7 } else { 5 };
    if parts.len() == n && parts[n - 1].is_empty() {
        parts.pop();
    }
    parts.join("|")
}

fn caster(v: Option<&Value>) -> Option<Caster> {
    Some(match v?.as_str()? {
        "full" => Caster::Full,
        "1/2" => Caster::Half,
        "1/3" => Caster::Third,
        "pact" => Caster::Pact,
        "artificer" => Caster::Artificer,
        _ => return None,
    })
}

fn u8_list(v: Option<&Value>) -> Vec<u8> {
    v.and_then(Value::as_array).map(|a| a.iter().map(|x| x.as_u64().unwrap_or(0) as u8).collect()).unwrap_or_default()
}

fn spellcasting(o: &Obj, list: String) -> Spellcasting {
    Spellcasting {
        ability: o.get("spellcastingAbility").and_then(Value::as_str).and_then(Ability::parse),
        caster: caster(o.get("casterProgression")),
        cantrips: u8_list(o.get("cantripProgression")),
        prepared: u8_list(o.get("preparedSpellsProgression")),
        known: u8_list(o.get("spellsKnownProgression")),
        spellbook: u8_list(o.get("spellsKnownProgressionFixed")),
        prepared_formula: o.get("preparedSpells").and_then(Value::as_str).map(String::from),
        list,
    }
}

/// Step-function totals from either `[n per level]` or `{ "level": total }`.
fn totals(v: Option<&Value>) -> Vec<u8> {
    match v {
        Some(Value::Array(a)) => a.iter().map(|x| x.as_u64().unwrap_or(0) as u8).collect(),
        Some(Value::Object(o)) => {
            let mut steps: Vec<(u8, u8)> =
                o.iter().filter_map(|(k, v)| Some((k.parse().ok()?, v.as_u64()? as u8))).collect();
            steps.sort();
            (1..=20u8).map(|lvl| steps.iter().filter(|(l, _)| *l <= lvl).map(|(_, n)| *n).next_back().unwrap_or(0)).collect()
        }
        _ => vec![],
    }
}

fn progressions(o: &Obj) -> Vec<Progression> {
    let mut out = Vec::new();
    for p in o.get("featProgression").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_object) {
        out.push(Progression {
            name: s(p, "name").to_string(),
            kind: ProgressionKind::Feats(
                p.get("category").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(String::from).collect()).unwrap_or_default(),
            ),
            totals: totals(p.get("progression")),
        });
    }
    for p in o.get("optionalfeatureProgression").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_object) {
        out.push(Progression {
            name: s(p, "name").to_string(),
            kind: ProgressionKind::OptionalFeatures(
                p.get("featureType").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(String::from).collect()).unwrap_or_default(),
            ),
            totals: totals(p.get("progression")),
        });
    }
    out
}

/// Collects `options` blocks of class/subclass feature references inside a feature.
fn option_refs(v: &Value, out: &mut Vec<(u8, Vec<String>)>) {
    match v {
        Value::Object(o) => {
            if s(o, "type") == "options" {
                let uids: Vec<String> = o
                    .get("entries")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|e| e.get("classFeature").or_else(|| e.get("subclassFeature")).and_then(Value::as_str))
                    .map(String::from)
                    .collect();
                if !uids.is_empty() {
                    out.push((o.get("count").and_then(Value::as_u64).unwrap_or(1) as u8, uids));
                }
                return;
            }
            o.values().for_each(|x| option_refs(x, out));
        }
        Value::Array(a) => a.iter().for_each(|x| option_refs(x, out)),
        _ => {}
    }
}

/// Top-level `refSubclassFeature` entries (sub-features presented under a header feature).
fn split_subfeature_refs(v: &Value) -> (Value, Vec<String>) {
    let mut refs = Vec::new();
    let cleaned = match v {
        Value::Array(a) => Value::Array(
            a.iter()
                .filter(|e| {
                    if e.get("type").and_then(Value::as_str) == Some("refSubclassFeature") {
                        if let Some(u) = e.get("subclassFeature").and_then(Value::as_str) {
                            refs.push(u.to_string());
                        }
                        false
                    } else {
                        true
                    }
                })
                .cloned()
                .collect(),
        ),
        other => other.clone(),
    };
    (cleaned, refs)
}

struct FeatureCtx<'a> {
    refs: &'a Refs,
    legacy_class: bool,
}

fn feature_record(f: &Obj, level: u8, subclass_slot: bool, ctx: &FeatureCtx, path: &str) -> ClassFeature {
    let name = s(f, "name").to_string();
    let entries = f.get("entries").cloned().unwrap_or(Value::Null);
    let mut grants = Vec::new();
    match name.as_str() {
        "Ability Score Improvement" => {
            let mut cats = vec!["G".to_string()];
            if ctx.legacy_class {
                cats.push(String::new());
            }
            grants.push(choose(&format!("{path}.asi"), "Ability Score Improvement or feat", 1, Options::Feats { categories: cats }));
        }
        "Expertise" => grants.push(choose(&format!("{path}.expertise"), "Expertise", 2, Options::SkillExpertise)),
        _ => {}
    }
    grants.extend(common_grants(f, path));
    let mut opts = Vec::new();
    option_refs(&entries, &mut opts);
    for (i, (count, uids)) in opts.into_iter().enumerate() {
        let bundles: Vec<Bundle> = uids
            .iter()
            .filter_map(|u| {
                let sub = ctx.refs.lookup(&normalize_uid(u, u.split('|').count() > 5))?.as_object()?;
                let feature = Feature {
                    name: s(sub, "name").to_string(),
                    body: text::entries(sub.get("entries").unwrap_or(&Value::Null), ctx.refs),
                };
                let mut bg = vec![Grant::Feature(feature)];
                bg.extend(common_grants(sub, &format!("{path}.opt{i}")));
                Some(Bundle { name: s(sub, "name").to_string(), grants: bg })
            })
            .collect();
        if !bundles.is_empty() {
            grants.push(Grant::Choose(Choice {
                id: format!("{path}.opt{i}"),
                label: name.clone(),
                count: Count::Fixed(count),
                options: Options::Bundles(bundles),
            }));
        }
    }
    ClassFeature { body: text::entries(&entries, ctx.refs), name, level, grants, subclass_slot }
}

fn feature_uid_level(uid: &str, subclass: bool) -> u8 {
    let idx = if subclass { 5 } else { 3 };
    uid.split('|').nth(idx).and_then(|x| x.trim().parse().ok()).unwrap_or(1)
}

fn starting_grants(sp: &Obj, prefix: &str) -> Vec<Grant> {
    let mut out = Vec::new();
    let strings = |k: &str| -> Vec<String> {
        sp.get(k)
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).map(|x| name_part(&strip(x))).filter(|x| !x.is_empty()).collect())
            .unwrap_or_default()
    };
    if sp.contains_key("armorProficiencies") {
        out.extend(alternatives(sp.get("armorProficiencies"), &format!("{prefix}.armor"), "Armor", g::armor));
    } else {
        out.extend(strings("armor").into_iter().map(|a| Grant::Prof(Prof::Armor(a))));
    }
    if sp.contains_key("weaponProficiencies") {
        out.extend(alternatives(sp.get("weaponProficiencies"), &format!("{prefix}.weapons"), "Weapons", g::weapons));
    } else {
        out.extend(strings("weapons").into_iter().map(|a| Grant::Prof(Prof::Weapon(a))));
    }
    if sp.contains_key("toolProficiencies") {
        out.extend(alternatives(sp.get("toolProficiencies"), &format!("{prefix}.tools"), "Tools", g::tools));
    } else {
        out.extend(strings("tools").into_iter().map(|a| Grant::Prof(Prof::Tool(a))));
    }
    out.extend(alternatives(sp.get("skills"), &format!("{prefix}.skills"), "Skills", g::skills));
    out
}

fn multiclass_req(o: &Obj) -> Option<String> {
    let req = o.get("multiclassing")?.get("requirements")?.as_object()?;
    let fmt = |m: &Obj| -> Vec<String> {
        m.iter()
            .filter_map(|(k, v)| Some(format!("{} {}", Ability::parse(k)?.abbr(), v.as_u64()?)))
            .collect()
    };
    let mut parts = fmt(req);
    if let Some(or) = req.get("or").and_then(Value::as_array) {
        for alt in or.iter().filter_map(Value::as_object) {
            parts.push(fmt(alt).join(" or "));
        }
    }
    Some(parts.join(", "))
}

pub fn classes(data: &ClassData) -> (Vec<Class>, Vec<Subclass>) {
    let classes = inherit::resolve_copies(data.class.clone(), |o| ident(o, &["name", "source"]));
    let subclasses = inherit::resolve_copies(data.subclass.clone(), |o| ident(o, &["name", "source", "className", "classSource"]));
    let cfs = inherit::resolve_copies(data.class_feature.clone(), |o| ident(o, &["name", "source", "className", "classSource", "level"]));
    let scfs = inherit::resolve_copies(data.subclass_feature.clone(), |o| {
        ident(o, &["name", "source", "className", "classSource", "subclassShortName", "subclassSource", "level"])
    });

    let mut refs = Refs::default();
    for f in cfs.iter().filter_map(Value::as_object) {
        for uid in class_feature_uid(f) {
            refs.features.insert(uid, Value::Object(f.clone()));
        }
    }
    for f in scfs.iter().filter_map(Value::as_object) {
        for uid in subclass_feature_uid(f) {
            refs.features.insert(uid, Value::Object(f.clone()));
        }
    }

    let mut out_classes = Vec::new();
    // Sidekick classes are NPC companions, not player options.
    for c in classes.iter().filter_map(Value::as_object).filter(|c| c.get("isSidekick").and_then(Value::as_bool) != Some(true)) {
        let name = s(c, "name").to_string();
        let source = s(c, "source").to_string();
        let key = Key::new(&name, &source);
        let ctx = FeatureCtx { refs: &refs, legacy_class: legacy(c) };
        let mut features = Vec::new();
        for (i, cf) in c.get("classFeatures").and_then(Value::as_array).into_iter().flatten().enumerate() {
            let (uid, slot) = match cf {
                Value::String(u) => (u.as_str(), false),
                Value::Object(o) => (s(o, "classFeature"), o.get("gainSubclassFeature").and_then(Value::as_bool).unwrap_or(false)),
                _ => continue,
            };
            let level = feature_uid_level(uid, false);
            let Some(f) = refs.lookup(&normalize_uid(uid, false)).and_then(Value::as_object) else {
                eprintln!("warn: missing class feature {uid}");
                continue;
            };
            features.push(feature_record(f, level, slot, &ctx, &format!("f{i}")));
        }
        let sp = c.get("startingProficiencies").and_then(Value::as_object).cloned().unwrap_or_default();
        let mut starting: Vec<Grant> = c
            .get("proficiency")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .filter_map(Ability::parse)
            .map(|a| Grant::Prof(Prof::Save(a)))
            .collect();
        starting.extend(starting_grants(&sp, "start"));
        let multiclass = c
            .get("multiclassing")
            .and_then(|m| m.get("proficienciesGained"))
            .and_then(Value::as_object)
            .map(|p| starting_grants(p, "multi"))
            .unwrap_or_default();
        let equipment = c
            .get("startingEquipment")
            .and_then(|e| e.get("defaultData"))
            .map(|d| g::equipment(Some(d), "equipment"))
            .unwrap_or_default();
        out_classes.push(Class {
            legacy: legacy(c),
            hit_die: c.get("hd").and_then(|h| h.get("faces")).and_then(Value::as_u64).unwrap_or(8) as u8,
            saves: c
                .get("proficiency")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .filter_map(Ability::parse)
                .collect(),
            primary: c
                .get("primaryAbility")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_object)
                .flat_map(|o| o.keys().filter_map(|k| Ability::parse(k)))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            spellcasting: spellcasting(c, name.to_lowercase()),
            starting,
            multiclass,
            multiclass_req: multiclass_req(c),
            equipment,
            features,
            subclass_title: c.get("subclassTitle").and_then(Value::as_str).unwrap_or("Subclass").to_string(),
            progressions: progressions(c),
            weapon_mastery: totals(c.get("itemMasteriesProgression")),
            grants: common_grants(c, "class"),
            key,
            name,
            source,
        });
    }

    let mut out_subs = Vec::new();
    for sc in subclasses.iter().filter_map(Value::as_object) {
        if s(sc, "className").is_empty() {
            continue;
        }
        let class_key = Key::new(s(sc, "className"), match s(sc, "classSource") {
            "" => "phb",
            x => x,
        });
        let Some(legacy_class) = out_classes.iter().find(|c| c.key == class_key).map(|c| c.legacy) else { continue };
        let ctx = FeatureCtx { refs: &refs, legacy_class };
        let mut features = Vec::new();
        for (i, uid) in sc.get("subclassFeatures").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).enumerate() {
            let Some(f) = refs.lookup(&normalize_uid(uid, true)).and_then(Value::as_object) else {
                continue;
            };
            let level = feature_uid_level(uid, true);
            let (cleaned, subrefs) = split_subfeature_refs(f.get("entries").unwrap_or(&Value::Null));
            let mut head = f.clone();
            head.insert("entries".into(), cleaned);
            let head_rec = feature_record(&head, level, false, &ctx, &format!("s{i}"));
            let has_text = !head_rec.body.is_empty() || !head_rec.grants.is_empty();
            if has_text {
                features.push(head_rec);
            }
            for (j, su) in subrefs.iter().enumerate() {
                if let Some(sf) = refs.lookup(&normalize_uid(su, true)).and_then(Value::as_object) {
                    let lvl = sf.get("level").and_then(Value::as_u64).map(|x| x as u8).unwrap_or(level);
                    features.push(feature_record(sf, lvl, false, &ctx, &format!("s{i}.{j}")));
                }
            }
        }
        // Subclass spell list: from `expanded: { all: "level=..|class=Wizard" }`, else the class's.
        let list = sc
            .get("additionalSpells")
            .map(|a| a.to_string())
            .and_then(|txt| txt.split("class=").nth(1).map(|rest| rest.chars().take_while(|c| c.is_alphabetic()).collect::<String>().to_lowercase()))
            .filter(|_| sc.contains_key("casterProgression"))
            .unwrap_or_else(|| class_key.name_part().to_string());
        let name = s(sc, "name").to_string();
        let short = match s(sc, "shortName") {
            "" => name.clone(),
            x => x.to_string(),
        };
        out_subs.push(Subclass {
            key: Key(format!("{}|{}|{}", short.to_lowercase(), s(sc, "source").to_lowercase(), class_key)),
            short_name: short,
            source: s(sc, "source").to_string(),
            spellcasting: spellcasting(sc, list),
            features,
            progressions: progressions(sc),
            grants: common_grants(sc, "subclass"),
            class: class_key,
            name,
        });
    }
    (out_classes, out_subs)
}

// ---------------------------------------------------------------- spells

fn spell_time(o: &Obj) -> String {
    o.get("time")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .map(|t| {
            let n = t.get("number").and_then(Value::as_u64).unwrap_or(1);
            let unit = t.get("unit").and_then(Value::as_str).unwrap_or("action");
            let unit = match unit {
                "bonus" => "bonus action",
                u => u,
            };
            let mut s = if n == 1 { format!("1 {unit}") } else { format!("{n} {unit}s") };
            if let Some(c) = t.get("condition").and_then(Value::as_str) {
                s.push_str(&format!(", {}", strip(c)));
            }
            s
        })
        .unwrap_or_default()
}

fn spell_range(o: &Obj) -> String {
    let Some(r) = o.get("range").and_then(Value::as_object) else { return String::new() };
    let d = r.get("distance").and_then(Value::as_object);
    let dtype = d.map(|d| s(d, "type")).unwrap_or("");
    let amount = d.and_then(|d| d.get("amount")).and_then(Value::as_u64);
    let dist = match (dtype, amount) {
        ("self", _) => "Self".to_string(),
        ("touch", _) => "Touch".to_string(),
        ("sight", _) => "Sight".to_string(),
        ("unlimited", _) => "Unlimited".to_string(),
        (u, Some(a)) => format!("{a} {}", if u == "feet" { "ft." } else { u }),
        (u, None) => title_case(u),
    };
    match s(r, "type") {
        "point" => dist,
        "special" => "Special".into(),
        shape => match amount {
            Some(a) => format!("Self ({a}-{} {shape})", if dtype == "feet" { "ft." } else { dtype }),
            None => format!("{dist} ({shape})"),
        },
    }
}

fn spell_components(o: &Obj) -> String {
    let Some(c) = o.get("components").and_then(Value::as_object) else { return String::new() };
    let mut parts = Vec::new();
    if c.contains_key("v") {
        parts.push("V".to_string());
    }
    if c.contains_key("s") {
        parts.push("S".to_string());
    }
    match c.get("m") {
        Some(Value::String(m)) => parts.push(format!("M ({})", strip(m))),
        Some(Value::Object(m)) => parts.push(format!("M ({})", strip(s(m, "text")))),
        Some(_) => parts.push("M".into()),
        None => {}
    }
    parts.join(", ")
}

fn spell_duration(o: &Obj) -> (String, bool) {
    let Some(d) = o.get("duration").and_then(Value::as_array).and_then(|a| a.first()).and_then(Value::as_object) else {
        return (String::new(), false);
    };
    let conc = d.get("concentration").and_then(Value::as_bool).unwrap_or(false);
    let text = match s(d, "type") {
        "instant" => "Instantaneous".to_string(),
        "permanent" => "Until dispelled".to_string(),
        "special" => "Special".to_string(),
        "timed" => {
            let dur = d.get("duration").and_then(Value::as_object);
            let n = dur.and_then(|x| x.get("amount")).and_then(Value::as_u64).unwrap_or(1);
            let unit = dur.map(|x| s(x, "type")).unwrap_or("round");
            let t = if n == 1 { format!("1 {unit}") } else { format!("{n} {unit}s") };
            if conc {
                format!("Concentration, up to {t}")
            } else {
                t
            }
        }
        other => title_case(other),
    };
    (text, conc)
}

pub fn spells(files: &[Value], lookup: &Value) -> Vec<Spell> {
    let mut out = Vec::new();
    for f in files {
        for o in arr(f, "spell") {
            let name = s(o, "name").to_string();
            let source = s(o, "source").to_string();
            let mut classes = BTreeSet::new();
            if let Some(entry) = lookup.get(source.to_lowercase()).and_then(|x| x.get(name.to_lowercase())) {
                for k in ["class", "classVariant"] {
                    for by_source in entry.get(k).and_then(Value::as_object).into_iter().flat_map(|m| m.values()) {
                        for cname in by_source.as_object().into_iter().flat_map(|m| m.keys()) {
                            classes.insert(cname.to_lowercase());
                        }
                    }
                }
            }
            let (duration, concentration) = spell_duration(o);
            let mut body = text::entries(o.get("entries").unwrap_or(&Value::Null), &Refs::default());
            if let Some(h) = o.get("entriesHigherLevel") {
                body.extend(text::entries(h, &Refs::default()));
            }
            out.push(Spell {
                key: Key::new(&name, &source),
                level: o.get("level").and_then(Value::as_u64).unwrap_or(0) as u8,
                school: s(o, "school").to_string(),
                time: spell_time(o),
                range: spell_range(o),
                components: spell_components(o),
                duration,
                concentration,
                ritual: o.get("meta").and_then(|m| m.get("ritual")).and_then(Value::as_bool).unwrap_or(false),
                classes: classes.into_iter().collect(),
                body,
                name,
                source,
            });
        }
    }
    out
}

// ---------------------------------------------------------------- items

fn damage_type(code: &str) -> &'static str {
    match code {
        "A" => "acid",
        "B" => "bludgeoning",
        "C" => "cold",
        "F" => "fire",
        "O" => "force",
        "L" => "lightning",
        "N" => "necrotic",
        "P" => "piercing",
        "I" => "poison",
        "Y" => "psychic",
        "R" => "radiant",
        "S" => "slashing",
        "T" => "thunder",
        _ => "",
    }
}

fn property_name(code: &str) -> String {
    match code.split('|').next().unwrap_or("") {
        "A" => "ammunition",
        "AF" => "ammunition (futuristic)",
        "BF" => "burst fire",
        "F" => "finesse",
        "H" => "heavy",
        "L" => "light",
        "LD" => "loading",
        "R" => "reach",
        "RLD" => "reload",
        "S" => "special",
        "T" => "thrown",
        "2H" => "two-handed",
        "V" => "versatile",
        other => return other.to_lowercase(),
    }
    .to_string()
}

fn item_kind(code: &str, o: &Obj) -> String {
    if o.get("wondrous").and_then(Value::as_bool) == Some(true) {
        return "Wondrous Item".into();
    }
    match code {
        "M" => "Melee Weapon",
        "R" => "Ranged Weapon",
        "A" | "AF" => "Ammunition",
        "LA" => "Light Armor",
        "MA" => "Medium Armor",
        "HA" => "Heavy Armor",
        "S" => "Shield",
        "G" => "Adventuring Gear",
        "AT" => "Artisan's Tools",
        "T" => "Tools",
        "GS" => "Gaming Set",
        "INS" => "Musical Instrument",
        "SCF" => "Spellcasting Focus",
        "P" => "Potion",
        "RG" => "Ring",
        "RD" => "Rod",
        "WD" => "Wand",
        "SC" => "Scroll",
        "MNT" => "Mount",
        "TAH" => "Tack and Harness",
        "FD" => "Food and Drink",
        "TG" => "Trade Good",
        "EXP" => "Explosive",
        "SHP" | "VEH" | "AIR" | "SPC" => "Vehicle",
        _ => "Other",
    }
    .to_string()
}

fn value_bonus(v: Option<&Value>) -> i8 {
    v.and_then(Value::as_str).and_then(|x| x.trim_start_matches('+').parse().ok()).unwrap_or(0)
}

fn item_record(o: &Obj) -> Option<Item> {
    let code = s(o, "type").split('|').next().unwrap_or("").to_string();
    if code.starts_with('$') || matches!(code.as_str(), "SHP" | "VEH" | "AIR" | "SPC") {
        return None;
    }
    let category = s(o, "weaponCategory");
    let is_weapon = o.get("weapon").and_then(Value::as_bool) == Some(true) || !category.is_empty() || code == "M" || code == "R";
    let properties: Vec<String> = o
        .get("property")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|p| p.as_str().map(String::from).or_else(|| p.get("uid").and_then(Value::as_str).map(String::from)))
        .map(|p| property_name(&p))
        .collect();
    let weapon = (is_weapon && o.contains_key("dmg1")).then(|| WeaponStats {
        martial: category == "martial",
        ranged: code == "R",
        damage: s(o, "dmg1").to_string(),
        damage_type: damage_type(s(o, "dmgType")).to_string(),
        versatile: o.get("dmg2").and_then(Value::as_str).map(String::from),
        properties: properties.clone(),
        range: o.get("range").and_then(Value::as_str).map(String::from),
        mastery: o
            .get("mastery")
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .and_then(Value::as_str)
            .map(|m| m.split('|').next().unwrap_or("").to_string()),
    });
    let armor_kind = match code.as_str() {
        "LA" => Some(ArmorKind::Light),
        "MA" => Some(ArmorKind::Medium),
        "HA" => Some(ArmorKind::Heavy),
        "S" => Some(ArmorKind::Shield),
        _ => None,
    };
    let armor = armor_kind.map(|kind| ArmorStats {
        kind,
        ac: o.get("ac").and_then(Value::as_u64).unwrap_or(if kind == ArmorKind::Shield { 2 } else { 10 }) as u8,
        strength: o.get("strength").and_then(|v| v.as_str().and_then(|x| x.parse().ok()).or(v.as_u64().map(|x| x as u8))),
        stealth_disadvantage: o.get("stealth").and_then(Value::as_bool).unwrap_or(false),
    });
    let mut tags = Vec::new();
    if weapon.is_some() {
        let cat = if category == "martial" { "Martial" } else { "Simple" };
        let reach = if code == "R" { "Ranged" } else { "Melee" };
        tags.push(format!("weapon{cat}"));
        tags.push(format!("weapon{cat}{reach}"));
    }
    match code.as_str() {
        "AT" => tags.push("toolArtisan".into()),
        "INS" => tags.push("instrumentMusical".into()),
        "GS" => tags.push("setGaming".into()),
        "LA" => tags.push("armorLight".into()),
        "MA" => tags.push("armorMedium".into()),
        "HA" => tags.push("armorHeavy".into()),
        "SCF" => match s(o, "scfType") {
            "arcane" => tags.push("focusSpellcastingArcane".into()),
            "druid" => tags.push("focusSpellcastingDruidic".into()),
            "holy" => tags.push("focusSpellcastingHoly".into()),
            _ => {}
        },
        _ => {}
    }
    let rarity = match s(o, "rarity") {
        "" | "none" => None,
        r => Some(r.to_string()),
    };
    let bonus = if armor.is_some() { value_bonus(o.get("bonusAc")) } else { value_bonus(o.get("bonusWeapon")) };
    Some(Item {
        key: Key::new(s(o, "name"), s(o, "source")),
        name: s(o, "name").to_string(),
        source: s(o, "source").to_string(),
        kind: item_kind(&code, o),
        weight: o.get("weight").and_then(Value::as_f64).unwrap_or(0.0) as f32,
        value_cp: o.get("value").and_then(Value::as_u64).map(|v| v as u32),
        rarity,
        attunement: o.get("reqAttune").is_some_and(|v| !v.is_null() && v.as_bool() != Some(false)),
        bonus,
        weapon,
        armor,
        tags,
        contents: o
            .get("packContents")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|c| match c {
                Value::String(x) => Some((ref_key(x, "phb"), 1)),
                Value::Object(x) => x.get("item").and_then(Value::as_str).map(|i| (ref_key(i, "phb"), x.get("quantity").and_then(Value::as_u64).unwrap_or(1) as u16)),
                _ => None,
            })
            .collect(),
        body: text::entries(o.get("entries").unwrap_or(&Value::Null), &Refs::default()),
    })
}

pub fn items(base: &Value, items: &Value) -> Vec<Item> {
    let base_list: Vec<Value> = base.get("baseitem").and_then(Value::as_array).cloned().unwrap_or_default();
    let base_index: HashMap<String, Obj> = base_list
        .iter()
        .filter_map(Value::as_object)
        .map(|o| (format!("{}|{}", s(o, "name").to_lowercase(), s(o, "source").to_lowercase()), o.clone()))
        .collect();
    let item_list: Vec<Value> = items.get("item").and_then(Value::as_array).cloned().unwrap_or_default();
    let resolved = inherit::resolve_copies(item_list, |o| ident(o, &["name", "source"]));

    let mut out: Vec<Item> = base_list.iter().filter_map(Value::as_object).filter_map(item_record).collect();
    for v in resolved {
        let Some(o) = v.as_object() else { continue };
        // Magic items built on a base item inherit its stats.
        let merged = match o.get("baseItem").and_then(Value::as_str) {
            Some(b) => {
                let k = ref_key(b, "phb").0;
                let mut m = base_index.get(&k).cloned().unwrap_or_default();
                m.remove("entries");
                for (k, x) in o {
                    m.insert(k.clone(), x.clone());
                }
                m
            }
            None => o.clone(),
        };
        if let Some(it) = item_record(&merged) {
            out.push(it);
        }
    }
    out
}

// ---------------------------------------------------------------- languages & sources

pub fn languages(data: &Value) -> Vec<Language> {
    arr(data, "language")
        .into_iter()
        .map(|o| Language {
            key: Key::new(s(o, "name"), s(o, "source")),
            name: s(o, "name").to_string(),
            source: s(o, "source").to_string(),
            kind: match s(o, "type") {
                "" => "other".to_string(),
                t => t.to_string(),
            },
        })
        .collect()
}

pub fn sources(books: &Value, adventures: &Value) -> Vec<SourceInfo> {
    let mut out = Vec::new();
    for (v, k, default) in [(books, "book", SourceGroup::Supplement), (adventures, "adventure", SourceGroup::Adventure)] {
        for o in arr(v, k) {
            let group = match s(o, "group") {
                "core" => SourceGroup::Core,
                "supplement" | "supplement-alt" if k == "book" => SourceGroup::Supplement,
                "setting" | "setting-alt" => SourceGroup::Setting,
                "screen" | "recipe" | "homecraft" | "organized-play" | "other" => SourceGroup::Other,
                _ => default,
            };
            let id = match s(o, "source") {
                "" => s(o, "id"),
                x => x,
            };
            out.push(SourceInfo {
                id: id.to_string(),
                name: s(o, "name").to_string(),
                group,
                published: o.get("published").and_then(Value::as_str).map(String::from),
            });
        }
    }
    out
}

/// Sources referenced by content but missing from the book/adventure lists.
pub fn extra_source(id: &str) -> SourceInfo {
    let group = if id.starts_with("UA") || id.starts_with("XUA") || id.contains("Playtest") {
        SourceGroup::Playtest
    } else {
        SourceGroup::Partnered
    };
    SourceInfo { id: id.to_string(), name: id.to_string(), group, published: None }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn step_totals() {
        let t = totals(Some(&json!({"3": 3, "7": 5})));
        assert_eq!(t[1], 0);
        assert_eq!(t[2], 3);
        assert_eq!(t[6], 5);
        assert_eq!(t[19], 5);
    }

    #[test]
    fn uid_normalization() {
        assert_eq!(normalize_uid("Fighting Style|Fighter||1", false), "fighting style|fighter|phb|1");
        assert_eq!(normalize_uid("Abjurer|Wizard|XPHB|Abjurer|XPHB|3", true), "abjurer|wizard|xphb|abjurer|xphb|3");
    }
}
