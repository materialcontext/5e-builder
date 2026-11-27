//! Rules engine: expands a [`Character`]'s decisions against the
//! [`compendium::Compendium`] into pending choices and a derived [`Sheet`].
//!
//! Everything here is a pure function of `(Character, Compendium)`; the UI
//! keeps only the `Character` and recomputes the rest on every change.

pub mod audit;
pub mod build;
pub mod character;
pub mod options;
pub mod sheet;
pub mod spellcasting;
pub mod selections;
pub mod suggest;

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
    if b.classes.len() > 1 {
        for cs in &b.classes {
            let Some(class) = comp.classes.get(&cs.key) else { continue };
            for group in &class.multiclass_abilities {
                if !group.iter().any(|(a, min)| b.score(*a) >= *min as i32) {
                    let need: Vec<String> = group.iter().map(|(a, n)| format!("{} {n}", a.abbr())).collect();
                    push(Step::Class, format!("Multiclassing into or out of {} requires {}.", class.name, need.join(" or ")));
                }
            }
        }
    }
    for (i, l) in ch.levels.iter().enumerate().skip(1) {
        if !ch.settings.average_hp && l.hp_roll.is_none() {
            push(Step::Class, format!("Roll hit points for level {}.", i + 1));
        }
    }
    for dup in repeated_feat_options(b, comp) {
        push(Step::Background, dup);
    }
    let attuned = ch.inventory.iter().filter(|i| i.attuned).count();
    if attuned > 3 {
        push(Step::Equipment, format!("Attuned to {attuned} items; the limit is 3."));
    }
    if ch.name.trim().is_empty() {
        push(Step::Details, "Give your character a name.".into());
    }
    for w in &b.warnings {
        push(Step::Class, w.clone());
    }
    out
}

/// Repeatable feats such as Magic Initiate must use a different option (spell
/// list) each time, including versions granted by backgrounds.
fn repeated_feat_options(b: &Build, comp: &Compendium) -> Vec<String> {
    let mut taken: Vec<(String, String)> = Vec::new();
    for (k, _) in &b.feats {
        if let Some(f) = comp.feats.get(k) {
            if let (Some(base), Some(list)) = (&f.version_of, f.name.split("; ").nth(1)) {
                taken.push((base.name_part().to_string(), list.to_lowercase()));
            }
        }
    }
    for c in &b.choices {
        let Options::Bundles(bundles) = &c.options else { continue };
        if !bundles.iter().all(|x| x.grants.iter().all(|g| matches!(g, Grant::Spells(_)))) {
            continue;
        }
        let feat = c.path.split('/').filter_map(|seg| comp.feats.get(&Key(seg.trim_start_matches("feat:").to_string()))).last();
        if let (Some(f), Some(sel)) = (feat, c.selected.first()) {
            if f.repeatable && f.version_of.is_none() {
                let list = sel.split_whitespace().next().unwrap_or("").to_lowercase();
                taken.push((f.key.name_part().to_string(), list));
            }
        }
    }
    let mut out = Vec::new();
    for (i, t) in taken.iter().enumerate() {
        if taken[..i].contains(t) {
            out.push(format!("{} is taken twice with the {} list; each time must use a different list.", title_case(&t.0), title_case(&t.1)));
        }
    }
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

/// Identifies the current starting-equipment selections (class, background
/// and every equipment choice). Changes when the player picks differently.
pub fn starting_signature(ch: &Character, b: &Build) -> String {
    let mut sig = format!(
        "{}|{}",
        ch.levels.first().map(|l| l.class.0.as_str()).unwrap_or(""),
        ch.background.as_ref().map(|k| k.0.as_str()).unwrap_or("")
    );
    for c in b.choices.iter().filter(|c| c.step == Step::Equipment) {
        sig.push_str(&format!(";{}={}", c.path, c.selected.join(",")));
    }
    sig
}

/// Replaces starting-equipment items in the inventory and adjusts coins by
/// the change in starting coins (keeping coins the player added or spent).
pub fn apply_starting_equipment(ch: &mut Character, comp: &Compendium) {
    let b = build(ch, comp);
    let (items, coins) = starting_equipment(&b, comp);
    ch.inventory.retain(|i| !i.starting);
    let mut all = items;
    all.append(&mut ch.inventory);
    ch.inventory = all;
    ch.coins = ch.coins.adjusted(coins as i64 - ch.starting_coins_cp as i64);
    ch.starting_coins_cp = coins;
    ch.starting_signature = starting_signature(ch, &b);
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
