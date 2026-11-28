//! The compendium: all content the builder can offer, in a normalized form.
//!
//! Produced by the `importer` crate from 5etools data and loaded by the web
//! app. Nothing in here knows about 5etools' raw format.

pub mod content;
pub mod core;
pub mod grant;
pub mod text;

pub use content::*;
pub use core::*;
pub use grant::*;
pub use text::*;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Common accessors for keyed records.
pub trait Record {
    fn key(&self) -> &Key;
    fn name(&self) -> &str;
    fn source(&self) -> &str;
}

macro_rules! record {
    ($($t:ty),*) => {$(
        impl Record for $t {
            fn key(&self) -> &Key { &self.key }
            fn name(&self) -> &str { &self.name }
            fn source(&self) -> &str { &self.source }
        }
    )*};
}
record!(Species, Background, Feat, Class, Subclass, OptionalFeature, Spell, Item, Language, Rule);

/// A list of records with a key index (index rebuilt after deserializing).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Table<T> {
    items: Vec<T>,
    #[serde(skip)]
    index: HashMap<Key, usize>,
}

impl<T> Default for Table<T> {
    fn default() -> Self {
        Table { items: Vec::new(), index: HashMap::new() }
    }
}

impl<T: Record> Table<T> {
    pub fn new(items: Vec<T>) -> Self {
        let mut t = Table { items, index: HashMap::new() };
        t.reindex();
        t
    }
    fn reindex(&mut self) {
        self.index = self.items.iter().enumerate().map(|(i, r)| (r.key().clone(), i)).collect();
    }
    pub fn get(&self, key: &Key) -> Option<&T> {
        self.index.get(key).map(|&i| &self.items[i])
    }
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.items.iter()
    }
    pub fn len(&self) -> usize {
        self.items.len()
    }
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// Format of `compendium.json`. Bump it whenever the importer starts
/// writing data the app relies on, so the app can tell a stale file (built
/// by an older importer, which still loads because new fields default to
/// empty) from a current one.
pub const SCHEMA: u32 = 3;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Compendium {
    /// [`SCHEMA`] of the importer that wrote this file (0: before versioning).
    #[serde(default)]
    pub schema: u32,
    /// Version of the upstream data this was built from.
    pub data_version: String,
    pub sources: Vec<SourceInfo>,
    pub species: Table<Species>,
    pub backgrounds: Table<Background>,
    pub feats: Table<Feat>,
    pub classes: Table<Class>,
    pub subclasses: Table<Subclass>,
    pub optional_features: Table<OptionalFeature>,
    pub spells: Table<Spell>,
    pub items: Table<Item>,
    pub languages: Table<Language>,
    #[serde(default)]
    pub rules: Table<Rule>,
}

impl Compendium {
    /// Parses a compendium from JSON and builds the lookup indexes.
    pub fn from_json(s: &str) -> serde_json::Result<Self> {
        let mut c: Compendium = serde_json::from_str(s)?;
        c.reindex();
        Ok(c)
    }

    pub fn reindex(&mut self) {
        self.species.reindex();
        self.backgrounds.reindex();
        self.feats.reindex();
        self.classes.reindex();
        self.subclasses.reindex();
        self.optional_features.reindex();
        self.spells.reindex();
        self.items.reindex();
        self.languages.reindex();
        self.rules.reindex();
    }

    pub fn source(&self, id: &str) -> Option<&SourceInfo> {
        self.sources.iter().find(|s| s.id.eq_ignore_ascii_case(id))
    }

    pub fn source_name(&self, id: &str) -> String {
        self.source(id).map(|s| s.name.clone()).unwrap_or_else(|| id.to_string())
    }

    /// The campaign setting a book belongs to, for setting-specific content
    /// such as languages. None for setting-neutral books and for the
    /// Forgotten Realms (the default setting of the core rules).
    pub fn campaign_setting(source: &str) -> Option<&'static str> {
        Some(match source.to_uppercase().as_str() {
            "ERLW" | "EFA" | "EGW:EBERRON" | "RMBRE" | "WGTE" => "Eberron",
            "EGW" | "TDCSR" | "CRCOTN" | "LOX" => "Exandria",
            "GGR" | "RMR" => "Ravnica",
            "MOT" => "Theros",
            "DSOTDQ" | "DLTTOS" => "Dragonlance",
            "SCC" => "Strixhaven",
            "VRGR" | "COS" => "Ravenloft",
            "AAG" | "BAM" | "LLK" | "SJA" => "Spelljammer",
            "SATO" | "SATOM" | "PS" => "Planescape",
            "PSA" => "Amonkhet",
            "PSI" => "Innistrad",
            "PSK" => "Kaladesh",
            "PSX" => "Ixalan",
            "PSD" => "Dominaria",
            "PSZ" => "Zendikar",
            "WOW" => "World of Warcraft",
            "AWM" | "UTHFTLH" => "Other settings",
            _ => return None,
        })
    }

    /// A glossary entry by kind and name, preferring the 2024 version.
    pub fn rule(&self, kind: &str, name: &str) -> Option<&Rule> {
        let mut found: Vec<&Rule> = self.rules.iter().filter(|r| r.kind == kind && r.name.eq_ignore_ascii_case(name)).collect();
        found.sort_by_key(|r| !matches!(r.source.as_str(), "XPHB"));
        found.into_iter().next()
    }

    /// A language by name, preferring the 2024 version.
    pub fn language(&self, name: &str) -> Option<&Language> {
        let mut found: Vec<&Language> = self.languages.iter().filter(|l| l.name.eq_ignore_ascii_case(name)).collect();
        // The 2024 book's entries are mostly bare; prefer one with text.
        found.sort_by_key(|l| (l.body.is_empty(), !matches!(l.source.as_str(), "XPHB" | "PHB")));
        found.into_iter().next()
    }

    /// An ordinary (non-magic) item by name, preferring the 2024 version.
    pub fn base_item(&self, name: &str) -> Option<&Item> {
        let mut found: Vec<&Item> = self.items.iter().filter(|i| i.rarity.is_none() && i.name.eq_ignore_ascii_case(name)).collect();
        found.sort_by_key(|i| (!matches!(i.source.as_str(), "XPHB"), i.body.is_empty()));
        found.into_iter().next()
    }

    /// The 2024 class that replaces an older (legacy) class, if it was
    /// reprinted. Under the 2024 rules the reprint is used, and the older
    /// class's subclasses are all available on it.
    pub fn replacement_class(&self, class: &Key) -> Option<&Class> {
        let old = self.classes.get(class).filter(|c| c.legacy)?;
        let mut newer: Vec<&Class> = self.classes.iter().filter(|c| !c.legacy && c.name == old.name).collect();
        newer.sort_by_key(|c| !matches!(c.source.as_str(), "XPHB" | "EFA"));
        newer.into_iter().next()
    }

    /// Subclasses available to a class (including borrowed ones).
    pub fn subclasses_of<'a>(&'a self, class: &'a Key) -> impl Iterator<Item = &'a Subclass> + 'a {
        let borrowed: Vec<Key> = self.classes.get(class).map(|c| c.subclass_from.clone()).unwrap_or_default();
        self.subclasses.iter().filter(move |s| &s.class == class || borrowed.contains(&s.class))
    }

    /// Display name for a key of any kind, falling back to the key's name part.
    pub fn display_name(&self, key: &Key) -> String {
        self.spells
            .get(key)
            .map(|r| r.name.clone())
            .or_else(|| self.items.get(key).map(|r| r.name.clone()))
            .or_else(|| self.feats.get(key).map(|r| r.name.clone()))
            .or_else(|| self.optional_features.get(key).map(|r| r.name.clone()))
            .unwrap_or_else(|| title_case(key.name_part()))
    }
}
