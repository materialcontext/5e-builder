//! Small vocabulary types shared by every part of the model.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Stable identifier for any piece of content: lowercase `name|source`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct Key(pub String);

impl Key {
    pub fn new(name: &str, source: &str) -> Self {
        Key(format!("{}|{}", name.trim().to_lowercase(), source.trim().to_lowercase()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn name_part(&self) -> &str {
        self.0.split('|').next().unwrap_or("")
    }
    pub fn source_part(&self) -> &str {
        self.0.split('|').nth(1).unwrap_or("")
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Ability {
    Str,
    Dex,
    Con,
    Int,
    Wis,
    Cha,
}

impl Ability {
    pub const ALL: [Ability; 6] = [Ability::Str, Ability::Dex, Ability::Con, Ability::Int, Ability::Wis, Ability::Cha];

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.trim().to_lowercase().as_str() {
            "str" | "strength" => Ability::Str,
            "dex" | "dexterity" => Ability::Dex,
            "con" | "constitution" => Ability::Con,
            "int" | "intelligence" => Ability::Int,
            "wis" | "wisdom" => Ability::Wis,
            "cha" | "charisma" => Ability::Cha,
            _ => return None,
        })
    }
    pub fn abbr(self) -> &'static str {
        match self {
            Ability::Str => "STR",
            Ability::Dex => "DEX",
            Ability::Con => "CON",
            Ability::Int => "INT",
            Ability::Wis => "WIS",
            Ability::Cha => "CHA",
        }
    }
    pub fn id(self) -> &'static str {
        match self {
            Ability::Str => "str",
            Ability::Dex => "dex",
            Ability::Con => "con",
            Ability::Int => "int",
            Ability::Wis => "wis",
            Ability::Cha => "cha",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Ability::Str => "Strength",
            Ability::Dex => "Dexterity",
            Ability::Con => "Constitution",
            Ability::Int => "Intelligence",
            Ability::Wis => "Wisdom",
            Ability::Cha => "Charisma",
        }
    }
    pub fn index(self) -> usize {
        self as usize
    }
}

macro_rules! skills {
    ($($v:ident => $id:literal, $name:literal, $ab:ident;)*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        pub enum Skill { $($v,)* }
        impl Skill {
            pub const ALL: &'static [Skill] = &[$(Skill::$v,)*];
            /// 5etools id (lowercase, spaced).
            pub fn id(self) -> &'static str { match self { $(Skill::$v => $id,)* } }
            pub fn name(self) -> &'static str { match self { $(Skill::$v => $name,)* } }
            pub fn ability(self) -> Ability { match self { $(Skill::$v => Ability::$ab,)* } }
            pub fn parse(s: &str) -> Option<Self> {
                let s = s.trim().to_lowercase();
                $(if s == $id { return Some(Skill::$v); })*
                None
            }
        }
    };
}

skills! {
    Acrobatics => "acrobatics", "Acrobatics", Dex;
    AnimalHandling => "animal handling", "Animal Handling", Wis;
    Arcana => "arcana", "Arcana", Int;
    Athletics => "athletics", "Athletics", Str;
    Deception => "deception", "Deception", Cha;
    History => "history", "History", Int;
    Insight => "insight", "Insight", Wis;
    Intimidation => "intimidation", "Intimidation", Cha;
    Investigation => "investigation", "Investigation", Int;
    Medicine => "medicine", "Medicine", Wis;
    Nature => "nature", "Nature", Int;
    Perception => "perception", "Perception", Wis;
    Performance => "performance", "Performance", Cha;
    Persuasion => "persuasion", "Persuasion", Cha;
    Religion => "religion", "Religion", Int;
    SleightOfHand => "sleight of hand", "Sleight of Hand", Dex;
    Stealth => "stealth", "Stealth", Dex;
    Survival => "survival", "Survival", Wis;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Size {
    Tiny,
    Small,
    Medium,
    Large,
    Huge,
    Gargantuan,
}

impl Size {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "T" => Size::Tiny,
            "S" => Size::Small,
            "M" => Size::Medium,
            "L" => Size::Large,
            "H" => Size::Huge,
            "G" => Size::Gargantuan,
            _ => return None,
        })
    }
    pub fn name(self) -> &'static str {
        match self {
            Size::Tiny => "Tiny",
            Size::Small => "Small",
            Size::Medium => "Medium",
            Size::Large => "Large",
            Size::Huge => "Huge",
            Size::Gargantuan => "Gargantuan",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Speed {
    pub walk: u16,
    #[serde(default)]
    pub fly: u16,
    #[serde(default)]
    pub swim: u16,
    #[serde(default)]
    pub climb: u16,
    #[serde(default)]
    pub burrow: u16,
}

/// Formats a signed modifier as `+2` / `-1`.
pub fn signed(n: i32) -> String {
    if n >= 0 {
        format!("+{n}")
    } else {
        n.to_string()
    }
}

/// Ability modifier for a score.
pub fn modifier(score: i32) -> i32 {
    (score - 10).div_euclid(2)
}

/// Formats copper pieces as the largest sensible coin, e.g. `15 GP`, `5 SP`.
pub fn format_coins(cp: u32) -> String {
    if cp >= 100 && cp % 100 == 0 {
        format!("{} GP", cp / 100)
    } else if cp >= 10 && cp % 10 == 0 {
        format!("{} SP", cp / 10)
    } else {
        format!("{cp} CP")
    }
}
