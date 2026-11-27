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
record!(Species, Background, Feat, Class, Subclass, OptionalFeature, Spell, Item, Language);

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

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Compendium {
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
    }

    pub fn source(&self, id: &str) -> Option<&SourceInfo> {
        self.sources.iter().find(|s| s.id.eq_ignore_ascii_case(id))
    }

    pub fn source_name(&self, id: &str) -> String {
        self.source(id).map(|s| s.name.clone()).unwrap_or_else(|| id.to_string())
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
