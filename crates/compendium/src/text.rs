//! Rich text, simplified from the 5etools entry tree into a handful of blocks.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Entry {
    /// A paragraph of plain text (inline tags already flattened).
    P(String),
    /// A titled (or untitled) group of entries.
    Section { name: Option<String>, body: Vec<Entry> },
    /// Bulleted list.
    List(Vec<Entry>),
    Table { caption: Option<String>, head: Vec<String>, rows: Vec<Vec<String>> },
}

impl Entry {
    /// Flattens entries to plain text, one paragraph per line.
    pub fn plain(entries: &[Entry]) -> String {
        let mut out = String::new();
        for e in entries {
            e.write_plain(&mut out);
        }
        out.trim_end().to_string()
    }

    fn write_plain(&self, out: &mut String) {
        match self {
            Entry::P(s) => {
                out.push_str(s);
                out.push('\n');
            }
            Entry::Section { name, body } => {
                if let Some(n) = name {
                    out.push_str(n);
                    out.push_str(". ");
                }
                for b in body {
                    b.write_plain(out);
                }
            }
            Entry::List(items) => {
                for i in items {
                    out.push_str("• ");
                    i.write_plain(out);
                }
            }
            Entry::Table { caption, head, rows } => {
                if let Some(c) = caption {
                    out.push_str(c);
                    out.push('\n');
                }
                out.push_str(&head.join(" | "));
                out.push('\n');
                for r in rows {
                    out.push_str(&r.join(" | "));
                    out.push('\n');
                }
            }
        }
    }
}

/// A named block of rules text shown on the sheet (species trait, class feature, ...).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Feature {
    pub name: String,
    pub body: Vec<Entry>,
}
