//! 5etools content inheritance: `_copy` (with `_mod`) and `_versions`.

use serde_json::{Map, Value};
use std::collections::HashMap;

/// Fields never inherited from a `_copy` base (they describe the base's own publication).
const NOT_INHERITED: &[&str] = &[
    "page", "srd", "srd52", "basicRules", "basicRules2024", "otherSources", "reprintedAs", "hasFluff",
    "hasFluffImages", "_versions", "isReprinted",
];

/// Resolves `_copy` for every record in `items`. `identity` produces the
/// lookup key for a record (from its own fields) and for a `_copy` block.
pub fn resolve_copies(items: Vec<Value>, identity: impl Fn(&Map<String, Value>) -> String) -> Vec<Value> {
    let index: HashMap<String, usize> = items
        .iter()
        .enumerate()
        .filter_map(|(i, v)| v.as_object().map(|o| (identity(o), i)))
        .collect();
    let mut resolved: Vec<Option<Value>> = vec![None; items.len()];
    for i in 0..items.len() {
        resolve_one(i, &items, &index, &identity, &mut resolved, 0);
    }
    resolved.into_iter().map(|v| v.unwrap_or(Value::Null)).collect()
}

fn resolve_one(
    i: usize,
    items: &[Value],
    index: &HashMap<String, usize>,
    identity: &impl Fn(&Map<String, Value>) -> String,
    resolved: &mut Vec<Option<Value>>,
    depth: u8,
) -> Value {
    if let Some(v) = &resolved[i] {
        return v.clone();
    }
    let item = &items[i];
    let out = match item.get("_copy").and_then(Value::as_object) {
        Some(copy) if depth < 8 => {
            let base = index
                .get(&identity(copy))
                .map(|&bi| resolve_one(bi, items, index, identity, resolved, depth + 1));
            match base {
                Some(base) => apply_copy(&base, item, copy),
                None => strip_copy(item),
            }
        }
        _ => strip_copy(item),
    };
    resolved[i] = Some(out.clone());
    out
}

fn strip_copy(item: &Value) -> Value {
    let mut o = item.as_object().cloned().unwrap_or_default();
    o.remove("_copy");
    Value::Object(o)
}

fn apply_copy(base: &Value, item: &Value, copy: &Map<String, Value>) -> Value {
    let preserve: Vec<String> = copy
        .get("_preserve")
        .and_then(Value::as_object)
        .map(|p| p.keys().cloned().collect())
        .unwrap_or_default();
    let mut out = base.as_object().cloned().unwrap_or_default();
    out.retain(|k, _| !NOT_INHERITED.contains(&k.as_str()) || preserve.contains(k));
    for (k, v) in item.as_object().into_iter().flatten() {
        if k != "_copy" {
            out.insert(k.clone(), v.clone());
        }
    }
    if let Some(m) = copy.get("_mod").and_then(Value::as_object) {
        apply_mods(&mut out, m, &Map::new());
    }
    Value::Object(out)
}

/// Applies a `_mod` block. `vars` are `{{name}}` substitutions for `_versions` templates.
pub fn apply_mods(obj: &mut Map<String, Value>, mods: &Map<String, Value>, vars: &Map<String, Value>) {
    for (prop, m) in mods {
        let list: Vec<Value> = match m {
            Value::Array(a) => a.clone(),
            other => vec![other.clone()],
        };
        for m in list {
            let m = substitute(&m, vars);
            if prop == "*" || prop == "_" {
                // Text replacement across all entries; other global mods are monster-only.
                if m.get("mode").and_then(Value::as_str) == Some("replaceTxt") {
                    if let Some(e) = obj.get_mut("entries") {
                        replace_txt(e, &m);
                    }
                }
                continue;
            }
            apply_mod(obj, prop, &m);
        }
    }
}

fn apply_mod(obj: &mut Map<String, Value>, prop: &str, m: &Value) {
    let mode = m.get("mode").and_then(Value::as_str).unwrap_or("");
    let items: Vec<Value> = match m.get("items") {
        Some(Value::Array(a)) => a.clone(),
        Some(v) => vec![v.clone()],
        None => vec![],
    };
    if mode == "setProp" {
        if let Some(v) = m.get("value") {
            let path = m.get("prop").and_then(Value::as_str).unwrap_or(prop);
            obj.insert(path.to_string(), v.clone());
        }
        return;
    }
    if mode == "replaceTxt" {
        if let Some(e) = obj.get_mut(prop) {
            replace_txt(e, m);
        }
        return;
    }
    let arr = obj.entry(prop.to_string()).or_insert_with(|| Value::Array(vec![]));
    if !arr.is_array() {
        *arr = Value::Array(vec![arr.clone()]);
    }
    let arr = arr.as_array_mut().unwrap();
    match mode {
        "appendArr" => arr.extend(items),
        "appendIfNotExistsArr" => {
            for it in items {
                if !arr.contains(&it) {
                    arr.push(it);
                }
            }
        }
        "prependArr" => {
            let mut v = items;
            v.extend(arr.drain(..));
            *arr = v;
        }
        "insertArr" => {
            let idx = m.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
            let idx = idx.min(arr.len());
            for (o, it) in items.into_iter().enumerate() {
                arr.insert(idx + o, it);
            }
        }
        "replaceArr" => {
            if let Some(pos) = find_pos(arr, m.get("replace")) {
                arr.splice(pos..pos + 1, items);
            }
        }
        "removeArr" => {
            if let Some(names) = m.get("names") {
                let names: Vec<Value> = match names {
                    Value::Array(a) => a.clone(),
                    v => vec![v.clone()],
                };
                for n in names {
                    if let Some(pos) = find_pos(arr, Some(&n)) {
                        arr.remove(pos);
                    }
                }
            } else {
                arr.retain(|x| !items.contains(x));
            }
        }
        _ => {}
    }
}

fn find_pos(arr: &[Value], target: Option<&Value>) -> Option<usize> {
    match target? {
        Value::String(name) => arr.iter().position(|x| match x {
            Value::String(s) => s == name,
            Value::Object(o) => o.get("name").and_then(Value::as_str) == Some(name),
            _ => false,
        }),
        Value::Object(o) => o.get("index").and_then(Value::as_u64).map(|i| i as usize).filter(|&i| i < arr.len()),
        _ => None,
    }
}

fn replace_txt(v: &mut Value, m: &Value) {
    let (Some(from), Some(with)) = (m.get("replace").and_then(Value::as_str), m.get("with").and_then(Value::as_str))
    else {
        return;
    };
    fn walk(v: &mut Value, from: &str, with: &str) {
        match v {
            Value::String(s) => *s = s.replace(from, with),
            Value::Array(a) => a.iter_mut().for_each(|x| walk(x, from, with)),
            Value::Object(o) => o
                .iter_mut()
                .filter(|(k, _)| k.as_str() != "type")
                .for_each(|(_, x)| walk(x, from, with)),
            _ => {}
        }
    }
    walk(v, from, with);
}

/// Replaces `{{var}}` placeholders in every string.
fn substitute(v: &Value, vars: &Map<String, Value>) -> Value {
    if vars.is_empty() {
        return v.clone();
    }
    match v {
        Value::String(s) => {
            let mut s = s.clone();
            for (k, val) in vars {
                let rep = match val {
                    Value::String(x) => x.clone(),
                    other => other.to_string(),
                };
                s = s.replace(&format!("{{{{{k}}}}}"), &rep);
            }
            Value::String(s)
        }
        Value::Array(a) => Value::Array(a.iter().map(|x| substitute(x, vars)).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.clone(), substitute(x, vars))).collect()),
        other => other.clone(),
    }
}

/// Expands `_versions` of a record into standalone records (the base excluded).
pub fn versions(base: &Value) -> Vec<Value> {
    let Some(vs) = base.get("_versions").and_then(Value::as_array) else { return vec![] };
    let mut root = base.as_object().cloned().unwrap_or_default();
    root.remove("_versions");
    root.retain(|k, _| !matches!(k.as_str(), "hasFluff" | "hasFluffImages" | "reprintedAs"));
    let mut out = Vec::new();
    for v in vs {
        if let (Some(abs), Some(impls)) = (v.get("_abstract"), v.get("_implementations").and_then(Value::as_array)) {
            for imp in impls {
                let vars = imp.get("_variables").and_then(Value::as_object).cloned().unwrap_or_default();
                let mut merged = abs.as_object().cloned().unwrap_or_default();
                for (k, x) in imp.as_object().into_iter().flatten() {
                    if k != "_variables" {
                        merged.insert(k.clone(), x.clone());
                    }
                }
                out.push(build_version(&root, &Value::Object(merged), &vars));
            }
        } else {
            out.push(build_version(&root, v, &Map::new()));
        }
    }
    out
}

fn build_version(root: &Map<String, Value>, version: &Value, vars: &Map<String, Value>) -> Value {
    let version = substitute(version, vars);
    let mut out = root.clone();
    for (k, x) in version.as_object().into_iter().flatten() {
        if k != "_mod" {
            out.insert(k.clone(), x.clone());
        }
    }
    if let Some(m) = version.get("_mod").and_then(Value::as_object) {
        apply_mods(&mut out, m, vars);
    }
    Value::Object(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn id(o: &Map<String, Value>) -> String {
        format!("{}|{}", o["name"].as_str().unwrap(), o["source"].as_str().unwrap())
    }

    #[test]
    fn copy_with_mods() {
        let items = vec![
            json!({"name": "A", "source": "X", "page": 3, "speed": 30, "entries": [{"name": "One"}, {"name": "Two"}]}),
            json!({"name": "B", "source": "Y", "_copy": {"name": "A", "source": "X", "_mod": {
                "entries": [{"mode": "replaceArr", "replace": "Two", "items": {"name": "Deux"}},
                            {"mode": "appendArr", "items": {"name": "Three"}}]}}}),
        ];
        let out = resolve_copies(items, id);
        assert_eq!(out[1]["speed"], 30);
        assert!(out[1].get("page").is_none());
        let names: Vec<_> = out[1]["entries"].as_array().unwrap().iter().map(|e| e["name"].clone()).collect();
        assert_eq!(names, vec![json!("One"), json!("Deux"), json!("Three")]);
    }

    #[test]
    fn abstract_versions() {
        let base = json!({"name": "D", "source": "X", "_versions": [{
            "_abstract": {"name": "D ({{c}})", "_mod": {"entries": {"mode": "appendArr", "items": "{{c}} breath"}}},
            "_implementations": [{"_variables": {"c": "Red"}, "resist": ["fire"]}]
        }], "entries": []});
        let v = versions(&base);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0]["name"], "D (Red)");
        assert_eq!(v[0]["entries"][0], "Red breath");
        assert_eq!(v[0]["resist"][0], "fire");
    }
}
