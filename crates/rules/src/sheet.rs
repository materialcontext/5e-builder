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
    /// For saving-throw spells: the save DC and ability (shown instead of a bonus).
    pub save: Option<(i32, Ability)>,
    /// Ability the attack or save DC uses (STR, DEX, or a spellcasting ability).
    pub ability: Ability,
    pub proficient: bool,
    /// "5 ft.", "Reach 10 ft.", "80/320 ft.", "120 ft.", "Touch".
    pub range: String,
    /// Dice and modifier: "1d8+3", "4d10".
    pub dice: String,
    /// "Slashing", "Fire or Cold".
    pub damage_type: String,
    /// The line under the attack: properties, versatile damage, mastery, spell notes.
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
    pub attuned: bool,
    pub notes: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClassSummary {
    pub name: String,
    pub level: u8,
    pub subclass: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Sheet {
    pub name: String,
    pub summary: String,
    pub class_line: String,
    /// Each class with its level and subclass, in the order taken.
    pub classes: Vec<ClassSummary>,
    /// A shield is equipped (its bonus is already in `armor_class`).
    pub shield: bool,
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
    /// Features and free spells with limited uses, for tracking at the table.
    pub limited: Vec<crate::play::LimitedUse>,
    /// What the character can do with an action, bonus action and reaction.
    pub actions: crate::play::Actions,
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
    for g in &build.grants {
        if let Grant::ToolExpertise(t) = &g.grant {
            let label = title_case(t);
            for x in tools.iter_mut().filter(|x| **x == label) {
                x.push_str(" (expertise)");
            }
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
    for g in &build.grants {
        if let Grant::DarkvisionBonus { base, bonus } = g.grant {
            darkvision = if darkvision == 0 { base } else { darkvision + bonus };
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


    // Armor class: worn armor, else the best unarmored formula; then shield and bonuses.
    let dex = m(Ability::Dex);
    let medium_cap = if build.feats.iter().any(|(k, _)| k.name_part() == "medium armor master") { 3 } else { 2 };
    let worn = worn_armor.and_then(|a| a.armor.as_ref().map(|s| (a, s)));
    let (mut ac, mut ac_note) = match worn {
        Some((it, a)) => {
            let base = a.ac as i32 + it.bonus as i32;
            match a.kind {
                ArmorKind::Light => (base + dex, it.name.clone()),
                ArmorKind::Medium => (base + dex.min(medium_cap), it.name.clone()),
                _ => (base, it.name.clone()),
            }
        }
        None => {
            let mut best = (10 + dex, "Unarmored".to_string());
            for g in &build.grants {
                if let Grant::NaturalArmor { base, abilities, shield: shield_ok } = &g.grant {
                    if shield.is_some() && !shield_ok {
                        continue;
                    }
                    let v = *base as i32 + abilities.iter().map(|a| m(*a)).sum::<i32>();
                    if v > best.0 {
                        best = (v, g.origin.clone());
                    }
                }
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
    let heavy_worn = worn.is_some_and(|(_, a)| a.kind == ArmorKind::Heavy);
    for g in &build.grants {
        if let Grant::AcBonus { amount, when } = g.grant {
            let applies = match when {
                AcWhen::Always => true,
                AcWhen::Armored => worn.is_some(),
                AcWhen::HeavyArmor => heavy_worn,
                AcWhen::NotHeavyArmor => !heavy_worn,
                AcWhen::Unarmored => worn.is_none(),
            };
            if applies {
                ac += amount as i32;
                ac_note.push_str(&format!(" + {}", g.origin));
            }
        }
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
    hp += build
        .grants
        .iter()
        .filter_map(|g| match g.grant {
            Grant::HpBonus(n) => Some(n),
            _ => None,
        })
        .sum::<i32>();
    let mut dice: BTreeMap<u8, u8> = BTreeMap::new();
    for l in &ch.levels {
        let hd = comp.classes.get(&l.class).map(|c| c.hit_die).unwrap_or(8);
        *dice.entry(hd).or_default() += 1;
    }
    let hit_dice = dice.iter().rev().map(|(d, n)| format!("{n}d{d}")).collect::<Vec<_>>().join(" + ");

    // Speed bonuses.
    let heavy = worn_armor.and_then(|a| a.armor.as_ref()).is_some_and(|a| a.kind == ArmorKind::Heavy);
    if build.has_feature_named("Fast Movement") && !heavy {
        speed.walk += 10;
    }
    // 2024 Ranger Roving: +10 out of heavy armor; climb and swim equal to speed.
    if build.has_feature_named("Roving") && build.classes.iter().any(|c| comp.classes.get(&c.key).is_some_and(|x| !x.legacy)) {
        if !heavy {
            speed.walk += 10;
        }
        speed.climb = speed.climb.max(speed.walk);
        speed.swim = speed.swim.max(speed.walk);
    }
    if let Some(monk) = build.classes.iter().find(|c| comp.classes.get(&c.key).is_some_and(|x| x.features.iter().any(|f| f.name == "Unarmored Movement"))) {
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

    // Speed grants: bonuses first, then speeds equal to (or set to) a value.
    for g in &build.grants {
        if let Grant::SpeedBonus { mode, amount } = &g.grant {
            match mode.as_str() {
                "walk" => speed.walk += amount,
                "fly" => speed.fly += amount,
                "swim" => speed.swim += amount,
                "climb" => speed.climb += amount,
                "burrow" => speed.burrow += amount,
                _ => {}
            }
        }
    }
    for g in &build.grants {
        if let Grant::SpeedSet { mode, amount } = &g.grant {
            let v = amount.unwrap_or(speed.walk);
            let slot = match mode.as_str() {
                "fly" => &mut speed.fly,
                "swim" => &mut speed.swim,
                "climb" => &mut speed.climb,
                "burrow" => &mut speed.burrow,
                _ => continue,
            };
            *slot = (*slot).max(v);
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
            InventoryLine { name: i.name.clone(), qty: i.qty, weight: it.map(|x| x.weight).unwrap_or(0.0) * i.qty as f32, equipped: i.equipped, attuned: i.attuned, notes }
        })
        .collect();
    let carried_weight = inventory.iter().map(|i| i.weight).fold(0.0, |a, b| a + b);

    // Only what was chosen: menus in feature text are cut down to the selections.
    let mut features = crate::selections::prune(build.features.clone(), &crate::selections::names(build, comp));
    features.sort_by_key(|f| match f.kind {
        FeatureKind::Class => 0,
        FeatureKind::Species => 1,
        FeatureKind::Background => 2,
        FeatureKind::Feat => 3,
        FeatureKind::Other => 4,
    });

    let limited = crate::play::limited_uses(build, comp, &features, pb);
    let actions = crate::play::actions(ch, build, comp, &features);

    let classes: Vec<ClassSummary> = build
        .classes
        .iter()
        .filter_map(|c| {
            let class = comp.classes.get(&c.key)?;
            let sub = c.subclass.as_ref().and_then(|k| comp.subclasses.get(k)).map(|s| s.name.clone());
            Some(ClassSummary { name: class.name.clone(), level: c.level, subclass: sub })
        })
        .collect();
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
        classes,
        shield: shield.is_some(),
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
        limited,
        actions,
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
            || weapon_matches_filter(p, w)
    })
}

/// "martial weapons with the light or finesse property": the category
/// matches and the weapon has one of the properties.
pub fn weapon_matches_filter(label: &str, w: &WeaponStats) -> bool {
    let label = label.to_lowercase();
    let Some((kind, rest)) = label.split_once(" weapons with the ") else { return false };
    let kind_ok = match kind {
        "martial" => w.martial,
        "simple" => !w.martial,
        "melee" => !w.ranged,
        "ranged" => w.ranged,
        _ => false,
    };
    let props = rest.trim_end_matches(" property").trim_end_matches(" properties");
    kind_ok && props.split(" or ").any(|p| w.properties.iter().any(|x| x.split('|').next() == Some(p.trim())))
}

fn attacks(ch: &Character, build: &Build, comp: &Compendium, pb: i32, profs: &[String]) -> Vec<Attack> {
    let m = |a: Ability| build.modifier(a);
    let masteries: Vec<String> =
        build.features.iter().filter_map(|f| f.feature.name.strip_prefix("Weapon Mastery: ")).map(|s| s.split(" (").next().unwrap_or("").to_lowercase()).collect();
    let mut out = Vec::new();
    for inv in &ch.inventory {
        let Some(it) = inv.item.as_ref().and_then(|k| comp.items.get(k)) else { continue };
        let Some(w) = &it.weapon else { continue };
        let has = |p: &str| w.properties.iter().any(|x| x.split('|').next() == Some(p));
        let ability = if w.ranged || (has("finesse") && m(Ability::Dex) > m(Ability::Str)) { Ability::Dex } else { Ability::Str };
        let prof = weapon_proficient(it, w, profs);
        let bonus = m(ability) + if prof { pb } else { 0 } + it.bonus as i32;
        let dmg_mod = m(ability) + it.bonus as i32;
        let fmt = |dice: &str| if dmg_mod == 0 { dice.to_string() } else { format!("{dice}{}", signed(dmg_mod)) };
        let range = match (&w.range, w.ranged) {
            (Some(r), true) => format!("{r} ft."),
            (Some(r), false) => format!("{} · {r}", if has("reach") { "10 ft." } else { "5 ft." }),
            (None, _) => if has("reach") { "10 ft.".into() } else { "5 ft.".into() },
        };
        // Properties a player needs at the table (range and reach have their own column).
        let mut notes: Vec<String> = w
            .properties
            .iter()
            .map(|p| p.split('|').next().unwrap_or(p).to_string())
            .filter(|p| !matches!(p.as_str(), "versatile" | "reach" | "thrown" | "ammunition"))
            .map(|p| title_case(&p))
            .collect();
        if has("thrown") {
            notes.insert(0, "Thrown".into());
        }
        if has("ammunition") {
            notes.insert(0, "Ammunition".into());
        }
        if let Some(v) = &w.versatile {
            notes.push(format!("Versatile ({})", fmt(v)));
        }
        if let Some(ms) = &w.mastery {
            if masteries.contains(&it.name.to_lowercase()) {
                let dc = 8 + m(ability) + pb;
                notes.push(format!("{ms}: {}", mastery_effect(ms, dc)));
            }
        }
        if !prof {
            notes.push("Not proficient".into());
        }
        out.push(Attack {
            name: inv.name.clone(),
            bonus,
            save: None,
            ability,
            proficient: prof,
            range,
            dice: fmt(&w.damage),
            damage_type: title_case(&w.damage_type),
            notes: notes.join(", "),
        });
    }
    let str_mod = m(Ability::Str);
    out.push(Attack {
        name: "Unarmed Strike".into(),
        bonus: str_mod + pb,
        save: None,
        ability: Ability::Str,
        proficient: true,
        range: "5 ft.".into(),
        dice: (1 + str_mod).max(1).to_string(),
        damage_type: "Bludgeoning".into(),
        notes: format!("Or Grapple or Shove instead: Str or Dex save, DC {}", 8 + str_mod + pb),
    });
    out.extend(damage_cantrips(ch, build, comp, pb));
    out
}

/// What a weapon mastery does, short enough for one line.
fn mastery_effect(mastery: &str, dc: i32) -> String {
    match mastery.to_lowercase().as_str() {
        "cleave" => "on a hit, attack a 2nd creature within 5 ft. (once per turn, no ability mod to damage)".into(),
        "graze" => "on a miss, deal damage equal to your ability modifier".into(),
        "nick" => "the extra Light-weapon attack is part of the Attack action".into(),
        "push" => "push a Large or smaller target up to 10 ft. away".into(),
        "sap" => "target has Disadvantage on its next attack roll".into(),
        "slow" => "target's Speed is reduced by 10 ft. until your next turn".into(),
        "topple" => format!("target makes a Con save (DC {dc}) or falls Prone"),
        "vex" => "Advantage on your next attack roll against the target".into(),
        _ => String::new(),
    }
}

impl Attack {
    /// "+5", or "DC 13 DEX" for a saving-throw spell.
    pub fn to_hit(&self) -> String {
        match self.save {
            Some((dc, a)) => format!("DC {dc} {}", a.abbr()),
            None => signed(self.bonus),
        }
    }

    /// Dice and type together: "1d8+3 Slashing".
    pub fn damage(&self) -> String {
        format!("{} {}", self.dice, self.damage_type).trim().to_string()
    }
}

/// Every damage-dealing cantrip the character has (class, feat, species…),
/// with its spell attack bonus or save DC and its dice at the character's level.
fn damage_cantrips(ch: &Character, build: &Build, comp: &Compendium, pb: i32) -> Vec<Attack> {
    let level = ch.total_level().max(1);
    let first_casting = build.spells.iter().find_map(|k| k.class.as_ref().and_then(|c| comp.classes.get(c)).and_then(|c| c.spellcasting.ability));
    let mut out: Vec<Attack> = Vec::new();
    for k in &build.spells {
        let Some(sp) = comp.spells.get(&k.key) else { continue };
        if sp.level != 0 || sp.damage.is_empty() || (sp.attack.is_empty() && sp.save.is_empty()) || out.iter().any(|a| a.name == sp.name) {
            continue;
        }
        let ability = k
            .ability
            .or_else(|| k.class.as_ref().and_then(|c| comp.classes.get(c)).and_then(|c| c.spellcasting.ability))
            .or(first_casting)
            .unwrap_or(Ability::Int);
        let m = build.modifier(ability);
        let at = |steps: &[(u8, String)]| steps.iter().rev().find(|(l, _)| *l <= level).map(|(_, d)| d.clone());
        let dice: Vec<String> = sp.scaling.iter().filter_map(|(_, steps)| at(steps)).collect();
        let types = sp.damage.iter().map(|t| title_case(t)).collect::<Vec<_>>().join(" or ");
        let mut notes = vec![format!("Cantrip, {}", sp.components.split(" (").next().unwrap_or(""))];
        let mut dice_text = match dice.as_slice() {
            [] => sp.base_damage.clone().unwrap_or_default(),
            [one] => one.clone(),
            // Toll the Dead: the second die applies to a wounded target.
            [first, rest @ ..] => {
                notes.push(format!("{} if the target is already damaged", rest.join(", ")));
                first.clone()
            }
        };
        if sp.name == "Eldritch Blast" {
            let beams = match level {
                1..=4 => 1,
                5..=10 => 2,
                11..=16 => 3,
                _ => 4,
            };
            if build.has_feature_named("Agonizing Blast") && m != 0 {
                dice_text = format!("{}{}", sp.base_damage.clone().unwrap_or_else(|| "1d10".into()), signed(m));
            }
            notes.push(format!("{beams} beam{} (separate attacks)", if beams > 1 { "s" } else { "" }));
        }
        let save = sp.save.first().and_then(|a| Ability::parse(a)).map(|a| (8 + pb + m, a));
        if let Some((_, a)) = save {
            notes.push(format!("{} save", a.name()));
        }
        if sp.concentration {
            notes.push("Concentration".into());
        }
        out.push(Attack {
            name: sp.name.clone(),
            bonus: pb + m,
            save,
            ability,
            proficient: true,
            range: sp.range.clone(),
            dice: dice_text.trim().to_string(),
            damage_type: types,
            notes: notes.join(", "),
        });
    }
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
