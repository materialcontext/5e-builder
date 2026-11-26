//! Parsers from 5etools proficiency/ability/spell/equipment blocks to [`Grant`]s.
//!
//! 5etools convention: an array of objects lists *alternatives*; a single
//! element is the common case. Alternatives become a bundle choice.

use crate::text::strip;
use compendium::*;
use serde_json::{Map, Value};

type Obj = Map<String, Value>;

/// Parses an array of alternatives with `f`; several alternatives become a choice.
pub fn alternatives(v: Option<&Value>, id: &str, label: &str, f: impl Fn(&Obj, &str) -> Vec<Grant>) -> Vec<Grant> {
    let Some(arr) = v.and_then(Value::as_array) else { return vec![] };
    let alts: Vec<&Obj> = arr.iter().filter_map(Value::as_object).collect();
    match alts.len() {
        0 => vec![],
        1 => f(alts[0], id),
        _ => {
            let bundles: Vec<Bundle> = alts
                .iter()
                .enumerate()
                .map(|(i, o)| {
                    let grants = f(o, &format!("{id}.{i}"));
                    Bundle { name: describe(&grants), grants }
                })
                .collect();
            vec![choose(id, label, 1, Options::Bundles(bundles))]
        }
    }
}

pub fn choose(id: &str, label: &str, count: u8, options: Options) -> Grant {
    Grant::Choose(Choice { id: id.to_string(), label: label.to_string(), count: Count::Fixed(count), options })
}

fn count_of(v: &Value) -> u8 {
    match v {
        Value::Number(n) => n.as_u64().unwrap_or(1) as u8,
        Value::Object(o) => o.get("count").and_then(Value::as_u64).unwrap_or(1) as u8,
        _ => 1,
    }
}

fn str_list(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(|s| s.to_string()).collect())
        .unwrap_or_default()
}

pub fn name_part(s: &str) -> String {
    strip(s).split('|').next().unwrap_or("").trim().to_string()
}

pub fn ref_key(s: &str, default_source: &str) -> Key {
    let s = s.split('#').next().unwrap_or(s);
    let mut parts = s.split('|');
    let name = parts.next().unwrap_or("").trim();
    let source = parts.next().filter(|x| !x.trim().is_empty()).unwrap_or(default_source);
    Key::new(name, source)
}

pub fn skills(o: &Obj, id: &str) -> Vec<Grant> {
    let mut out = Vec::new();
    for (k, v) in o {
        match k.as_str() {
            "choose" => {
                let from: Vec<Skill> = str_list(v.get("from")).iter().filter_map(|s| Skill::parse(s)).collect();
                let opts = if from.is_empty() { Options::AnySkill } else { Options::Skills(from) };
                out.push(choose(&format!("{id}.choose"), "Skill proficiency", count_of(v), opts));
            }
            "any" => out.push(choose(&format!("{id}.any"), "Skill proficiency", count_of(v), Options::AnySkill)),
            _ => {
                if let Some(s) = Skill::parse(k) {
                    out.push(Grant::Prof(Prof::Skill(s)));
                } else if let Some(name) = Some(name_part(k)).filter(|n| !n.is_empty()) {
                    // Tools are occasionally listed among skills.
                    out.push(Grant::Prof(Prof::Tool(name)));
                }
            }
        }
    }
    out
}

pub fn languages(o: &Obj, id: &str) -> Vec<Grant> {
    let mut out = Vec::new();
    for (k, v) in o {
        let cid = format!("{id}.{k}");
        match k.as_str() {
            "anyStandard" => out.push(choose(&cid, "Language", count_of(v), Options::AnyStandardLanguage)),
            "any" => out.push(choose(&cid, "Language", count_of(v), Options::AnyLanguage)),
            "anyExotic" => out.push(choose(&cid, "Language", count_of(v), Options::AnyExoticLanguage)),
            "choose" => {
                let from = str_list(v.get("from")).iter().map(|s| title_case(&name_part(s))).collect();
                out.push(choose(&cid, "Language", count_of(v), Options::Languages(from)));
            }
            "other" => {}
            _ => out.push(Grant::Prof(Prof::Language(title_case(&name_part(k))))),
        }
    }
    out
}

pub fn tools(o: &Obj, id: &str) -> Vec<Grant> {
    let mut out = Vec::new();
    for (k, v) in o {
        let cid = format!("{id}.{k}");
        match k.as_str() {
            "any" => out.push(choose(&cid, "Tool proficiency", count_of(v), Options::AnyTool)),
            "anyArtisansTool" => out.push(choose(&cid, "Artisan's tools", count_of(v), Options::AnyArtisanTool)),
            "anyMusicalInstrument" => out.push(choose(&cid, "Musical instrument", count_of(v), Options::AnyInstrument)),
            "anyGamingSet" => out.push(choose(&cid, "Gaming set", count_of(v), Options::AnyGamingSet)),
            "choose" => {
                let from = str_list(v.get("from")).iter().map(|s| name_part(s)).collect();
                out.push(choose(&cid, "Tool proficiency", count_of(v), Options::Tools(from)));
            }
            _ => out.push(Grant::Prof(Prof::Tool(name_part(k)))),
        }
    }
    out
}

pub fn weapons(o: &Obj, _id: &str) -> Vec<Grant> {
    o.iter()
        .filter(|(k, v)| k.as_str() != "choose" && v.as_bool() != Some(false))
        .map(|(k, _)| Grant::Prof(Prof::Weapon(name_part(k))))
        .collect()
}

pub fn armor(o: &Obj, _id: &str) -> Vec<Grant> {
    o.iter()
        .filter(|(_, v)| v.as_bool() != Some(false))
        .map(|(k, _)| Grant::Prof(Prof::Armor(name_part(k))))
        .collect()
}

/// `skillToolLanguageProficiencies`.
pub fn skill_tool_language(o: &Obj, id: &str) -> Vec<Grant> {
    fn opt(s: &str) -> Option<Options> {
        Some(match s {
            "anySkill" => Options::AnySkill,
            "anyTool" => Options::AnyTool,
            "anyLanguage" => Options::AnyLanguage,
            "anyArtisansTool" => Options::AnyArtisanTool,
            "anyMusicalInstrument" => Options::AnyInstrument,
            other => Options::Skills(vec![Skill::parse(other)?]),
        })
    }
    let mut out = Vec::new();
    if let Some(list) = o.get("choose").and_then(Value::as_array) {
        for (i, c) in list.iter().enumerate() {
            let parts: Vec<Options> = str_list(c.get("from")).iter().filter_map(|s| opt(s)).collect();
            out.push(choose(&format!("{id}.{i}"), "Proficiency", count_of(c), Options::Mixed(parts)));
        }
    }
    for (k, v) in o.iter().filter(|(k, _)| k.as_str() != "choose") {
        if let Some(op) = opt(k) {
            out.push(choose(&format!("{id}.{k}"), "Proficiency", count_of(v), op));
        }
    }
    out
}

pub fn expertise(o: &Obj, id: &str) -> Vec<Grant> {
    o.iter()
        .filter_map(|(k, v)| match k.as_str() {
            "anyProficientSkill" => Some(choose(&format!("{id}.any"), "Expertise", count_of(v), Options::SkillExpertise)),
            "choose" => {
                let from = str_list(v.get("from")).iter().filter_map(|s| Skill::parse(s)).collect();
                Some(choose(&format!("{id}.choose"), "Expertise", count_of(v), Options::ExpertiseFrom(from)))
            }
            other => Skill::parse(other).map(Grant::Expertise),
        })
        .collect()
}

pub fn ability(o: &Obj, id: &str) -> Vec<Grant> {
    let mut out = Vec::new();
    let max = o.get("max").and_then(Value::as_u64).map(|m| m as u8);
    for (k, v) in o {
        if k == "choose" {
            let (from, amounts) = if let Some(w) = v.get("weighted") {
                let from = str_list(w.get("from"));
                let weights =
                    w.get("weights").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_i64).map(|x| x as i8).collect()).unwrap_or_default();
                (from, weights)
            } else {
                let count = v.get("count").and_then(Value::as_u64).unwrap_or(1) as usize;
                let amount = v.get("amount").and_then(Value::as_i64).unwrap_or(1) as i8;
                (str_list(v.get("from")), vec![amount; count])
            };
            let from = from.iter().filter_map(|s| Ability::parse(s)).collect();
            out.push(choose(&format!("{id}.choose"), "Ability score increase", 1, Options::Abilities { from, amounts, max }));
        } else if let (Some(a), Some(n)) = (Ability::parse(k), v.as_i64()) {
            out.push(Grant::Ability { ability: a, amount: n as i8 });
            if let Some(m) = max {
                out.push(Grant::AbilityMax { ability: a, max: m });
            }
        }
    }
    out
}

/// `savingThrowProficiencies`.
pub fn saves(o: &Obj, id: &str) -> Vec<Grant> {
    let mut out = Vec::new();
    for (k, v) in o {
        if k == "choose" {
            let from = str_list(v.get("from")).iter().filter_map(|s| Ability::parse(s)).collect();
            out.push(choose(&format!("{id}.choose"), "Saving throw proficiency", count_of(v), Options::Saves(from)));
        } else if let Some(a) = Ability::parse(k) {
            out.push(Grant::Prof(Prof::Save(a)));
        }
    }
    out
}

/// `senses` / `bonusSenses`: `[{ "blindsight": 10 }]`.
pub fn senses(v: Option<&Value>) -> Vec<Grant> {
    let mut out = Vec::new();
    for o in v.and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_object) {
        for (k, r) in o {
            let Some(r) = r.as_u64() else { continue };
            out.push(match k.as_str() {
                "darkvision" => Grant::Darkvision(r as u16),
                other => Grant::Sense { sense: other.to_string(), range: r as u16 },
            });
        }
    }
    out
}

/// `optionalfeatureProgression` / `featProgression` on feats and options
/// (`{ "*": n }`: n picks, whatever the level).
pub fn flat_progressions(o: &Obj, id: &str) -> Vec<Grant> {
    let mut out = Vec::new();
    for (field, is_feat) in [("optionalfeatureProgression", false), ("featProgression", true)] {
        for (i, p) in o.get(field).and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_object).enumerate() {
            let n = p.get("progression").and_then(|x| x.get("*")).and_then(Value::as_u64).unwrap_or(1) as u8;
            let name = p.get("name").and_then(Value::as_str).unwrap_or("Option");
            let options = if is_feat {
                Options::Feats { categories: str_list(p.get("category")) }
            } else {
                Options::OptionalFeatures { types: str_list(p.get("featureType")) }
            };
            out.push(choose(&format!("{id}.{field}{i}"), name, n, options));
        }
    }
    out
}

pub fn feats(o: &Obj, id: &str) -> Vec<Grant> {
    o.iter()
        .filter_map(|(k, v)| match k.as_str() {
            "anyFromCategory" => {
                let cats = str_list(v.get("category"));
                Some(choose(&format!("{id}.cat"), "Feat", count_of(v), Options::Feats { categories: cats }))
            }
            "any" => Some(choose(&format!("{id}.any"), "Feat", count_of(v), Options::Feats { categories: vec![] })),
            _ => Some(Grant::Feat(ref_key(k, "phb"))),
        })
        .collect()
}

/// `resist` / `immune` / `conditionImmune` lists.
pub fn damage_list(v: Option<&Value>, id: &str, make: fn(String) -> Grant) -> Vec<Grant> {
    let Some(a) = v.and_then(Value::as_array) else { return vec![] };
    a.iter()
        .enumerate()
        .filter_map(|(i, x)| match x {
            Value::String(s) => Some(make(s.clone())),
            Value::Object(o) => o.get("choose").map(|c| {
                choose(&format!("{id}.{i}"), "Damage resistance", count_of(c), Options::Resist(str_list(c.get("from"))))
            }),
            _ => None,
        })
        .collect()
}

pub fn spell_filter(s: &str) -> SpellFilter {
    let mut f = SpellFilter::default();
    for part in s.split('|') {
        let Some((k, v)) = part.split_once('=') else { continue };
        let vals: Vec<&str> = v.split(';').map(str::trim).collect();
        match k.trim().to_lowercase().as_str() {
            "level" => f.levels = vals.iter().filter_map(|x| x.parse().ok()).collect(),
            "class" => f.classes = vals.iter().map(|x| x.to_lowercase()).collect(),
            "school" => f.schools = vals.iter().map(|x| x.to_uppercase()).collect(),
            "components & miscellaneous" => f.ritual = vals.iter().any(|x| x.eq_ignore_ascii_case("ritual")),
            "spell attack" => f.spell_attack = vals.iter().map(|x| x.to_lowercase()).collect(),
            "source" => {
                for v in vals {
                    match v.strip_prefix('!') {
                        Some(x) => f.exclude_sources.push(x.to_uppercase()),
                        None => f.sources.push(v.to_uppercase()),
                    }
                }
            }
            _ => {}
        }
    }
    f
}

/// `"3"` is a class/character level; `"s3"` is a *spell* level (no level gate); `"_"` is immediate.
fn level_key(k: &str) -> u8 {
    if k.starts_with('s') {
        0
    } else {
        k.parse().unwrap_or(0)
    }
}

fn spell_list(v: &Value, level: u8, kind: &SpellKind, out: &mut Vec<SpellGrant>) {
    let push = |out: &mut Vec<SpellGrant>, x: &Value| match x {
        Value::String(s) => out.push(SpellGrant { level, kind: kind.clone(), pick: SpellPick::Fixed(ref_key(s, "phb")) }),
        Value::Object(o) => {
            if let Some(c) = o.get("choose").and_then(Value::as_str) {
                let count = o.get("count").and_then(Value::as_u64).unwrap_or(1) as u8;
                out.push(SpellGrant { level, kind: kind.clone(), pick: SpellPick::Choose { filter: spell_filter(c), count } });
            } else if let Some(a) = o.get("all").and_then(Value::as_str) {
                out.push(SpellGrant { level, kind: kind.clone(), pick: SpellPick::All(spell_filter(a)) });
            }
        }
        _ => {}
    };
    match v {
        Value::Array(a) => a.iter().for_each(|x| push(out, x)),
        Value::Object(o) => {
            // `{ "_": [...] }`, `{ "daily": { "1": [...] } }`, `{ "will": [...] }`, `{ "rest": {...} }`
            for (k, x) in o {
                match k.as_str() {
                    "daily" | "rest" | "resource" => {
                        if let Some(per) = x.as_object() {
                            for (n, list) in per {
                                let n = n.trim_end_matches('e');
                                let label = match k.as_str() {
                                    "daily" => format!("{n}/long rest"),
                                    "rest" => format!("{n}/short rest"),
                                    _ => format!("{n} uses"),
                                };
                                spell_list(list, level, &SpellKind::Innate(label), out);
                            }
                        }
                    }
                    "will" => spell_list(x, level, &SpellKind::Innate("at will".into()), out),
                    "ritual" => spell_list(x, level, &SpellKind::Innate("ritual".into()), out),
                    _ => spell_list(x, level, kind, out),
                }
            }
        }
        _ => {}
    }
}

/// One `additionalSpells` entry.
pub fn spell_group(o: &Obj) -> SpellGroup {
    // No ability (or "inherit"): resolved by the engine from the granting feat or class.
    let ability = match o.get("ability") {
        Some(Value::String(s)) => Ability::parse(s).map(SpellAbility::Fixed).unwrap_or(SpellAbility::Inherit),
        Some(Value::Object(c)) => SpellAbility::Choose(str_list(c.get("choose")).iter().filter_map(|s| Ability::parse(s)).collect()),
        _ => SpellAbility::Inherit,
    };
    let mut spells = Vec::new();
    for (field, kind) in [
        ("known", SpellKind::Known),
        ("prepared", SpellKind::Prepared),
        ("innate", SpellKind::Innate("1/long rest".into())),
        ("expanded", SpellKind::Expanded),
    ] {
        if let Some(levels) = o.get(field).and_then(Value::as_object) {
            for (lk, list) in levels {
                let kind = if field == "innate" && list.is_array() { SpellKind::Innate("at will".into()) } else { kind.clone() };
                spell_list(list, level_key(lk), &kind, &mut spells);
            }
        }
    }
    SpellGroup { name: o.get("name").and_then(Value::as_str).map(strip), ability, spells }
}

pub fn additional_spells(v: Option<&Value>, id: &str) -> Vec<Grant> {
    let Some(arr) = v.and_then(Value::as_array) else { return vec![] };
    let groups: Vec<SpellGroup> = arr.iter().filter_map(Value::as_object).map(spell_group).collect();
    match groups.len() {
        0 => vec![],
        1 => vec![Grant::Spells(groups.into_iter().next().unwrap())],
        _ => {
            let bundles = groups
                .into_iter()
                .enumerate()
                .map(|(i, g)| {
                    let fixed: Vec<String> = g
                        .spells
                        .iter()
                        .filter_map(|s| match &s.pick {
                            SpellPick::Fixed(k) => Some(title_case(k.name_part())),
                            _ => None,
                        })
                        .collect();
                    let name = g.name.clone().unwrap_or_else(|| if fixed.is_empty() { format!("Option {}", i + 1) } else { fixed.join(", ") });
                    Bundle { name, grants: vec![Grant::Spells(g)] }
                })
                .collect();
            vec![choose(id, "Spells", 1, Options::Bundles(bundles))]
        }
    }
}

fn item_spec(v: &Value, id: &str) -> Vec<Grant> {
    match v {
        Value::String(s) => vec![Grant::Item { item: ItemRef::Key(ref_key(s, "phb")), qty: 1 }],
        Value::Object(o) => {
            let qty = o.get("quantity").and_then(Value::as_u64).unwrap_or(1) as u16;
            let mut out = Vec::new();
            if let Some(s) = o.get("item").and_then(Value::as_str) {
                out.push(Grant::Item { item: ItemRef::Key(ref_key(s, "phb")), qty });
            } else if let Some(s) = o.get("special").and_then(Value::as_str) {
                out.push(Grant::Item { item: ItemRef::Custom(title_case(&strip(s))), qty });
            } else if let Some(t) = o.get("equipmentType").and_then(Value::as_str) {
                out.push(Grant::Choose(Choice {
                    id: id.to_string(),
                    label: equipment_type_label(t).to_string(),
                    count: Count::Fixed(qty as u8),
                    options: Options::Items { equipment_type: t.to_string() },
                }));
            }
            if let Some(v) = o.get("value").or_else(|| o.get("containsValue")).and_then(Value::as_u64) {
                out.push(Grant::Coins(v as u32));
            }
            out
        }
        _ => vec![],
    }
}

pub fn equipment_type_label(t: &str) -> &'static str {
    match t {
        "weaponSimple" => "Simple weapon",
        "weaponMartial" => "Martial weapon",
        "weaponSimpleMelee" => "Simple melee weapon",
        "weaponMartialMelee" => "Martial melee weapon",
        "weaponSimpleRanged" => "Simple ranged weapon",
        "weaponMartialRanged" => "Martial ranged weapon",
        "instrumentMusical" => "Musical instrument",
        "toolArtisan" => "Artisan's tools",
        "setGaming" => "Gaming set",
        "armorLight" => "Light armor",
        "armorMedium" => "Medium armor",
        "armorHeavy" => "Heavy armor",
        "focusSpellcastingArcane" => "Arcane focus",
        "focusSpellcastingDruidic" => "Druidic focus",
        "focusSpellcastingHoly" => "Holy symbol",
        _ => "Item",
    }
}

/// `startingEquipment` blocks: `{ "_": [...] }` is fixed, lettered keys are alternatives.
pub fn equipment(v: Option<&Value>, id: &str) -> Vec<Grant> {
    let Some(arr) = v.and_then(Value::as_array) else { return vec![] };
    let mut out = Vec::new();
    for (bi, block) in arr.iter().enumerate() {
        let Some(o) = block.as_object() else { continue };
        if let Some(fixed) = o.get("_").and_then(Value::as_array) {
            for (i, x) in fixed.iter().enumerate() {
                out.extend(item_spec(x, &format!("{id}.{bi}._{i}")));
            }
        }
        let alts: Vec<(&String, &Value)> = o.iter().filter(|(k, _)| k.as_str() != "_").collect();
        if alts.is_empty() {
            continue;
        }
        let bundles = alts
            .iter()
            .map(|(k, list)| {
                let grants: Vec<Grant> = list
                    .as_array()
                    .into_iter()
                    .flatten()
                    .enumerate()
                    .flat_map(|(i, x)| item_spec(x, &format!("{id}.{bi}.{k}{i}")))
                    .collect();
                Bundle { name: format!("({}) {}", k.to_uppercase(), describe(&grants)), grants }
            })
            .collect();
        out.push(choose(&format!("{id}.{bi}"), "Starting equipment", 1, Options::Bundles(bundles)));
    }
    out
}

pub fn prereq(v: Option<&Value>) -> Option<Prereq> {
    let alts = v.and_then(Value::as_array)?;
    let first = alts.first()?.as_object()?;
    let mut p = Prereq::default();
    p.level = match first.get("level") {
        Some(Value::Number(n)) => n.as_u64().map(|x| x as u8),
        Some(Value::Object(o)) => o.get("level").and_then(Value::as_u64).map(|x| x as u8),
        _ => None,
    };
    if let Some(abs) = first.get("ability").and_then(Value::as_array) {
        for a in abs.iter().filter_map(Value::as_object) {
            for (k, v) in a {
                if let (Some(ab), Some(n)) = (Ability::parse(k), v.as_u64()) {
                    p.abilities.push((ab, n as u8));
                }
            }
        }
    }
    if let Some(Value::Object(l)) = first.get("level") {
        p.level_class = l.get("class").and_then(|c| c.get("name")).and_then(Value::as_str).map(|s| s.to_lowercase());
    }
    let names = |k: &str| -> Vec<String> {
        first
            .get(k)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|x| x.as_str().map(String::from).or_else(|| x.get("name").and_then(Value::as_str).map(String::from)))
            .map(|x| name_part(&x).split('#').next().unwrap_or("").to_lowercase())
            .collect()
    };
    p.species = names("race");
    p.feats = names("feat");
    p.spells = names("spell").into_iter().filter(|s| !s.is_empty()).collect();
    p.features = names("feature").into_iter().chain(names("optionalfeature")).collect();
    if let Some(pact) = first.get("pact").and_then(Value::as_str) {
        p.features.push(format!("pact of the {}", pact.to_lowercase()));
    }
    for prof in first.get("proficiency").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_object) {
        for v in prof.values().filter_map(Value::as_str) {
            p.proficiencies.push(v.to_lowercase());
        }
    }
    p.spellcasting = first.contains_key("spellcasting");
    p.spellcasting_feature =
        first.contains_key("spellcasting2020") || first.contains_key("spellcastingFeature") || first.contains_key("spellcastingPrepared");
    let texts: Vec<String> = alts.iter().filter_map(Value::as_object).map(prereq_text).filter(|s| !s.is_empty()).collect();
    p.text = texts.join("; or ");
    Some(p)
}

fn prereq_text(o: &Obj) -> String {
    let mut parts = Vec::new();
    for (k, v) in o {
        match k.as_str() {
            "level" => match v {
                Value::Number(n) => parts.push(format!("Level {n}+")),
                Value::Object(l) => {
                    let lv = l.get("level").and_then(Value::as_u64).unwrap_or(1);
                    let class = l.get("class").and_then(|c| c.get("name")).and_then(Value::as_str);
                    parts.push(match class {
                        Some(c) => format!("Level {lv} {c}"),
                        None => format!("Level {lv}+"),
                    });
                }
                _ => {}
            },
            "ability" => {
                let opts: Vec<String> = v
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_object)
                    .flat_map(|a| a.iter().map(|(k, n)| format!("{} {}+", Ability::parse(k).map(|x| x.name()).unwrap_or(k), n)))
                    .collect();
                parts.push(opts.join(" or "));
            }
            "spellcasting" | "spellcasting2020" | "spellcastingFeature" => parts.push("Spellcasting feature".into()),
            "spellcastingPrepared" => parts.push("Prepared spellcasting".into()),
            "race" | "species" => {
                let names: Vec<String> = v
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|r| r.get("name").and_then(Value::as_str))
                    .map(title_case)
                    .collect();
                parts.push(names.join(" or "));
            }
            "feat" => {
                let names: Vec<String> = v.as_array().into_iter().flatten().filter_map(Value::as_str).map(|s| title_case(&name_part(s))).collect();
                parts.push(format!("{} feat", names.join(" or ")));
            }
            "proficiency" => {
                for p in v.as_array().into_iter().flatten().filter_map(Value::as_object) {
                    for (kind, what) in p {
                        parts.push(format!("{} {} proficiency", title_case(what.as_str().unwrap_or("")), kind));
                    }
                }
            }
            "background" => {
                let names: Vec<String> =
                    v.as_array().into_iter().flatten().filter_map(|r| r.get("name").and_then(Value::as_str)).map(String::from).collect();
                parts.push(format!("{} background", names.join(" or ")));
            }
            "other" | "otherSummary" => {
                if let Some(s) = v.as_str().or_else(|| v.get("entry").and_then(Value::as_str)) {
                    parts.push(strip(s));
                }
            }
            "pact" => parts.push(format!("Pact of the {}", v.as_str().unwrap_or(""))),
            "campaign" => {
                let names: Vec<&str> = v.as_array().into_iter().flatten().filter_map(Value::as_str).collect();
                parts.push(format!("{} campaign", names.join(" or ")));
            }
            "feature" | "optionalfeature" => {
                let names: Vec<String> = v.as_array().into_iter().flatten().filter_map(Value::as_str).map(|s| title_case(&name_part(s))).collect();
                parts.push(names.join(" or "));
            }
            "item" => {
                let names: Vec<String> = v.as_array().into_iter().flatten().filter_map(Value::as_str).map(|s| title_case(&name_part(s))).collect();
                parts.push(names.join(" or "));
            }
            "patron" => parts.push(format!("{} patron", v.as_str().unwrap_or(""))),
            "spell" => {
                let names: Vec<String> = v.as_array().into_iter().flatten().filter_map(Value::as_str).map(|s| title_case(&name_part(s))).collect();
                parts.push(format!("{} spell", names.join(" or ")));
            }
            _ => {}
        }
    }
    parts.retain(|s| !s.is_empty());
    parts.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn background_ability_alternatives() {
        let v = json!([
            {"choose": {"weighted": {"from": ["int", "wis", "cha"], "weights": [2, 1]}}},
            {"choose": {"weighted": {"from": ["int", "wis", "cha"], "weights": [1, 1, 1]}}}
        ]);
        let g = alternatives(Some(&v), "asi", "Ability scores", ability);
        let Grant::Choose(c) = &g[0] else { panic!() };
        let Options::Bundles(b) = &c.options else { panic!() };
        assert_eq!(b.len(), 2);
    }

    #[test]
    fn filters() {
        let f = spell_filter("level=0;1|class=Wizard|school=A");
        assert_eq!(f.levels, vec![0, 1]);
        assert_eq!(f.classes, vec!["wizard"]);
        assert_eq!(f.schools, vec!["A"]);
    }

    #[test]
    fn innate_spells() {
        let v = json!({"innate": {"3": {"daily": {"1": ["faerie fire|xphb"]}}}, "known": {"1": ["dancing lights|xphb#c"]}, "ability": {"choose": ["int", "wis"]}});
        let g = spell_group(v.as_object().unwrap());
        assert_eq!(g.spells.len(), 2);
        assert!(g.spells.iter().any(|s| s.level == 3 && s.kind == SpellKind::Innate("1/long rest".into())));
        assert!(g.spells.iter().any(|s| s.pick == SpellPick::Fixed(Key::new("dancing lights", "xphb"))));
    }
}
