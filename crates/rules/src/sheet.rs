//! The derived character sheet: every number a player reads off the page.

use crate::build::{Build, FeatureEntry, FeatureKind, KnownSpell, SpellMode};
use crate::character::Character;
use crate::spellcasting::*;
use compendium::*;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub struct AbilityLine {
    pub ability: Ability,
    pub score: i32,
    pub modifier: i32,
    pub save: i32,
    pub save_prof: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Proficiency {
    None,
    Half,
    Proficient,
    Expertise,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SkillLine {
    pub skill: Skill,
    pub bonus: i32,
    pub prof: Proficiency,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Attack {
    pub name: String,
    pub bonus: i32,
    pub damage: String,
    pub notes: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpellLine {
    pub key: Key,
    pub name: String,
    pub level: u8,
    pub school: String,
    pub time: String,
    pub range: String,
    pub components: String,
    pub duration: String,
    pub concentration: bool,
    pub ritual: bool,
    /// Prepared / always prepared / innate usage, for the sheet's marker column.
    pub status: String,
    pub source_label: String,
    pub ability: Option<Ability>,
    pub body: Vec<Entry>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Casting {
    pub label: String,
    pub ability: Ability,
    pub save_dc: i32,
    pub attack: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InventoryLine {
    pub name: String,
    pub qty: u16,
    pub weight: f32,
    pub equipped: bool,
    pub notes: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Sheet {
    pub name: String,
    pub summary: String,
    pub class_line: String,
    pub species: String,
    pub background: String,
    pub level: u8,
    pub proficiency_bonus: i32,
    pub abilities: Vec<AbilityLine>,
    pub skills: Vec<SkillLine>,
    pub passive_perception: i32,
    pub passive_insight: i32,
    pub passive_investigation: i32,
    pub initiative: i32,
    pub armor_class: i32,
    pub ac_note: String,
    pub hp_max: i32,
    pub hit_dice: String,
    pub speed: Speed,
    pub size: String,
    pub darkvision: u16,
    pub armor_profs: Vec<String>,
    pub weapon_profs: Vec<String>,
    pub tool_profs: Vec<String>,
    pub languages: Vec<String>,
    pub resistances: Vec<String>,
    pub immunities: Vec<String>,
    pub condition_immunities: Vec<String>,
    pub vulnerabilities: Vec<String>,
    /// Special senses other than darkvision, e.g. "Blindsight 10 ft.".
    pub senses: Vec<String>,
    pub attacks: Vec<Attack>,
    pub casting: Vec<Casting>,
    pub slots: [u8; 9],
    pub pact_slots: Option<(u8, u8)>,
    pub spells: Vec<SpellLine>,
    pub features: Vec<FeatureEntry>,
    pub inventory: Vec<InventoryLine>,
    pub carried_weight: f32,
    pub carry_capacity: i32,
    pub coins: crate::character::Coins,
    pub warnings: Vec<String>,
}

pub fn proficiency_bonus(level: u8) -> i32 {
    2 + (level.max(1) as i32 - 1) / 4
}

pub fn sheet(ch: &Character, build: &Build, comp: &Compendium) -> Sheet {
    let level = ch.total_level().max(1);
    let pb = proficiency_bonus(level);
    let m = |a: Ability| build.modifier(a);

    // Proficiency lists.
    let mut armor = Vec::new();
    let mut weapons = Vec::new();
    let mut tools = Vec::new();
    let mut languages = Vec::new();
    for p in build.profs() {
        let list = match p {
            Prof::Armor(x) => (&mut armor, x),
            Prof::Weapon(x) => (&mut weapons, x),
            Prof::Tool(x) => (&mut tools, x),
            Prof::Language(x) => (&mut languages, x),
            _ => continue,
        };
        let label = title_case(list.1);
        if !list.0.contains(&label) {
            list.0.push(label);
        }
    }

    let abilities: Vec<AbilityLine> = Ability::ALL
        .iter()
        .map(|&a| {
            let prof = build.has_prof(&Prof::Save(a));
            AbilityLine { ability: a, score: build.score(a), modifier: m(a), save: m(a) + if prof { pb } else { 0 }, save_prof: prof }
        })
        .collect();

    let jack = build.has_feature_named("Jack of All Trades");
    let skills: Vec<SkillLine> = Skill::ALL
        .iter()
        .map(|&s| {
            let prof = if build.grants.iter().any(|g| g.grant == Grant::Expertise(s)) && build.has_prof(&Prof::Skill(s)) {
                Proficiency::Expertise
            } else if build.has_prof(&Prof::Skill(s)) {
                Proficiency::Proficient
            } else if jack {
                Proficiency::Half
            } else {
                Proficiency::None
            };
            let extra = match prof {
                Proficiency::Expertise => pb * 2,
                Proficiency::Proficient => pb,
                Proficiency::Half => pb / 2,
                Proficiency::None => 0,
            };
            SkillLine { skill: s, bonus: m(s.ability()) + extra, prof }
        })
        .collect();
    let skill_bonus = |s: Skill| skills.iter().find(|x| x.skill == s).map(|x| x.bonus).unwrap_or(0);

    // Species basics.
    let species = ch.species.as_ref().and_then(|k| comp.species.get(k));
    let lineage = species.and_then(|sp| {
        let sel = ch.selected("species/lineage").first()?;
        sp.lineages.iter().find(|l| &l.key.0 == sel)
    });
    let mut speed = lineage.and_then(|l| l.speed.clone()).or_else(|| species.map(|s| s.speed.clone())).unwrap_or(Speed { walk: 30, ..Default::default() });
    let mut darkvision = lineage.and_then(|l| l.darkvision).or_else(|| species.and_then(|s| s.darkvision)).unwrap_or(0);
    for g in &build.grants {
        if let Grant::Darkvision(r) = g.grant {
            darkvision = darkvision.max(r);
        }
    }

    // Equipment.
    let item = |k: &Option<Key>| k.as_ref().and_then(|k| comp.items.get(k));
    let worn_armor = ch.inventory.iter().filter(|i| i.equipped).filter_map(|i| item(&i.item)).find(|it| {
        it.armor.as_ref().is_some_and(|a| a.kind != ArmorKind::Shield)
    });
    let shield = ch
        .inventory
        .iter()
        .filter(|i| i.equipped)
        .filter_map(|i| item(&i.item))
        .find(|it| it.armor.as_ref().is_some_and(|a| a.kind == ArmorKind::Shield));

    let class_names: Vec<String> = build.classes.iter().filter_map(|c| comp.classes.get(&c.key)).map(|c| c.name.clone()).collect();
    let has_class = |n: &str| class_names.iter().any(|c| c == n);

    // Armor class: best of the available calculations.
    let dex = m(Ability::Dex);
    let (mut ac, mut ac_note) = match worn_armor.and_then(|a| a.armor.as_ref().map(|s| (a, s))) {
        Some((it, a)) => {
            let base = a.ac as i32 + it.bonus as i32;
            match a.kind {
                ArmorKind::Light => (base + dex, it.name.clone()),
                ArmorKind::Medium => (base + dex.min(2), it.name.clone()),
                _ => (base, it.name.clone()),
            }
        }
        None => {
            let mut best = (10 + dex, "Unarmored".to_string());
            if has_class("Barbarian") && build.has_feature_named("Unarmored Defense") {
                best = best.max((10 + dex + m(Ability::Con), "Unarmored Defense".into()));
            }
            if has_class("Monk") && build.has_feature_named("Unarmored Defense") && shield.is_none() {
                best = best.max((10 + dex + m(Ability::Wis), "Unarmored Defense".into()));
            }
            if build.has_feature_named("Draconic Resilience") {
                let v = if comp.classes.iter().any(|c| c.name == "Sorcerer" && !c.legacy && build.classes.iter().any(|b| b.key == c.key)) {
                    10 + dex + m(Ability::Cha)
                } else {
                    13 + dex
                };
                best = best.max((v, "Draconic Resilience".into()));
            }
            best
        }
    };
    if let Some(s) = shield.and_then(|s| s.armor.as_ref().map(|a| (s, a))) {
        ac += s.1.ac as i32 + s.0.bonus as i32;
        ac_note.push_str(" + shield");
    }
    if worn_armor.is_some() && build.has_feat_named("Defense") {
        ac += 1;
        ac_note.push_str(" + Defense");
    }

    // Hit points.
    let con = m(Ability::Con);
    let mut hp = 0i32;
    for (i, l) in ch.levels.iter().enumerate() {
        let hd = comp.classes.get(&l.class).map(|c| c.hit_die).unwrap_or(8) as i32;
        let gain = if i == 0 {
            hd
        } else if ch.settings.average_hp {
            hd / 2 + 1
        } else {
            l.hp_roll.map(|r| r as i32).unwrap_or(hd / 2 + 1)
        };
        hp += (gain + con).max(1);
    }
    let per_level: i32 = build
        .grants
        .iter()
        .filter_map(|g| match g.grant {
            Grant::HpPerLevel(n) => Some(n as i32),
            _ => None,
        })
        .sum();
    hp += per_level * level as i32;
    let mut dice: BTreeMap<u8, u8> = BTreeMap::new();
    for l in &ch.levels {
        let hd = comp.classes.get(&l.class).map(|c| c.hit_die).unwrap_or(8);
        *dice.entry(hd).or_default() += 1;
    }
    let hit_dice = dice.iter().rev().map(|(d, n)| format!("{n}d{d}")).collect::<Vec<_>>().join(" + ");

    // Speed bonuses.
    let heavy = worn_armor.and_then(|a| a.armor.as_ref()).is_some_and(|a| a.kind == ArmorKind::Heavy);
    if has_class("Barbarian") && build.has_feature_named("Fast Movement") && !heavy {
        speed.walk += 10;
    }
    if let Some(monk) = build.classes.iter().find(|c| comp.classes.get(&c.key).is_some_and(|x| x.name == "Monk")) {
        if build.has_feature_named("Unarmored Movement") && worn_armor.is_none() && shield.is_none() {
            speed.walk += match monk.level {
                0..=5 => 10,
                6..=9 => 15,
                10..=13 => 20,
                14..=17 => 25,
                _ => 30,
            };
        }
    }

    let initiative = dex + if build.feats.iter().any(|(k, _)| k.0 == "alert|xphb") { pb } else { 0 };

    let attacks = attacks(ch, build, comp, pb, &weapons);

    // Spellcasting.
    let mut casting = Vec::new();
    for cs in &build.classes {
        let Some(class) = comp.classes.get(&cs.key) else { continue };
        let sub = cs.subclass.as_ref().and_then(|k| comp.subclasses.get(k));
        let ability = class.spellcasting.ability.or_else(|| sub.and_then(|s| s.spellcasting.ability));
        let is_caster = class.spellcasting.is_caster() || sub.is_some_and(|s| s.spellcasting.is_caster());
        if let (Some(a), true) = (ability, is_caster) {
            casting.push(Casting { label: class.name.clone(), ability: a, save_dc: 8 + pb + m(a), attack: pb + m(a) });
        }
    }
    for s in &build.spells {
        if let Some(a) = s.ability {
            if s.class.is_none() && !casting.iter().any(|c| c.ability == a) {
                casting.push(Casting { label: s.origin.clone(), ability: a, save_dc: 8 + pb + m(a), attack: pb + m(a) });
            }
        }
    }
    let (slots, pact) = slots(build, comp);
    let spells = spell_lines(&build.spells, comp);

    // Inventory.
    let inventory: Vec<InventoryLine> = ch
        .inventory
        .iter()
        .map(|i| {
            let it = item(&i.item);
            let mut notes = i.notes.clone();
            if let Some(it) = it {
                if !it.contents.is_empty() && notes.is_empty() {
                    notes = it
                        .contents
                        .iter()
                        .map(|(k, q)| {
                            let n = comp.display_name(k);
                            if *q > 1 {
                                format!("{q} {n}")
                            } else {
                                n
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                }
            }
            InventoryLine { name: i.name.clone(), qty: i.qty, weight: it.map(|x| x.weight).unwrap_or(0.0) * i.qty as f32, equipped: i.equipped, notes }
        })
        .collect();
    let carried_weight = inventory.iter().map(|i| i.weight).sum();

    let mut features = build.features.clone();
    features.sort_by_key(|f| match f.kind {
        FeatureKind::Class => 0,
        FeatureKind::Species => 1,
        FeatureKind::Background => 2,
        FeatureKind::Feat => 3,
        FeatureKind::Other => 4,
    });

    let class_line = build
        .classes
        .iter()
        .filter_map(|c| {
            let class = comp.classes.get(&c.key)?;
            let sub = c.subclass.as_ref().and_then(|k| comp.subclasses.get(k)).map(|s| format!(" ({})", s.short_name)).unwrap_or_default();
            Some(format!("{} {}{}", class.name, c.level, sub))
        })
        .collect::<Vec<_>>()
        .join(" / ");
    let species_name = match (species, lineage) {
        (Some(s), Some(l)) => format!("{} ({})", s.name, l.name),
        (Some(s), None) => s.name.clone(),
        _ => String::new(),
    };
    let background = ch.background.as_ref().and_then(|k| comp.backgrounds.get(k)).map(|b| b.name.clone()).unwrap_or_default();

    let collect = |f: fn(&Grant) -> Option<&String>| -> Vec<String> {
        let mut v: Vec<String> = build.grants.iter().filter_map(|g| f(&g.grant)).map(|s| title_case(s)).collect();
        v.sort();
        v.dedup();
        v
    };

    Sheet {
        name: if ch.name.is_empty() { "Unnamed hero".into() } else { ch.name.clone() },
        summary: [species_name.as_str(), class_line.as_str()].iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(" · "),
        class_line,
        species: species_name,
        background,
        level,
        proficiency_bonus: pb,
        passive_perception: 10 + skill_bonus(Skill::Perception),
        passive_insight: 10 + skill_bonus(Skill::Insight),
        passive_investigation: 10 + skill_bonus(Skill::Investigation),
        abilities,
        skills,
        initiative,
        armor_class: ac,
        ac_note,
        hp_max: hp.max(1),
        hit_dice,
        speed,
        size: match species.map(|s| s.sizes.as_slice()) {
            Some([one]) => one.name().to_string(),
            Some(_) => ch.selected("species/size").first().cloned().unwrap_or_else(|| "Medium".into()),
            None => "Medium".into(),
        },
        darkvision,
        armor_profs: armor,
        weapon_profs: weapons,
        tool_profs: tools,
        languages,
        resistances: collect(|g| match g {
            Grant::Resist(s) => Some(s),
            _ => None,
        }),
        immunities: collect(|g| match g {
            Grant::Immune(s) => Some(s),
            _ => None,
        }),
        condition_immunities: collect(|g| match g {
            Grant::CondImmune(s) => Some(s),
            _ => None,
        }),
        vulnerabilities: collect(|g| match g {
            Grant::Vulnerable(s) => Some(s),
            _ => None,
        }),
        senses: {
            let mut best: BTreeMap<String, u16> = BTreeMap::new();
            for g in &build.grants {
                if let Grant::Sense { sense, range } = &g.grant {
                    let e = best.entry(title_case(sense)).or_default();
                    *e = (*e).max(*range);
                }
            }
            best.into_iter().map(|(k, v)| format!("{k} {v} ft.")).collect()
        },
        attacks,
        casting,
        slots,
        pact_slots: pact,
        spells,
        features,
        inventory,
        carried_weight,
        carry_capacity: build.score(Ability::Str) * 15,
        coins: ch.coins.clone(),
        warnings: build.warnings.clone(),
    }
}

fn weapon_proficient(it: &Item, w: &WeaponStats, profs: &[String]) -> bool {
    let lower: Vec<String> = profs.iter().map(|p| p.to_lowercase()).collect();
    let cat = if w.martial { "martial" } else { "simple" };
    let name = it.name.to_lowercase();
    lower.iter().any(|p| {
        p == cat
            || p.starts_with(&format!("{cat} weapon"))
            || p == &name
            || p.trim_end_matches('s') == name
            || (p == "martial weapons with the finesse or light property" && w.martial && w.properties.iter().any(|x| x == "finesse" || x == "light"))
    })
}

fn attacks(ch: &Character, build: &Build, comp: &Compendium, pb: i32, profs: &[String]) -> Vec<Attack> {
    let m = |a: Ability| build.modifier(a);
    let masteries: Vec<String> =
        build.features.iter().filter_map(|f| f.feature.name.strip_prefix("Weapon Mastery: ")).map(|s| s.split(" (").next().unwrap_or("").to_lowercase()).collect();
    let mut out = Vec::new();
    for inv in &ch.inventory {
        let Some(it) = inv.item.as_ref().and_then(|k| comp.items.get(k)) else { continue };
        let Some(w) = &it.weapon else { continue };
        let finesse = w.properties.iter().any(|p| p == "finesse");
        let ability = if w.ranged {
            Ability::Dex
        } else if finesse && m(Ability::Dex) > m(Ability::Str) {
            Ability::Dex
        } else {
            Ability::Str
        };
        let prof = weapon_proficient(it, w, profs);
        let bonus = m(ability) + if prof { pb } else { 0 } + it.bonus as i32;
        let dmg_mod = m(ability) + it.bonus as i32;
        let fmt = |dice: &str| if dmg_mod == 0 { dice.to_string() } else { format!("{dice}{}", signed(dmg_mod)) };
        let mut damage = format!("{} {}", fmt(&w.damage), w.damage_type);
        if let Some(v) = &w.versatile {
            damage.push_str(&format!(" ({} two-handed)", fmt(v)));
        }
        let mut notes: Vec<String> = w.properties.iter().filter(|p| p.as_str() != "versatile").cloned().collect();
        if let Some(r) = &w.range {
            notes.push(format!("range {r}"));
        }
        if let Some(ms) = &w.mastery {
            if masteries.contains(&it.name.to_lowercase()) {
                notes.push(format!("mastery: {ms}"));
            }
        }
        if !prof {
            notes.push("not proficient".into());
        }
        out.push(Attack { name: inv.name.clone(), bonus, damage, notes: notes.join(", ") });
    }
    out.push(Attack {
        name: "Unarmed Strike".into(),
        bonus: m(Ability::Str) + pb,
        damage: format!("{} bludgeoning", (1 + m(Ability::Str)).max(1)),
        notes: String::new(),
    });
    out
}

fn slots(build: &Build, comp: &Compendium) -> ([u8; 9], Option<(u8, u8)>) {
    let mut casters = Vec::new();
    let mut pact = None;
    for cs in &build.classes {
        let Some(class) = comp.classes.get(&cs.key) else { continue };
        let sub = cs.subclass.as_ref().and_then(|k| comp.subclasses.get(k));
        let caster = class.spellcasting.caster.or_else(|| sub.and_then(|s| s.spellcasting.caster));
        match caster {
            Some(Caster::Pact) => pact = Some(pact_slots(cs.level)),
            Some(c) => casters.push((c, cs.level, class.legacy)),
            None => {}
        }
    }
    let cl = match casters.as_slice() {
        [] => 0,
        [(c, l, legacy)] => single_class_caster_level(Some(*c), *l, *legacy),
        many => many.iter().map(|(c, l, _)| multiclass_caster_level(Some(*c), *l)).sum(),
    };
    (slots_for_caster_level(cl), pact)
}

fn spell_lines(known: &[KnownSpell], comp: &Compendium) -> Vec<SpellLine> {
    let mut by_key: BTreeMap<(u8, String, Key), SpellLine> = BTreeMap::new();
    for k in known {
        let Some(s) = comp.spells.get(&k.key) else { continue };
        let status = match &k.mode {
            SpellMode::Cantrip => "",
            SpellMode::Prepared => "Prepared",
            SpellMode::AlwaysPrepared => "Always",
            SpellMode::Known => "Known",
            SpellMode::Spellbook => "Book",
            SpellMode::Innate(u) => u.as_str(),
        };
        let entry = by_key.entry((s.level, s.name.clone(), s.key.clone())).or_insert_with(|| SpellLine {
            key: s.key.clone(),
            name: s.name.clone(),
            level: s.level,
            school: s.school_name().to_string(),
            time: s.time.clone(),
            range: s.range.clone(),
            components: s.components.clone(),
            duration: s.duration.clone(),
            concentration: s.concentration,
            ritual: s.ritual,
            status: String::new(),
            source_label: k.origin.clone(),
            ability: k.ability,
            body: s.body.clone(),
        });
        // Prepared outranks "in spellbook"; innate notes accumulate.
        let rank = |st: &str| match st {
            "Always" => 4,
            "Prepared" => 3,
            "Known" => 2,
            "Book" => 1,
            "" => 0,
            _ => 5,
        };
        if rank(status) > rank(&entry.status) {
            entry.status = status.to_string();
            entry.source_label = k.origin.clone();
        }
    }
    by_key.into_values().collect()
}
