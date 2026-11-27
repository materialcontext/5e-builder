//! Grants: the uniform way content gives a character something.
//!
//! Every piece of content (species, background, class level, feat, ...) is
//! described as a list of [`Grant`]s. Fixed grants apply directly; a
//! [`Grant::Choose`] needs a selection from the player, and the selected
//! option expands into more grants. The rules engine walks this tree; the
//! wizard renders every unresolved [`Choice`] generically.

use crate::core::{Ability, Key, Skill};
use crate::text::Feature;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Grant {
    Prof(Prof),
    Expertise(Skill),
    Ability { ability: Ability, amount: i8 },
    /// Raise an ability score maximum (e.g. epic boons to 30).
    AbilityMax { ability: Ability, max: u8 },
    Feat(Key),
    Spells(SpellGroup),
    Resist(String),
    Immune(String),
    CondImmune(String),
    Darkvision(u16),
    /// Darkvision of `base` feet, or +`bonus` feet if the character already has darkvision.
    DarkvisionBonus { base: u16, bonus: u16 },
    /// Blindsight, truesight, tremorsense (darkvision uses [`Grant::Darkvision`]).
    Sense { sense: String, range: u16 },
    Vulnerable(String),
    /// Adds to a speed (`walk`, `fly`, `swim`, `climb`, `burrow`).
    SpeedBonus { mode: String, amount: u16 },
    /// Grants a speed: `amount` feet, or equal to the walking speed when `None`.
    SpeedSet { mode: String, amount: Option<u16> },
    /// Extra hit points per level of the owning class (Draconic Resilience).
    HpPerClassLevel(i8),
    /// Flat hit point maximum bonus (resolved from per-class-level grants).
    HpBonus(i32),
    /// Expertise with a tool (thieves' tools for 2014 rogues).
    ToolExpertise(String),
    /// An unarmored AC formula: `base` + the listed ability modifiers
    /// (Unarmored Defense, natural armor). `shield`: usable with a shield.
    NaturalArmor { base: u8, abilities: Vec<Ability>, shield: bool },
    /// A flat AC bonus under a condition.
    AcBonus { amount: i8, when: AcWhen },
    /// Extra hit points per character level (Tough, Dwarven Toughness).
    HpPerLevel(i8),
    Feature(Feature),
    Item { item: ItemRef, qty: u16 },
    /// Coins, in copper pieces.
    Coins(u32),
    Choose(Choice),
    /// Grants that only apply once the scope's level (class level for class
    /// content, character level otherwise) reaches `level`.
    AtLevel { level: u8, grants: Vec<Grant> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AcWhen {
    Always,
    /// Wearing any armor.
    Armored,
    HeavyArmor,
    /// Not wearing heavy armor.
    NotHeavyArmor,
    Unarmored,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Prof {
    Skill(Skill),
    Save(Ability),
    Armor(String),
    Weapon(String),
    Tool(String),
    Language(String),
}

impl Prof {
    pub fn label(&self) -> String {
        match self {
            Prof::Skill(s) => s.name().to_string(),
            Prof::Save(a) => format!("{} saves", a.name()),
            Prof::Armor(s) | Prof::Weapon(s) | Prof::Tool(s) | Prof::Language(s) => title_case(s),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemRef {
    Key(Key),
    Custom(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Choice {
    /// Locally unique id; the engine prefixes it with the owning path.
    pub id: String,
    pub label: String,
    pub count: Count,
    pub options: Options,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Count {
    Fixed(u8),
    /// Total number of picks at scope level `i + 1`.
    ByLevel(Vec<u8>),
}

impl Count {
    pub fn at(&self, level: u8) -> u8 {
        match self {
            Count::Fixed(n) => *n,
            Count::ByLevel(v) => {
                if level == 0 || v.is_empty() {
                    0
                } else {
                    v[(level as usize - 1).min(v.len() - 1)]
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Options {
    Skills(Vec<Skill>),
    AnySkill,
    /// Expertise in a skill the character is already proficient in.
    SkillExpertise,
    /// Expertise in one of these skills (must be proficient).
    ExpertiseFrom(Vec<Skill>),
    /// Saving throw proficiencies.
    Saves(Vec<Ability>),
    /// Expertise with one of these tools (must be proficient).
    ToolExpertise(Vec<String>),
    Tools(Vec<String>),
    AnyTool,
    AnyArtisanTool,
    AnyInstrument,
    AnyGamingSet,
    Languages(Vec<String>),
    AnyLanguage,
    AnyStandardLanguage,
    AnyExoticLanguage,
    Weapons(Vec<String>),
    /// Ability bonuses: one pick per entry of `amounts`, all distinct.
    /// Pick `i` receives `amounts[i]`.
    Abilities {
        from: Vec<Ability>,
        amounts: Vec<i8>,
        /// Raised score maximum for the chosen abilities (epic boons: 30).
        #[serde(default)]
        max: Option<u8>,
    },
    /// Feats in any of these categories (empty: any feat).
    Feats { categories: Vec<String> },
    Spells(SpellFilter),
    OptionalFeatures { types: Vec<String> },
    Subclass { class: Key },
    /// A lineage/variant of a species.
    Lineage { species: Key },
    Resist(Vec<String>),
    /// Weapons whose mastery property you can use.
    WeaponMastery,
    /// Items matching a 5etools equipment type (`weaponMartial`, `toolArtisan`, ...).
    Items { equipment_type: String },
    /// Pick one named bundle of grants.
    Bundles(Vec<Bundle>),
    /// The union of several option sets.
    Mixed(Vec<Options>),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bundle {
    pub name: String,
    pub grants: Vec<Grant>,
    /// Lowest scope level (class level for class choices) at which this
    /// option may be picked; 0 for no restriction.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub min_level: u8,
}

impl Bundle {
    pub fn new(name: impl Into<String>, grants: Vec<Grant>) -> Self {
        Bundle { name: name.into(), grants, min_level: 0 }
    }
}

fn is_zero(n: &u8) -> bool {
    *n == 0
}

#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SpellFilter {
    #[serde(default)]
    pub levels: Vec<u8>,
    /// Highest spell level allowed (applied on top of `levels`).
    #[serde(default)]
    pub max_level: Option<u8>,
    /// Lowercase class names whose spell list the spell must be on.
    #[serde(default)]
    pub classes: Vec<String>,
    /// School abbreviations (A, C, D, E, V, I, N, T).
    #[serde(default)]
    pub schools: Vec<String>,
    #[serde(default)]
    pub ritual: bool,
    /// Spell attack kinds (`m`, `r`, `o`); non-empty: the spell must make a spell attack.
    #[serde(default)]
    pub spell_attack: Vec<String>,
    /// Allowed sources (empty: any).
    #[serde(default)]
    pub sources: Vec<String>,
    /// Excluded sources.
    #[serde(default)]
    pub exclude_sources: Vec<String>,
    /// Restrict to the spells selected for another choice (full choice id).
    #[serde(default)]
    pub within_choice: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpellGroup {
    pub name: Option<String>,
    pub ability: SpellAbility,
    pub spells: Vec<SpellGrant>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SpellAbility {
    /// Use the ability raised by the granting feat (Fey-Touched, Ritual
    /// Caster, ...), else the owning class's spellcasting ability.
    Inherit,
    Fixed(Ability),
    Choose(Vec<Ability>),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpellGrant {
    /// Scope level at which the spell is gained (0 or 1: immediately).
    pub level: u8,
    pub kind: SpellKind,
    pub pick: SpellPick,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpellKind {
    Known,
    /// Always prepared.
    Prepared,
    /// Castable without a slot, e.g. "1/long rest" or "at will".
    Innate(String),
    /// Added to the class spell list.
    Expanded,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SpellPick {
    Fixed(Key),
    /// Every spell matching the filter (used for expanded spell lists).
    All(SpellFilter),
    Choose { filter: SpellFilter, count: u8 },
}

pub fn title_case(s: &str) -> String {
    s.split(' ')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) if !matches!(w, "of" | "the" | "and") => f.to_uppercase().collect::<String>() + c.as_str(),
                Some(_) => w.to_string(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

impl Options {
    /// Short description of what can be picked, e.g. "skill", "origin feat".
    pub fn noun(&self) -> String {
        match self {
            Options::Skills(_) | Options::AnySkill => "skill".into(),
            Options::SkillExpertise | Options::ExpertiseFrom(_) => "skill expertise".into(),
            Options::Saves(_) => "saving throw".into(),
            Options::ToolExpertise(_) => "tool expertise".into(),
            Options::Tools(_) | Options::AnyTool => "tool".into(),
            Options::AnyArtisanTool => "artisan's tool".into(),
            Options::AnyInstrument => "musical instrument".into(),
            Options::AnyGamingSet => "gaming set".into(),
            Options::Languages(_) | Options::AnyLanguage | Options::AnyStandardLanguage | Options::AnyExoticLanguage => {
                "language".into()
            }
            Options::Weapons(_) => "weapon".into(),
            Options::Abilities { .. } => "ability score".into(),
            Options::Feats { categories } => match categories.first().map(String::as_str) {
                Some("O") => "origin feat".into(),
                Some("G") => "feat".into(),
                Some("FS") => "fighting style".into(),
                Some("EB") => "epic boon".into(),
                _ => "feat".into(),
            },
            Options::Spells(f) if f.levels == [0] => "cantrip".into(),
            Options::Spells(_) => "spell".into(),
            Options::OptionalFeatures { .. } => "option".into(),
            Options::Subclass { .. } => "subclass".into(),
            Options::Lineage { .. } => "lineage".into(),
            Options::Resist(_) => "damage resistance".into(),
            Options::WeaponMastery => "weapon mastery".into(),
            Options::Items { .. } => "item".into(),
            Options::Bundles(_) => "option".into(),
            Options::Mixed(_) => "proficiency".into(),
        }
    }
}

/// Human readable one-line summary of a list of grants.
pub fn describe(grants: &[Grant]) -> String {
    let parts: Vec<String> = grants.iter().filter_map(describe_one).collect();
    parts.join(", ")
}

fn describe_one(g: &Grant) -> Option<String> {
    Some(match g {
        Grant::Prof(p) => p.label(),
        Grant::Expertise(s) => format!("Expertise: {}", s.name()),
        Grant::Ability { ability, amount } => format!("{} {}", ability.abbr(), crate::core::signed(*amount as i32)),
        Grant::AbilityMax { ability, max } => format!("{} max {max}", ability.abbr()),
        Grant::Feat(k) => title_case(k.name_part()),
        Grant::Spells(g) => match &g.name {
            Some(n) => format!("{n} spells"),
            None => "Spells".into(),
        },
        Grant::Resist(d) => format!("{} resistance", title_case(d)),
        Grant::Immune(d) => format!("{} immunity", title_case(d)),
        Grant::CondImmune(d) => format!("Immune to {d}"),
        Grant::Darkvision(r) => format!("Darkvision {r} ft."),
        Grant::DarkvisionBonus { base, bonus } => format!("Darkvision {base} ft. (or +{bonus} ft.)"),
        Grant::Sense { sense, range } => format!("{} {range} ft.", title_case(sense)),
        Grant::Vulnerable(d) => format!("{} vulnerability", title_case(d)),
        Grant::SpeedBonus { mode, amount } => format!("{} speed +{amount} ft.", title_case(mode)),
        Grant::SpeedSet { mode, amount: Some(a) } => format!("{} speed {a} ft.", title_case(mode)),
        Grant::SpeedSet { mode, amount: None } => format!("{} speed equal to walking speed", title_case(mode)),
        Grant::HpPerClassLevel(n) => format!("+{n} HP per class level"),
        Grant::HpBonus(n) => format!("+{n} HP"),
        Grant::ToolExpertise(t) => format!("Expertise: {}", title_case(t)),
        Grant::NaturalArmor { base, abilities, .. } => {
            let mods: Vec<&str> = abilities.iter().map(|a| a.abbr()).collect();
            if mods.is_empty() { format!("Natural AC {base}") } else { format!("Unarmored AC {base} + {}", mods.join(" + ")) }
        }
        Grant::AcBonus { amount, .. } => format!("AC +{amount}"),
        Grant::HpPerLevel(n) => format!("+{n} HP per level"),
        Grant::Feature(f) => f.name.clone(),
        Grant::Item { item, qty } => {
            let n = match item {
                ItemRef::Key(k) => title_case(k.name_part()),
                ItemRef::Custom(s) => s.clone(),
            };
            if *qty > 1 {
                format!("{qty} × {n}")
            } else {
                n
            }
        }
        Grant::Coins(cp) => crate::core::format_coins(*cp),
        Grant::Choose(Choice { options: Options::Abilities { from, amounts, .. }, .. }) => {
            let amts: Vec<String> = amounts.iter().map(|a| crate::core::signed(*a as i32)).collect();
            let from: Vec<&str> = from.iter().map(|a| a.abbr()).collect();
            if from.len() == 6 {
                format!("{} to any abilities", amts.join("/"))
            } else {
                format!("{} among {}", amts.join("/"), from.join(", "))
            }
        }
        Grant::Choose(c) => {
            let n = c.count.at(1).max(1);
            format!("choose {n} {}", c.options.noun())
        }
        Grant::AtLevel { .. } => return None,
    })
}
