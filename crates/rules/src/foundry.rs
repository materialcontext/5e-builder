//! Export to Foundry VTT's D&D 5e system (dnd5e 6.x data model, checked
//! against 6.1.2).
//!
//! The result is the JSON Foundry's "Import Data" accepts for a character
//! actor: the actor's own `system` data plus embedded items (class,
//! subclass, species, background, features, feats, spells, equipment).
//!
//! The approach is "bake, then let Foundry derive": final numbers (ability
//! scores, proficiencies, speeds, senses) are written to the actor, while
//! things Foundry derives itself (AC, hit points, spell slots, attack
//! bonuses) are set up so its own calculation lands on the same number as
//! this app's sheet. Where it would not, the difference is written as a
//! bonus or an override. That keeps the actor working in Foundry after
//! import: equip different armor, level up, or change a score, and Foundry
//! recalculates.
//!
//! Item ids are derived from the character id and a label, so exporting the
//! same character twice gives the same ids (re-importing updates in place).

use crate::build::{Build, FeatureKind, KnownSpell, SpellMode};
use crate::character::Character;
use crate::sheet::{Proficiency, Sheet};
use compendium::{
    title_case, Ability, ArmorKind, Caster, Compendium, Entry, Grant, Item, Key, Prof, Skill, Spell,
};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

/// The Foundry actor for `ch`, as a JSON value.
pub fn actor(ch: &Character, comp: &Compendium) -> Value {
    let b = crate::build(ch, comp);
    let s = crate::sheet(ch, &b, comp);
    Export { ch, b: &b, s: &s, comp, ids: Ids::new(&ch.id), items: Vec::new() }.actor()
}

/// The Foundry actor for `ch`, as pretty-printed JSON ready to save.
pub fn actor_json(ch: &Character, comp: &Compendium) -> String {
    serde_json::to_string_pretty(&actor(ch, comp)).unwrap_or_default()
}

/// File name for the export: `<name>.foundry.json`.
pub fn file_name(ch: &Character) -> String {
    let base = slug(&ch.name);
    format!("{}.foundry.json", if base.is_empty() { "character" } else { &base })
}

struct Export<'a> {
    ch: &'a Character,
    b: &'a Build,
    s: &'a Sheet,
    comp: &'a Compendium,
    ids: Ids,
    items: Vec<Value>,
}

impl Export<'_> {
    fn actor(mut self) -> Value {
        let classes = self.class_items();
        let original_class = classes.first().cloned().unwrap_or_default();
        let race = self.species_item();
        let background = self.background_item();
        self.feature_items();
        self.spell_items();
        self.inventory_items();
        self.other_proficiencies_item();

        let system = json!({
            "abilities": self.abilities(),
            "skills": self.skills(),
            "tools": self.tools(),
            "attributes": self.attributes(),
            "details": self.details(original_class, race, background),
            "traits": self.traits(),
            "currency": {
                "pp": self.ch.coins.pp, "gp": self.ch.coins.gp, "ep": self.ch.coins.ep,
                "sp": self.ch.coins.sp, "cp": self.ch.coins.cp,
            },
            "spells": self.spell_slots(),
            "identifier": slug(&self.ch.name),
            "bonuses": self.global_bonuses(),
        });
        let name = if self.ch.name.trim().is_empty() { "Unnamed character".to_string() } else { self.ch.name.clone() };
        let mut flags = Map::new();
        flags.insert(
            "5e-builder".into(),
            json!({ "characterId": self.ch.id, "exported": self.ch.updated, "dataVersion": self.comp.data_version }),
        );
        if self.b.has_feature_named("Jack of All Trades") {
            flags.insert("dnd5e".into(), json!({ "jackOfAllTrades": true }));
        }
        json!({
            "name": name,
            "type": "character",
            "system": system,
            "items": self.items,
            "effects": [],
            "flags": flags,
            "prototypeToken": {
                "name": name,
                "actorLink": true,
                "disposition": 1,
                "sight": { "enabled": true, "range": self.s.darkvision },
            },
        })
    }

    // ---------------------------------------------------------------- actor

    fn abilities(&self) -> Value {
        let mut out = Map::new();
        for line in &self.s.abilities {
            let a = line.ability;
            let mut v = json!({ "value": line.score, "proficient": i32::from(line.save_prof) });
            // A raised maximum (epic boons, some magic items).
            let max = self.b.grants.iter().filter_map(|g| match g.grant {
                Grant::AbilityMax { ability, max } if ability == a => Some(max as i32),
                _ => None,
            });
            if let Some(m) = max.max().filter(|m| *m > 20) {
                v["max"] = json!(m);
            }
            // Anything beyond modifier + proficiency (e.g. an aura) as a save bonus.
            let expected = line.modifier + if line.save_prof { self.s.proficiency_bonus } else { 0 };
            if line.save != expected {
                v["save"] = json!({ "roll": { "bonus": signed(line.save - expected) } });
            }
            out.insert(ability_id(a).into(), v);
        }
        Value::Object(out)
    }

    fn skills(&self) -> Value {
        let mut out = Map::new();
        for line in &self.s.skills {
            let (value, mult) = match line.prof {
                Proficiency::None => (0.0, 0.0),
                Proficiency::Half => (0.5, 0.5),
                Proficiency::Proficient => (1.0, 1.0),
                Proficiency::Expertise => (2.0, 2.0),
            };
            let ability = line.skill.ability();
            let mut v = json!({ "value": value, "ability": ability_id(ability) });
            let expected = self.b.modifier(ability) + (mult * self.s.proficiency_bonus as f32).floor() as i32;
            if line.bonus != expected {
                v["roll"] = json!({ "bonus": signed(line.bonus - expected) });
            }
            out.insert(skill_id(line.skill).into(), v);
        }
        Value::Object(out)
    }

    fn tools(&self) -> Value {
        let expert: BTreeSet<String> = self
            .b
            .grants
            .iter()
            .filter_map(|g| match &g.grant {
                Grant::ToolExpertise(t) => Some(t.to_lowercase()),
                _ => None,
            })
            .collect();
        let mut out = Map::new();
        for p in self.b.profs() {
            let Prof::Tool(name) = p else { continue };
            if let Some((id, ability)) = tool_id(name) {
                let value = if expert.contains(&name.to_lowercase()) { 2 } else { 1 };
                out.insert(id.into(), json!({ "value": value, "ability": ability }));
            }
        }
        Value::Object(out)
    }

    fn attributes(&self) -> Value {
        let s = self.s;
        let mut speeds = Map::new();
        for (k, v) in [
            ("walk", s.speed.walk),
            ("fly", s.speed.fly),
            ("swim", s.speed.swim),
            ("climb", s.speed.climb),
            ("burrow", s.speed.burrow),
        ] {
            if v > 0 || k == "walk" {
                speeds.insert(k.into(), json!(v.to_string()));
            }
        }
        let (ranges, special) = self.senses();
        let init_bonus = s.initiative - self.b.modifier(Ability::Dex);
        let caster = s.casting.first().map(|c| ability_id(c.ability)).unwrap_or("");
        json!({
            "ac": self.armor_class(),
            "hp": self.hit_points(),
            "init": { "ability": "", "roll": { "bonus": if init_bonus != 0 { signed(init_bonus) } else { String::new() } } },
            "movement": { "speeds": speeds, "units": "ft", "hover": false },
            "senses": { "ranges": ranges, "units": "ft", "special": special },
            "spellcasting": caster,
            "inspiration": false,
            "death": { "success": 0, "failure": 0 },
            "exhaustion": 0,
        })
    }

    fn senses(&self) -> (Value, String) {
        let mut ranges = Map::new();
        if self.s.darkvision > 0 {
            ranges.insert("darkvision".into(), json!(self.s.darkvision));
        }
        let mut special = Vec::new();
        for line in &self.s.senses {
            // "Blindsight 10 ft."
            let mut parts = line.split_whitespace();
            let name = parts.next().unwrap_or("").to_lowercase();
            let range: Option<u16> = parts.next().and_then(|n| n.parse().ok());
            match (name.as_str(), range) {
                ("blindsight" | "tremorsense" | "truesight" | "darkvision", Some(r)) => {
                    let e = ranges.entry(name).or_insert(json!(0));
                    *e = json!(e.as_u64().unwrap_or(0).max(r as u64));
                }
                _ => special.push(line.clone()),
            }
        }
        (Value::Object(ranges), special.join("; "))
    }

    /// Foundry picks the best of its enabled AC formulas (plus shield). Enable
    /// the special formula that matches this character's (Unarmored Defense,
    /// Draconic Resilience, ...) and fall back to a flat override only when
    /// no formula reproduces the sheet's number.
    fn armor_class(&self) -> Value {
        let m = |a| self.b.modifier(a);
        let dex = m(Ability::Dex);
        let equipped = |kind: fn(&ArmorKind) -> bool| {
            self.ch
                .inventory
                .iter()
                .filter(|i| i.equipped)
                .filter_map(|i| i.item.as_ref().and_then(|k| self.comp.items.get(k)))
                .find(|it| it.armor.as_ref().is_some_and(|a| kind(&a.kind)))
        };
        let armor = equipped(|k| *k != ArmorKind::Shield);
        let shield = equipped(|k| *k == ArmorKind::Shield);
        let shield_ac = shield.and_then(|s| s.armor.as_ref().map(|a| a.ac as i32 + s.bonus as i32)).unwrap_or(0);
        let armored = armor.and_then(|it| {
            let a = it.armor.as_ref()?;
            let base = a.ac as i32 + it.bonus as i32;
            Some(match a.kind {
                ArmorKind::Light => base + dex,
                ArmorKind::Medium => base + dex.min(2),
                _ => base,
            })
        });
        let base = armored.unwrap_or(10 + dex);
        let target = self.s.armor_class;
        let mut calcs = vec!["unarmored", "armored"];
        if base + shield_ac != target && armor.is_none() {
            // Only the formulas this character's features provide.
            let has_class = |name: &str| self.ch.class_levels().iter().any(|(k, _)| k.name_part() == name);
            let special = [
                ("unarmoredBarb", 10 + dex + m(Ability::Con), true, has_class("barbarian")),
                ("unarmoredMonk", 10 + dex + m(Ability::Wis), false, has_class("monk")),
                ("unarmoredBard", 10 + dex + m(Ability::Cha), true, self.b.has_feature_named("Dazzling Footwork")),
                ("draconic", 13 + dex, true, self.b.has_feature_named("Draconic Resilience")),
            ];
            if let Some((id, ..)) = special.iter().filter(|x| x.3).find(|(_, v, shield_ok, _)| {
                let with_shield = if *shield_ok { shield_ac } else if shield.is_some() { -999 } else { 0 };
                v + with_shield == target
            }) {
                calcs.push(id);
                return json!({ "calcs": calcs, "flat": null, "override": null });
            }
        }
        let over = (base + shield_ac != target).then_some(target);
        json!({ "calcs": calcs, "flat": null, "override": over })
    }

    /// Hit points come from the class items' HitPoints advancement plus the
    /// Constitution modifier; per-level and flat extras become bonuses, and
    /// whatever is left over (so Foundry's total matches) an overall bonus.
    fn hit_points(&self) -> Value {
        let level = self.ch.total_level().max(1) as i32;
        let con = self.b.modifier(Ability::Con);
        let mut from_dice = 0;
        for (i, l) in self.ch.levels.iter().enumerate() {
            let hd = self.comp.classes.get(&l.class).map(|c| c.hit_die).unwrap_or(8) as i32;
            from_dice += if i == 0 {
                hd
            } else if self.ch.settings.average_hp {
                hd / 2 + 1
            } else {
                l.hp_roll.map(i32::from).unwrap_or(hd / 2 + 1)
            };
        }
        let per_level: i32 = self
            .b
            .grants
            .iter()
            .filter_map(|g| match g.grant {
                Grant::HpPerLevel(n) => Some(n as i32),
                _ => None,
            })
            .sum();
        let overall = self.s.hp_max - from_dice - con * level - per_level * level;
        json!({
            "value": self.s.hp_max,
            "max": null,
            "temp": 0,
            "tempmax": 0,
            "bonuses": {
                "level": if per_level != 0 { per_level.to_string() } else { String::new() },
                "overall": if overall != 0 { overall.to_string() } else { String::new() },
            },
        })
    }

    fn details(&self, original_class: String, race: String, background: String) -> Value {
        let d = &self.ch.details;
        let mut bio = String::new();
        for (title, text) in [("Backstory", &d.backstory), ("Allies & Organizations", &d.allies), ("Notes", &d.notes)] {
            if !text.trim().is_empty() {
                bio.push_str(&format!("<h3>{title}</h3>{}", paragraphs(text)));
            }
        }
        json!({
            "biography": { "value": bio, "public": "" },
            "alignment": d.alignment,
            "ideal": d.ideals,
            "bond": d.bonds,
            "flaw": d.flaws,
            "trait": d.personality,
            "appearance": d.appearance,
            "race": race,
            "background": background,
            "originalClass": original_class,
            "xp": { "value": d.xp },
            "gender": d.pronouns,
            "eyes": d.eyes,
            "height": d.height,
            "faith": d.faith,
            "hair": d.hair,
            "skin": d.skin,
            "age": d.age,
            "weight": d.weight,
        })
    }

    fn traits(&self) -> Value {
        let s = self.s;
        let (langs, lang_custom) = split_ids(&s.languages, language_id);
        let (armor, armor_custom) = split_ids(&s.armor_profs, armor_prof_id);
        let (weapons, weapon_custom) = split_ids(&s.weapon_profs, weapon_prof_id);
        // "Martial weapons with the light property": Foundry has no such
        // category, so list the base weapons it covers.
        let mut weapons = weapons;
        let mut weapon_custom: Vec<String> = weapon_custom.split(';').filter(|x| !x.is_empty()).map(String::from).collect();
        weapon_custom.retain(|label| {
            let covered: Vec<String> = self
                .comp
                .items
                .iter()
                .filter(|it| it.bonus == 0 && it.rarity.is_none())
                .filter(|it| it.weapon.as_ref().is_some_and(|w| crate::sheet::weapon_matches_filter(label, w)))
                .filter_map(|it| weapon_id(&it.name))
                .collect();
            for id in &covered {
                if !weapons.contains(id) {
                    weapons.push(id.clone());
                }
            }
            covered.is_empty()
        });
        let weapon_custom = weapon_custom.join(";");
        let damage = |list: &[String]| {
            let (v, c) = split_ids(list, damage_id);
            json!({ "value": v, "bypasses": [], "custom": c })
        };
        let (ci, ci_custom) = split_ids(&s.condition_immunities, condition_id);
        json!({
            "size": size_id(&s.size),
            "languages": { "value": langs, "custom": lang_custom },
            "armorProf": { "value": armor, "custom": armor_custom },
            "weaponProf": { "value": weapons, "custom": weapon_custom, "mastery": { "value": self.masteries(), "bonus": [] } },
            "dr": damage(&s.resistances),
            "di": damage(&s.immunities),
            "dv": damage(&s.vulnerabilities),
            "ci": { "value": ci, "custom": ci_custom },
        })
    }

    /// Weapons chosen for Weapon Mastery, as Foundry base-weapon ids.
    fn masteries(&self) -> Vec<String> {
        let mut out = Vec::new();
        for f in &self.s.features {
            if let Some(rest) = f.feature.name.strip_prefix("Weapon Mastery: ") {
                let name = rest.split(" (").next().unwrap_or("");
                if let Some(id) = weapon_id(name) {
                    if !out.contains(&id) {
                        out.push(id);
                    }
                }
            }
        }
        out
    }

    fn spell_slots(&self) -> Value {
        let mut out = Map::new();
        for (i, n) in self.s.slots.iter().enumerate() {
            out.insert(format!("spell{}", i + 1), json!({ "value": n, "override": null }));
        }
        if let Some((n, _)) = self.s.pact_slots {
            out.insert("pact".into(), json!({ "value": n, "override": null }));
        }
        Value::Object(out)
    }

    /// Spell save DC: Foundry computes 8 + ability + proficiency; any extra
    /// (e.g. an item) goes in the global spell DC bonus.
    fn global_bonuses(&self) -> Value {
        let extra = self
            .s
            .casting
            .first()
            .map(|c| c.save_dc - (8 + self.b.modifier(c.ability) + self.s.proficiency_bonus))
            .unwrap_or(0);
        json!({ "spell": { "dc": if extra != 0 { signed(extra) } else { String::new() } } })
    }

    // ---------------------------------------------------------------- items

    fn push(&mut self, label: &str, name: &str, kind: &str, system: Value) -> String {
        let id = self.ids.next(label);
        self.items.push(json!({
            "_id": id,
            "name": name,
            "type": kind,
            "system": system,
            "effects": [],
            "flags": {},
        }));
        id
    }

    fn source(&self, code: &str, legacy: bool) -> Value {
        json!({
            "book": self.comp.source_name(code),
            "custom": "",
            "rules": if legacy { "2014" } else { "2024" },
            "page": "",
        })
    }

    /// One item per class, with a HitPoints advancement holding the hit
    /// points taken at each level (so Foundry's total matches and levelling
    /// up there continues from the right place). Returns the class item ids.
    fn class_items(&mut self) -> Vec<String> {
        let mut ids = Vec::new();
        let first = self.ch.levels.first().map(|l| l.class.clone());
        for (key, levels) in self.ch.class_levels() {
            let Some(class) = self.comp.classes.get(&key) else { continue };
            let mut hp = Map::new();
            let mut n = 0;
            for (i, l) in self.ch.levels.iter().enumerate() {
                if l.class != key {
                    continue;
                }
                n += 1;
                let v = if i == 0 && Some(&key) == first.as_ref() {
                    json!("max")
                } else if self.ch.settings.average_hp || l.hp_roll.is_none() {
                    json!("avg")
                } else {
                    json!(l.hp_roll)
                };
                hp.insert(n.to_string(), v);
            }
            let adv_id = self.ids.next(&format!("adv-hp-{key}"));
            let mut advancement = Map::new();
            advancement.insert(
                adv_id.clone(),
                json!({ "_id": adv_id, "type": "HitPoints", "configuration": {}, "value": hp, "flags": {}, "hint": "" }),
            );
            let identifier = slug(&class.name);
            // Prepared-spell limit by level, as a scale value the class's
            // preparation formula reads (so it keeps up when levelling).
            let mut casting = spellcasting(class.spellcasting.caster, class.spellcasting.ability);
            let sc = &class.spellcasting;
            let table = if !sc.prepared.is_empty() { &sc.prepared } else { &sc.known };
            if !table.is_empty() {
                let mut scale = Map::new();
                let mut last = None;
                for (lvl, n) in table.iter().enumerate() {
                    if *n > 0 && last != Some(*n) {
                        scale.insert((lvl + 1).to_string(), json!({ "value": n }));
                        last = Some(*n);
                    }
                }
                let sv_id = self.ids.next(&format!("adv-prepared-{key}"));
                advancement.insert(
                    sv_id.clone(),
                    json!({
                        "_id": sv_id, "type": "ScaleValue", "name": "Max Prepared Spells",
                        "configuration": { "identifier": "max-prepared", "type": "number", "scale": scale, "distance": { "units": "" } },
                        "value": {}, "flags": {}, "hint": "",
                    }),
                );
                casting["preparation"]["formula"] = json!(format!("@scale.{identifier}.max-prepared"));
            }
            let state = self.b.classes.iter().find(|c| c.key == key);
            let description = class
                .features
                .iter()
                .filter(|f| f.level <= levels && !f.subclass_slot)
                .map(|f| format!("<h3>{} (level {})</h3>{}", escape(&f.name), f.level, Entry::html(&f.body)))
                .collect::<String>();
            let system = json!({
                "identifier": identifier,
                "description": { "value": description, "chat": "" },
                "source": self.source(&class.source, class.legacy),
                "levels": levels,
                "hd": { "denomination": format!("d{}", class.hit_die), "spent": 0, "additional": "" },
                "primaryAbility": { "value": class.primary.iter().map(|a| ability_id(*a)).collect::<Vec<_>>(), "all": false },
                "spellcasting": casting,
                "advancement": advancement,
                "properties": [],
            });
            let id = self.push(&format!("class-{key}"), &class.name, "class", system);
            ids.push(id);
            if let Some(sub) = state.and_then(|s| s.subclass.as_ref()).and_then(|k| self.comp.subclasses.get(k)) {
                let description = sub
                    .features
                    .iter()
                    .filter(|f| f.level <= levels)
                    .map(|f| format!("<h3>{} (level {})</h3>{}", escape(&f.name), f.level, Entry::html(&f.body)))
                    .collect::<String>();
                let system = json!({
                    "identifier": slug(&sub.short_name),
                    "classIdentifier": identifier,
                    "description": { "value": description, "chat": "" },
                    "source": self.source(&sub.source, sub.reprinted.is_some()),
                    "spellcasting": spellcasting(sub.spellcasting.caster, sub.spellcasting.ability),
                    "advancement": {},
                });
                self.push(&format!("subclass-{}", sub.key), &sub.name, "subclass", system);
            }
        }
        ids
    }

    fn species_item(&mut self) -> String {
        let Some(sp) = self.ch.species.as_ref().and_then(|k| self.comp.species.get(k)) else { return String::new() };
        let mut speeds = Map::new();
        for (k, v) in [("walk", sp.speed.walk), ("fly", sp.speed.fly), ("swim", sp.speed.swim), ("climb", sp.speed.climb), ("burrow", sp.speed.burrow)] {
            if v > 0 {
                speeds.insert(k.into(), json!(v.to_string()));
            }
        }
        let mut ranges = Map::new();
        if let Some(dv) = sp.darkvision {
            ranges.insert("darkvision".into(), json!(dv));
        }
        let description: String =
            sp.features.iter().map(|f| format!("<h3>{}</h3>{}", escape(&f.name), Entry::html(&f.body))).collect();
        let system = json!({
            "identifier": slug(&sp.name),
            "description": { "value": description, "chat": "" },
            "source": self.source(&sp.source, sp.legacy),
            "movement": { "speeds": speeds, "units": "ft", "hover": false },
            "senses": { "ranges": ranges, "units": "ft", "special": "" },
            "type": { "value": sp.creature_type.to_lowercase(), "subtype": "", "custom": "" },
            "advancement": {},
        });
        // The sheet's species line includes the chosen lineage, if any.
        let name = if self.s.species.is_empty() { sp.name.clone() } else { self.s.species.clone() };
        self.push(&format!("race-{}", sp.key), &name, "race", system)
    }

    fn background_item(&mut self) -> String {
        let Some(bg) = self.ch.background.as_ref().and_then(|k| self.comp.backgrounds.get(k)) else { return String::new() };
        let description: String =
            bg.features.iter().map(|f| format!("<h3>{}</h3>{}", escape(&f.name), Entry::html(&f.body))).collect();
        let system = json!({
            "identifier": slug(&bg.name),
            "description": { "value": description, "chat": "" },
            "source": self.source(&bg.source, bg.legacy),
            "advancement": {},
        });
        self.push(&format!("background-{}", bg.key), &bg.name, "background", system)
    }

    /// Class features, species traits, feats and the rest, one item each
    /// (identical repeats, e.g. several Ability Score Improvements, once).
    fn feature_items(&mut self) {
        let mut seen: Vec<(String, Vec<Entry>)> = Vec::new();
        let features = self.s.features.clone();
        for (i, f) in features.iter().enumerate() {
            let name = &f.feature.name;
            // Represented as weapon mastery proficiencies instead.
            if name.starts_with("Weapon Mastery: ") {
                continue;
            }
            if seen.iter().any(|(n, body)| n == name && *body == f.feature.body) {
                continue;
            }
            seen.push((name.clone(), f.feature.body.clone()));
            let optional = self.optional_feature(name);
            let (kind, subtype) = match f.kind {
                _ if optional.is_some() => ("class", optional.unwrap_or("")),
                FeatureKind::Class => ("class", ""),
                FeatureKind::Species => ("race", ""),
                FeatureKind::Background => ("background", ""),
                FeatureKind::Feat => ("feat", self.feat_subtype(name)),
                FeatureKind::Other => ("", ""),
            };
            let system = json!({
                "description": { "value": Entry::html(&f.feature.body), "chat": "" },
                "identifier": slug(name),
                "type": { "value": kind, "subtype": subtype },
                "requirements": self.requirements(f),
                "source": { "book": "", "custom": f.origin, "rules": "2024", "page": "" },
                "activities": {},
                "uses": { "max": "", "spent": 0, "recovery": [] },
                "advancement": {},
                "properties": [],
            });
            self.push(&format!("feature-{i}-{name}"), name, "feat", system);
        }
    }

    /// What Foundry shows under the name: the class level for features, the
    /// prerequisite for feats.
    fn requirements(&self, f: &crate::build::FeatureEntry) -> String {
        if f.kind != FeatureKind::Feat {
            return f.origin.clone();
        }
        let feat = self.b.feats.iter().filter_map(|(k, _)| self.comp.feats.get(k)).find(|x| x.name == f.feature.name);
        feat.and_then(|x| x.prereq.as_ref()).map(|p| p.text.clone()).unwrap_or_default()
    }

    fn feat_subtype(&self, name: &str) -> &'static str {
        let feat = self.b.feats.iter().filter_map(|(k, _)| self.comp.feats.get(k)).find(|f| f.name == name);
        match feat.and_then(|f| f.category.as_deref()) {
            Some("O") => "origin",
            Some("G") => "general",
            Some(c) if c.starts_with("FS") => "fightingStyle",
            Some("EB") => "epicBoon",
            _ => "",
        }
    }

    /// For a chosen optional class feature (invocation, maneuver, ...): its
    /// Foundry subtype ("" when Foundry has none).
    fn optional_feature(&self, name: &str) -> Option<&'static str> {
        let of = self.b.optional_features.iter().filter_map(|(k, _)| self.comp.optional_features.get(k)).find(|o| o.name == name)?;
        Some(match of.types.first().map(String::as_str).unwrap_or("") {
            "EI" => "eldritchInvocation",
            "MM" => "metamagic",
            "AI" => "artificerInfusion",
            t if t.starts_with("MV") => "maneuver",
            t if t.starts_with("FS") => "fightingStyle",
            "AS" | "AS:V1-UA" | "AS:V2-UA" => "arcaneShot",
            "ED" => "elementalDiscipline",
            "RN" => "rune",
            _ => "",
        })
    }

    fn spell_items(&mut self) {
        // One item per spell and casting method; a spell known two ways
        // (e.g. prepared, and once a day from a feat) appears in both sections.
        let mut done: Vec<(Key, &'static str)> = Vec::new();
        let mut spells: Vec<KnownSpell> = self.b.spells.clone();
        // Strongest preparation first, so always-prepared wins over prepared.
        spells.sort_by_key(|k| match k.mode {
            SpellMode::AlwaysPrepared => 0,
            SpellMode::Prepared | SpellMode::Known | SpellMode::Cantrip => 1,
            _ => 2,
        });
        for (i, k) in spells.iter().enumerate() {
            let Some(spell) = self.comp.spells.get(&k.key) else { continue };
            let class = k.class.as_ref().and_then(|c| self.comp.classes.get(c));
            let pact = class.is_some_and(|c| c.spellcasting.caster == Some(Caster::Pact));
            let (method, prepared, uses) = spell_method(&k.mode, spell.level, pact);
            if done.contains(&(k.key.clone(), method)) {
                continue;
            }
            done.push((k.key.clone(), method));
            let class_ability = class.and_then(|c| c.spellcasting.ability);
            // Only set when it differs from the class's (feats, species).
            let ability = match k.ability {
                Some(a) if Some(a) != class_ability => ability_id(a),
                _ => "",
            };
            let source_item = class.map(|c| format!("class:{}", slug(&c.name))).unwrap_or_default();
            let activity_id = self.ids.next(&format!("act-spell-{i}-{}", k.key));
            let consumes_uses = uses.is_some();
            let system = json!({
                "description": { "value": Entry::html(&spell.body), "chat": "" },
                "identifier": slug(&spell.name),
                "source": self.source(&spell.source, false),
                "level": spell.level,
                "school": school_id(&spell.school),
                "properties": spell_properties(spell),
                "materials": materials(&spell.components),
                "activation": activation(&spell.time),
                "duration": duration(&spell.duration),
                "range": range(&spell.range),
                "target": spell_target(spell),
                "method": method,
                "prepared": prepared,
                "ability": ability,
                "sourceItem": source_item,
                "uses": uses.unwrap_or_else(|| json!({ "max": "", "spent": 0, "recovery": [] })),
                "activities": spell_activities(&activity_id, spell, consumes_uses),
            });
            self.push(&format!("spell-{i}-{}", k.key), &spell.name, "spell", system);
        }
    }

    fn inventory_items(&mut self) {
        let inventory = self.ch.inventory.clone();
        for (i, inv) in inventory.iter().enumerate() {
            let label = format!("item-{i}-{}", inv.name);
            let Some(it) = inv.item.as_ref().and_then(|k| self.comp.items.get(k)) else {
                let system = json!({
                    "description": { "value": paragraphs(&inv.notes), "chat": "" },
                    "quantity": inv.qty,
                    "weight": { "value": 0, "units": "lb" },
                    "type": { "value": "gear", "subtype": "" },
                    "identified": true,
                });
                self.push(&label, &inv.name, "loot", system);
                continue;
            };
            if it.contents.is_empty() {
                let name = if inv.name.is_empty() { it.name.clone() } else { inv.name.clone() };
                let held = Held { qty: inv.qty, equipped: inv.equipped, attuned: inv.attuned, notes: inv.notes.clone(), container: None };
                self.physical_item(&label, &name, it, held);
            } else {
                self.pack(&label, it, inv.qty);
            }
        }
    }

    /// An equipment pack: its contents as separate items, placed inside the
    /// pack's own container (its backpack, chest, ...), as Foundry shows them.
    fn pack(&mut self, label: &str, pack: &Item, qty: u16) {
        let contents: Vec<(&Item, u16)> =
            pack.contents.iter().filter_map(|(k, n)| self.comp.items.get(k).map(|it| (it, n * qty))).collect();
        let holder = contents.iter().position(|(it, _)| container_capacity(&it.name).is_some());
        let note = format!("From {}.", pack.name);
        let container = match holder {
            Some(h) => {
                let (it, n) = contents[h];
                let held = Held { qty: n, equipped: false, attuned: false, notes: note, container: None };
                self.physical_item(&format!("{label}-{h}"), &it.name, it, held)
            }
            // No bag in the pack: the pack itself holds everything.
            None => {
                let system = json!({
                    "description": { "value": Entry::html(&pack.body), "chat": "" },
                    "identifier": slug(&pack.name),
                    "source": self.source(&pack.source, false),
                    "quantity": qty,
                    "weight": { "value": 0, "units": "lb" },
                    "price": price(pack.value_cp),
                    "capacity": { "weight": { "value": null, "units": "lb" } },
                    "currency": { "pp": 0, "gp": 0, "ep": 0, "sp": 0, "cp": 0 },
                    "identified": true,
                });
                self.push(label, &pack.name, "container", system)
            }
        };
        for (j, (it, n)) in contents.into_iter().enumerate() {
            if Some(j) == holder {
                continue;
            }
            let held = Held { qty: n, equipped: false, attuned: false, notes: String::new(), container: Some(container.clone()) };
            self.physical_item(&format!("{label}-{j}"), &it.name, it, held);
        }
    }

    fn physical_item(&mut self, label: &str, name: &str, it: &Item, held: Held) -> String {
        let mut description = Entry::html(&it.body);
        if !held.notes.trim().is_empty() {
            description.push_str(&paragraphs(&held.notes));
        }
        let mut system = json!({
            "description": { "value": description, "chat": "" },
            "identifier": slug(&it.name),
            "source": self.source(&it.source, false),
            "quantity": held.qty,
            "weight": { "value": it.weight, "units": "lb" },
            "price": price(it.value_cp),
            "rarities": rarity(it.rarity.as_deref()).into_iter().collect::<Vec<_>>(),
            "identified": true,
            "container": held.container,
        });
        let (kind, extra) = self.physical(it, label);
        if kind != "loot" {
            // Loot can't be equipped or attuned.
            system["attunement"] = json!(if it.attunement { "required" } else { "" });
            system["attuned"] = json!(held.attuned);
            system["equipped"] = json!(held.equipped);
        }
        if let (Value::Object(base), Value::Object(extra)) = (&mut system, extra) {
            base.extend(extra);
        }
        self.push(label, name, kind, system)
    }

    /// Item type and type-specific system data.
    fn physical(&mut self, it: &Item, label: &str) -> (&'static str, Value) {
        let magic = it.bonus != 0 || it.rarity.as_deref().is_some_and(|r| r != "none" && r != "mundane");
        if let Some(w) = &it.weapon {
            let kind = match (w.martial, w.ranged) {
                (false, false) => "simpleM",
                (false, true) => "simpleR",
                (true, false) => "martialM",
                (true, true) => "martialR",
            };
            let mut props: Vec<&str> = w.properties.iter().filter_map(|p| weapon_property(p)).collect();
            if magic {
                props.push("mgc");
            }
            let (short, long) = w.range.as_deref().map(parse_range_pair).unwrap_or((None, None));
            let base = dice(&w.damage).map(|(n, d)| json!({ "number": n, "denomination": d, "types": [w.damage_type.to_lowercase()], "bonus": "", "custom": { "enabled": false, "formula": "" } }));
            let base = base.unwrap_or_else(|| {
                json!({ "number": null, "denomination": null, "types": [w.damage_type.to_lowercase()], "bonus": "", "custom": { "enabled": true, "formula": w.damage } })
            });
            let versatile = w.versatile.as_deref().and_then(dice).map(|(n, d)| json!({ "number": n, "denomination": d, "types": [w.damage_type.to_lowercase()], "bonus": "", "custom": { "enabled": false, "formula": "" } }));
            let act = self.ids.next(&format!("act-{label}"));
            let attack_kind = if w.ranged { "ranged" } else { "melee" };
            let mut activities = Map::new();
            activities.insert(
                act.clone(),
                json!({
                    "_id": act, "type": "attack", "sort": 0, "name": "",
                    "activation": { "type": "action", "value": 1, "override": false },
                    "attack": { "ability": "", "bonus": "", "flat": false, "type": { "value": attack_kind, "classification": "weapon" }, "critical": { "threshold": null } },
                    "damage": { "includeBase": true, "parts": [], "critical": { "bonus": "" } },
                    "consumption": { "targets": [], "scaling": { "allowed": false }, "spellSlot": false },
                    "uses": { "spent": 0, "max": "", "recovery": [] },
                }),
            );
            return (
                "weapon",
                json!({
                    "type": { "value": kind, "baseItem": weapon_id(&it.name).unwrap_or_default() },
                    "damage": { "base": base, "versatile": versatile.unwrap_or(json!({ "types": [] })) },
                    "properties": props,
                    "mastery": w.mastery.as_deref().map(str::to_lowercase).unwrap_or_default(),
                    "range": { "value": short, "long": long, "units": "ft", "reach": if w.ranged { Value::Null } else if w.properties.iter().any(|p| p.starts_with("reach")) { json!(10) } else { json!(5) } },
                    "magicalBonus": if it.bonus != 0 { it.bonus.to_string() } else { String::new() },
                    "proficient": null,
                    "activities": activities,
                }),
            );
        }
        if let Some(a) = &it.armor {
            let (kind, dex) = match a.kind {
                ArmorKind::Light => ("light", Value::Null),
                ArmorKind::Medium => ("medium", json!(2)),
                ArmorKind::Heavy => ("heavy", json!(0)),
                ArmorKind::Shield => ("shield", Value::Null),
            };
            let mut props = Vec::new();
            if a.stealth_disadvantage {
                props.push("stealthDisadvantage");
            }
            if magic {
                props.push("mgc");
            }
            return (
                "equipment",
                json!({
                    "type": { "value": kind, "baseItem": armor_id(&it.name).unwrap_or_default() },
                    "armor": { "value": a.ac, "dex": dex, "magicalBonus": if it.bonus != 0 { it.bonus.to_string() } else { String::new() } },
                    "strength": a.strength,
                    "properties": props,
                    "proficient": null,
                }),
            );
        }
        let kind = it.kind.to_lowercase();
        if let Some(capacity) = container_capacity(&it.name) {
            return ("container", json!({ "capacity": capacity, "currency": { "pp": 0, "gp": 0, "ep": 0, "sp": 0, "cp": 0 }, "properties": [] }));
        }
        let lower = it.name.to_lowercase();
        if ["clothes", "robe", "costume", "vestments"].iter().any(|c| lower.contains(c)) && it.armor.is_none() {
            return ("equipment", json!({ "type": { "value": "clothing", "baseItem": "" }, "armor": { "value": null, "dex": null }, "properties": [] }));
        }
        if let Some(sub) = consumable_gear(&it.name) {
            let t = if sub == "food" { "food" } else { "" };
            return ("consumable", json!({ "type": { "value": t, "subtype": "" }, "uses": { "max": "1", "spent": 0, "autoDestroy": true, "recovery": [] }, "properties": if t == "food" { vec!["nutritious"] } else { vec![] } }));
        }
        if let Some((id, ability)) = tool_id(&it.name) {
            let tool_type = if kind.contains("artisan") {
                "art"
            } else if kind.contains("instrument") {
                "music"
            } else if kind.contains("gaming") {
                "game"
            } else {
                ""
            };
            return ("tool", json!({ "type": { "value": tool_type, "baseItem": id }, "ability": ability, "proficient": null }));
        }
        let consumable = |t: &str, sub: &str| ("consumable", json!({ "type": { "value": t, "subtype": sub }, "uses": { "max": "1", "spent": 0, "autoDestroy": true, "recovery": [] } }));
        let equipment = |t: &str| ("equipment", json!({ "type": { "value": t, "baseItem": "" }, "armor": { "value": null }, "properties": if magic { vec!["mgc"] } else { vec![] } }));
        match kind.as_str() {
            "potion" => consumable("potion", ""),
            "scroll" | "spell scroll" => consumable("scroll", ""),
            "ammunition" => {
                let n = it.name.to_lowercase();
                let sub = if n.contains("arrow") {
                    "arrow"
                } else if n.contains("bolt") {
                    "crossbowBolt"
                } else if n.contains("needle") {
                    "blowgunNeedle"
                } else if n.contains("bullet") {
                    "slingBullet"
                } else {
                    ""
                };
                consumable("ammo", sub)
            }
            "food and drink" => consumable("food", ""),
            "poison" => consumable("poison", ""),
            "ring" => equipment("ring"),
            "rod" => equipment("rod"),
            "wand" => equipment("wand"),
            "wondrous item" => equipment("wondrous"),
            "staff" => equipment("wondrous"),
            "gemstone" => ("loot", json!({ "type": { "value": "gem", "subtype": "" } })),
            "art object" => ("loot", json!({ "type": { "value": "art", "subtype": "" } })),
            "trade good" => ("loot", json!({ "type": { "value": "material", "subtype": "" } })),
            _ => {
                let mut props = Vec::new();
                if magic {
                    props.push("mgc");
                }
                if kind == "spellcasting focus" {
                    props.push("foc");
                }
                ("loot", json!({ "type": { "value": "gear", "subtype": "" }, "properties": props }))
            }
        }
    }

    /// Tool proficiencies Foundry has no id for (vehicles, uncommon kits)
    /// are listed on a single feature so nothing is lost.
    fn other_proficiencies_item(&mut self) {
        let other: Vec<String> = self
            .b
            .profs()
            .filter_map(|p| match p {
                Prof::Tool(t) if tool_id(t).is_none() => Some(title_case(t)),
                _ => None,
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if other.is_empty() {
            return;
        }
        let list: String = other.iter().map(|t| format!("<li>{}</li>", escape(t))).collect();
        let system = json!({
            "description": { "value": format!("<p>Proficient with:</p><ul>{list}</ul>"), "chat": "" },
            "type": { "value": "", "subtype": "" },
            "activities": {},
        });
        self.push("other-proficiencies", "Other Tool Proficiencies", "feat", system);
    }
}

// -------------------------------------------------------------------- ids

/// Deterministic 16-character alphanumeric ids, as Foundry uses.
struct Ids {
    seed: String,
    used: BTreeSet<String>,
}

impl Ids {
    fn new(seed: &str) -> Self {
        Ids { seed: seed.to_string(), used: BTreeSet::new() }
    }

    fn next(&mut self, label: &str) -> String {
        const ALPHABET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
        let mut salt = 0u32;
        loop {
            let mut a = fnv(format!("{}\u{1}{label}\u{1}{salt}", self.seed).as_bytes());
            let mut b = fnv(format!("{salt}\u{2}{label}\u{2}{}", self.seed).as_bytes());
            let mut id = String::with_capacity(16);
            for i in 0..16 {
                let h = if i < 8 { &mut a } else { &mut b };
                id.push(ALPHABET[(*h % 62) as usize] as char);
                *h /= 62;
            }
            if self.used.insert(id.clone()) {
                return id;
            }
            salt += 1;
        }
    }
}

fn fnv(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ---------------------------------------------------------------- mapping

fn ability_id(a: Ability) -> &'static str {
    match a {
        Ability::Str => "str",
        Ability::Dex => "dex",
        Ability::Con => "con",
        Ability::Int => "int",
        Ability::Wis => "wis",
        Ability::Cha => "cha",
    }
}

fn skill_id(s: Skill) -> &'static str {
    match s.id() {
        "acrobatics" => "acr",
        "animal handling" => "ani",
        "arcana" => "arc",
        "athletics" => "ath",
        "deception" => "dec",
        "history" => "his",
        "insight" => "ins",
        "intimidation" => "itm",
        "investigation" => "inv",
        "medicine" => "med",
        "nature" => "nat",
        "perception" => "prc",
        "performance" => "prf",
        "persuasion" => "per",
        "religion" => "rel",
        "sleight of hand" => "slt",
        "stealth" => "ste",
        _ => "sur",
    }
}

fn size_id(size: &str) -> &'static str {
    match size.trim().to_lowercase().as_str() {
        "tiny" => "tiny",
        "small" => "sm",
        "large" => "lg",
        "huge" => "huge",
        "gargantuan" => "grg",
        _ => "med",
    }
}

fn school_id(abbr: &str) -> &'static str {
    match abbr {
        "A" => "abj",
        "C" => "con",
        "D" => "div",
        "E" => "enc",
        "V" => "evo",
        "I" => "ill",
        "N" => "nec",
        "T" => "trs",
        _ => "",
    }
}

/// Lowercase letters and digits only, for matching names to Foundry ids.
fn squash(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_lowercase()
}

/// Kebab-case identifier, as Foundry uses for classes, spells, ...
fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if c != '\'' && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

const WEAPONS: &[&str] = &[
    "battleaxe", "blowgun", "club", "dagger", "dart", "flail", "glaive", "greataxe", "greatclub", "greatsword",
    "halberd", "handaxe", "handcrossbow", "heavycrossbow", "javelin", "lance", "lightcrossbow", "lighthammer",
    "longbow", "longsword", "mace", "maul", "morningstar", "musket", "pike", "pistol", "quarterstaff", "rapier",
    "scimitar", "shortsword", "sickle", "spear", "shortbow", "sling", "trident", "warpick", "warhammer", "whip",
];

fn weapon_id(name: &str) -> Option<String> {
    // Proficiency lists use plurals: "Longswords", "Hand Crossbows".
    let s = squash(name);
    let candidates = [s.as_str(), s.strip_suffix('s').unwrap_or(""), s.strip_suffix("es").unwrap_or("")];
    let found = candidates.into_iter().find(|x| WEAPONS.iter().any(|w| w == x)).map(str::to_string);
    found
}

fn armor_id(name: &str) -> Option<String> {
    let s = squash(name).replace("armor", "");
    let id = match s.as_str() {
        "breastplate" => "breastplate",
        "chainmail" => "chainmail",
        "chainshirt" => "chainshirt",
        "halfplate" => "halfplate",
        "hide" => "hide",
        "leather" => "leather",
        "padded" => "padded",
        "plate" => "plate",
        "ringmail" => "ringmail",
        "scalemail" => "scalemail",
        "splint" => "splint",
        "studdedleather" => "studded",
        "shield" => "shield",
        _ => return None,
    };
    Some(id.to_string())
}

/// Foundry tool id and its default ability.
fn tool_id(name: &str) -> Option<(&'static str, &'static str)> {
    let s = squash(name);
    let s = s.trim_end_matches('s');
    Some(match s {
        "alchemistssupplie" => ("alchemist", "int"),
        "bagpipe" => ("bagpipes", "cha"),
        "brewerssupplie" => ("brewer", "int"),
        "calligrapherssupplie" => ("calligrapher", "dex"),
        "playingcardset" | "playingcard" => ("card", "wis"),
        "carpenterstool" => ("carpenter", "str"),
        "cartographerstool" => ("cartographer", "wis"),
        "dragonchessset" => ("chess", "wis"),
        "cobblerstool" => ("cobbler", "dex"),
        "cooksutensil" => ("cook", "wis"),
        "diceset" => ("dice", "wis"),
        "disguisekit" => ("disg", "cha"),
        "drum" => ("drum", "cha"),
        "dulcimer" => ("dulcimer", "cha"),
        "flute" => ("flute", "cha"),
        "forgerykit" => ("forg", "dex"),
        "glassblowerstool" => ("glassblower", "int"),
        "herbalismkit" => ("herb", "int"),
        "horn" => ("horn", "cha"),
        "jewelerstool" => ("jeweler", "int"),
        "leatherworkerstool" => ("leatherworker", "dex"),
        "lute" => ("lute", "cha"),
        "lyre" => ("lyre", "cha"),
        "masonstool" => ("mason", "str"),
        "navigatorstool" => ("navg", "wis"),
        "painterssupplie" => ("painter", "wis"),
        "panflute" => ("panflute", "cha"),
        "poisonerskit" => ("pois", "int"),
        "potterstool" => ("potter", "int"),
        "shawm" => ("shawm", "cha"),
        "smithstool" => ("smith", "str"),
        "thievestool" => ("thief", "dex"),
        "tinkerstool" => ("tinker", "dex"),
        "viol" => ("viol", "cha"),
        "weaverstool" => ("weaver", "dex"),
        "woodcarverstool" => ("woodcarver", "dex"),
        _ => return None,
    })
}

fn language_id(name: &str) -> Option<String> {
    let id = match squash(name).as_str() {
        "common" => "common",
        "commonsignlanguage" | "signlanguage" => "sign",
        "draconic" => "draconic",
        "dwarvish" => "dwarvish",
        "elvish" => "elvish",
        "giant" => "giant",
        "gnomish" => "gnomish",
        "goblin" => "goblin",
        "halfling" => "halfling",
        "orc" => "orc",
        "aarakocra" => "aarakocra",
        "abyssal" => "abyssal",
        "thievescant" => "cant",
        "celestial" => "celestial",
        "deepspeech" => "deep",
        "druidic" => "druidic",
        "gith" => "gith",
        "gnoll" => "gnoll",
        "infernal" => "infernal",
        "primordial" => "primordial",
        "aquan" => "aquan",
        "auran" => "auran",
        "ignan" => "ignan",
        "terran" => "terran",
        "sylvan" => "sylvan",
        "undercommon" => "undercommon",
        _ => return None,
    };
    Some(id.to_string())
}

fn armor_prof_id(name: &str) -> Option<String> {
    let s = name.to_lowercase();
    let id = if s.starts_with("light") {
        "lgt"
    } else if s.starts_with("medium") {
        "med"
    } else if s.starts_with("heavy") {
        "hvy"
    } else if s.starts_with("shield") {
        "shl"
    } else {
        return None;
    };
    Some(id.to_string())
}

fn weapon_prof_id(name: &str) -> Option<String> {
    let s = name.to_lowercase();
    match s.as_str() {
        "simple" | "simple weapons" | "simple weapon" => Some("sim".into()),
        "martial" | "martial weapons" | "martial weapon" => Some("mar".into()),
        _ => weapon_id(&s),
    }
}

const DAMAGE: &[&str] = &[
    "acid", "bludgeoning", "cold", "fire", "force", "lightning", "necrotic", "piercing", "poison", "psychic",
    "radiant", "slashing", "thunder",
];

fn damage_id(name: &str) -> Option<String> {
    let s = name.trim().to_lowercase();
    DAMAGE.contains(&s.as_str()).then_some(s)
}

const CONDITIONS: &[&str] = &[
    "blinded", "charmed", "deafened", "diseased", "exhaustion", "frightened", "grappled", "incapacitated",
    "invisible", "paralyzed", "petrified", "poisoned", "prone", "restrained", "stunned",
];

fn condition_id(name: &str) -> Option<String> {
    let s = name.trim().to_lowercase();
    CONDITIONS.contains(&s.as_str()).then_some(s)
}

/// Splits labels into Foundry ids and a semicolon-separated custom list.
fn split_ids(list: &[String], id: impl Fn(&str) -> Option<String>) -> (Vec<String>, String) {
    let mut ids = Vec::new();
    let mut custom = Vec::new();
    for x in list {
        match id(x) {
            Some(i) if !ids.contains(&i) => ids.push(i),
            Some(_) => {}
            None => custom.push(x.clone()),
        }
    }
    (ids, custom.join(";"))
}

fn weapon_property(p: &str) -> Option<&'static str> {
    let p = p.to_lowercase();
    Some(match p.split(['|', ' ']).next().unwrap_or("") {
        "ammunition" => "amm",
        "finesse" => "fin",
        "heavy" => "hvy",
        "light" => "lgt",
        "loading" => "lod",
        "reach" => "rch",
        "special" => "spc",
        "thrown" => "thr",
        "two-handed" => "two",
        "versatile" => "ver",
        "silvered" => "sil",
        _ => return None,
    })
}

fn spellcasting(caster: Option<Caster>, ability: Option<Ability>) -> Value {
    let progression = match caster {
        Some(Caster::Full) => "full",
        Some(Caster::Half) => "half",
        Some(Caster::Third) => "third",
        Some(Caster::Pact) => "pact",
        Some(Caster::Artificer) => "artificer",
        None => "none",
    };
    json!({
        "progression": progression,
        "ability": ability.map(ability_id).unwrap_or(""),
        "preparation": { "formula": "" },
    })
}

/// Foundry's casting method, preparation state (0 unprepared, 1 prepared,
/// 2 always) and, for limited free casts, the item's uses.
fn spell_method(mode: &SpellMode, level: u8, pact: bool) -> (&'static str, u8, Option<Value>) {
    let class_method = if pact && level > 0 { "pact" } else { "spell" };
    match mode {
        SpellMode::Cantrip => ("spell", 1, None),
        SpellMode::Prepared | SpellMode::Known => (class_method, 1, None),
        SpellMode::AlwaysPrepared => (class_method, 2, None),
        SpellMode::Spellbook => ("spell", 0, None),
        SpellMode::Innate(usage) => {
            let u = usage.trim().to_lowercase();
            if u.is_empty() || u == "at will" {
                return ("atwill", 0, None);
            }
            if u == "ritual" {
                return ("ritual", 0, None);
            }
            let (count, period) = match u.split_once('/') {
                Some((n, "long rest")) => (n.to_string(), "lr"),
                Some((n, "short rest")) => (n.to_string(), "sr"),
                Some((n, "day")) => (n.to_string(), "day"),
                _ => (u.trim_end_matches(" uses").trim_end_matches(" use").to_string(), ""),
            };
            let max = match count.trim() {
                "pb" => "@prof".to_string(),
                "int" | "int mod" => "@abilities.int.mod".into(),
                "wis" | "wis mod" => "@abilities.wis.mod".into(),
                "cha" | "cha mod" => "@abilities.cha.mod".into(),
                n => n.to_string(),
            };
            let recovery = if period.is_empty() { json!([]) } else { json!([{ "period": period, "type": "recoverAll" }]) };
            ("innate", 0, Some(json!({ "max": max, "spent": 0, "recovery": recovery })))
        }
    }
}

fn spell_properties(spell: &Spell) -> Vec<&'static str> {
    let c = spell.components.to_uppercase();
    let head = c.split('(').next().unwrap_or("");
    let mut out = Vec::new();
    if head.contains('V') {
        out.push("vocal");
    }
    if head.contains('S') {
        out.push("somatic");
    }
    if head.contains('M') {
        out.push("material");
    }
    if spell.concentration {
        out.push("concentration");
    }
    if spell.ritual {
        out.push("ritual");
    }
    out
}

fn materials(components: &str) -> Value {
    let text = components
        .split_once('(')
        .map(|(_, rest)| rest.trim_end().trim_end_matches(')').to_string())
        .unwrap_or_default();
    let lower = text.to_lowercase();
    // "worth 100+ GP", "worth at least 50 gp"
    let cost = lower
        .split("worth")
        .nth(1)
        .and_then(|w| w.split(|c: char| !c.is_ascii_digit() && c != ',').find(|t| !t.is_empty()))
        .and_then(|n| n.replace(',', "").parse::<u32>().ok())
        .unwrap_or(0);
    json!({ "value": text, "consumed": lower.contains("consume"), "cost": cost, "supply": 0 })
}

fn activation(time: &str) -> Value {
    let lower = time.to_lowercase();
    let (head, condition) = match lower.split_once(',') {
        Some((h, _)) => (h.trim().to_string(), time.split_once(',').map(|x| x.1.trim().to_string()).unwrap_or_default()),
        None => (lower.trim().to_string(), String::new()),
    };
    let n: Option<u32> = head.split_whitespace().next().and_then(|x| x.parse().ok());
    let kind = if head.contains("bonus action") {
        "bonus"
    } else if head.contains("reaction") {
        "reaction"
    } else if head.contains("action") {
        "action"
    } else if head.contains("minute") {
        "minute"
    } else if head.contains("hour") {
        "hour"
    } else {
        "special"
    };
    json!({ "type": kind, "value": n, "condition": condition })
}

fn duration(d: &str) -> Value {
    let lower = d.to_lowercase();
    let lower = lower.trim_start_matches("concentration, up to ").trim();
    let n = lower.split_whitespace().next().and_then(|x| x.parse::<u32>().ok());
    let unit = if lower.starts_with("instant") {
        "inst"
    } else if lower.contains("until dispelled") || lower.contains("permanent") {
        "perm"
    } else if lower.contains("round") {
        "round"
    } else if lower.contains("minute") {
        "minute"
    } else if lower.contains("hour") {
        "hour"
    } else if lower.contains("day") {
        "day"
    } else if lower.contains("turn") {
        "turn"
    } else {
        "spec"
    };
    let special = if unit == "spec" { d.to_string() } else { String::new() };
    json!({ "value": n.map(|n| n.to_string()).unwrap_or_default(), "units": unit, "special": special })
}

fn range(r: &str) -> Value {
    let lower = r.to_lowercase();
    let n = lower.split_whitespace().next().and_then(|x| x.replace(',', "").parse::<u32>().ok());
    let (units, value) = if lower.starts_with("self") {
        ("self", None)
    } else if lower.starts_with("touch") {
        ("touch", None)
    } else if lower.contains("mile") {
        ("mi", n)
    } else if n.is_some() {
        ("ft", n)
    } else if lower.contains("unlimited") {
        ("any", None)
    } else {
        ("spec", None)
    };
    let special = if units == "spec" || lower.contains('(') { r.to_string() } else { String::new() };
    json!({ "value": value.map(|v| v.to_string()), "units": units, "special": special })
}

/// "20/60" or "20/60 ft." → (20, 60).
fn parse_range_pair(r: &str) -> (Option<u32>, Option<u32>) {
    let nums: Vec<u32> =
        r.split(|c: char| !c.is_ascii_digit()).filter(|t| !t.is_empty()).filter_map(|t| t.parse().ok()).collect();
    (nums.first().copied(), nums.get(1).copied())
}

/// "2d6" → (2, 6). Formulas with modifiers return None.
fn dice(s: &str) -> Option<(u32, u32)> {
    let (n, d) = s.trim().split_once('d')?;
    let n = if n.is_empty() { 1 } else { n.parse().ok()? };
    Some((n, d.parse().ok()?))
}

fn spell_activities(id: &str, spell: &Spell, consumes_uses: bool) -> Value {
    let types: Vec<String> = spell.damage.iter().map(|d| d.to_lowercase()).filter(|d| DAMAGE.contains(&d.as_str())).collect();
    let text = Entry::plain(&spell.body);
    let upcast = upcast_dice(&text);
    let parts: Vec<Value> = match spell.base_damage.as_deref() {
        Some(f) if !types.is_empty() => {
            let scaling = scaling(spell.level, dice(f), upcast);
            vec![match dice(f) {
                Some((n, d)) => json!({ "number": n, "denomination": d, "bonus": "", "types": types, "custom": { "enabled": false, "formula": "" }, "scaling": scaling }),
                None => json!({ "number": null, "denomination": null, "bonus": "", "types": types, "custom": { "enabled": true, "formula": f }, "scaling": scaling }),
            }]
        }
        _ => vec![],
    };
    let consumption = if consumes_uses {
        json!({ "targets": [{ "type": "itemUses", "value": "1", "target": "", "scaling": {} }], "scaling": { "allowed": false }, "spellSlot": false })
    } else {
        json!({ "targets": [], "scaling": { "allowed": spell.level > 0 }, "spellSlot": true })
    };
    let mut act = json!({
        "_id": id,
        "sort": 0,
        "name": "",
        "activation": { "override": false },
        "duration": { "override": false, "concentration": spell.concentration },
        "range": { "override": false },
        "target": { "override": false, "prompt": true },
        "consumption": consumption,
        "uses": { "spent": 0, "max": "", "recovery": [] },
        "effects": [],
    });
    let save = spell.save.iter().find_map(|s| Ability::parse(s)).map(ability_id);
    let mut map = Map::new();
    if let (Some(_), Some(ab)) = (spell.attack.first(), save) {
        // Attack and save (Ice Knife): the save gets its own activity.
        // Same id with a different last character (never equal to the first's).
        let last = if id.ends_with('s') { 't' } else { 's' };
        let save_id = format!("{}{last}", &id[..15]);
        let mut second = act.clone();
        second["_id"] = json!(save_id);
        second["sort"] = json!(1);
        second["type"] = json!("save");
        second["save"] = json!({ "ability": [ab], "dc": { "calculation": "spellcasting", "formula": "" } });
        second["damage"] = json!({ "onSave": "none", "parts": [] });
        second["consumption"]["spellSlot"] = json!(false);
        map.insert(save_id, second);
    }
    if let Some(kind) = spell.attack.first() {
        act["type"] = json!("attack");
        act["attack"] = json!({
            "ability": "", "bonus": "", "flat": false, "critical": { "threshold": null },
            "type": { "value": if kind.eq_ignore_ascii_case("m") { "melee" } else { "ranged" }, "classification": "spell" },
        });
        act["damage"] = json!({ "includeBase": true, "parts": parts, "critical": { "bonus": "" } });
    } else if let Some(ab) = save {
        // Several listed saves are usually separate effects; Foundry rolls the first.
        act["type"] = json!("save");
        act["save"] = json!({ "ability": [ab], "dc": { "calculation": "spellcasting", "formula": "" } });
        // Cantrips usually do nothing on a save; leveled damage spells half.
        act["damage"] = json!({ "onSave": if spell.level == 0 { "none" } else { "half" }, "parts": parts });
    } else if !parts.is_empty() {
        act["type"] = json!("damage");
        act["damage"] = json!({ "critical": { "allow": false, "bonus": "" }, "parts": parts });
    } else if let Some((n, d, bonus)) = healing_dice(&text) {
        let dice = (n > 0).then_some((n, d));
        act["type"] = json!("heal");
        act["healing"] = json!({
            "number": dice.map(|x| x.0), "denomination": dice.map(|x| x.1), "bonus": bonus, "types": ["healing"],
            "custom": { "enabled": false, "formula": "" }, "scaling": scaling(spell.level, dice, upcast),
        });
    } else {
        act["type"] = json!("utility");
        act["roll"] = json!({ "formula": "", "name": "", "prompt": false, "visible": false });
    }
    map.insert(id.to_string(), act);
    Value::Object(map)
}

/// Damage or healing scaling: cantrips add their dice at levels 5, 11 and
/// 17; leveled spells add the "increases by NdM for each spell slot level
/// above" dice per slot level.
fn scaling(level: u8, base: Option<(u32, u32)>, upcast: Option<(u32, u32)>) -> Value {
    match (level, base, upcast) {
        (0, Some((n, _)), _) => json!({ "mode": "whole", "number": n, "formula": "" }),
        (1.., Some((_, d)), Some((un, ud))) if d == ud => json!({ "mode": "whole", "number": un, "formula": "" }),
        _ => json!({ "mode": "", "number": null, "formula": "" }),
    }
}

/// Dice added per slot level, from the sentence that mentions it: "The
/// damage increases by 1d6 for each spell slot level above 3", "… for every
/// spell slot level above 1", "For each spell slot level above 3, … the
/// damage of Venomous Bite increases by 1d6".
fn upcast_dice(text: &str) -> Option<(u32, u32)> {
    text.split(". ").filter(|s| s.to_lowercase().contains("slot level above")).find_map(|sentence| {
        sentence
            .split(|c: char| !c.is_ascii_alphanumeric())
            .find_map(|tok| dice(tok).filter(|_| tok.chars().next().is_some_and(|c| c.is_ascii_digit())))
    })
}

/// Hit points a spell restores: "regains a number of Hit Points equal to 2d8
/// plus your spellcasting ability modifier" → (2, 8, "@mod"); "regains 70
/// Hit Points" → (0, 0, "70"); "4d8 + 15" → (4, 8, "15").
fn healing_dice(text: &str) -> Option<(u32, u32, String)> {
    let lower = text.to_lowercase();
    for verb in ["regain", "restor"] {
        let mut from = 0;
        while let Some(i) = lower[from..].find(verb) {
            let start = from + i;
            from = start + verb.len();
            let window = &lower[start..lower.len().min(start + 140)];
            if !window.contains("hit point") {
                continue;
            }
            let Some(pos) = window.find(|c: char| c.is_ascii_digit()) else { continue };
            let tok: String = window[pos..].chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
            let after = &window[pos + tok.len()..];
            let near = after.get(..60).unwrap_or(after);
            let flat: String = near.trim_start().strip_prefix("+ ").unwrap_or("").chars().take_while(|c| c.is_ascii_digit()).collect();
            let bonus = if near.contains("spellcasting ability modifier") { "@mod".to_string() } else { flat };
            match dice(&tok) {
                Some((n, d)) => return Some((n, d, bonus)),
                None if tok.chars().all(|c| c.is_ascii_digit()) && near.trim_start().starts_with("hit point") => {
                    return Some((0, 0, tok));
                }
                None => {}
            }
        }
    }
    None
}

/// The area a spell fills ("Self (15-ft. cone)", "a 20-foot-radius Sphere"),
/// so Foundry can place a template.
fn spell_target(spell: &Spell) -> Value {
    let text = format!("{} {}", spell.range, Entry::plain(&spell.body)).to_lowercase();
    let mut template = json!({ "count": "", "contiguous": false, "type": "", "size": "", "width": "", "height": "", "units": "ft" });
    if let Some((shape, size)) = area(&text) {
        template["type"] = json!(shape);
        template["size"] = json!(size.to_string());
    }
    json!({ "affects": { "count": "", "type": "", "choice": false, "special": "" }, "template": template })
}

/// Foundry sizes templates by radius (sphere, cylinder), length (line,
/// cone) or side (cube, square): "a 10-foot-radius, 40-foot-high Cylinder"
/// is a cylinder of 10.
fn area(text: &str) -> Option<(&'static str, u32)> {
    const SHAPES: [(&str, &str); 7] = [
        ("sphere", "sphere"), ("cube", "cube"), ("cone", "cone"), ("cylinder", "cylinder"),
        ("line", "line"), ("emanation", "radius"), ("square", "square"),
    ];
    // Every "N-foot" / "N-ft." with the word after it ("radius", "long", ...).
    let mut sizes: Vec<(usize, u32, String)> = Vec::new();
    for marker in ["-foot", "-ft."] {
        let mut from = 0;
        while let Some(i) = text[from..].find(marker) {
            let at = from + i;
            let digits: String = text[..at].chars().rev().take_while(|c| c.is_ascii_digit()).collect::<Vec<_>>().into_iter().rev().collect();
            let qual: String = text[at + marker.len()..].trim_start_matches(['-', ' ']).chars().take_while(|c| c.is_ascii_alphabetic()).collect();
            if let Ok(n) = digits.parse() {
                sizes.push((at, n, qual));
            }
            from = at + marker.len();
        }
    }
    let mut best: Option<(usize, &'static str, u32)> = None;
    for (word, shape) in SHAPES {
        let mut from = 0;
        while let Some(i) = text[from..].find(word) {
            let at = from + i;
            from = at + word.len();
            // A shape word on its own ("line of sight") isn't an area: it needs a size nearby.
            let near: Vec<&(usize, u32, String)> = sizes.iter().filter(|(p, ..)| *p + 40 >= at && *p <= at + 40).collect();
            let preferred = match shape {
                "sphere" | "cylinder" => "radius",
                "line" => "long",
                _ => "",
            };
            // The size named for this shape, else the one closest to the word.
            let pick = near
                .iter()
                .find(|(.., q)| !preferred.is_empty() && q == preferred)
                .or_else(|| near.iter().min_by_key(|(p, ..)| p.abs_diff(at)));
            if let Some((_, n, _)) = pick {
                if best.is_none_or(|(p, ..)| at < p) {
                    best = Some((at, shape, *n));
                }
                break;
            }
        }
    }
    best.map(|(_, shape, n)| (shape, n))
}

fn price(cp: Option<u32>) -> Value {
    match cp {
        Some(cp) if cp % 100 == 0 => json!({ "value": cp / 100, "denomination": "gp" }),
        Some(cp) if cp % 10 == 0 => json!({ "value": cp / 10, "denomination": "sp" }),
        Some(cp) => json!({ "value": cp, "denomination": "cp" }),
        None => json!({ "value": 0, "denomination": "gp" }),
    }
}

fn rarity(r: Option<&str>) -> Option<&'static str> {
    Some(match r.map(str::to_lowercase).as_deref() {
        Some("common") => "common",
        Some("uncommon") => "uncommon",
        Some("rare") => "rare",
        Some("very rare") => "veryRare",
        Some("legendary") => "legendary",
        Some("artifact") => "artifact",
        _ => return None,
    })
}

/// How a carried item is held.
struct Held {
    qty: u16,
    equipped: bool,
    attuned: bool,
    notes: String,
    /// Id of the container item it is in.
    container: Option<String>,
}

/// Bags and boxes Foundry should treat as containers, with what they hold.
fn container_capacity(name: &str) -> Option<Value> {
    let n = name.to_lowercase();
    let weight = |lb: u32| json!({ "weight": { "value": lb, "units": "lb" } });
    Some(match n.as_str() {
        "backpack" => weight(30),
        "pouch" => weight(6),
        "sack" => weight(30),
        "basket" => weight(40),
        "chest" => weight(300),
        "barrel" => weight(400),
        "quiver" => json!({ "count": 20 }),
        "crossbow bolt case" => json!({ "count": 20 }),
        "map or scroll case" | "map case" | "scroll case" => json!({ "count": 10 }),
        _ => return None,
    })
}

/// Everyday gear that is used up (Foundry "consumable"), by name.
fn consumable_gear(name: &str) -> Option<&'static str> {
    Some(match name.to_lowercase().as_str() {
        "rations" | "rations (1 day)" => "food",
        "acid" | "acid (vial)" | "alchemist's fire" | "alchemist's fire (flask)" | "holy water" | "holy water (flask)"
        | "oil" | "oil (flask)" | "antitoxin" | "antitoxin (vial)" | "torch" | "ball bearings" | "caltrops" => "",
        _ => return None,
    })
}

fn signed(n: i32) -> String {
    if n >= 0 {
        format!("+{n}")
    } else {
        n.to_string()
    }
}

fn escape(s: &str) -> String {
    compendium::escape_html(s)
}

/// Free text (blank-line separated) as HTML paragraphs.
fn paragraphs(text: &str) -> String {
    text.split("\n\n")
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(|p| format!("<p>{}</p>", escape(p).replace('\n', "<br>")))
        .collect()
}

#[cfg(test)]
mod parse_tests {
    use super::*;

    // Each case is a spell whose Foundry import (dnd5e 6.1.1) was compared
    // against this export.
    #[test]
    fn healing_from_spell_text() {
        let cure = "A creature you touch regains a number of Hit Points equal to 2d8 plus your spellcasting ability modifier.";
        assert_eq!(healing_dice(cure), Some((2, 8, "@mod".into())));
        assert_eq!(healing_dice("Positive energy washes through the target, restoring 70 Hit Points."), Some((0, 0, "70".into())));
        assert_eq!(healing_dice("The target regains 4d8 + 15 Hit Points."), Some((4, 8, "15".into())));
        assert_eq!(healing_dice("You regain the use of your reaction."), None);
    }

    #[test]
    fn upcast_uses_the_per_slot_sentence() {
        let call_lightning = "The damage increases by 1d10. The damage increases by 1d10 for each spell slot level above 3.";
        assert_eq!(upcast_dice(call_lightning), Some((1, 10)));
        assert_eq!(upcast_dice("The healing increases by 2d8 for each spell slot level above 1."), Some((2, 8)));
        assert_eq!(upcast_dice("Its speed increases by 10 feet."), None);
        // Wordings from Heroes of Faerûn and the Warcraft book.
        assert_eq!(upcast_dice("The damage increases by 2d4 for every spell slot level above 1."), Some((2, 4)));
        assert_eq!(upcast_dice("Both the initial and the later damage increase by 1d8 for each spell slot level above 1."), Some((1, 8)));
        let viper = "For each spell slot level above 3, the number of Temporary Hit Points you gain from this spell increases by 5, and the damage of Venomous Bite increases by 1d6.";
        assert_eq!(upcast_dice(viper), Some((1, 6)));
        assert_eq!(upcast_dice("Creatures regain 1d8 additional Hit Points for each spell slot level above 6."), Some((1, 8)));
    }

    #[test]
    fn templates_sized_like_foundry() {
        assert_eq!(area("a 10-foot-radius, 40-foot-high cylinder"), Some(("cylinder", 10)));
        assert_eq!(area("self (60-ft. line) you launch a sunbeam in a 5-foot-wide, 60-foot-long line."), Some(("line", 60)));
        assert_eq!(area("each creature in a 20-foot-radius sphere centered on that point"), Some(("sphere", 20)));
        assert_eq!(area("self (15-ft. cone)"), Some(("cone", 15)));
        assert_eq!(area("grasping weeds sprout in a 20-foot square starting from a point"), Some(("square", 20)));
        assert_eq!(area("a creature you can see within range"), None);
    }
}
