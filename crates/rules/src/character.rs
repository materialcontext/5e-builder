//! The persisted character: only the player's decisions, never derived values.

use compendium::{Ability, Key};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const FORMAT_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Character {
    pub format: u32,
    pub id: String,
    pub name: String,
    pub details: Details,
    pub settings: Settings,
    pub abilities: BaseAbilities,
    pub species: Option<Key>,
    pub background: Option<Key>,
    /// Level gains in the order taken; the first entry is the starting class.
    pub levels: Vec<LevelUp>,
    /// Selections keyed by full choice path (see [`crate::build`]).
    pub choices: BTreeMap<String, Vec<String>>,
    pub inventory: Vec<InventoryItem>,
    pub coins: Coins,
    /// Whether starting equipment has been placed in the inventory.
    pub starting_equipment_applied: bool,
    /// Signature of the starting-equipment selections last applied; the
    /// inventory is re-synced only when it changes, so manual edits stick.
    pub starting_signature: String,
    /// Starting coins (cp) last applied, so a re-sync adjusts by the difference.
    pub starting_coins_cp: u32,
    /// Ad hoc additions outside the rules as written (house rules, DM
    /// rewards, homebrew): ability adjustments, proficiencies, feats, spells,
    /// features, bonuses. Applied like any other grant, labelled "Custom".
    pub custom: Vec<compendium::Grant>,
    /// The player's edits to what the printed sheet shows.
    pub sheet: SheetEdits,
    pub created: String,
    pub updated: String,
}

impl Default for Character {
    fn default() -> Self {
        Character {
            format: FORMAT_VERSION,
            id: String::new(),
            name: String::new(),
            details: Details::default(),
            settings: Settings::default(),
            abilities: BaseAbilities::default(),
            species: None,
            background: None,
            levels: Vec::new(),
            choices: BTreeMap::new(),
            inventory: Vec::new(),
            coins: Coins::default(),
            starting_equipment_applied: false,
            starting_signature: String::new(),
            starting_coins_cp: 0,
            custom: Vec::new(),
            sheet: SheetEdits::default(),
            created: String::new(),
            updated: String::new(),
        }
    }
}

/// What the player changed about the printed sheet, per section (see
/// [`crate::play::section`]): entries to always show (placed first) or
/// never show, and entries of their own.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SheetEdits {
    /// Section → entry name → true (always show, first) or false (never show).
    pub show: BTreeMap<String, BTreeMap<String, bool>>,
    /// Section → entries the player wrote.
    pub added: BTreeMap<String, Vec<SheetEntry>>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SheetEntry {
    pub name: String,
    pub detail: String,
}

impl SheetEdits {
    /// Some(true): always show; Some(false): never; None: automatic.
    pub fn state(&self, section: &str, name: &str) -> Option<bool> {
        self.show.get(section).and_then(|m| m.get(name)).copied()
    }

    pub fn set(&mut self, section: &str, name: &str, state: Option<bool>) {
        let m = self.show.entry(section.to_string()).or_default();
        match state {
            Some(v) => {
                m.insert(name.to_string(), v);
            }
            None => {
                m.remove(name);
            }
        }
        if m.is_empty() {
            self.show.remove(section);
        }
    }

    pub fn added(&self, section: &str) -> &[SheetEntry] {
        self.added.get(section).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Orders a section's candidates as the sheet prints them: always-shown
    /// first, then those shown by default; never-shown and hidden-by-default
    /// ones are dropped. `items` pairs each entry with whether it shows by default.
    pub fn arrange<T>(&self, section: &str, items: Vec<(T, bool)>, name: impl Fn(&T) -> &str) -> Vec<T> {
        let (mut pinned, mut rest) = (Vec::new(), Vec::new());
        for (item, default) in items {
            match self.state(section, name(&item)) {
                Some(true) => pinned.push(item),
                Some(false) => {}
                None if default => rest.push(item),
                None => {}
            }
        }
        pinned.extend(rest);
        pinned
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Details {
    pub player: String,
    pub alignment: String,
    pub pronouns: String,
    pub age: String,
    pub height: String,
    pub weight: String,
    pub eyes: String,
    pub hair: String,
    pub skin: String,
    pub faith: String,
    pub appearance: String,
    pub personality: String,
    pub ideals: String,
    pub bonds: String,
    pub flaws: String,
    pub backstory: String,
    pub allies: String,
    pub notes: String,
    pub xp: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Sources the player has switched off.
    pub disabled_sources: BTreeSet<String>,
    /// Use legacy (2014) species ability bonuses instead of background ones.
    pub legacy_species_asi: bool,
    /// Take the fixed average hit points on level up instead of rolling.
    pub average_hp: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { disabled_sources: BTreeSet::new(), legacy_species_asi: false, average_hp: true }
    }
}

impl Settings {
    pub fn source_enabled(&self, source: &str) -> bool {
        !self.disabled_sources.contains(source)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AbilityMethod {
    #[default]
    PointBuy,
    StandardArray,
    /// 4d6, drop the lowest die, six times; assign each roll once.
    Rolled,
    Manual,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BaseAbilities {
    pub method: AbilityMethod,
    /// Indexed by [`Ability::index`].
    pub scores: [u8; 6],
    /// The six rolled totals for [`AbilityMethod::Rolled`].
    pub rolls: Vec<u8>,
}

impl Default for BaseAbilities {
    fn default() -> Self {
        BaseAbilities { method: AbilityMethod::PointBuy, scores: [8; 6], rolls: Vec::new() }
    }
}

impl BaseAbilities {
    pub fn get(&self, a: Ability) -> u8 {
        self.scores[a.index()]
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LevelUp {
    pub class: Key,
    /// Hit die roll for this level (ignored for the first level and when using averages).
    #[serde(default)]
    pub hp_roll: Option<u8>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InventoryItem {
    /// Compendium item, or `None` for a custom item named by `name`.
    pub item: Option<Key>,
    pub name: String,
    pub qty: u16,
    #[serde(default)]
    pub equipped: bool,
    #[serde(default)]
    pub attuned: bool,
    /// Came from starting equipment (replaced when starting choices change).
    #[serde(default)]
    pub starting: bool,
    #[serde(default)]
    pub notes: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Coins {
    pub cp: u32,
    pub sp: u32,
    pub ep: u32,
    pub gp: u32,
    pub pp: u32,
}

impl Coins {
    pub fn from_cp(cp: u32) -> Self {
        Coins { gp: cp / 100, sp: (cp % 100) / 10, cp: cp % 10, ..Default::default() }
    }
    pub fn total_cp(&self) -> u32 {
        self.cp + self.sp * 10 + self.ep * 50 + self.gp * 100 + self.pp * 1000
    }

    /// Adds (or removes) `delta_cp`, keeping existing denominations where possible.
    pub fn adjusted(&self, delta_cp: i64) -> Coins {
        if delta_cp >= 0 {
            let add = Coins::from_cp(delta_cp as u32);
            return Coins { cp: self.cp + add.cp, sp: self.sp + add.sp, gp: self.gp + add.gp, ..self.clone() };
        }
        let total = self.total_cp() as i64 + delta_cp;
        Coins::from_cp(total.max(0) as u32)
    }
}

impl Character {
    pub fn new(id: impl Into<String>) -> Self {
        Character { id: id.into(), ..Default::default() }
    }

    pub fn total_level(&self) -> u8 {
        self.levels.len() as u8
    }

    /// Classes in the order first taken, with their level.
    pub fn class_levels(&self) -> Vec<(Key, u8)> {
        let mut out: Vec<(Key, u8)> = Vec::new();
        for l in &self.levels {
            match out.iter_mut().find(|(k, _)| k == &l.class) {
                Some((_, n)) => *n += 1,
                None => out.push((l.class.clone(), 1)),
            }
        }
        out
    }

    pub fn class_level(&self, class: &Key) -> u8 {
        self.levels.iter().filter(|l| &l.class == class).count() as u8
    }

    pub fn selected(&self, path: &str) -> &[String] {
        self.choices.get(path).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Sets a selection, dropping selections nested under it that no longer apply.
    pub fn set_choice(&mut self, path: &str, values: Vec<String>) {
        let old = self.choices.get(path).cloned().unwrap_or_default();
        for removed in old.iter().filter(|v| !values.contains(v)) {
            let prefix = format!("{path}/{removed}/");
            self.choices.retain(|k, _| !k.starts_with(&prefix));
        }
        if values.is_empty() {
            self.choices.remove(path);
        } else {
            self.choices.insert(path.to_string(), values);
        }
    }

    /// Drops every selection under a path prefix (e.g. when the species changes).
    pub fn clear_choices_under(&mut self, prefix: &str) {
        let with_slash = format!("{prefix}/");
        self.choices.retain(|k, _| k != prefix && !k.starts_with(&with_slash));
    }
}

/// 2024 point-buy costs for scores 8..=15.
pub fn point_cost(score: u8) -> Option<u8> {
    Some(match score {
        8 => 0,
        9 => 1,
        10 => 2,
        11 => 3,
        12 => 4,
        13 => 5,
        14 => 7,
        15 => 9,
        _ => return None,
    })
}

pub const POINT_BUY_BUDGET: u8 = 27;
pub const STANDARD_ARRAY: [u8; 6] = [15, 14, 13, 12, 10, 8];

/// Rolls 4d6 and drops the lowest die, six times. `rand` returns values in [0, 1).
pub fn roll_ability_scores(mut rand: impl FnMut() -> f64) -> Vec<u8> {
    (0..6)
        .map(|_| {
            let mut d: Vec<u8> = (0..4).map(|_| (rand() * 6.0) as u8 + 1).collect();
            d.sort_unstable();
            d[1..].iter().sum()
        })
        .collect()
}

/// Validates base scores for the chosen method; returns a problem description.
pub fn ability_problem(b: &BaseAbilities) -> Option<String> {
    match b.method {
        AbilityMethod::PointBuy => {
            let mut spent = 0u32;
            for s in b.scores {
                match point_cost(s) {
                    Some(c) => spent += c as u32,
                    None => return Some("Point buy scores must be between 8 and 15.".into()),
                }
            }
            match spent.cmp(&(POINT_BUY_BUDGET as u32)) {
                std::cmp::Ordering::Greater => Some(format!("{spent} of {POINT_BUY_BUDGET} points spent — too many.")),
                std::cmp::Ordering::Less => Some(format!("{} points left to spend.", POINT_BUY_BUDGET as u32 - spent)),
                std::cmp::Ordering::Equal => None,
            }
        }
        AbilityMethod::StandardArray => {
            let mut a = b.scores;
            a.sort_unstable_by(|x, y| y.cmp(x));
            (a != STANDARD_ARRAY).then(|| "Assign each standard array value (15, 14, 13, 12, 10, 8) once.".into())
        }
        AbilityMethod::Rolled => {
            if b.rolls.len() != 6 {
                return Some("Roll your ability scores.".into());
            }
            let mut want = b.rolls.clone();
            want.sort_unstable();
            let mut have = b.scores.to_vec();
            have.sort_unstable();
            (want != have).then(|| "Assign each rolled score to one ability.".into())
        }
        AbilityMethod::Manual => b
            .scores
            .iter()
            .any(|&s| !(1..=30).contains(&s))
            .then(|| "Scores must be between 1 and 30.".into()),
    }
}
