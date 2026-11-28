//! Content records. Every record carries its [`Key`], display name and source.

use crate::core::{Ability, Key, Size, Speed};
use crate::grant::Grant;
use crate::text::{Entry, Feature};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SourceGroup {
    Core,
    Supplement,
    Setting,
    Adventure,
    Partnered,
    Playtest,
    Other,
}

impl SourceGroup {
    pub fn label(self) -> &'static str {
        match self {
            SourceGroup::Core => "Core rules",
            SourceGroup::Supplement => "Supplements",
            SourceGroup::Setting => "Setting books",
            SourceGroup::Adventure => "Adventures",
            SourceGroup::Partnered => "Partnered & third party",
            SourceGroup::Playtest => "Playtest (UA)",
            SourceGroup::Other => "Other",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SourceInfo {
    /// Source id as used in 5etools data, e.g. `XPHB`.
    pub id: String,
    pub name: String,
    pub group: SourceGroup,
    pub published: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Species {
    pub key: Key,
    pub name: String,
    /// Set when this (older) record was reprinted: `name|source` of the
    /// newer version. Shown folded away under "older versions".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reprinted: Option<String>,
    pub source: String,
    /// Pre-2024 content (shown with a "legacy" badge).
    pub legacy: bool,
    pub sizes: Vec<Size>,
    pub speed: Speed,
    pub creature_type: String,
    pub darkvision: Option<u16>,
    pub grants: Vec<Grant>,
    /// Legacy fixed ability bonuses; ignored under 2024 rules unless enabled.
    pub legacy_asi: Vec<Grant>,
    pub lineages: Vec<Lineage>,
    pub features: Vec<Feature>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Lineage {
    pub key: Key,
    pub name: String,
    pub source: String,
    /// The lineage is a complete variant of the species: its grants and
    /// features replace the species' own instead of adding to them.
    pub replaces_base: bool,
    /// Species grants to use instead of the species' own when this lineage
    /// overwrites some of them (e.g. its own languages).
    #[serde(default)]
    pub base_override: Option<Vec<Grant>>,
    /// The lineage's ability bonuses replace the species' legacy ones.
    #[serde(default)]
    pub overrides_asi: bool,
    pub speed: Option<Speed>,
    pub darkvision: Option<u16>,
    pub grants: Vec<Grant>,
    pub legacy_asi: Vec<Grant>,
    pub features: Vec<Feature>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Background {
    pub key: Key,
    pub name: String,
    /// Set when this (older) record was reprinted: `name|source` of the
    /// newer version. Shown folded away under "older versions".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reprinted: Option<String>,
    pub source: String,
    pub legacy: bool,
    /// Whether the background itself grants ability score increases (2024 style).
    pub has_asi: bool,
    /// Whether the background itself grants an origin feat (2024 style).
    pub has_feat: bool,
    #[serde(default)]
    pub prereq: Option<String>,
    pub grants: Vec<Grant>,
    pub features: Vec<Feature>,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct Prereq {
    pub level: Option<u8>,
    /// Class whose level `level` refers to (lowercase); character level when absent.
    #[serde(default)]
    pub level_class: Option<String>,
    /// Lowercase species names; any one satisfies.
    #[serde(default)]
    pub species: Vec<String>,
    /// Lowercase feat names; any one satisfies.
    #[serde(default)]
    pub feats: Vec<String>,
    /// Lowercase spell names; any one satisfies.
    #[serde(default)]
    pub spells: Vec<String>,
    /// Lowercase class feature / optional feature names; any one satisfies.
    #[serde(default)]
    pub features: Vec<String>,
    /// Armor or weapon proficiency names (lowercase); all required.
    #[serde(default)]
    pub proficiencies: Vec<String>,
    /// Any one of these minimums satisfies the requirement.
    pub abilities: Vec<(Ability, u8)>,
    pub spellcasting: bool,
    /// Requires the Spellcasting or Pact Magic class feature (not just any spell).
    #[serde(default)]
    pub spellcasting_feature: bool,
    /// Human readable summary of all prerequisites.
    pub text: String,
    /// Other ways to qualify: meeting any one of these instead of the fields
    /// above also satisfies the prerequisite (5etools lists alternatives,
    /// e.g. Ritual Caster: Intelligence, Wisdom or Charisma 13).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternatives: Vec<Prereq>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Feat {
    pub key: Key,
    pub name: String,
    /// Set when this (older) record was reprinted: `name|source` of the
    /// newer version. Shown folded away under "older versions".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reprinted: Option<String>,
    pub source: String,
    pub category: Option<String>,
    pub prereq: Option<Prereq>,
    pub repeatable: bool,
    /// Set for pre-configured versions (e.g. "Magic Initiate; Cleric"); these
    /// are referenced by backgrounds but hidden from feat pickers.
    pub version_of: Option<Key>,
    pub grants: Vec<Grant>,
    pub body: Vec<Entry>,
}

impl Feat {
    pub fn category_label(&self) -> &'static str {
        match self.category.as_deref() {
            Some("O") => "Origin",
            Some("G") => "General",
            Some("FS") | Some("FS:P") | Some("FS:R") => "Fighting Style",
            Some("EB") => "Epic Boon",
            Some("D") => "Dragonmark",
            _ => "Feat",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Caster {
    Full,
    Half,
    Third,
    Pact,
    Artificer,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct Spellcasting {
    pub ability: Option<Ability>,
    pub caster: Option<Caster>,
    /// Counts indexed by class level - 1. Empty when unused.
    pub cantrips: Vec<u8>,
    /// 2024-style prepared spell counts.
    pub prepared: Vec<u8>,
    /// Spells known (2014 bards, sorcerers, ...).
    pub known: Vec<u8>,
    /// Spells added to a spellbook at each level (wizards).
    pub spellbook: Vec<u8>,
    /// 2014 prepared formula, e.g. `<$level$> + <$wis_mod$>`.
    pub prepared_formula: Option<String>,
    /// Class whose spell list is used (lowercase).
    pub list: String,
}

impl Spellcasting {
    pub fn is_caster(&self) -> bool {
        self.caster.is_some() || !self.cantrips.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClassFeature {
    pub name: String,
    pub level: u8,
    pub body: Vec<Entry>,
    pub grants: Vec<Grant>,
    /// "Gain a subclass feature" placeholder at this level.
    pub subclass_slot: bool,
    /// Optional class feature (Tasha's): the player opts in.
    #[serde(default)]
    pub optional: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ProgressionKind {
    Feats(Vec<String>),
    OptionalFeatures(Vec<String>),
}

/// One column of a class's (or subclass's) level table, e.g. "Rages" or
/// "Focus Points": the value at each level, as printed ("3", "+2", "1d8").
/// Spell slot and spells-known columns are not kept (spellcasting has those).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TableColumn {
    pub label: String,
    /// Index 0 is level 1.
    pub values: Vec<String>,
}

impl TableColumn {
    /// The value at `level` (1-based), if the column has one.
    pub fn at(&self, level: u8) -> Option<&str> {
        self.values.get(level.max(1) as usize - 1).map(String::as_str).filter(|v| !v.is_empty() && *v != "—" && *v != "-")
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Progression {
    pub name: String,
    pub kind: ProgressionKind,
    /// Total picks at class level `i + 1`.
    pub totals: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Class {
    pub key: Key,
    pub name: String,
    /// Set when this (older) record was reprinted: `name|source` of the
    /// newer version. Shown folded away under "older versions".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reprinted: Option<String>,
    pub source: String,
    pub legacy: bool,
    pub hit_die: u8,
    pub saves: Vec<Ability>,
    pub primary: Vec<Ability>,
    pub spellcasting: Spellcasting,
    /// Proficiencies and skill choices for a first-level character.
    pub starting: Vec<Grant>,
    /// Proficiencies gained when multiclassing into this class.
    pub multiclass: Vec<Grant>,
    pub multiclass_req: Option<String>,
    /// Multiclass requirement: every group must be met by any one of its scores.
    #[serde(default)]
    pub multiclass_abilities: Vec<Vec<(Ability, u8)>>,
    pub equipment: Vec<Grant>,
    pub features: Vec<ClassFeature>,
    pub subclass_title: String,
    pub progressions: Vec<Progression>,
    /// Weapon masteries known at class level `i + 1`.
    pub weapon_mastery: Vec<u8>,
    /// Resource columns of the class table (Rages, Channel Divinity, ...).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub table: Vec<TableColumn>,
    pub grants: Vec<Grant>,
    /// Other classes whose subclasses this class may also take.
    #[serde(default)]
    pub subclass_from: Vec<Key>,
}

impl Class {
    pub fn subclass_level(&self) -> Option<u8> {
        self.features.iter().filter(|f| f.subclass_slot).map(|f| f.level).min()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Subclass {
    pub key: Key,
    pub name: String,
    /// Set when this (older) record was reprinted: `name|source` of the
    /// newer version. Shown folded away under "older versions".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reprinted: Option<String>,
    pub short_name: String,
    pub source: String,
    pub class: Key,
    pub spellcasting: Spellcasting,
    pub features: Vec<ClassFeature>,
    pub progressions: Vec<Progression>,
    /// Resource columns of the subclass table (Psionic Energy Dice, ...).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub table: Vec<TableColumn>,
    pub grants: Vec<Grant>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OptionalFeature {
    pub key: Key,
    pub name: String,
    /// Set when this (older) record was reprinted: `name|source` of the
    /// newer version. Shown folded away under "older versions".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reprinted: Option<String>,
    pub source: String,
    pub types: Vec<String>,
    pub prereq: Option<Prereq>,
    pub grants: Vec<Grant>,
    pub body: Vec<Entry>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Spell {
    pub key: Key,
    pub name: String,
    pub source: String,
    pub level: u8,
    pub school: String,
    pub time: String,
    pub range: String,
    pub components: String,
    pub duration: String,
    pub concentration: bool,
    pub ritual: bool,
    /// Spell attack kinds: `m` (melee), `r` (ranged).
    #[serde(default)]
    pub attack: Vec<String>,
    /// Abilities of the saving throw the spell forces (lowercase names).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub save: Vec<String>,
    /// Damage types it deals (lowercase).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub damage: Vec<String>,
    /// Damage dice by character level (cantrips: 1, 5, 11, 17), per damage
    /// line, e.g. ("Fire damage", [(1, "1d10"), (5, "2d10"), ...]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scaling: Vec<(String, Vec<(u8, String)>)>,
    /// The first damage roll in its text, when it has no scaling table
    /// (e.g. Eldritch Blast's 1d10 per beam).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_damage: Option<String>,
    /// Lowercase class names whose list includes this spell.
    pub classes: Vec<String>,
    pub body: Vec<Entry>,
}

impl Spell {
    pub fn school_name(&self) -> &'static str {
        school_name(&self.school)
    }
}

pub fn school_name(abbr: &str) -> &'static str {
    match abbr {
        "A" => "Abjuration",
        "C" => "Conjuration",
        "D" => "Divination",
        "E" => "Enchantment",
        "V" => "Evocation",
        "I" => "Illusion",
        "N" => "Necromancy",
        "T" => "Transmutation",
        "P" => "Psionic",
        _ => "Other",
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArmorKind {
    Light,
    Medium,
    Heavy,
    Shield,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArmorStats {
    pub kind: ArmorKind,
    pub ac: u8,
    pub strength: Option<u8>,
    pub stealth_disadvantage: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WeaponStats {
    pub martial: bool,
    pub ranged: bool,
    pub damage: String,
    pub damage_type: String,
    pub versatile: Option<String>,
    /// Lowercase property names: finesse, light, thrown, two-handed, ...
    pub properties: Vec<String>,
    pub range: Option<String>,
    pub mastery: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub key: Key,
    pub name: String,
    pub source: String,
    /// Short type label (Weapon, Armor, Adventuring Gear, Wondrous Item, ...).
    pub kind: String,
    pub weight: f32,
    pub value_cp: Option<u32>,
    pub rarity: Option<String>,
    pub attunement: bool,
    /// Flat magic bonus to attack and damage, or to AC for armor.
    pub bonus: i8,
    pub weapon: Option<WeaponStats>,
    pub armor: Option<ArmorStats>,
    /// Equipment types this item satisfies (`weaponMartial`, `toolArtisan`, ...).
    pub tags: Vec<String>,
    /// Contents for equipment packs.
    pub contents: Vec<(Key, u16)>,
    pub body: Vec<Entry>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Language {
    pub key: Key,
    pub name: String,
    pub source: String,
    /// standard, exotic, rare, secret, ...
    pub kind: String,
    /// Typical speakers, script and notes.
    #[serde(default)]
    pub body: Vec<Entry>,
}

/// A rules glossary entry: skills, weapon masteries and properties,
/// conditions, senses. Used to explain options when choosing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    pub key: Key,
    /// `skill`, `mastery`, `property`, `condition`, `sense`.
    pub kind: String,
    pub name: String,
    pub source: String,
    pub body: Vec<Entry>,
}
