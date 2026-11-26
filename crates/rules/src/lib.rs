//! Rules engine: expands a [`Character`]'s decisions against the
//! [`compendium::Compendium`] into pending choices and a derived [`Sheet`].
//!
//! Everything here is a pure function of `(Character, Compendium)`; the UI
//! keeps only the `Character` and recomputes the rest on every change.

pub mod build;
pub mod character;
pub mod options;
pub mod sheet;
pub mod spellcasting;

pub use build::{build, Build, ChoiceState, Step};
pub use character::*;
pub use options::{options, OptionItem};
pub use sheet::{sheet, Sheet};

use compendium::*;

#[derive(Clone, Debug, PartialEq)]
pub struct Issue {
    pub step: Step,
    pub text: String,
}

/// Everything still missing or invalid, grouped by wizard step.
pub fn issues(ch: &Character, b: &Build, comp: &Compendium) -> Vec<Issue> {
    let mut out = Vec::new();
    let mut push = |step, text: String| out.push(Issue { step, text });
    if ch.levels.is_empty() {
        push(Step::Class, "Choose a class.".into());
    }
    if ch.background.is_none() {
        push(Step::Background, "Choose a background.".into());
    }
    if ch.species.is_none() {
        push(Step::Species, "Choose a species.".into());
    }
    if let Some(p) = ability_problem(&ch.abilities) {
        push(Step::Abilities, p);
    }
    for c in &b.choices {
        let n = c.picked();
        if n < c.count as usize {
            let what = c.options.noun();
            let left = c.count as usize - n;
            push(c.step, format!("{} — {}: choose {left} more {what}{}", c.origin, c.label, if left == 1 { "" } else { "s" }));
        } else if n > c.count as usize {
            push(c.step, format!("{} — {}: {n} chosen, only {} allowed", c.origin, c.label, c.count));
        }
    }
    for (i, l) in ch.levels.iter().enumerate().skip(1) {
        if !ch.settings.average_hp && l.hp_roll.is_none() {
            push(Step::Class, format!("Roll hit points for level {}.", i + 1));
        }
    }
    if ch.name.trim().is_empty() {
        push(Step::Details, "Give your character a name.".into());
    }
    for w in &b.warnings {
        push(Step::Class, w.clone());
    }
    let _ = comp;
    out
}

/// Items and coins granted by the selected starting-equipment options.
pub fn starting_equipment(b: &Build, comp: &Compendium) -> (Vec<InventoryItem>, u32) {
    let mut items: Vec<InventoryItem> = Vec::new();
    let mut coins = 0u32;
    for g in &b.grants {
        match &g.grant {
            Grant::Item { item, qty } => {
                let (key, name) = match item {
                    ItemRef::Key(k) => (Some(k.clone()), comp.display_name(k)),
                    ItemRef::Custom(n) => (None, n.clone()),
                };
                match items.iter_mut().find(|i| i.item == key && i.name == name) {
                    Some(existing) => existing.qty += qty,
                    None => {
                        let equip = key.as_ref().and_then(|k| comp.items.get(k)).is_some_and(|it| it.armor.is_some());
                        items.push(InventoryItem { item: key, name, qty: *qty, equipped: equip, attuned: false, starting: true, notes: String::new() })
                    }
                }
            }
            Grant::Coins(cp) => coins += cp,
            _ => {}
        }
    }
    (items, coins)
}

/// Replaces starting-equipment items (and starting coins) in the inventory.
pub fn apply_starting_equipment(ch: &mut Character, comp: &Compendium) {
    let b = build(ch, comp);
    let (items, coins) = starting_equipment(&b, comp);
    ch.inventory.retain(|i| !i.starting);
    let mut all = items;
    all.append(&mut ch.inventory);
    ch.inventory = all;
    ch.coins = Coins::from_cp(coins);
    ch.starting_equipment_applied = true;
}

impl Character {
    pub fn set_species(&mut self, key: Option<Key>) {
        if self.species != key {
            self.clear_choices_under("species");
            self.species = key;
        }
    }

    pub fn set_background(&mut self, key: Option<Key>) {
        if self.background != key {
            self.clear_choices_under("background");
            self.background = key;
        }
    }

    /// Sets the starting class; levels already taken in it switch to the new class.
    pub fn set_starting_class(&mut self, key: Key) {
        match self.levels.first().map(|l| l.class.clone()) {
            None => self.levels.push(LevelUp { class: key, hp_roll: None }),
            Some(old) if old != key => {
                self.clear_choices_under(&format!("class:{old}"));
                for l in self.levels.iter_mut().filter(|l| l.class == old) {
                    l.class = key.clone();
                }
            }
            Some(_) => {}
        }
    }

    /// Sets the number of levels in the starting class (1..=20 total).
    pub fn set_starting_level(&mut self, level: u8) {
        let Some(first) = self.levels.first().map(|l| l.class.clone()) else { return };
        let others = self.levels.iter().filter(|l| l.class != first).count();
        let want = (level as usize).clamp(1, 20 - others.min(19));
        let have = self.levels.len() - others;
        if want > have {
            let at = self.levels.iter().rposition(|l| l.class == first).map(|i| i + 1).unwrap_or(0);
            for _ in have..want {
                self.levels.insert(at, LevelUp { class: first.clone(), hp_roll: None });
            }
        } else {
            for _ in want..have {
                if let Some(i) = self.levels.iter().rposition(|l| l.class == first) {
                    self.levels.remove(i);
                }
            }
        }
    }

    pub fn level_up(&mut self, class: Key) {
        if self.levels.len() < 20 {
            self.levels.push(LevelUp { class, hp_roll: None });
        }
    }

    /// Removes the most recent level, dropping choices that only it granted.
    pub fn level_down(&mut self) {
        if let Some(last) = self.levels.pop() {
            if !self.levels.iter().any(|l| l.class == last.class) {
                self.clear_choices_under(&format!("class:{}", last.class));
            }
        }
    }

    /// Drops selections whose choice no longer exists (e.g. after a level-down).
    pub fn prune_choices(&mut self, comp: &Compendium) {
        let b = build(self, comp);
        self.choices.retain(|path, _| b.choices.iter().any(|c| &c.path == path));
    }
}

#[cfg(test)]
mod tests;
