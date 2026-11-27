//! 5etools inline tags (`{@spell fireball|XPHB}`) and entry trees.

use compendium::{Ability, Entry, Feature};
use serde_json::Value;
use std::collections::HashMap;

/// Flattens inline `{@tag ...}` markup into display text.
pub fn strip(s: &str) -> String {
    book_refs(&strip_tags(s))
}

/// Spells out page references written into the source text ("DMG p143")
/// so players never see a bare book code.
fn book_refs(s: &str) -> String {
    const BOOKS: [(&str, &str); 6] = [
        ("XDMG", "Dungeon Master's Guide (2024)"),
        ("XPHB", "Player's Handbook (2024)"),
        ("XMM", "Monster Manual (2025)"),
        ("DMG", "Dungeon Master's Guide (2014)"),
        ("PHB", "Player's Handbook (2014)"),
        ("MM", "Monster Manual (2014)"),
    ];
    let mut out = s.to_string();
    for (code, name) in BOOKS {
        let needle = format!("{code} p");
        let mut from = 0;
        while let Some(i) = out[from..].find(&needle).map(|i| i + from) {
            let before_ok = i == 0 || !out[..i].ends_with(|c: char| c.is_ascii_alphanumeric());
            let digits: String = out[i + needle.len()..].chars().take_while(|c| c.is_ascii_digit()).collect();
            if before_ok && !digits.is_empty() {
                let replacement = format!("{name}, page {digits}");
                out.replace_range(i..i + needle.len() + digits.len(), &replacement);
                from = i + replacement.len();
            } else {
                from = i + needle.len();
            }
        }
    }
    out
}

fn strip_tags(s: &str) -> String {
    if !s.contains("{@") {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find("{@") {
        out.push_str(&rest[..start]);
        let after = &rest[start..];
        match matching_brace(after) {
            Some(end) => {
                let inner = strip(&after[2..end]);
                out.push_str(&render_tag(&inner));
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(after);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

fn matching_brace(s: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (i, c) in s.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

fn render_tag(inner: &str) -> String {
    let (tag, body) = inner.split_once(' ').unwrap_or((inner, ""));
    let parts: Vec<&str> = body.split('|').collect();
    let first = parts.first().copied().unwrap_or("").trim();
    match tag {
        "dc" => format!("DC {first}"),
        "hit" => {
            let n: i32 = first.parse().unwrap_or(0);
            compendium::signed(n)
        }
        "chance" => format!("{first} percent"),
        "recharge" => format!("(Recharge {}–6)", if first.is_empty() { "6" } else { first }),
        "atk" | "atkr" => attack_label(first, tag == "atkr"),
        "h" => "Hit: ".into(),
        "m" => "Miss: ".into(),
        "hom" => "Hit or Miss: ".into(),
        "actSave" => format!("{} Saving Throw:", Ability::parse(first).map(|a| a.name()).unwrap_or(first)),
        "actSaveFail" => "Failure:".into(),
        "actSaveSuccess" => "Success:".into(),
        "actSaveSuccessOrFail" => "Failure or Success:".into(),
        "actTrigger" => "Trigger:".into(),
        "actResponse" => "Response:".into(),
        "scaledice" | "scaledamage" => parts.last().copied().unwrap_or("").to_string(),
        "dice" | "damage" | "d20" | "autodice" => parts.get(1).filter(|s| !s.is_empty()).copied().unwrap_or(first).to_string(),
        "filter" | "book" | "adventure" | "5etools" | "link" | "footnote" | "homebrew" | "quickref" => first.to_string(),
        _ => {
            // Where the optional display text sits depends on the tag:
            // {@spell name|source|display}, {@classFeature name|class|classSource|level|source|display},
            // {@subclassFeature name|class|classSource|subclass|subclassSource|level|source|display}, ...
            let display_at = match tag {
                "classFeature" => 5,
                "subclassFeature" => 7,
                "subclass" => 4,
                "deity" | "card" => 3,
                _ => 2,
            };
            let shown = parts.get(display_at).filter(|s| !s.is_empty()).copied().unwrap_or(first);
            drop_bracket(shown).to_string()
        }
    }
}

fn attack_label(code: &str, roll: bool) -> String {
    let kinds: Vec<&str> = code
        .split(',')
        .map(|c| match c.trim() {
            "m" => "Melee",
            "r" => "Ranged",
            "mw" => "Melee Weapon",
            "rw" => "Ranged Weapon",
            "ms" => "Melee Spell",
            "rs" => "Ranged Spell",
            _ => "",
        })
        .collect();
    format!("{} {}:", kinds.join(" or "), if roll { "Attack Roll" } else { "Attack" })
}

/// `Sphere [Area of Effect]` → `Sphere`.
fn drop_bracket(s: &str) -> &str {
    match s.find(" [") {
        Some(i) if s.ends_with(']') => &s[..i],
        _ => s,
    }
}

/// Resolves `refClassFeature` / `refSubclassFeature` / `refOptionalfeature`.
#[derive(Default)]
pub struct Refs {
    pub features: HashMap<String, Value>,
}

impl Refs {
    pub fn lookup(&self, uid: &str) -> Option<&Value> {
        self.features.get(&uid.to_lowercase())
    }
}

pub fn entries(v: &Value, refs: &Refs) -> Vec<Entry> {
    match v {
        Value::Array(a) => a.iter().flat_map(|e| entry(e, refs)).collect(),
        other => entry(other, refs),
    }
}

fn entry(v: &Value, refs: &Refs) -> Vec<Entry> {
    match v {
        Value::String(s) => vec![Entry::P(strip(s))],
        Value::Number(n) => vec![Entry::P(n.to_string())],
        Value::Object(o) => {
            let ty = o.get("type").and_then(Value::as_str).unwrap_or("entries");
            let name = o.get("name").and_then(Value::as_str).map(strip);
            let body = |k: &str| o.get(k).map(|e| entries(e, refs)).unwrap_or_default();
            match ty {
                "list" => vec![Entry::List(
                    o.get("items")
                        .and_then(Value::as_array)
                        .map(|a| a.iter().map(|i| collapse(entry(i, refs))).collect())
                        .unwrap_or_default(),
                )],
                "table" => vec![table(o)],
                "item" | "itemSub" | "itemSpell" => {
                    let mut b = body("entries");
                    if let Some(e) = o.get("entry") {
                        b.extend(entry(e, refs));
                    }
                    vec![Entry::Section { name, body: b }]
                }
                "options" => vec![Entry::List(
                    o.get("entries")
                        .and_then(Value::as_array)
                        .map(|a| a.iter().map(|i| collapse(entry(i, refs))).collect())
                        .unwrap_or_default(),
                )],
                "refClassFeature" | "refSubclassFeature" => {
                    let uid = o
                        .get("classFeature")
                        .or_else(|| o.get("subclassFeature"))
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    match refs.lookup(uid) {
                        Some(f) => vec![Entry::Section {
                            name: f.get("name").and_then(Value::as_str).map(strip),
                            body: f.get("entries").map(|e| entries(e, refs)).unwrap_or_default(),
                        }],
                        None => vec![Entry::P(uid.split('|').next().unwrap_or("").to_string())],
                    }
                }
                "refOptionalfeature" => {
                    let n = o.get("optionalfeature").and_then(Value::as_str).unwrap_or("");
                    vec![Entry::P(n.split('|').next().unwrap_or("").to_string())]
                }
                "abilityDc" => vec![Entry::P(format!(
                    "{} save DC = 8 + your proficiency bonus + your {} modifier",
                    name.unwrap_or_default(),
                    ability_list(o.get("attributes"))
                ))],
                "abilityAttackMod" => vec![Entry::P(format!(
                    "{} attack modifier = your proficiency bonus + your {} modifier",
                    name.unwrap_or_default(),
                    ability_list(o.get("attributes"))
                ))],
                "inline" | "inlineBlock" => {
                    let text = Entry::plain(&body("entries")).replace('\n', " ");
                    vec![Entry::P(text)]
                }
                "link" => vec![Entry::P(o.get("text").and_then(Value::as_str).map(strip).unwrap_or_default())],
                "image" | "gallery" | "hr" | "statblock" | "statblockInline" | "flowchart" | "spellcasting" => vec![],
                _ => {
                    let b = body("entries");
                    if name.is_none() && b.is_empty() {
                        vec![]
                    } else if name.is_none() {
                        b
                    } else {
                        vec![Entry::Section { name, body: b }]
                    }
                }
            }
        }
        _ => vec![],
    }
}

fn collapse(mut v: Vec<Entry>) -> Entry {
    if v.len() == 1 {
        v.pop().unwrap()
    } else {
        Entry::Section { name: None, body: v }
    }
}

fn ability_list(v: Option<&Value>) -> String {
    v.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(|s| Ability::parse(s).map(|a| a.name()).unwrap_or(s))
                .collect::<Vec<_>>()
                .join(" or ")
        })
        .unwrap_or_default()
}

fn table(o: &serde_json::Map<String, Value>) -> Entry {
    let head = o
        .get("colLabels")
        .and_then(Value::as_array)
        .map(|a| a.iter().map(cell).collect())
        .unwrap_or_default();
    let rows = o
        .get("rows")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .map(|r| match r {
                    Value::Array(cells) => cells.iter().map(cell).collect(),
                    Value::Object(ro) => ro
                        .get("row")
                        .and_then(Value::as_array)
                        .map(|c| c.iter().map(cell).collect())
                        .unwrap_or_default(),
                    _ => vec![],
                })
                .collect()
        })
        .unwrap_or_default();
    Entry::Table { caption: o.get("caption").and_then(Value::as_str).map(strip), head, rows }
}

fn cell(v: &Value) -> String {
    match v {
        Value::String(s) => strip(s),
        Value::Number(n) => n.to_string(),
        Value::Object(o) => {
            if let Some(r) = o.get("roll") {
                let min = r.get("min").and_then(Value::as_i64);
                let max = r.get("max").and_then(Value::as_i64);
                let exact = r.get("exact").and_then(Value::as_i64);
                match (exact, min, max) {
                    (Some(e), _, _) => e.to_string(),
                    (_, Some(a), Some(b)) => format!("{a}–{b}"),
                    _ => String::new(),
                }
            } else if let Some(e) = o.get("entries").or_else(|| o.get("entry")) {
                Entry::plain(&entries(e, &Refs::default())).replace('\n', " ")
            } else {
                o.get("name").and_then(Value::as_str).map(strip).unwrap_or_default()
            }
        }
        _ => String::new(),
    }
}

/// Splits top-level named entries into features; unnamed text is kept under `fallback`.
pub fn features(v: Option<&Value>, refs: &Refs, fallback: &str) -> Vec<Feature> {
    let Some(v) = v else { return vec![] };
    let mut out = Vec::new();
    let mut loose = Vec::new();
    for e in entries(v, refs) {
        match e {
            Entry::Section { name: Some(n), body } => out.push(Feature { name: n, body }),
            Entry::Section { name: None, body } => {
                for inner in body {
                    match inner {
                        Entry::Section { name: Some(n), body } => out.push(Feature { name: n, body }),
                        other => loose.push(other),
                    }
                }
            }
            other => loose.push(other),
        }
    }
    if !loose.is_empty() {
        out.insert(0, Feature { name: fallback.to_string(), body: loose });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_tags() {
        assert_eq!(strip("You learn {@spell fire bolt|XPHB}."), "You learn fire bolt.");
        assert_eq!(strip("a {@variantrule Sphere [Area of Effect]|XPHB|Sphere} of"), "a Sphere of");
        assert_eq!(strip("{@dc 15} and {@hit 5}"), "DC 15 and +5");
        assert_eq!(strip("{@i see {@item rope|phb}}"), "see rope");
        assert_eq!(strip("{@damage 1d6} Acid"), "1d6 Acid");
        assert_eq!(strip("{@dice 5d4 × 10|5d4 × 10|Starting Gold}"), "5d4 × 10");
        // Class/subclass feature tags keep the class's book in field 3: never show it.
        assert_eq!(strip("replaces the Barbarian's {@classFeature Primal Knowledge|Barbarian|XPHB|3|XPHB} feature"), "replaces the Barbarian's Primal Knowledge feature");
        assert_eq!(strip("{@classFeature Rage|Barbarian|XPHB|1|XPHB|your Rage}"), "your Rage");
        assert_eq!(strip("{@subclassFeature Bonus Proficiency|Cleric|XPHB|Life|PHB|1}"), "Bonus Proficiency");
        assert_eq!(strip("{@subclass Evoker|Wizard|XPHB|XPHB}"), "Evoker");
        assert_eq!(strip("(see \"Special Features\" DMG p143)"), "(see \"Special Features\" Dungeon Master's Guide (2014), page 143)");
    }
}
