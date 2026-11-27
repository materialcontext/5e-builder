//! Shows only what the player chose.
//!
//! Many features describe a menu ("choose one of the following": fighting
//! styles, hunter's prey, totem animals, lineages, maneuver lists…). Once the
//! choice is made, the sheet should show the selection, not the menu. For
//! every choice with selections we know all option names and the chosen ones;
//! list items, sections and table rows that name an unchosen option are
//! removed from feature text. A block is only treated as a menu when at least
//! two of its entries name options, so passing mentions are left alone.

use crate::build::{Build, FeatureEntry};
use compendium::{Compendium, Entry, Feature, Key, Options};
use std::collections::BTreeSet;

#[derive(Default, Debug)]
pub struct Names {
    /// Every option of every choice that has a selection.
    pub all: BTreeSet<String>,
    /// The selected options.
    pub chosen: BTreeSet<String>,
    /// Damage-type choices (e.g. a 2014 dragonborn's ancestry), which pick
    /// table rows by a damage column rather than by name.
    pub damage_all: BTreeSet<String>,
    pub damage_chosen: BTreeSet<String>,
}

pub fn norm(s: &str) -> String {
    s.trim().trim_end_matches(['.', ':']).replace(['’', '‘'], "'").to_lowercase()
}

fn add(names: &mut Names, opts: &Options, selected: &[String], comp: &Compendium) {
    let chosen = |names: &mut Names, n: &str| {
        names.chosen.insert(norm(n));
    };
    match opts {
        Options::OptionalFeatures { types } => {
            for o in comp.optional_features.iter().filter(|o| o.types.iter().any(|t| types.contains(t))) {
                names.all.insert(norm(&o.name));
            }
            for v in selected {
                if let Some(o) = comp.optional_features.get(&Key(v.clone())) {
                    chosen(names, &o.name);
                }
            }
        }
        Options::Feats { categories } => {
            for f in comp.feats.iter().filter(|f| categories.is_empty() || categories.iter().any(|c| f.category.as_deref() == Some(c.as_str()))) {
                names.all.insert(norm(&f.name));
            }
            for v in selected {
                if let Some(f) = comp.feats.get(&Key(v.clone())) {
                    chosen(names, &f.name);
                }
            }
        }
        Options::Bundles(bundles) => {
            for b in bundles {
                names.all.insert(norm(&b.name));
            }
            for v in selected {
                chosen(names, v);
            }
        }
        Options::Subclass { class } => {
            for s in comp.subclasses_of(class) {
                names.all.insert(norm(&s.name));
                names.all.insert(norm(&s.short_name));
            }
            for v in selected {
                if let Some(s) = comp.subclasses.get(&Key(v.clone())) {
                    chosen(names, &s.name);
                    chosen(names, &s.short_name);
                }
            }
        }
        Options::Lineage { species } => {
            if let Some(sp) = comp.species.get(species) {
                for l in &sp.lineages {
                    names.all.insert(norm(&l.name));
                }
                for v in selected {
                    if let Some(l) = sp.lineages.iter().find(|l| &l.key.0 == v) {
                        chosen(names, &l.name);
                    }
                }
            }
        }
        Options::Resist(list) => {
            names.damage_all.extend(list.iter().map(|d| norm(d)));
            names.damage_chosen.extend(selected.iter().map(|d| norm(d)));
        }
        Options::Mixed(parts) => {
            for p in parts {
                add(names, p, selected, comp);
            }
        }
        _ => {}
    }
}

/// Option names for every choice the player has made.
pub fn names(build: &Build, comp: &Compendium) -> Names {
    let mut n = Names::default();
    for c in build.choices.iter().filter(|c| !c.selected.is_empty()) {
        add(&mut n, &c.options, &c.selected, comp);
    }
    // A chosen name is never also "unchosen" because another choice lists it.
    n
}

/// The option an entry describes, if it looks like one: a section title, a
/// short list item, the lead-in of a list item ("Name. text…"), or a table
/// row's first cell.
fn entry_name(e: &Entry) -> Option<String> {
    match e {
        Entry::Section { name: Some(n), .. } => Some(norm(n)),
        Entry::P(t) => {
            let head = t.split_once(". ").map(|(h, _)| h).or_else(|| t.split_once(": ").map(|(h, _)| h)).unwrap_or(t);
            (head.chars().count() <= 60).then(|| norm(head))
        }
        _ => None,
    }
}

struct Pruner<'a> {
    n: &'a Names,
    /// Chosen names that also exist as features of their own.
    standalone: &'a BTreeSet<String>,
    removed: usize,
    moved: Vec<String>,
}

impl Pruner<'_> {
    /// `Some(keep)` for an option entry, `None` for ordinary text.
    fn verdict(&mut self, name: &str) -> Option<bool> {
        if !self.n.all.contains(name) && !self.n.chosen.contains(name) {
            return None;
        }
        if !self.n.chosen.contains(name) {
            self.removed += 1;
            return Some(false);
        }
        if self.standalone.contains(name) {
            self.moved.push(name.to_string());
            return Some(false);
        }
        Some(true)
    }

    fn is_option(&self, name: &Option<String>) -> bool {
        name.as_ref().is_some_and(|n| self.n.all.contains(n) || self.n.chosen.contains(n))
    }

    fn entries(&mut self, entries: &[Entry]) -> Vec<Entry> {
        let menu = entries.iter().filter(|e| matches!(e, Entry::Section { .. }) && self.is_option(&entry_name(e))).count() >= 2;
        let mut out = Vec::new();
        for e in entries {
            match e {
                Entry::Section { name, body } => {
                    if menu {
                        if let Some(n) = entry_name(e) {
                            if self.verdict(&n) == Some(false) {
                                continue;
                            }
                        }
                    }
                    out.push(Entry::Section { name: name.clone(), body: self.entries(body) });
                }
                Entry::List(items) => {
                    let menu = items.iter().filter(|i| self.is_option(&entry_name(i))).count() >= 2;
                    let mut kept = Vec::new();
                    for i in items {
                        if menu {
                            if let Some(n) = entry_name(i) {
                                if self.verdict(&n) == Some(false) {
                                    continue;
                                }
                            }
                        }
                        kept.push(match i {
                            Entry::Section { name, body } => Entry::Section { name: name.clone(), body: self.entries(body) },
                            other => other.clone(),
                        });
                    }
                    if !kept.is_empty() {
                        out.push(Entry::List(kept));
                    }
                }
                Entry::Table { caption, head, rows } => {
                    let first = |r: &Vec<String>| r.first().map(|c| norm(c));
                    let menu = rows.iter().filter(|r| self.is_option(&first(r))).count() >= 2;
                    // Otherwise a column of damage types picked by a damage-type choice.
                    let width = rows.iter().map(Vec::len).max().unwrap_or(0);
                    let damage_col = (!menu)
                        .then(|| {
                            (1..width).find(|&j| {
                                let cells: Vec<String> = rows.iter().filter_map(|r| r.get(j)).map(|c| norm(c)).collect();
                                cells.iter().filter(|c| self.n.damage_all.contains(*c)).count() >= 2
                                    && cells.iter().any(|c| self.n.damage_chosen.contains(c))
                            })
                        })
                        .flatten();
                    let mut kept = Vec::new();
                    for r in rows {
                        // Table rows stay in place when chosen (they carry the details).
                        let by_name = menu && first(r).is_some_and(|n| self.is_option(&Some(n.clone())) && !self.n.chosen.contains(&n));
                        let by_damage = damage_col.and_then(|j| r.get(j)).map(|c| norm(c)).is_some_and(|c| self.n.damage_all.contains(&c) && !self.n.damage_chosen.contains(&c));
                        let drop = by_name || by_damage;
                        if drop {
                            self.removed += 1;
                        } else {
                            kept.push(r.clone());
                        }
                    }
                    out.push(Entry::Table { caption: caption.clone(), head: head.clone(), rows: kept });
                }
                Entry::P(_) => out.push(e.clone()),
            }
        }
        out
    }
}

/// Feature text with unchosen options removed. Options chosen here that are
/// also listed as features of their own are named in a closing "Chosen:" line
/// instead of being repeated. Pure option catalogs ("Maneuver Options") are
/// dropped once everything in them has been accounted for.
pub fn prune(features: Vec<FeatureEntry>, names: &Names) -> Vec<FeatureEntry> {
    let own: BTreeSet<String> = features.iter().map(|f| norm(&f.feature.name)).collect();
    features
        .iter()
        .filter_map(|f| {
            let me = norm(&f.feature.name);
            let standalone: BTreeSet<String> = own.iter().filter(|n| **n != me).cloned().collect();
            let mut p = Pruner { n: names, standalone: &standalone, removed: 0, moved: Vec::new() };
            let mut body = p.entries(&f.feature.body);
            if p.removed == 0 && p.moved.is_empty() {
                return Some(f.clone());
            }
            let catalog = me.ends_with(" options") || me.ends_with(" list");
            if catalog {
                return None;
            }
            if !p.moved.is_empty() {
                let label: Vec<String> = p
                    .moved
                    .iter()
                    .filter_map(|m| features.iter().find(|x| norm(&x.feature.name) == *m).map(|x| x.feature.name.clone()))
                    .collect();
                body.push(Entry::P(format!("Chosen: {}.", label.join(", "))));
            }
            Some(FeatureEntry { feature: Feature { name: f.feature.name.clone(), body }, ..f.clone() })
        })
        .collect()
}
