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
    /// Filter group for long lists (spell level, feat category, ...).
    pub group: Option<String>,
    /// Set for an older version that was reprinted (and the reprint is
    /// available): shown folded away under "older versions".
    pub older: Option<String>,
    /// Filter tags as (facet, value), e.g. ("Ability increase", "Strength").
    /// The picker builds its filter menu from whatever facets a list has.
    pub facets: Vec<(String, String)>,
}

impl OptionItem {
    fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        OptionItem { value: value.into(), label: label.into(), detail: String::new(), disabled: None, warning: None, body: vec![], group: None, facets: vec![], older: None }
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
        Options::ToolExpertise(list) => list
            .iter()
            .map(|t| {
                let p = Prof::Tool(t.clone());
                let mut o = OptionItem::new(encode_prof(&p), format!("{} (expertise)", title_case(t)));
                let proficient = build.profs().any(|x| matches!(x, Prof::Tool(n) if n.eq_ignore_ascii_case(t)));
                if !proficient {
                    o.disabled = Some("Requires proficiency".into());
                }
                o
            })
            .collect(),
        Options::Saves(list) => list.iter().map(|a| prof_item(Prof::Save(*a), format!("{} saving throws", a.name()))).collect(),
        Options::Tools(list) => list.iter().map(|t| prof_item(Prof::Tool(t.clone()), title_case(t))).collect(),
        Options::AnyTool => tool_names(comp, &enabled, &[]).into_iter().map(|t| prof_item(Prof::Tool(t.clone()), title_case(&t))).collect(),
        Options::AnyArtisanTool => tool_names(comp, &enabled, &["toolArtisan"]).into_iter().map(|t| prof_item(Prof::Tool(t.clone()), title_case(&t))).collect(),
        Options::AnyInstrument => {
            tool_names(comp, &enabled, &["instrumentMusical"]).into_iter().map(|t| prof_item(Prof::Tool(t.clone()), title_case(&t))).collect()
        }
        Options::AnyGamingSet => tool_names(comp, &enabled, &["setGaming"]).into_iter().map(|t| prof_item(Prof::Tool(t.clone()), title_case(&t))).collect(),
        Options::Languages(list) => list
            .iter()
            .map(|l| {
                let mut o = prof_item(Prof::Language(l.clone()), l.clone());
                if let Some(lang) = comp.language(l) {
                    o.detail = language_kind_label(&lang.kind).into();
                }
                o.group = Some("Listed options".into());
                o
            })
            .collect(),
        Options::AnyLanguage | Options::AnyStandardLanguage | Options::AnyExoticLanguage => {
            // "Any language" is the full list: standard (common) languages,
            // then rare, exotic and setting-specific (uncommon) ones.
            let kinds: &[&str] = match &choice.options {
                Options::AnyStandardLanguage => &["standard"],
                Options::AnyExoticLanguage => &["exotic", "rare"],
                _ => &["standard", "exotic", "rare", "other"],
            };
            // A language's kind comes from its most authoritative source (2024 first).
            let langs: Vec<&Language> = comp.languages.iter().filter(|l| enabled(&l.source)).collect();
            let mut names: BTreeMap<String, (String, String)> = BTreeMap::new();
            for l in dedupe(langs.into_iter()).into_iter().filter(|l| kinds.contains(&l.kind.as_str())) {
                names.insert(l.name.to_lowercase(), (l.kind.clone(), l.source.clone()));
            }
            if names.is_empty() {
                for n in ["Common", "Dwarvish", "Elvish", "Giant", "Gnomish", "Goblin", "Halfling", "Orc"] {
                    names.insert(n.to_lowercase(), ("standard".into(), "XPHB".into()));
                }
            }
            // Common (standard) languages, then uncommon ones, then languages
            // of specific campaign settings (other than the Forgotten Realms),
            // each setting in its own group.
            let mut items: Vec<(u8, OptionItem)> = names
                .into_iter()
                .map(|(n, (kind, source))| {
                    let setting = Compendium::campaign_setting(&source);
                    let mut o = prof_item(Prof::Language(title_case(&n)), title_case(&n)).detail(match setting {
                        Some(s) => format!("{} · {s}", language_kind_label(&kind)),
                        None => language_kind_label(&kind).to_string(),
                    });
                    let (rank, group) = match (setting, kind.as_str()) {
                        (Some(s), _) => (3, format!("{s} languages")),
                        (None, "standard") => (1, "Standard languages (common)".to_string()),
                        (None, _) => (2, "Uncommon languages".to_string()),
                    };
                    o.group = Some(group);
                    if matches!(n.as_str(), "druidic" | "thieves' cant") {
                        o.warning = Some("Normally learned only through a class".into());
                    }
                    (rank, o)
                })
                .collect();
            items.sort_by(|a, b| (a.0, &a.1.group, &a.1.label).cmp(&(b.0, &b.1.group, &b.1.label)));
            items.into_iter().map(|(_, o)| o).collect()
        }
        Options::Weapons(list) => list.iter().map(|w| prof_item(Prof::Weapon(w.clone()), title_case(w))).collect(),
        Options::Abilities { from, max, .. } => {
            let cap = max.map(|m| m as i32).unwrap_or(20);
            from.iter()
                .map(|a| {
                    let mut it = OptionItem::new(a.id(), a.name()).detail(format!("{} {}", build.score(*a), signed(build.modifier(*a))));
                    if build.score(*a) >= cap && !choice.selected.iter().any(|v| v == a.id()) {
                        it.warning = Some(format!("Already at {cap}"));
                    }
                    it
                })
                .collect()
        }
        Options::Feats { categories } => feat_options(categories, build, ch, comp),
        Options::Spells(filter) => spell_options(filter, choice, build, ch, comp),
        Options::OptionalFeatures { types } => {
            let taken: BTreeSet<&Key> = build.optional_features.iter().map(|(k, _)| k).collect();
            let items: Vec<&OptionalFeature> =
                comp.optional_features.iter().filter(|o| o.types.iter().any(|t| types.contains(t)) && enabled(&o.source)).collect();
            // Reprinted older versions are kept, but folded away.
            let note = |o: &OptionalFeature| reprint_note(&o.reprinted, ch, comp, |n, s| comp.optional_features.get(&Key::new(n, s)).is_some());
            let (old_items, current): (Vec<&OptionalFeature>, Vec<&OptionalFeature>) = items.into_iter().partition(|o| note(o).is_some());
            dedupe(current.into_iter())
                .into_iter()
                .chain(old_items)
                .map(|o| {
                    let mut it = OptionItem::new(o.key.0.clone(), o.name.clone())
                        .detail(o.prereq.as_ref().map(|p| p.text.clone()).unwrap_or_else(|| comp.source_name(&o.source)))
                        .body(o.body.clone());
                    it.older = note(o);
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
                // The reprint must be a subclass of this same class version.
                let older = reprint_note(&s.reprinted, ch, comp, |n, src| {
                    comp.subclasses.iter().any(|x| x.class == s.class && x.short_name.eq_ignore_ascii_case(n) && x.source.eq_ignore_ascii_case(src))
                });
                let summary: Vec<&str> = s.features.iter().map(|f| f.name.as_str()).take(4).collect();
                let mut o = OptionItem::new(s.key.0.clone(), s.name.clone())
                    .detail(format!("{} · {}", summary.join(", "), comp.source_name(&s.source)))
                    .body(crate::explain::subclass(comp, s));
                o.older = older;
                o
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
                if b.min_level > scope_level(choice, ch) && !choice.selected.contains(&b.name) {
                    o.disabled = Some(format!("Requires level {}", b.min_level));
                }
                o
            })
            .collect(),
        Options::Mixed(parts) => {
            // Each option once, the first time it appears ("listed" before "any").
            let mut seen = BTreeSet::new();
            parts
                .iter()
                .flat_map(|p| {
                    let sub = ChoiceState { options: p.clone(), ..choice.clone() };
                    options(&sub, build, ch, comp)
                })
                .filter(|o| seen.insert(o.value.to_lowercase()))
                .collect()
        }
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
    // Every option can be expanded to its full description.
    for o in out.iter_mut() {
        crate::explain::option(o, &choice.options, comp);
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
    if p.spellcasting_feature {
        let has = comp.is_some_and(|comp| has_spellcasting_feature(build, comp));
        if !has {
            return Some("Requires the Spellcasting or Pact Magic feature".into());
        }
    }
    None
}

/// Whether a class (or subclass) gives the character the Spellcasting or Pact Magic feature.
pub fn has_spellcasting_feature(build: &Build, comp: &Compendium) -> bool {
    build.classes.iter().any(|cs| {
        let class = comp.classes.get(&cs.key);
        let sub = cs.subclass.as_ref().and_then(|k| comp.subclasses.get(k));
        let casts = |sc: &Spellcasting| {
            crate::spellcasting::max_spell_level(sc.caster, cs.level, class.is_some_and(|c| c.legacy)) > 0
                || sc.cantrips.get(cs.level.saturating_sub(1) as usize).is_some_and(|n| *n > 0)
        };
        class.is_some_and(|c| casts(&c.spellcasting)) || sub.is_some_and(|s| casts(&s.spellcasting))
    })
}

fn language_kind_label(kind: &str) -> &'static str {
    match kind {
        "standard" => "Standard language",
        "rare" => "Rare language",
        "exotic" => "Exotic language",
        "secret" => "Secret language",
        _ => "Setting language",
    }
}

/// "Reprinted in <book> as <name>" when `reprinted` (`name|source`) names a
/// record that `exists` and whose book is enabled; None otherwise, so an older
/// version stays in the main list when its reprint isn't available.
pub fn reprint_note(reprinted: &Option<String>, ch: &Character, comp: &Compendium, exists: impl Fn(&str, &str) -> bool) -> Option<String> {
    let (name, source) = reprinted.as_ref()?.split_once('|')?;
    (exists(name, source) && ch.settings.source_enabled(source)).then(|| format!("Replaced by {name} ({})", comp.source_name(source)))
}

fn feat_options(categories: &[String], build: &Build, ch: &Character, comp: &Compendium) -> Vec<OptionItem> {
    let taken: BTreeSet<&Key> = build.feats.iter().map(|(k, _)| k).collect();
    // Older (uncategorised) feats that were reprinted in 2024 give way to the
    // reprint, whatever category the reprint landed in: they are offered only
    // in the folded "older versions" section.
    let newer: BTreeSet<String> =
        comp.feats.iter().filter(|f| f.category.is_some() && ch.settings.source_enabled(&f.source)).map(|f| f.name.to_lowercase()).collect();
    let older_note = |f: &Feat| {
        reprint_note(&f.reprinted, ch, comp, |n, s| comp.feats.get(&Key::new(n, s)).is_some()).or_else(|| {
            (f.category.is_none() && newer.contains(&f.name.to_lowercase())).then(|| format!("Replaced by {} (2024)", f.name))
        })
    };
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
    let (old_feats, current): (Vec<&Feat>, Vec<&Feat>) = feats.into_iter().partition(|f| older_note(f).is_some());
    let mut out: Vec<OptionItem> = dedupe(current.into_iter())
        .into_iter()
        .chain(old_feats)
        .map(|f| {
            // Category, ability increases and grants are shown as tags; the
            // detail line carries the prerequisite and the book.
            let detail = match &f.prereq {
                Some(p) if !p.text.is_empty() => format!("Requires {} · {}", p.text, comp.source_name(&f.source)),
                _ => format!("No prerequisite · {}", comp.source_name(&f.source)),
            };
            let mut o = OptionItem::new(f.key.0.clone(), f.name.clone()).detail(detail).body(f.body.clone());
            o.group = Some(f.category_label().to_string());
            o.older = older_note(f);
            o.facets = feat_facets(f, comp);
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
        .filter(|s| filter.spell_attack.is_empty() || !s.attack.is_empty())
        .filter(|s| filter.sources.is_empty() || filter.sources.iter().any(|x| x.eq_ignore_ascii_case(&s.source)))
        .filter(|s| !filter.exclude_sources.iter().any(|x| x.eq_ignore_ascii_case(&s.source)))
        .filter(|s| within.as_ref().is_none_or(|w| w.contains(s.key.0.as_str())))
        .collect();
    let mut out: Vec<OptionItem> = dedupe(spells.into_iter())
        .into_iter()
        .map(|s| {
            let lvl = if s.level == 0 { "Cantrip".to_string() } else { format!("Level {}", s.level) };
            let mut o = OptionItem::new(s.key.0.clone(), s.name.clone())
                .detail(format!("{lvl} · {}{}", s.school_name(), if s.ritual { " · Ritual" } else { "" }))
                .body({
                    let mut b = vec![Entry::P(format!(
                        "Casting time: {} · Range: {} · Components: {} · Duration: {}",
                        s.time, s.range, s.components, s.duration
                    ))];
                    b.extend(s.body.clone());
                    b
                });
            o.group = Some(lvl.clone());
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

/// The level a choice's options are gated by: the class level for class
/// choices (paths start with `class:<key>/`), the character level otherwise.
fn scope_level(choice: &ChoiceState, ch: &Character) -> u8 {
    choice
        .path
        .strip_prefix("class:")
        .and_then(|rest| rest.split('/').next())
        .map(|k| ch.class_level(&Key(k.to_string())))
        .unwrap_or_else(|| ch.total_level())
}

/// Filter tags for a feat: category, what it grants, what it requires,
/// edition and source.
pub fn feat_facets(f: &Feat, comp: &Compendium) -> Vec<(String, String)> {
    let mut out: BTreeSet<(String, String)> = BTreeSet::new();
    let mut tag = |facet: &str, value: &str| {
        out.insert((facet.to_string(), value.to_string()));
    };
    tag("Category", if f.category.is_none() { "Older feat (2014)" } else { f.category_label() });
    facet_walk(&f.grants, &mut tag);
    match &f.prereq {
        None => tag("Prerequisite", "None"),
        Some(p) => {
            let mut any = false;
            let mut req = |v: &str| {
                any = true;
                tag("Prerequisite", v);
            };
            if !p.abilities.is_empty() {
                req("Ability score");
            }
            if p.level.is_some() {
                req("Level");
            }
            if p.spellcasting || p.spellcasting_feature || !p.spells.is_empty() {
                req("Spellcasting");
            }
            if !p.proficiencies.is_empty() {
                req("Armor or weapon proficiency");
            }
            if !p.species.is_empty() {
                req("Species");
            }
            if !p.feats.is_empty() || !p.features.is_empty() {
                req("Feat or class feature");
            }
            if !any {
                tag("Prerequisite", "Other");
            }
        }
    }
    if f.repeatable {
        tag("Other", "Repeatable");
    }
    tag("Edition", if f.category.is_some() { "2024" } else { "Older (2014)" });
    tag("Source", &comp.source_name(&f.source));
    out.into_iter().collect()
}

/// Filter tags for what a list of grants gives (shared by feats, species,
/// backgrounds and classes).
pub fn facet_walk(grants: &[Grant], tag: &mut dyn FnMut(&str, &str)) {
    for g in grants {
        match g {
            Grant::Ability { ability, amount } if *amount > 0 => tag("Ability increase", ability.name()),
            Grant::AbilityMax { .. } => {}
            Grant::Spells(_) => tag("Grants", "Spells"),
            Grant::Prof(p) => tag(
                "Grants",
                match p {
                    Prof::Skill(_) => "Skill proficiency",
                    Prof::Save(_) => "Saving throw proficiency",
                    Prof::Armor(_) | Prof::Weapon(_) => "Armor or weapon training",
                    Prof::Tool(_) => "Tool proficiency",
                    Prof::Language(_) => "Languages",
                },
            ),
            Grant::Expertise(_) | Grant::ToolExpertise(_) => tag("Grants", "Expertise"),
            Grant::Resist(_) | Grant::Immune(_) | Grant::CondImmune(_) => tag("Grants", "Resistance or immunity"),
            Grant::Darkvision(_) | Grant::DarkvisionBonus { .. } | Grant::Sense { .. } => tag("Grants", "Senses"),
            Grant::SpeedBonus { .. } | Grant::SpeedSet { .. } => tag("Grants", "Speed"),
            Grant::HpPerLevel(_) | Grant::HpBonus(_) | Grant::HpPerClassLevel(_) => tag("Grants", "Hit points"),
            Grant::AcBonus { .. } | Grant::NaturalArmor { .. } => tag("Grants", "Armor Class"),
            Grant::AtLevel { grants, .. } => facet_walk(grants, tag),
            Grant::Choose(c) => facet_options(&c.options, tag),
            _ => {}
        }
    }
}
pub fn facet_options(o: &Options, tag: &mut dyn FnMut(&str, &str)) {
    match o {
        Options::Abilities { from, amounts, .. } if amounts.iter().any(|a| *a > 0) => {
            for a in from {
                tag("Ability increase", a.name());
            }
        }
        Options::Skills(_) | Options::AnySkill => tag("Grants", "Skill proficiency"),
        Options::SkillExpertise | Options::ExpertiseFrom(_) | Options::ToolExpertise(_) => tag("Grants", "Expertise"),
        Options::Saves(_) => tag("Grants", "Saving throw proficiency"),
        Options::Tools(_) | Options::AnyTool | Options::AnyArtisanTool | Options::AnyInstrument | Options::AnyGamingSet => {
            tag("Grants", "Tool proficiency")
        }
        Options::Languages(_) | Options::AnyLanguage | Options::AnyStandardLanguage | Options::AnyExoticLanguage => tag("Grants", "Languages"),
        Options::Weapons(_) | Options::WeaponMastery => tag("Grants", "Armor or weapon training"),
        Options::Spells(_) => tag("Grants", "Spells"),
        Options::OptionalFeatures { .. } => tag("Grants", "Maneuvers, invocations or similar"),
        Options::Resist(_) => tag("Grants", "Resistance or immunity"),
        Options::Bundles(bs) => {
            for b in bs {
                facet_walk(&b.grants, tag);
            }
        }
        Options::Mixed(parts) => {
            for p in parts {
                facet_options(p, tag);
            }
        }
        _ => {}
    }
}
