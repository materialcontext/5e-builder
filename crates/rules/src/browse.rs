//! Classes, species and backgrounds as browsable options: a details line,
//! filter facets, and a full description (everything the record gives, then
//! its own text) so the player can read everything before choosing.

use crate::explain;
use crate::options::{facet_walk, reprint_note, OptionItem};
use crate::Character;
use compendium::*;
use std::collections::BTreeSet;

fn p(s: impl Into<String>) -> Entry {
    Entry::P(s.into())
}

fn section(name: &str, body: Vec<Entry>) -> Entry {
    Entry::Section { name: Some(name.to_string()), body }
}

fn edition(legacy: bool) -> &'static str {
    if legacy { "Older (2014)" } else { "2024" }
}

struct Tags(BTreeSet<(String, String)>);

impl Tags {
    fn new() -> Self {
        Tags(BTreeSet::new())
    }
    fn add(&mut self, f: &str, v: &str) {
        self.0.insert((f.to_string(), v.to_string()));
    }
    fn grants(&mut self, grants: &[Grant]) {
        let mut tag = |f: &str, v: &str| {
            self.0.insert((f.to_string(), v.to_string()));
        };
        facet_walk(grants, &mut tag);
    }
    fn done(self) -> Vec<(String, String)> {
        self.0.into_iter().collect()
    }
}

fn item(value: &Key, label: &str, detail: String, body: Vec<Entry>, facets: Vec<(String, String)>) -> OptionItem {
    OptionItem { value: value.0.clone(), label: label.to_string(), detail, disabled: None, warning: None, body, group: None, facets, older: None }
}

/// One line per thing a list of grants gives ("Skills: Insight, Religion").
fn summary_lines(comp: &Compendium, grants: &[Grant]) -> Vec<String> {
    let mut skills = Vec::new();
    let mut tools = Vec::new();
    let mut langs = Vec::new();
    let mut weapons = Vec::new();
    let mut other = Vec::new();
    for g in grants {
        match g {
            Grant::Prof(Prof::Skill(s)) => skills.push(s.name().to_string()),
            Grant::Prof(Prof::Tool(t)) => tools.push(title_case(t)),
            Grant::Prof(Prof::Language(l)) => langs.push(title_case(l)),
            Grant::Prof(Prof::Weapon(w)) => weapons.push(format!("{} weapons", title_case(w))),
            Grant::Prof(Prof::Armor(a)) => weapons.push(format!("{} armor", title_case(a))),
            Grant::Prof(Prof::Save(a)) => other.push(format!("{} saving throws", a.name())),
            Grant::Feat(k) => other.push(format!("Feat: {}", comp.feats.get(k).map(|f| f.name.clone()).unwrap_or_else(|| title_case(k.name_part())))),
            Grant::Item { .. } | Grant::Coins(_) | Grant::Feature(_) => {}
            Grant::Choose(c) if c.id.contains("equipment") => {}
            Grant::Choose(c) => {
                let what = match &c.options {
                    Options::Abilities { from, amounts, .. } => {
                        format!("Ability scores: {} among {}", amounts.iter().map(|a| signed(*a as i32)).collect::<Vec<_>>().join(" / "), from.iter().map(|a| a.name()).collect::<Vec<_>>().join(", "))
                    }
                    Options::Feats { .. } => format!("{} (choose)", c.label),
                    _ => format!("{}: choose {}", c.label, describe_count(&c.count)),
                };
                other.push(what);
            }
            g => {
                let d = describe(std::slice::from_ref(g));
                if !d.is_empty() {
                    other.push(d);
                }
            }
        }
    }
    let mut out = Vec::new();
    if !skills.is_empty() {
        out.push(format!("Skills: {}", skills.join(", ")));
    }
    if !tools.is_empty() {
        out.push(format!("Tools: {}", tools.join(", ")));
    }
    if !weapons.is_empty() {
        out.push(format!("Training: {}", weapons.join(", ")));
    }
    if !langs.is_empty() {
        out.push(format!("Languages: {}", langs.join(", ")));
    }
    out.extend(other);
    out
}

fn describe_count(c: &Count) -> String {
    match c {
        Count::Fixed(n) => n.to_string(),
        Count::ByLevel(v) => v.first().map(|n| n.to_string()).unwrap_or_default(),
    }
}

/// Spells a list of grants gives, in words, and how ("added to your class
/// spell list", "always prepared"...).
fn spell_lines(comp: &Compendium, grants: &[Grant], out: &mut Vec<Entry>, how: &mut BTreeSet<&'static str>) {
    for g in grants {
        match g {
            Grant::Spells(group) => {
                let mut by_kind: Vec<(&'static str, Vec<String>)> = Vec::new();
                for sg in &group.spells {
                    let kind = match &sg.kind {
                        SpellKind::Expanded => "Added to your class spell list",
                        SpellKind::Prepared => "Always prepared",
                        SpellKind::Known => "Known",
                        SpellKind::Innate(_) => "Cast without a slot",
                    };
                    how.insert(match &sg.kind {
                        SpellKind::Expanded => "Adds spells to your class spell list",
                        _ => "Grants spells",
                    });
                    let name = match &sg.pick {
                        SpellPick::Fixed(k) => comp.spells.get(k).map(|s| if s.level == 0 { format!("{} (cantrip)", s.name) } else { format!("{} (level {})", s.name, s.level) }).unwrap_or_else(|| title_case(k.name_part())),
                        SpellPick::Choose { count, .. } => format!("{count} spell(s) of your choice"),
                        SpellPick::All(_) => "a list of spells".into(),
                    };
                    let name = if sg.level > 1 { format!("{name} — from level {}", sg.level) } else { name };
                    match by_kind.iter_mut().find(|(k, _)| *k == kind) {
                        Some((_, v)) => v.push(name),
                        None => by_kind.push((kind, vec![name])),
                    }
                }
                for (kind, names) in by_kind {
                    out.push(p(format!("{kind}: {}.", names.join(", "))));
                }
            }
            Grant::Feat(k) => {
                if let Some(f) = comp.feats.get(k) {
                    let mut sub = Vec::new();
                    let mut sub_how = BTreeSet::new();
                    spell_lines(comp, &f.grants, &mut sub, &mut sub_how);
                    let picks = f.grants.iter().any(|g| matches!(g, Grant::Choose(c) if matches!(c.options, Options::Spells(_))));
                    if !sub.is_empty() || picks {
                        how.insert("Spells through its origin feat");
                        out.push(p(format!("Through the {} feat: {}", f.name, if sub.is_empty() { "spells you choose (see the feat).".into() } else { Entry::plain(&sub).replace('\n', " ") })));
                    }
                }
            }
            Grant::Choose(c) => match &c.options {
                Options::Spells(_) => {
                    how.insert("Grants spells");
                    out.push(p(format!("{}: choose {}.", c.label, describe_count(&c.count))));
                }
                Options::Bundles(bs) => {
                    for b in bs {
                        spell_lines(comp, &b.grants, out, how);
                    }
                }
                _ => {}
            },
            Grant::AtLevel { grants, .. } => spell_lines(comp, grants, out, how),
            _ => {}
        }
    }
}

pub fn backgrounds(ch: &Character, comp: &Compendium) -> Vec<OptionItem> {
    let mut out: Vec<OptionItem> = comp
        .backgrounds
        .iter()
        .filter(|b| ch.settings.source_enabled(&b.source))
        .map(|b| {
            let mut t = Tags::new();
            t.grants(&b.grants);
            t.add("Edition", edition(b.legacy));
            t.add("Source", &comp.source_name(&b.source));
            // Skills by name: the most common thing players look for.
            for g in &b.grants {
                if let Grant::Prof(Prof::Skill(s)) = g {
                    t.add("Skill", s.name());
                }
                if let Grant::Feat(k) = g {
                    if let Some(f) = comp.feats.get(k) {
                        t.add("Origin feat", f.name.split(';').next().unwrap_or(&f.name).trim());
                    }
                }
            }
            if !b.has_feat {
                t.add("Origin feat", "Any (older background)");
            }
            if !b.has_asi {
                for a in Ability::ALL {
                    t.add("Ability increase", a.name());
                }
            }
            let mut spells = Vec::new();
            let mut how = BTreeSet::new();
            spell_lines(comp, &b.grants, &mut spells, &mut how);
            for h in &how {
                t.add("Spells", h);
            }
            let mut lines = summary_lines(comp, &b.grants);
            if !b.has_asi {
                lines.insert(0, "Ability scores (2024 rules for older backgrounds): +2 / +1 or +1 / +1 / +1 among any abilities".into());
            }
            if !b.has_feat {
                lines.push("Origin feat of your choice (2024 rules for older backgrounds)".into());
            }
            let detail = lines.iter().filter(|l| l.starts_with("Skills") || l.starts_with("Feat") || l.starts_with("Tools")).cloned().collect::<Vec<_>>().join(" · ");
            let mut body = vec![section("What you get", vec![Entry::List(lines.into_iter().map(p).collect())])];
            if !spells.is_empty() {
                body.push(section("Spells", spells));
            }
            for g in &b.grants {
                if let Grant::Feat(k) = g {
                    if let Some(f) = comp.feats.get(k) {
                        body.push(section(&format!("Origin feat: {}", f.name), f.body.clone()));
                    }
                }
            }
            body.extend(b.features.iter().map(|f| section(&f.name, f.body.clone())));
            if let Some(pr) = &b.prereq {
                body.insert(0, p(format!("Prerequisite: {pr}")));
            }
            let mut o = item(&b.key, &b.name, format!("{detail} · {}", comp.source_name(&b.source)), body, t.done());
            o.older = reprint_note(&b.reprinted, ch, comp, |n, s| comp.backgrounds.get(&Key::new(n, s)).is_some());
            o
        })
        .collect();
    out.sort_by(|a, b| (a.facets.iter().any(|f| f.1 == "Older (2014)"), &a.label).cmp(&(b.facets.iter().any(|f| f.1 == "Older (2014)"), &b.label)));
    out
}

pub fn species(ch: &Character, comp: &Compendium) -> Vec<OptionItem> {
    let mut out: Vec<OptionItem> = comp
        .species
        .iter()
        .filter(|s| ch.settings.source_enabled(&s.source))
        .map(|s| {
            let mut t = Tags::new();
            t.grants(&s.grants);
            for f in &s.lineages {
                t.grants(&f.grants);
            }
            for z in &s.sizes {
                t.add("Size", z.name());
            }
            t.add("Darkvision", if s.darkvision.is_some() || s.grants.iter().any(|g| matches!(g, Grant::Darkvision(_))) { "Yes" } else { "No" });
            for (mode, v) in [("Fly", s.speed.fly), ("Swim", s.speed.swim), ("Climb", s.speed.climb), ("Burrow", s.speed.burrow)] {
                if v > 0 {
                    t.add("Movement", mode);
                }
            }
            if s.speed.walk > 30 {
                t.add("Movement", "Fast (over 30 ft.)");
            }
            if !s.lineages.is_empty() {
                t.add("Other", "Has lineages");
            }
            if !s.creature_type.is_empty() {
                t.add("Creature type", &title_case(&s.creature_type));
            }
            let mut spells = Vec::new();
            let mut how = BTreeSet::new();
            spell_lines(comp, &s.grants, &mut spells, &mut how);
            for l in &s.lineages {
                spell_lines(comp, &l.grants, &mut spells, &mut how);
            }
            if !how.is_empty() {
                t.add("Spells", "Has innate spells");
            }
            t.add("Edition", edition(s.legacy));
            t.add("Source", &comp.source_name(&s.source));
            let sizes = s.sizes.iter().map(|z| z.name()).collect::<Vec<_>>().join(" or ");
            let mut facts = vec![format!("{sizes} {}", if s.creature_type.is_empty() { "Humanoid".into() } else { title_case(&s.creature_type) }), format!("Speed {} ft.", s.speed.walk)];
            if let Some(dv) = s.darkvision {
                facts.push(format!("Darkvision {dv} ft."));
            }
            let mut body = vec![section("Basics", vec![p(facts.join(" · "))])];
            let lines = summary_lines(comp, &s.grants);
            if !lines.is_empty() {
                body.push(section("What you get", vec![Entry::List(lines.into_iter().map(p).collect())]));
            }
            if !spells.is_empty() {
                body.push(section("Spells", spells));
            }
            body.extend(s.features.iter().map(|f| section(&f.name, f.body.clone())));
            for l in &s.lineages {
                let mut lb = Vec::new();
                let ll = summary_lines(comp, &l.grants);
                if !ll.is_empty() {
                    lb.push(p(ll.join(" · ")));
                }
                lb.extend(l.features.iter().map(|f| section(&f.name, f.body.clone())));
                body.push(section(&format!("Lineage: {}", l.name), lb));
            }
            let mut o = item(&s.key, &s.name, format!("{} · {}", facts.join(" · "), comp.source_name(&s.source)), body, t.done());
            o.older = reprint_note(&s.reprinted, ch, comp, |n, src| comp.species.get(&Key::new(n, src)).is_some());
            o
        })
        .collect();
    out.sort_by(|a, b| (a.facets.iter().any(|f| f.1 == "Older (2014)"), &a.label).cmp(&(b.facets.iter().any(|f| f.1 == "Older (2014)"), &b.label)));
    out
}

pub fn classes(ch: &Character, comp: &Compendium) -> Vec<OptionItem> {
    let mut out: Vec<OptionItem> = comp
        .classes
        .iter()
        .filter(|c| ch.settings.source_enabled(&c.source))
        .map(|c| {
            let mut t = Tags::new();
            for a in &c.primary {
                t.add("Primary ability", a.name());
            }
            t.add("Hit die", &format!("d{}", c.hit_die));
            t.add(
                "Spellcasting",
                match c.spellcasting.caster {
                    Some(Caster::Full) => "Full caster",
                    Some(Caster::Half) | Some(Caster::Artificer) => "Half caster",
                    Some(Caster::Third) => "Third caster",
                    Some(Caster::Pact) => "Pact magic",
                    None => "None",
                },
            );
            let armor: Vec<String> = c.starting.iter().filter_map(|g| if let Grant::Prof(Prof::Armor(a)) = g { Some(a.to_lowercase()) } else { None }).collect();
            t.add(
                "Armor",
                if armor.iter().any(|a| a == "heavy") {
                    "Heavy"
                } else if armor.iter().any(|a| a == "medium") {
                    "Medium"
                } else if armor.iter().any(|a| a == "light") {
                    "Light"
                } else {
                    "None"
                },
            );
            t.add("Edition", edition(c.legacy));
            t.add("Source", &comp.source_name(&c.source));
            let caster = c.spellcasting.ability.map(|a| format!(" · Spellcasting: {}", a.name())).unwrap_or_default();
            let detail = format!(
                "d{} hit die · Primary: {} · Saves: {}{caster}",
                c.hit_die,
                c.primary.iter().map(|a| a.abbr()).collect::<Vec<_>>().join(" or "),
                c.saves.iter().map(|a| a.abbr()).collect::<Vec<_>>().join(", ")
            );
            let mut core = vec![p(detail.clone())];
            let lines = summary_lines(comp, &c.starting);
            if !lines.is_empty() {
                core.push(Entry::List(lines.into_iter().map(p).collect()));
            }
            if let Some(r) = &c.multiclass_req {
                core.push(p(format!("Multiclassing requires: {r}")));
            }
            let mut body = vec![section("Core traits", core)];
            if !c.equipment.is_empty() {
                let mut eq = Vec::new();
                for g in &c.equipment {
                    if let Grant::Choose(ch) = g {
                        if let Options::Bundles(bs) = &ch.options {
                            for b in bs {
                                eq.push(section(&b.name, explain::grants(comp, &b.grants)));
                            }
                        }
                    }
                }
                if !eq.is_empty() {
                    body.push(section("Starting equipment (choose one)", eq));
                }
            }
            for f in &c.features {
                body.push(section(&format!("Level {} — {}", f.level, f.name), f.body.clone()));
            }
            let mut o = item(&c.key, &c.name, format!("{detail} · {}", comp.source_name(&c.source)), body, t.done());
            o.older = reprint_note(&c.reprinted, ch, comp, |n, s| comp.classes.get(&Key::new(n, s)).is_some());
            o
        })
        .collect();
    out.sort_by(|a, b| (a.facets.iter().any(|f| f.1 == "Older (2014)"), &a.label).cmp(&(b.facets.iter().any(|f| f.1 == "Older (2014)"), &b.label)));
    out
}
