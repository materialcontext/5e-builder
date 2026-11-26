//! Enumerates the concrete options of a choice, with reasons for options
//! that can't be picked (already proficient, prerequisite unmet, ...).

use crate::build::{decode_prof, encode_prof, Build, ChoiceState};
use crate::character::Character;
use compendium::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq)]
pub struct OptionItem {
    pub value: String,
    pub label: String,
    /// Secondary text: source, school, summary.
    pub detail: String,
    /// Why this option can't be picked, if so.
    pub disabled: Option<String>,
    /// A non-blocking caution (e.g. the spell is already known elsewhere).
    pub warning: Option<String>,
    /// Longer description shown on demand.
    pub body: Vec<Entry>,
}

impl OptionItem {
    fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        OptionItem { value: value.into(), label: label.into(), detail: String::new(), disabled: None, warning: None, body: vec![] }
    }
    fn detail(mut self, d: impl Into<String>) -> Self {
        self.detail = d.into();
        self
    }
    fn body(mut self, b: Vec<Entry>) -> Self {
        self.body = b;
        self
    }
}

/// Lower is preferred when the same name exists in several sources.
pub fn source_rank(source: &str) -> u8 {
    match source {
        "XPHB" | "XDMG" | "XMM" => 0,
        "PHB" | "DMG" | "MM" => 2,
        _ => 1,
    }
}

/// Keeps one record per name (case-insensitive), preferring 2024 sources.
fn dedupe<'a, T: Record>(items: impl Iterator<Item = &'a T>) -> Vec<&'a T>
where
    T: 'a,
{
    let mut best: BTreeMap<String, &T> = BTreeMap::new();
    for it in items {
        let k = it.name().to_lowercase();
        match best.get(&k) {
            Some(cur) if source_rank(cur.source()) <= source_rank(it.source()) => {}
            _ => {
                best.insert(k, it);
            }
        }
    }
    best.into_values().collect()
}

pub fn options(choice: &ChoiceState, build: &Build, ch: &Character, comp: &Compendium) -> Vec<OptionItem> {
    let enabled = |src: &str| ch.settings.source_enabled(src);
    // Proficiencies granted elsewhere (not by this choice) can't be picked again.
    let own: BTreeSet<&str> = choice.selected.iter().map(String::as_str).collect();
    let have = |p: &Prof| -> bool {
        build.grants.iter().filter(|g| matches!(&g.grant, Grant::Prof(x) if x == p)).count() > usize::from(own.contains(encode_prof(p).as_str()))
    };
    let prof_item = |p: Prof, label: String| {
        let mut o = OptionItem::new(encode_prof(&p), label);
        if have(&p) {
            o.disabled = Some("Already proficient".into());
        }
        o
    };

    let mut out: Vec<OptionItem> = match &choice.options {
        Options::Skills(list) => list.iter().map(|s| prof_item(Prof::Skill(*s), s.name().into()).detail(s.ability().abbr())).collect(),
        Options::AnySkill => Skill::ALL.iter().map(|s| prof_item(Prof::Skill(*s), s.name().into()).detail(s.ability().abbr())).collect(),
        Options::SkillExpertise | Options::ExpertiseFrom(_) => {
            let allowed: Vec<Skill> = match &choice.options {
                Options::ExpertiseFrom(list) if !list.is_empty() => list.clone(),
                _ => Skill::ALL.to_vec(),
            };
            let expert: BTreeSet<Skill> = build
                .grants
                .iter()
                .filter_map(|g| match g.grant {
                    Grant::Expertise(s) => Some(s),
                    _ => None,
                })
                .collect();
            allowed
                .iter()
                .map(|s| {
                    let mut o = OptionItem::new(encode_prof(&Prof::Skill(*s)), s.name()).detail(s.ability().abbr());
                    let own_pick = own.contains(o.value.as_str());
                    if !build.has_prof(&Prof::Skill(*s)) {
                        o.disabled = Some("Requires proficiency".into());
                    } else if expert.contains(s) && !own_pick {
                        o.disabled = Some("Already has expertise".into());
                    }
                    o
                })
                .collect()
        }
        Options::Saves(list) => list.iter().map(|a| prof_item(Prof::Save(*a), format!("{} saving throws", a.name()))).collect(),
        Options::Tools(list) => list.iter().map(|t| prof_item(Prof::Tool(t.clone()), title_case(t))).collect(),
        Options::AnyTool => tool_names(comp, &enabled, &[]).into_iter().map(|t| prof_item(Prof::Tool(t.clone()), title_case(&t))).collect(),
        Options::AnyArtisanTool => tool_names(comp, &enabled, &["toolArtisan"]).into_iter().map(|t| prof_item(Prof::Tool(t.clone()), title_case(&t))).collect(),
        Options::AnyInstrument => {
            tool_names(comp, &enabled, &["instrumentMusical"]).into_iter().map(|t| prof_item(Prof::Tool(t.clone()), title_case(&t))).collect()
        }
        Options::AnyGamingSet => tool_names(comp, &enabled, &["setGaming"]).into_iter().map(|t| prof_item(Prof::Tool(t.clone()), title_case(&t))).collect(),
        Options::Languages(list) => list.iter().map(|l| prof_item(Prof::Language(l.clone()), l.clone())).collect(),
        Options::AnyLanguage | Options::AnyStandardLanguage | Options::AnyExoticLanguage => {
            let kinds: &[&str] = match &choice.options {
                Options::AnyStandardLanguage => &["standard"],
                Options::AnyExoticLanguage => &["exotic", "rare"],
                _ => &["standard", "exotic", "rare"],
            };
            // A language's kind comes from its most authoritative source (2024 first).
            let langs: Vec<&Language> = comp.languages.iter().filter(|l| enabled(&l.source)).collect();
            let mut names: BTreeMap<String, String> = BTreeMap::new();
            for l in dedupe(langs.into_iter()).into_iter().filter(|l| kinds.contains(&l.kind.as_str())) {
                names.insert(l.name.to_lowercase(), l.kind.clone());
            }
            if names.is_empty() {
                for n in ["Common", "Dwarvish", "Elvish", "Giant", "Gnomish", "Goblin", "Halfling", "Orc"] {
                    names.insert(n.to_lowercase(), "standard".into());
                }
            }
            names
                .into_iter()
                .map(|(n, kind)| prof_item(Prof::Language(title_case(&n)), title_case(&n)).detail(title_case(&kind)))
                .collect()
        }
        Options::Weapons(list) => list.iter().map(|w| prof_item(Prof::Weapon(w.clone()), title_case(w))).collect(),
        Options::Abilities { from, .. } => from
            .iter()
            .map(|a| OptionItem::new(a.id(), a.name()).detail(format!("{} {}", build.score(*a), signed(build.modifier(*a)))))
            .collect(),
        Options::Feats { categories } => feat_options(categories, build, ch, comp),
        Options::Spells(filter) => spell_options(filter, choice, build, ch, comp),
        Options::OptionalFeatures { types } => {
            let taken: BTreeSet<&Key> = build.optional_features.iter().map(|(k, _)| k).collect();
            let items: Vec<&OptionalFeature> =
                comp.optional_features.iter().filter(|o| o.types.iter().any(|t| types.contains(t)) && enabled(&o.source)).collect();
            dedupe(items.into_iter())
                .into_iter()
                .map(|o| {
                    let mut it = OptionItem::new(o.key.0.clone(), o.name.clone())
                        .detail(o.prereq.as_ref().map(|p| p.text.clone()).unwrap_or_else(|| comp.source_name(&o.source)))
                        .body(o.body.clone());
                    if taken.contains(&o.key) && !own.contains(o.key.0.as_str()) {
                        it.disabled = Some("Already chosen".into());
                    } else if let Some(p) = &o.prereq {
                        it.disabled = prereq_problem_in(p, build, ch, Some(comp));
                    }
                    it
                })
                .collect()
        }
        Options::Subclass { class } => comp
            .subclasses_of(class)
            .filter(|s| enabled(&s.source))
            .map(|s| {
                let summary: Vec<&str> = s.features.iter().map(|f| f.name.as_str()).take(4).collect();
                OptionItem::new(s.key.0.clone(), s.name.clone())
                    .detail(comp.source_name(&s.source))
                    .body(vec![Entry::P(summary.join(", "))])
            })
            .collect(),
        Options::Lineage { species } => comp
            .species
            .get(species)
            .map(|sp| {
                sp.lineages
                    .iter()
                    .filter(|l| enabled(&l.source))
                    .map(|l| {
                        let body = l.features.iter().map(|f| Entry::Section { name: Some(f.name.clone()), body: f.body.clone() }).collect();
                        OptionItem::new(l.key.0.clone(), l.name.clone()).detail(comp.source_name(&l.source)).body(body)
                    })
                    .collect()
            })
            .unwrap_or_default(),
        Options::Resist(list) => list.iter().map(|d| OptionItem::new(d.clone(), title_case(d))).collect(),
        Options::WeaponMastery => {
            // Ordinary weapons only: no magic items, no futuristic firearms.
            let weapons: Vec<&Item> = comp
                .items
                .iter()
                .filter(|i| i.rarity.is_none() && enabled(&i.source))
                .filter(|i| i.weapon.as_ref().is_some_and(|w| w.mastery.is_some() && !w.properties.iter().any(|p| p.contains("futuristic") || p == "reload")))
                .collect();
            dedupe(weapons.into_iter())
                .into_iter()
                .map(|i| {
                    let w = i.weapon.as_ref().unwrap();
                    OptionItem::new(i.key.0.clone(), i.name.clone()).detail(format!(
                        "{} · {}",
                        w.mastery.clone().unwrap_or_default(),
                        if w.martial { "Martial" } else { "Simple" }
                    ))
                })
                .collect()
        }
        Options::Items { equipment_type } => {
            let items: Vec<&Item> =
                comp.items.iter().filter(|i| i.rarity.is_none() && i.tags.contains(equipment_type) && enabled(&i.source)).collect();
            dedupe(items.into_iter()).into_iter().map(|i| OptionItem::new(i.key.0.clone(), i.name.clone()).detail(i.kind.clone())).collect()
        }
        Options::Bundles(bundles) => bundles
            .iter()
            .map(|b| {
                let mut o = OptionItem::new(b.name.clone(), b.name.clone());
                let feats: Vec<Entry> = b
                    .grants
                    .iter()
                    .filter_map(|g| match g {
                        Grant::Feature(f) => Some(Entry::Section { name: Some(f.name.clone()), body: f.body.clone() }),
                        _ => None,
                    })
                    .collect();
                let summary = describe(&b.grants);
                if summary != b.name && !summary.is_empty() {
                    o.detail = summary;
                }
                o.body = feats;
                o
            })
            .collect(),
        Options::Mixed(parts) => parts
            .iter()
            .flat_map(|p| {
                let sub = ChoiceState { options: p.clone(), ..choice.clone() };
                options(&sub, build, ch, comp)
            })
            .collect(),
    };
    // Keep the current selections visible even if their source was disabled.
    for v in &choice.selected {
        if !out.iter().any(|o| &o.value == v) {
            let label = decode_prof(v).map(|p| p.label()).unwrap_or_else(|| comp.display_name(&Key(v.clone())));
            let mut o = OptionItem::new(v.clone(), label);
            o.disabled = Some("Not available with the current sources".into());
            out.push(o);
        }
    }
    out
}

fn tool_names(comp: &Compendium, enabled: &dyn Fn(&str) -> bool, tags: &[&str]) -> Vec<String> {
    let mut names = BTreeSet::new();
    for i in comp.items.iter().filter(|i| i.rarity.is_none() && enabled(&i.source)) {
        let is_tool = matches!(i.kind.as_str(), "Artisan's Tools" | "Tools" | "Gaming Set" | "Musical Instrument");
        if is_tool && (tags.is_empty() || tags.iter().any(|t| i.tags.iter().any(|x| x == t))) {
            names.insert(i.name.to_lowercase());
        }
    }
    names.into_iter().collect()
}

pub fn prereq_problem(p: &Prereq, build: &Build, ch: &Character) -> Option<String> {
    prereq_problem_in(p, build, ch, None)
}

/// Checks a prerequisite; `comp` enables name-based checks (species, spells, classes).
pub fn prereq_problem_in(p: &Prereq, build: &Build, ch: &Character, comp: Option<&Compendium>) -> Option<String> {
    if let Some(l) = p.level {
        match (&p.level_class, comp) {
            (Some(class), Some(comp)) => {
                let have = build
                    .classes
                    .iter()
                    .filter(|c| comp.classes.get(&c.key).is_some_and(|x| x.name.to_lowercase() == *class))
                    .map(|c| c.level)
                    .max()
                    .unwrap_or(0);
                if have < l {
                    return Some(format!("Requires {} level {l}", title_case(class)));
                }
            }
            _ => {
                if ch.total_level() < l {
                    return Some(format!("Requires level {l}"));
                }
            }
        }
    }
    if !p.feats.is_empty() && !p.feats.iter().any(|f| build.has_feat_named(f)) {
        return Some(format!("Requires {}", p.text));
    }
    if !p.features.is_empty() {
        let have = |n: &str| {
            build.features.iter().any(|f| f.feature.name.to_lowercase() == n || f.feature.name.to_lowercase().ends_with(&format!(": {n}")))
        };
        if !p.features.iter().any(|f| have(f)) {
            return Some(format!("Requires {}", p.text));
        }
    }
    if let Some(comp) = comp {
        if !p.species.is_empty() {
            let sp = ch.species.as_ref().and_then(|k| comp.species.get(k)).map(|s| s.name.to_lowercase()).unwrap_or_default();
            if !p.species.iter().any(|x| sp == *x || sp.starts_with(&format!("{x} ")) || x.starts_with(&sp)) {
                return Some(format!("Requires {}", p.text));
            }
        }
        if !p.spells.is_empty() {
            let known: Vec<String> = build.spells.iter().filter_map(|s| comp.spells.get(&s.key)).map(|s| s.name.to_lowercase()).collect();
            if !p.spells.iter().any(|x| known.contains(x)) {
                return Some(format!("Requires {}", p.text));
            }
        }
    }
    for need in &p.proficiencies {
        let ok = build.profs().any(|x| match x {
            Prof::Armor(a) | Prof::Weapon(a) => a.to_lowercase() == *need,
            _ => false,
        });
        if !ok {
            return Some(format!("Requires {need} proficiency"));
        }
    }
    if !p.abilities.is_empty() && !p.abilities.iter().any(|(a, min)| build.score(*a) >= *min as i32) {
        return Some(format!("Requires {}", p.text));
    }
    if p.spellcasting && build.spells.is_empty() {
        return Some("Requires the ability to cast spells".into());
    }
    None
}

fn feat_options(categories: &[String], build: &Build, ch: &Character, comp: &Compendium) -> Vec<OptionItem> {
    let taken: BTreeSet<&Key> = build.feats.iter().map(|(k, _)| k).collect();
    let feats: Vec<&Feat> = comp
        .feats
        .iter()
        .filter(|f| f.version_of.is_none() && ch.settings.source_enabled(&f.source))
        .filter(|f| {
            categories.is_empty()
                || categories.iter().any(|c| match &f.category {
                    Some(fc) => fc == c || fc.split(':').next() == Some(c.as_str()),
                    None => c.is_empty(),
                })
        })
        .collect();
    let mut out: Vec<OptionItem> = dedupe(feats.into_iter())
        .into_iter()
        .map(|f| {
            let detail = match &f.prereq {
                Some(p) if !p.text.is_empty() => format!("{} · {}", f.category_label(), p.text),
                _ => format!("{} · {}", f.category_label(), f.source),
            };
            let mut o = OptionItem::new(f.key.0.clone(), f.name.clone()).detail(detail).body(f.body.clone());
            if taken.contains(&f.key) && !f.repeatable {
                o.disabled = Some("Already taken".into());
            } else if let Some(p) = &f.prereq {
                o.disabled = prereq_problem_in(p, build, ch, Some(comp));
            }
            o
        })
        .collect();
    // Feats you took through this very choice stay selectable.
    out.sort_by(|a, b| a.label.cmp(&b.label));
    out
}

fn spell_options(filter: &SpellFilter, choice: &ChoiceState, build: &Build, ch: &Character, comp: &Compendium) -> Vec<OptionItem> {
    let within: Option<BTreeSet<&str>> = filter
        .within_choice
        .as_ref()
        .and_then(|p| build.choice(p))
        .map(|c| c.selected.iter().map(String::as_str).collect());
    // Spells the character already has from another choice or feature.
    let others: Vec<&crate::build::KnownSpell> = build
        .spells
        .iter()
        .filter(|s| !choice.selected.contains(&s.key.0))
        .filter(|s| filter.within_choice.is_none() || !matches!(s.mode, crate::build::SpellMode::Spellbook))
        .collect();
    // Spells added to this class's list by subclasses or features.
    let expansions: Vec<&crate::build::Expansion> = build
        .expanded
        .iter()
        .filter(|(class, _)| choice.class.is_some() && (class.is_none() || class == &choice.class))
        .map(|(_, e)| e)
        .collect();
    let class_list_choice = !filter.classes.is_empty() && choice.class.is_some();
    let expanded = |s: &Spell| {
        class_list_choice
            && expansions.iter().any(|e| match e {
                crate::build::Expansion::Spell(k) => k == &s.key || k.name_part() == s.name.to_lowercase(),
                crate::build::Expansion::Filter(f) => {
                    (f.levels.is_empty() || f.levels.contains(&s.level))
                        && (f.classes.is_empty() || f.classes.iter().any(|c| s.classes.contains(c)))
                        && (f.schools.is_empty() || f.schools.contains(&s.school))
                }
            })
    };
    let spells: Vec<&Spell> = comp
        .spells
        .iter()
        .filter(|s| ch.settings.source_enabled(&s.source))
        .filter(|s| filter.levels.is_empty() || filter.levels.contains(&s.level))
        .filter(|s| filter.max_level.is_none_or(|m| s.level <= m))
        .filter(|s| filter.classes.is_empty() || filter.classes.iter().any(|c| s.classes.contains(c)) || expanded(s))
        .filter(|s| filter.schools.is_empty() || filter.schools.contains(&s.school))
        .filter(|s| !filter.ritual || s.ritual)
        .filter(|s| within.as_ref().is_none_or(|w| w.contains(s.key.0.as_str())))
        .collect();
    let mut out: Vec<OptionItem> = dedupe(spells.into_iter())
        .into_iter()
        .map(|s| {
            let lvl = if s.level == 0 { "Cantrip".to_string() } else { format!("Level {}", s.level) };
            let mut o = OptionItem::new(s.key.0.clone(), s.name.clone())
                .detail(format!("{lvl} · {}{}", s.school_name(), if s.ritual { " · Ritual" } else { "" }))
                .body(s.body.clone());
            if let Some(k) = others.iter().find(|k| k.key == s.key) {
                if k.class.is_some() && k.class == choice.class {
                    o.disabled = Some("Already known".into());
                } else {
                    o.warning = Some(format!("Already known ({})", k.origin));
                }
            }
            o
        })
        .collect();
    out.sort_by(|a, b| {
        let la = comp.spells.get(&Key(a.value.clone())).map(|s| s.level).unwrap_or(0);
        let lb = comp.spells.get(&Key(b.value.clone())).map(|s| s.level).unwrap_or(0);
        (la, &a.label).cmp(&(lb, &b.label))
    });
    out
}
