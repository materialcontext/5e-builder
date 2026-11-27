//! Full descriptions for anything a player can choose, so every option can
//! be expanded to show all the information behind it: skills, saving throws,
//! tools, languages, weapons and masteries, abilities, damage types, items,
//! equipment packages, subclasses, classes, species and backgrounds.

use crate::build::decode_prof;
use compendium::*;

fn p(s: impl Into<String>) -> Entry {
    Entry::P(s.into())
}

fn section(name: &str, body: Vec<Entry>) -> Entry {
    Entry::Section { name: Some(name.to_string()), body }
}

/// What each ability measures (2024 Player's Handbook, chapter 1).
pub fn ability(a: Ability) -> Vec<Entry> {
    let (measures, used) = match a {
        Ability::Str => ("Physical might.", "Athletics checks, melee attacks with most weapons, carrying capacity, and Strength saving throws against being shoved or restrained."),
        Ability::Dex => ("Agility, reflexes, and balance.", "Acrobatics, Sleight of Hand and Stealth checks, ranged and Finesse weapon attacks, Initiative, Armor Class in light or medium armor, and Dexterity saving throws against area effects."),
        Ability::Con => ("Health and stamina.", "Hit points (your Constitution modifier is added for every level), Concentration on spells, and Constitution saving throws against poison and exhaustion."),
        Ability::Int => ("Reasoning and memory.", "Arcana, History, Investigation, Nature and Religion checks, and the spellcasting of Wizards and some subclasses."),
        Ability::Wis => ("Perceptiveness and mental fortitude.", "Animal Handling, Insight, Medicine, Perception and Survival checks, Passive Perception, and the spellcasting of Clerics, Druids and Rangers."),
        Ability::Cha => ("Confidence, poise, and charm.", "Deception, Intimidation, Performance and Persuasion checks, and the spellcasting of Bards, Paladins, Sorcerers and Warlocks."),
    };
    vec![p(format!("{} measures: {measures}", a.name())), p(format!("Used for: {used}"))]
}

pub fn skill(comp: &Compendium, s: Skill) -> Vec<Entry> {
    let mut out = vec![p(format!("{} skill — checks use your {} modifier.", s.name(), s.ability().name()))];
    if let Some(r) = comp.rule("skill", s.name()) {
        out.extend(r.body.clone());
    }
    out
}

pub fn save(a: Ability) -> Vec<Entry> {
    let mut out = vec![p(format!(
        "Proficiency in {} saving throws: add your Proficiency Bonus whenever you make a {} saving throw.",
        a.name(),
        a.name()
    ))];
    out.extend(ability(a));
    out
}

pub fn language(comp: &Compendium, name: &str) -> Vec<Entry> {
    let mut out = Vec::new();
    if let Some(l) = comp.language(name) {
        out.push(p(format!(
            "{} — a {} language from {}.",
            l.name,
            if l.kind == "other" { "rare".to_string() } else { l.kind.clone() },
            comp.source_name(&l.source)
        )));
        out.extend(l.body.clone());
        out.push(p("Knowing a language means you can speak, read and write it."));
    } else {
        out.push(p(format!("You can speak, read and write {}.", title_case(name))));
    }
    out
}

fn money(cp: u32) -> String {
    format_coins(cp)
}

/// Stats and rules text for an item; weapons include their properties and
/// mastery rules, armor its AC and requirements.
pub fn item(comp: &Compendium, it: &Item) -> Vec<Entry> {
    let mut facts = vec![it.kind.clone()];
    if let Some(v) = it.value_cp {
        facts.push(money(v));
    }
    if it.weight > 0.0 {
        facts.push(format!("{} lb.", it.weight));
    }
    let mut out = vec![p(facts.into_iter().filter(|f| !f.is_empty()).collect::<Vec<_>>().join(" · "))];
    if let Some(w) = &it.weapon {
        let mut line = format!(
            "{} {} weapon. Damage: {} {}",
            if w.martial { "Martial" } else { "Simple" },
            if w.ranged { "ranged" } else { "melee" },
            w.damage,
            w.damage_type
        );
        if let Some(v) = &w.versatile {
            line.push_str(&format!(" (versatile {v})"));
        }
        if let Some(r) = &w.range {
            line.push_str(&format!(", range {r} ft."));
        }
        out.push(p(format!("{line}.")));
        let props: Vec<Entry> = w
            .properties
            .iter()
            .filter_map(|prop| {
                let r = comp.rule("property", prop).or_else(|| comp.rule("property-abbr", prop))?;
                Some(section(&r.name, r.body.clone()))
            })
            .collect();
        if !w.properties.is_empty() {
            out.push(p(format!("Properties: {}.", w.properties.iter().map(|x| title_case(x)).collect::<Vec<_>>().join(", "))));
        }
        out.extend(props);
        if let Some(m) = &w.mastery {
            let body = comp.rule("mastery", m).map(|r| r.body.clone()).unwrap_or_default();
            out.push(section(&format!("Mastery: {m}"), body));
        }
    }
    if let Some(a) = &it.armor {
        let mut line = match a.kind {
            ArmorKind::Shield => format!("Shield: +{} Armor Class.", a.ac),
            ArmorKind::Light => format!("Light armor: AC {} + Dexterity modifier.", a.ac),
            ArmorKind::Medium => format!("Medium armor: AC {} + Dexterity modifier (max 2).", a.ac),
            ArmorKind::Heavy => format!("Heavy armor: AC {}.", a.ac),
        };
        if let Some(s) = a.strength {
            line.push_str(&format!(" Requires Strength {s}, or your Speed drops by 10 feet."));
        }
        if a.stealth_disadvantage {
            line.push_str(" Disadvantage on Dexterity (Stealth) checks.");
        }
        out.push(p(line));
    }
    if !it.contents.is_empty() {
        let list: Vec<String> = it
            .contents
            .iter()
            .map(|(k, n)| {
                let name = comp.items.get(k).map(|i| i.name.clone()).unwrap_or_else(|| title_case(k.name_part()));
                if *n > 1 { format!("{name} ×{n}") } else { name }
            })
            .collect();
        out.push(p(format!("Contains: {}.", list.join(", "))));
    }
    out.extend(it.body.clone());
    out
}

/// A tool by name (from a proficiency), with its 2024 rules text.
pub fn tool(comp: &Compendium, name: &str) -> Vec<Entry> {
    match comp.base_item(name).or_else(|| comp.base_item(&title_case(name))) {
        Some(it) => item(comp, it),
        None => vec![p(format!("Proficiency with {}: add your Proficiency Bonus to ability checks you make using it.", title_case(name)))],
    }
}

/// A weapon or armor proficiency: a category ("martial") or a specific item.
pub fn weapon_or_armor(comp: &Compendium, name: &str, armor: bool) -> Vec<Entry> {
    let lower = name.to_lowercase();
    if armor {
        let text = match lower.as_str() {
            "light" => "Light armor training: wear Padded, Leather and Studded Leather armor without penalty.",
            "medium" => "Medium armor training: wear Hide, Chain Shirt, Scale Mail, Breastplate and Half Plate armor without penalty.",
            "heavy" => "Heavy armor training: wear Ring Mail, Chain Mail, Splint and Plate armor without penalty.",
            "shield" | "shields" => "Shield training: gain the Shield's Armor Class bonus.",
            _ => "",
        };
        if !text.is_empty() {
            return vec![p(text), p("Without training, armor gives Disadvantage on any check, save or attack roll using Strength or Dexterity, and you can't cast spells.")];
        }
    } else {
        let category = match lower.as_str() {
            "simple" => Some(false),
            "martial" => Some(true),
            _ => None,
        };
        if let Some(martial) = category {
            let mut names: Vec<String> = comp
                .items
                .iter()
                .filter(|i| i.rarity.is_none() && i.source == "XPHB" && i.weapon.as_ref().is_some_and(|w| w.martial == martial))
                .map(|i| i.name.clone())
                .collect();
            names.sort();
            names.dedup();
            return vec![
                p(format!("Proficiency with all {} weapons: add your Proficiency Bonus to attack rolls with them.", if martial { "Martial" } else { "Simple" })),
                p(format!("Includes: {}.", names.join(", "))),
            ];
        }
    }
    match comp.base_item(name).or_else(|| comp.base_item(&title_case(name))) {
        Some(it) => item(comp, it),
        None => vec![p(format!("Proficiency with {}.", title_case(name)))],
    }
}

pub fn damage_type(name: &str) -> Vec<Entry> {
    let t = title_case(name);
    let what = match name.to_lowercase().as_str() {
        "acid" => "Corrosive liquids and digestive enzymes.",
        "bludgeoning" => "Blunt force: hammers, falling, constriction.",
        "cold" => "Freezing water and icy blasts.",
        "fire" => "Flames and unbearable heat.",
        "force" => "Pure magical energy.",
        "lightning" => "Electricity.",
        "necrotic" => "Life-draining energy.",
        "piercing" => "Puncturing and impaling: spears, bites.",
        "poison" => "Toxic gas and venom.",
        "psychic" => "Mind-rending energy.",
        "radiant" => "Holy energy and searing light.",
        "slashing" => "Cutting: swords, axes, claws.",
        "thunder" => "Concussive sound.",
        _ => "",
    };
    let mut out = vec![p(format!("Resistance to {t} damage: you take half damage of this type."))];
    if !what.is_empty() {
        out.push(p(format!("{t} damage: {what}")));
    }
    out
}

/// What a creature size means for a character (2024 rules).
pub fn size(name: &str) -> Option<Vec<Entry>> {
    let text = match name {
        "Small" => "Small: you occupy a 5-foot space. You have Disadvantage on attack rolls with Heavy weapons, and you can move through the space of a creature at least two sizes larger.",
        "Medium" => "Medium: you occupy a 5-foot space. This is the usual size for adventurers; no special rules apply.",
        "Tiny" => "Tiny: you occupy a 2½-foot space and can't wield Heavy weapons effectively.",
        "Large" => "Large: you occupy a 10-foot space.",
        _ => return None,
    };
    Some(vec![p(text)])
}

/// Every feature of a subclass with the class level it arrives at.
pub fn subclass(comp: &Compendium, s: &Subclass) -> Vec<Entry> {
    let mut out = vec![p(format!("{} · {}", s.name, comp.source_name(&s.source)))];
    let sub_level = comp.classes.get(&s.class).and_then(|c| c.subclass_level()).unwrap_or(1);
    for f in &s.features {
        out.push(section(&format!("Level {} — {}", f.level.max(sub_level), f.name), f.body.clone()));
    }
    out
}

/// The grants of a bundle (equipment package, size, option) in words, with
/// item details.
pub fn grants(comp: &Compendium, grants: &[Grant]) -> Vec<Entry> {
    let mut out = Vec::new();
    let mut items = Vec::new();
    for g in grants {
        match g {
            Grant::Feature(f) => out.push(section(&f.name, f.body.clone())),
            Grant::Item { item: what, qty } => {
                let (name, detail) = match what {
                    ItemRef::Key(k) => match comp.items.get(k) {
                        Some(it) => (it.name.clone(), Some(it)),
                        None => (title_case(k.name_part()), None),
                    },
                    ItemRef::Custom(s) => (s.clone(), None),
                };
                let label = if *qty > 1 { format!("{name} ×{qty}") } else { name };
                match detail {
                    Some(it) => items.push(section(&label, item(comp, it))),
                    None => items.push(p(label)),
                }
            }
            Grant::Coins(cp) => items.push(p(format!("{} in coins.", money(*cp)))),
            Grant::Feat(k) => {
                if let Some(f) = comp.feats.get(k) {
                    out.push(section(&format!("Feat: {}", f.name), f.body.clone()));
                }
            }
            Grant::Choose(c) if matches!(c.options, Options::Abilities { .. }) => {
                if let Options::Abilities { from, amounts, max } = &c.options {
                    let amounts: Vec<String> = amounts.iter().map(|a| signed(*a as i32)).collect();
                    let from: Vec<&str> = from.iter().map(|a| a.name()).collect();
                    let each = if amounts.len() == 1 { format!("{} to one ability", amounts[0]) } else { format!("{} to different abilities", amounts.join(" and ")) };
                    out.push(p(format!(
                        "Ability score increase: {each}, chosen from {}. A score can't go above {} this way.",
                        if from.len() == 6 { "any of your abilities".to_string() } else { from.join(", ") },
                        max.unwrap_or(20)
                    )));
                }
            }
            other => {
                let d = describe(std::slice::from_ref(other));
                if !d.is_empty() {
                    out.push(p(d));
                }
            }
        }
    }
    out.extend(items);
    out
}

/// Fills in a description for an option that has none, from what its value
/// refers to.
pub fn option(o: &mut crate::options::OptionItem, options: &Options, comp: &Compendium) {
    if !o.body.is_empty() {
        return;
    }
    let body = match options {
        Options::Abilities { .. } => Ability::parse(&o.value).map(ability),
        Options::Resist(_) => Some(damage_type(&o.value)),
        Options::Subclass { .. } => comp.subclasses.get(&Key(o.value.clone())).map(|s| subclass(comp, s)),
        Options::WeaponMastery | Options::Items { .. } => comp.items.get(&Key(o.value.clone())).map(|it| item(comp, it)),
        Options::Bundles(bs) => bs.iter().find(|b| b.name == o.value).map(|b| grants(comp, &b.grants)).filter(|b| !b.is_empty()).or_else(|| size(&o.value)),
        Options::Lineage { .. } => Some(vec![p("The standard version of this species, with no additional lineage traits.")]),
        Options::SkillExpertise | Options::ExpertiseFrom(_) => Skill::parse(o.value.trim_start_matches("skill:")).map(|s| {
            let mut v = vec![p("Expertise: your Proficiency Bonus is doubled for checks with this skill.")];
            v.extend(skill(comp, s));
            v
        }),
        Options::ToolExpertise(_) => Some({
            let mut v = vec![p("Expertise: your Proficiency Bonus is doubled for checks with this tool.")];
            v.extend(tool(comp, o.value.trim_start_matches("tool:")));
            v
        }),
        _ => decode_prof(&o.value).map(|pr| match pr {
            Prof::Skill(s) => skill(comp, s),
            Prof::Save(a) => save(a),
            Prof::Tool(t) => tool(comp, &t),
            Prof::Language(l) => language(comp, &l),
            Prof::Weapon(w) => weapon_or_armor(comp, &w, false),
            Prof::Armor(a) => weapon_or_armor(comp, &a, true),
        }),
    };
    if let Some(b) = body.filter(|b| !b.is_empty()) {
        o.body = b;
    } else {
        // Last resort: never leave an option without anything to read.
        o.body = vec![p(if o.detail.is_empty() { o.label.clone() } else { format!("{} — {}", o.label, o.detail) })];
    }
}
