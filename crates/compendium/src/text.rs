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

    /// Renders entries as simple HTML (paragraphs, bold run-in section
    /// names, lists, tables), e.g. for a VTT item description.
    pub fn html(entries: &[Entry]) -> String {
        let mut out = String::new();
        for e in entries {
            e.write_html(&mut out);
        }
        out
    }

    fn write_html(&self, out: &mut String) {
        match self {
            Entry::P(s) => {
                out.push_str("<p>");
                out.push_str(&escape_html(s));
                out.push_str("</p>");
            }
            Entry::Section { name, body } => {
                // Run-in title on the first paragraph, as in the books.
                let mut rest = body.as_slice();
                match (name, body.first()) {
                    (Some(n), Some(Entry::P(first))) => {
                        out.push_str(&format!("<p><strong>{}.</strong> {}</p>", escape_html(n), escape_html(first)));
                        rest = &body[1..];
                    }
                    (Some(n), _) => out.push_str(&format!("<p><strong>{}</strong></p>", escape_html(n))),
                    (None, _) => {}
                }
                for b in rest {
                    b.write_html(out);
                }
            }
            Entry::List(items) => {
                out.push_str("<ul>");
                for i in items {
                    out.push_str("<li>");
                    // A list item is usually a single paragraph: drop its <p>.
                    match i {
                        Entry::P(s) => out.push_str(&escape_html(s)),
                        other => other.write_html(out),
                    }
                    out.push_str("</li>");
                }
                out.push_str("</ul>");
            }
            Entry::Table { caption, head, rows } => {
                out.push_str("<table>");
                if let Some(c) = caption {
                    out.push_str(&format!("<caption>{}</caption>", escape_html(c)));
                }
                if !head.is_empty() {
                    out.push_str("<thead><tr>");
                    for h in head {
                        out.push_str(&format!("<th>{}</th>", escape_html(h)));
                    }
                    out.push_str("</tr></thead>");
                }
                out.push_str("<tbody>");
                for r in rows {
                    out.push_str("<tr>");
                    for c in r {
                        out.push_str(&format!("<td>{}</td>", escape_html(c)));
                    }
                    out.push_str("</tr>");
                }
                out.push_str("</tbody></table>");
            }
        }
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

/// Escapes text for HTML element content and attribute values.
pub fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            other => out.push(other),
        }
    }
    out
}
