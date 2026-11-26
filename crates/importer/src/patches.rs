//! Applies `patches.json`: structured data for mechanics 5etools only
//! describes in prose. Patches merge into raw records before parsing, so
//! they go through exactly the same parsers as upstream data.

use serde_json::{Map, Value};

const PATCHES: &str = include_str!("../patches.json");

pub struct Patches {
    root: Value,
    used: Vec<(String, usize, usize)>,
}

impl Patches {
    pub fn load() -> Self {
        let root = serde_json::from_str(PATCHES).expect("patches.json must be valid JSON");
        Patches { root, used: Vec::new() }
    }

    /// Patches every record of `kind` in `records`.
    pub fn apply(&mut self, kind: &str, records: &mut [Value]) {
        let Some(list) = self.root.get(kind).and_then(Value::as_array) else { return };
        for (i, p) in list.iter().enumerate() {
            let (Some(m), Some(set)) = (p.get("match").and_then(Value::as_object), p.get("set").and_then(Value::as_object)) else {
                continue;
            };
            let mut hits = 0;
            for r in records.iter_mut() {
                let Some(o) = r.as_object_mut() else { continue };
                if matches(o, m) {
                    for (k, v) in set {
                        o.insert(k.clone(), v.clone());
                    }
                    hits += 1;
                }
            }
            self.used.push((kind.to_string(), i, hits));
        }
    }

    /// Patches that matched nothing (likely a typo or upstream rename).
    pub fn unmatched(&self) -> Vec<String> {
        self.used
            .iter()
            .filter(|(_, _, hits)| *hits == 0)
            .map(|(kind, i, _)| format!("{kind}[{i}]: {}", self.root[kind][*i]["match"]))
            .collect()
    }
}

fn matches(o: &Map<String, Value>, m: &Map<String, Value>) -> bool {
    m.iter().all(|(k, want)| match (o.get(k), want) {
        (Some(Value::String(a)), Value::String(b)) => a.eq_ignore_ascii_case(b),
        (Some(a), b) => a == b,
        (None, _) => false,
    })
}
