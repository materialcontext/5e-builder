//! Table aids for the sheet: what a character can do with each kind of
//! action, and every limited-use feature with its uses and recharge.
//!
//! Both are derived from data where it exists (class table columns, spell
//! casting times, innate spell uses) and otherwise from the feature's own
//! rules text, which states its action and uses in a few set phrases
//! ("As a Bonus Action", "a number of times equal to your Proficiency
//! Bonus", "Once you use this feature, you can't do so again until you
//! finish a Long Rest").

use crate::build::{Build, FeatureEntry, FeatureKind, SpellMode};
use crate::character::Character;
use compendium::{Ability, Compendium, Entry};

/// One feature (or free spell) with limited uses.
#[derive(Clone, Debug, PartialEq)]
pub struct LimitedUse {
    pub name: String,
    /// Number of uses, or the size of a pool (see `unit`).
    pub uses: u16,
    /// For pools: what is counted ("HP", "points", "d8"). Empty for uses,
    /// which are printed as tick boxes.
    pub unit: String,
    /// "LR", "SR" (short or long rest), "1 SR · all LR", "Dawn".
    pub recharge: String,
    /// Where it comes from when the name doesn't say (a feat, a spell's source).
    pub note: String,
}

impl LimitedUse {
    /// Printed as tick boxes (few uses) rather than a "__ / N" field.
    pub fn ticks(&self) -> bool {
        self.unit.is_empty() && self.uses <= 8
    }
}

/// Things to do with each kind of action, beyond the attacks listed with
/// weapons. Each column is in the order it should fill a limited space:
/// spells the player pinned, then features, then other spells by
/// [`spell_rank`]. One-action spells aren't named: the Action column has a
/// single "Cast Spell" entry instead.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Actions {
    pub action: Vec<ActionEntry>,
    pub bonus: Vec<ActionEntry>,
    pub reaction: Vec<ActionEntry>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ActionEntry {
    pub name: String,
    /// The spell, for spells (features and other actions have none).
    pub spell: Option<compendium::Key>,
    /// The player asked for this entry to always be shown.
    pub pinned: bool,
    /// Shown unless the player says otherwise (one-action spells aren't:
    /// "Cast Spell" stands for them).
    pub default: bool,
    /// Written by the player on the Custom step.
    pub custom: bool,
}

impl ActionEntry {
    fn feature(name: &str) -> Self {
        ActionEntry { name: name.to_string(), spell: None, pinned: false, default: true, custom: false }
    }
}

/// Names of the sheet sections the player can edit (keys of
/// [`crate::character::SheetEdits`]).
pub mod section {
    pub const WEAPONS: &str = "weapons";
    pub const ACTIONS: &str = "actions";
    pub const BONUS_ACTIONS: &str = "bonus actions";
    pub const REACTIONS: &str = "reactions";
    pub const CLASS_FEATURES: &str = "class features";
    pub const TRAITS: &str = "species traits & feats";
}

/// Applies the player's edits to the candidate entries: never-shown ones go,
/// always-shown ones come first, then the player's own entries, then the rest
/// in their default order.
pub fn arrange_actions(candidates: &Actions, edits: &crate::character::SheetEdits) -> Actions {
    let column = |list: &[ActionEntry], sec: &str| {
        let items = list.iter().map(|e| (e.clone(), e.default)).collect();
        let mut out = edits.arrange(sec, items, |e: &ActionEntry| e.name.as_str());
        let pinned = out.iter().take_while(|e| edits.state(sec, &e.name) == Some(true)).count();
        for e in out.iter_mut().take(pinned) {
            e.pinned = true;
        }
        let own = edits.added(sec).iter().map(|x| ActionEntry { name: x.name.clone(), spell: None, pinned: true, default: true, custom: true });
        out.splice(pinned..pinned, own);
        out
    };
    Actions {
        action: column(&candidates.action, section::ACTIONS),
        bonus: column(&candidates.bonus, section::BONUS_ACTIONS),
        reaction: column(&candidates.reaction, section::REACTIONS),
    }
}

impl Actions {
    pub fn is_empty(&self) -> bool {
        self.action.is_empty() && self.bonus.is_empty() && self.reaction.is_empty()
    }

    /// Names in a column, in order.
    pub fn names(list: &[ActionEntry]) -> Vec<&str> {
        list.iter().map(|e| e.name.as_str()).collect()
    }
}

/// Which Actions column a spell goes in, by casting time; None for spells
/// that take longer than an action (rituals, 1 minute, ...).
pub fn spell_column(time: &str) -> Option<ActionKind> {
    let t = time.to_lowercase();
    if t.contains("bonus action") {
        Some(ActionKind::Bonus)
    } else if t.contains("reaction") {
        Some(ActionKind::Reaction)
    } else if t.starts_with("1 action") || t.starts_with("action") {
        Some(ActionKind::Action)
    } else {
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionKind {
    Action,
    Bonus,
    Reaction,
}

/// Default order for spells in the Actions box (lower first): free casts,
/// then spells that attack, force a save, deal damage or heal, strongest
/// first, then other spells, strongest first, then utility cantrips.
pub fn spell_rank(spell: &compendium::Spell, mode: &SpellMode) -> (u8, std::cmp::Reverse<u8>, String) {
    let free = matches!(mode, SpellMode::Innate(_));
    let text = Entry::plain(&spell.body).to_lowercase();
    let combat = !spell.attack.is_empty() || !spell.save.is_empty() || !spell.damage.is_empty() || text.contains("regains") || text.contains("hit points equal to");
    let group = match (free, combat, spell.level) {
        (true, _, _) => 0,
        (false, true, 1..) => 1,
        (false, false, 1..) => 2,
        (false, true, 0) => 3,
        (false, false, 0) => 4,
    };
    (group, std::cmp::Reverse(spell.level), spell.name.clone())
}

/// Class table columns that count uses, and the feature whose text says how
/// they recharge.
const TABLE_USES: &[(&str, &str)] = &[
    ("Rages", "Rage"),
    ("Channel Divinity", "Channel Divinity"),
    ("Wild Shape", "Wild Shape"),
    ("Second Wind", "Second Wind"),
    ("Focus Points", "Monk's Focus"),
    ("Ki Points", "Ki"),
    ("Sorcery Points", "Font of Magic"),
    ("Favored Enemy", "Favored Enemy"),
];

pub fn limited_uses(build: &Build, comp: &Compendium, features: &[FeatureEntry], pb: i32) -> Vec<LimitedUse> {
    let mut out: Vec<LimitedUse> = Vec::new();
    let mut covered: Vec<String> = Vec::new();
    let text_of = |name: &str| {
        features.iter().find(|f| f.feature.name.eq_ignore_ascii_case(name)).map(|f| Entry::plain(&f.feature.body).to_lowercase())
    };

    // 1. Class (and subclass) table columns.
    for cs in &build.classes {
        let Some(class) = comp.classes.get(&cs.key) else { continue };
        for col in &class.table {
            let Some((_, feature)) = TABLE_USES.iter().find(|(label, _)| *label == col.label) else { continue };
            let Some(n) = col.at(cs.level).and_then(|v| v.parse::<u16>().ok()).filter(|n| *n > 0) else { continue };
            let text = text_of(feature).unwrap_or_default();
            let pool = col.label.ends_with("Points");
            out.push(LimitedUse {
                name: if pool { col.label.clone() } else { feature.to_string() },
                uses: n,
                unit: if pool { "points".into() } else { String::new() },
                recharge: recharge(&text).unwrap_or_else(|| if pool && col.label != "Sorcery Points" { "SR".into() } else { "LR".into() }),
                note: String::new(),
            });
            covered.push(feature.to_lowercase());
        }
        // Psi Warrior, Soulknife: "Number" and "Die Size" of energy dice.
        if let Some(sub) = cs.subclass.as_ref().and_then(|k| comp.subclasses.get(k)) {
            let col = |l: &str| sub.table.iter().find(|c| c.label == l).and_then(|c| c.at(cs.level)).map(str::to_string);
            if let (Some(n), Some(die)) = (col("Number"), col("Die Size")) {
                if let Ok(n) = n.parse::<u16>() {
                    let name = if sub.short_name.contains("Soul") { "Psionic Energy Dice (Soulknife)" } else { "Psionic Energy Dice" };
                    out.push(LimitedUse {
                        name: name.into(),
                        uses: n,
                        unit: die.to_lowercase(),
                        recharge: "1 SR · all LR".into(),
                        note: String::new(),
                    });
                    covered.push("psionic power".into());
                }
            }
        }
    }

    // 2. Features whose text states their uses.
    let class_level = |name: &str| build.classes.iter().find(|c| c.key.name_part() == name).map(|c| c.level).unwrap_or(0);
    for f in features {
        let name = &f.feature.name;
        let lower = name.to_lowercase();
        if covered.contains(&lower) || matches!(lower.as_str(), "spellcasting" | "pact magic") || name.starts_with("Weapon Mastery") {
            continue;
        }
        let text = Entry::plain(&f.feature.body);
        // Origin reads "Fighter 9" or "Fighter 3 (Battle Master)"; feats and
        // species use the character level.
        let class_name = f.origin.split(" (").next().unwrap_or("").trim_end_matches(|c: char| c.is_ascii_digit() || c == ' ').to_lowercase();
        let level = match class_level(&class_name) {
            0 => build.classes.iter().map(|c| c.level).sum(),
            l => l,
        };
        let Some(u) = uses_from_text(&text, pb, level, |a| build.modifier(a), class_level) else { continue };
        if out.iter().any(|x| x.name.eq_ignore_ascii_case(name)) {
            continue;
        }
        let note = match f.kind {
            FeatureKind::Feat => "feat".to_string(),
            FeatureKind::Species => "species".to_string(),
            _ => String::new(),
        };
        out.push(LimitedUse { name: name.clone(), note, ..u });
    }

    // Font of Inspiration: Bardic Inspiration comes back on a Short Rest too.
    if features.iter().any(|f| f.feature.name == "Font of Inspiration") {
        if let Some(bi) = out.iter_mut().find(|u| u.name == "Bardic Inspiration") {
            bi.recharge = "SR".into();
        }
    }

    // 3. Spells cast without a slot a few times per rest (Fey Touched, species...).
    for k in &build.spells {
        let SpellMode::Innate(usage) = &k.mode else { continue };
        let Some(spell) = comp.spells.get(&k.key) else { continue };
        let Some((uses, recharge)) = innate_uses(usage, pb, |a| build.modifier(a), k.ability) else { continue };
        // Already counted by the feature that grants it ("Steps of the Fey"
        // lets you cast Misty Step a few times: one row, not two).
        let lower = spell.name.to_lowercase();
        let granted_by_listed = features.iter().any(|f| {
            out.iter().any(|u| u.name == f.feature.name) && Entry::plain(&f.feature.body).to_lowercase().contains(&lower)
        });
        if out.iter().any(|x| x.name == spell.name) || granted_by_listed {
            continue;
        }
        let from = if k.origin.starts_with("Warlock") && spell.level >= 6 { "Mystic Arcanum" } else { k.origin.rsplit(": ").next().unwrap_or(&k.origin) };
        out.push(LimitedUse { name: spell.name.clone(), uses, unit: String::new(), recharge, note: format!("spell, {from}") });
    }
    out
}

/// Uses and recharge stated in a feature's text, if it has limited uses.
/// `level`: the character's level in the feature's class (for counts that
/// grow with level); `class_level` looks up any class (for pools).
fn uses_from_text(text: &str, pb: i32, level: u8, modifier: impl Fn(Ability) -> i32, class_level: impl Fn(&str) -> u8) -> Option<LimitedUse> {
    let t = text.to_lowercase();
    let recharge = recharge(&t)?;
    let use_ = |n: i32| LimitedUse { name: String::new(), uses: n.max(1) as u16, unit: String::new(), recharge: recharge.clone(), note: String::new() };
    // Pools: Lay on Hands ("five times your Paladin level").
    if let Some(i) = t.find("times your ") {
        let before = &t[..i];
        let rest = &t[i + "times your ".len()..];
        if let (Some(n), Some(class)) = (number_word(before.split_whitespace().last().unwrap_or("")), rest.split_whitespace().next()) {
            if rest.split_whitespace().nth(1).is_some_and(|w| w.starts_with("level")) && t.contains("pool") {
                let lvl = class_level(class) as i32;
                if lvl > 0 {
                    return Some(LimitedUse { unit: "HP".into(), ..use_(n * lvl) });
                }
            }
        }
    }
    // 2014: "equal to your paladin level × 5"
    if t.contains("pool") {
        for (i, m) in t.match_indices(" level × ").chain(t.match_indices(" level x ")) {
            let n: String = t[i + m.len()..].chars().take_while(|c| c.is_ascii_digit()).collect();
            let class = t[..i].split_whitespace().last().unwrap_or("");
            if let (Ok(n), lvl) = (n.parse::<i32>(), class_level(class) as i32) {
                if lvl > 0 {
                    return Some(LimitedUse { unit: "HP".into(), ..use_(n * lvl) });
                }
            }
        }
    }
    // 2014: "a number of times equal to 1 + your Charisma modifier"
    if let Some(i) = t.find("a number of times equal to 1 + your ") {
        let word = t[i + "a number of times equal to 1 + your ".len()..].split_whitespace().next().unwrap_or("");
        if let Some(a) = Ability::parse(word) {
            return Some(use_(1 + modifier(a)));
        }
    }
    if t.contains("a number of times equal to your proficiency bonus") || t.contains("number of times equal to your proficiency bonus") {
        return Some(use_(pb));
    }
    if let Some(i) = t.find("a number of times equal to your ") {
        let word = t[i + "a number of times equal to your ".len()..].split_whitespace().next().unwrap_or("");
        if let Some(a) = Ability::parse(word) {
            return Some(use_(modifier(a)));
        }
    }
    // "You have four Superiority Dice, which are d8s"
    if let Some(i) = t.find("you have ") {
        let mut words = t[i + "you have ".len()..].split_whitespace();
        if let (Some(n), Some(w2), Some(w3)) = (words.next().and_then(number_word), words.next(), words.next()) {
            if w3.starts_with("dice") || w3.starts_with("die") {
                let die = t.find("which are d").map(|j| t[j + "which are ".len()..].chars().take_while(|c| c.is_ascii_alphanumeric()).collect::<String>().trim_end_matches('s').to_string());
                let _ = w2;
                return Some(LimitedUse { unit: die.unwrap_or_else(|| "dice".into()), ..use_(n) });
            }
        }
    }
    // "You can use this feature twice before a Long Rest starting at level 13
    // and three times … starting at level 17": counts tied to a level apply
    // from that level of the feature's class.
    let mut best: Option<i32> = None;
    for sentence in t.split(". ") {
        let clauses: Vec<&str> = sentence.split(" and ").collect();
        for clause in clauses {
            for (phrase, n) in [("twice", 2), ("two times", 2), ("three times", 3), ("four times", 4)] {
                if !clause.contains(phrase) {
                    continue;
                }
                let applies = level_in(clause).is_none_or(|l| l <= level);
                if applies {
                    best = Some(best.unwrap_or(0).max(n));
                }
            }
        }
    }
    if let Some(n) = best {
        return Some(use_(n));
    }
    let once = ["once you use this", "once you've used this", "once you do so", "once used", "can't do so again until", "can't use this feature again until", "can't use it again until", "can't use this trait again until", "can't use this benefit again until", "once you use it"];
    if once.iter().any(|p| t.contains(p)) {
        return Some(use_(1));
    }
    None
}

/// The level a clause names: "at level 17" (2024) or "at 17th level" (2014).
fn level_in(clause: &str) -> Option<u8> {
    if let Some(i) = clause.find("level ") {
        let digits: String = clause[i + 6..].chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(l) = digits.parse() {
            return Some(l);
        }
    }
    for suffix in ["th level", "st level", "nd level", "rd level"] {
        if let Some(i) = clause.find(suffix) {
            let digits: String = clause[..i].chars().rev().take_while(|c| c.is_ascii_digit()).collect::<Vec<_>>().into_iter().rev().collect();
            if let Ok(l) = digits.parse() {
                return Some(l);
            }
        }
    }
    None
}

/// How a feature's uses come back, from its text. Sentences about getting
/// uses back win over other mentions of rests ("this ceremony can be
/// performed during a short or long rest").
fn recharge(t: &str) -> Option<String> {
    let t = t.to_lowercase();
    let about_uses: String = t
        .split(". ")
        .filter(|s| ["regain", "restored", "expended", "again until", "replenish"].iter().any(|w| s.contains(w)))
        .collect::<Vec<_>>()
        .join(". ");
    if !about_uses.is_empty() && about_uses != t {
        if let Some(r) = rest_kind(&about_uses) {
            return Some(r);
        }
    }
    rest_kind(&t)
}

fn rest_kind(t: &str) -> Option<String> {
    let one_on_short = ["regain one expended use", "regain one of its expended uses", "regain one expended"].iter().any(|p| t.contains(p));
    if t.contains("short rest") && one_on_short && t.contains("long rest") {
        return Some("1 SR · all LR".into());
    }
    if t.contains("short or long rest") {
        return Some("SR".into());
    }
    if t.contains("long rest") {
        return Some("LR".into());
    }
    if t.contains("short rest") {
        return Some("SR".into());
    }
    if t.contains("dawn") {
        return Some("Dawn".into());
    }
    None
}

/// "1/long rest", "pb/long rest", "int/long rest", "2 uses" → (uses, recharge).
fn innate_uses(usage: &str, pb: i32, modifier: impl Fn(Ability) -> i32, ability: Option<Ability>) -> Option<(u16, String)> {
    let u = usage.trim().to_lowercase();
    let (count, rest) = match u.split_once('/') {
        Some((n, "long rest")) => (n.to_string(), "LR"),
        Some((n, "short rest")) => (n.to_string(), "SR"),
        Some((n, "day")) => (n.to_string(), "Dawn"),
        _ if u.ends_with(" uses") || u.ends_with(" use") => (u.trim_end_matches(" uses").trim_end_matches(" use").to_string(), "LR"),
        _ => return None,
    };
    let n = match count.trim() {
        "pb" => pb,
        "int mod" => modifier(Ability::Int),
        other => match Ability::parse(other) {
            Some(a) => modifier(a),
            None if other == "mod" => ability.map(&modifier).unwrap_or(1),
            None => other.parse().ok()?,
        },
    };
    Some((n.max(1) as u16, rest.into()))
}

fn number_word(w: &str) -> Option<i32> {
    Some(match w.trim_matches(|c: char| !c.is_ascii_alphanumeric()) {
        "one" => 1,
        "two" => 2,
        "three" => 3,
        "four" => 4,
        "five" => 5,
        "six" => 6,
        "seven" => 7,
        "eight" => 8,
        "ten" => 10,
        n => n.parse().ok()?,
    })
}

fn column_mut(a: &mut Actions, kind: ActionKind) -> &mut Vec<ActionEntry> {
    match kind {
        ActionKind::Action => &mut a.action,
        ActionKind::Bonus => &mut a.bonus,
        ActionKind::Reaction => &mut a.reaction,
    }
}

/// What the character can do with an action, a bonus action and a reaction:
/// features by their rules text, spells by casting time.
pub fn actions(ch: &Character, build: &Build, comp: &Compendium, features: &[FeatureEntry]) -> Actions {
    let mut a = Actions::default();
    let push = |list: &mut Vec<ActionEntry>, e: ActionEntry| {
        if !list.iter().any(|x| x.name == e.name) {
            list.push(e);
        }
    };
    for f in features {
        // "Channel Divinity: Preserve Life" reads as "Preserve Life" in a short list.
        let name = f.feature.name.strip_prefix("Channel Divinity: ").unwrap_or(&f.feature.name);
        if name.starts_with("Weapon Mastery") || matches!(name, "Spellcasting" | "Pact Magic" | "Extra Attack" | "Ability Score Improvement") {
            continue;
        }
        let act = activation(&Entry::plain(&f.feature.body));
        for kind in act.primary.into_iter().chain(act.also.iter().copied()) {
            push(column_mut(&mut a, kind), ActionEntry::feature(name));
        }
        if let Some(kind) = act.command {
            push(column_mut(&mut a, kind), ActionEntry::feature(&format!("{name} (command)")));
        }
    }
    // Two Light weapons: the extra attack (2024 Light property) is a Bonus Action.
    let light = ch
        .inventory
        .iter()
        .filter_map(|i| i.item.as_ref().and_then(|k| comp.items.get(k)))
        .filter(|it| it.weapon.as_ref().is_some_and(|w| !w.ranged && w.properties.iter().any(|p| p == "light")))
        .count();
    if light >= 2 {
        push(&mut a.bonus, ActionEntry::feature("Light weapon attack"));
    }
    push(&mut a.reaction, ActionEntry::feature("Opportunity Attack"));

    // Spells the character can cast now (not spellbook-only), by casting
    // time, in the default order. One-action spells are candidates for the
    // Action column but hidden by default: "Cast Spell" stands for them.
    let mut spells: Vec<(&compendium::Spell, &SpellMode)> = Vec::new();
    for k in build.spells.iter().filter(|k| k.mode != SpellMode::Spellbook) {
        let Some(s) = comp.spells.get(&k.key) else { continue };
        if !spells.iter().any(|(x, _)| x.key == s.key) {
            spells.push((s, &k.mode));
        }
    }
    spells.sort_by_key(|(s, mode)| spell_rank(s, mode));
    let mut casts_spells = false;
    for (s, _) in spells {
        let Some(kind) = spell_column(&s.time) else { continue };
        let e = ActionEntry { name: s.name.clone(), spell: Some(s.key.clone()), pinned: false, default: kind != ActionKind::Action, custom: false };
        match kind {
            ActionKind::Action => {
                casts_spells = true;
                push(&mut a.action, e);
            }
            ActionKind::Bonus => push(&mut a.bonus, e),
            ActionKind::Reaction => push(&mut a.reaction, e),
        }
    }
    if casts_spells {
        a.action.insert(0, ActionEntry::feature("Cast Spell"));
    }
    a
}

/// How a feature is used, read from its rules text.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Activation {
    /// The action it takes: the first activation phrase in the text ("As a
    /// Magic action, you…", "you can take a Reaction to…"). Later phrases
    /// describe follow-ups or other creatures.
    pub primary: Option<ActionKind>,
    /// A summon or effect you command with a different action ("you can take
    /// a Bonus Action on your turn to command it").
    pub command: Option<ActionKind>,
    /// Other actions the feature also lets you take (Polearm Master: a bonus
    /// action attack and a reaction attack).
    pub also: Vec<ActionKind>,
    /// Every activation phrase found, by position (for audits).
    pub found: Vec<(usize, ActionKind, &'static str)>,
}

const ACTIVATIONS: &[(&str, ActionKind)] = &[
    ("as a bonus action", ActionKind::Bonus),
    ("take a bonus action", ActionKind::Bonus),
    ("use a bonus action", ActionKind::Bonus),
    ("use your bonus action", ActionKind::Bonus),
    ("a bonus action on each", ActionKind::Bonus),
    ("take a reaction", ActionKind::Reaction),
    ("use your reaction", ActionKind::Reaction),
    ("use a reaction", ActionKind::Reaction),
    ("as a reaction", ActionKind::Reaction),
    ("your reaction", ActionKind::Reaction),
    ("as a magic action", ActionKind::Action),
    ("take a magic action", ActionKind::Action),
    ("take the magic action", ActionKind::Action),
    ("as an action", ActionKind::Action),
    ("use your action", ActionKind::Action),
    ("use an action", ActionKind::Action),
    ("take an action to", ActionKind::Action),
    ("as a utilize action", ActionKind::Action),
];

/// Reads how a feature is used from its (lowercase) rules text.
pub fn activation(t: &str) -> Activation {
    // Phrases that name an action without using one ("a spell that doesn't
    // require a Reaction to cast", "you can't take Reactions").
    let mut t = t.to_lowercase();
    for negated in ["doesn't require a reaction", "don't require a reaction", "can't take reactions", "can't take a reaction", "no action required", "doesn't require an action", "requires no action"] {
        t = t.replace(negated, &" ".repeat(negated.len()));
    }
    let mut found: Vec<(usize, ActionKind, &'static str)> = Vec::new();
    for (p, kind) in ACTIVATIONS {
        for (i, _) in t.match_indices(p) {
            if !found.iter().any(|(j, ..)| *j == i) {
                found.push((i, *kind, p));
            }
        }
    }
    found.sort_by_key(|f| f.0);
    let after = |i: usize, p: &str| {
        let end = (i + p.len() + 60).min(t.len());
        let end = (end..=t.len()).find(|&e| t.is_char_boundary(e)).unwrap_or(t.len());
        t[i..end].to_string()
    };
    let before = |i: usize, n: usize| {
        let start = (0..=i.saturating_sub(n)).rev().find(|&b| t.is_char_boundary(b)).unwrap_or(0);
        t[start..i].to_string()
    };
    // Commanding a summon or moving an effect ("take a Bonus Action on your
    // turn to command it", "to cause the cannon to activate", "move the spirit").
    let is_command = |i: usize, p: &str| {
        let w = after(i, p);
        ["command it", "command the familiar", "command your familiar", "command the drake", "to activate", "move the spirit", "move the illusion", "move the cube", "move the echo"]
            .iter()
            .any(|k| w.contains(k))
    };
    // Housekeeping that isn't how the feature is used.
    let is_minor = |i: usize, p: &str| {
        let w = after(i, p);
        let b = before(i, 60);
        ["dismiss", "revive", "retract", "doff", "end it", "end the ", "to leave"].iter().any(|k| w.contains(k))
            || ["has died", "if it dies", "has been destroyed", "retract", "end it", "end this effect", "dismiss"].iter().any(|k| b.contains(k))
    };
    // "When you use your action to cast a cantrip, …": a condition, not a use.
    // "Rather than as an action" compares; "…, or take the Magic action" is
    // an option inside another action (Fast Hands).
    let is_condition = |i: usize| {
        let b = before(i, 16);
        b.ends_with("when you ") || b.ends_with("whenever you ") || b.ends_with("rather than ") || b.ends_with("or ")
    };
    // Someone else's action ("your mount can take a Reaction", "each target
    // can …", "on its turns it can only … take a bonus action").
    let is_others = |i: usize| {
        let b = format!(" {}", before(i, 32));
        let sentence_start = t[..i].rfind(". ").map(|j| j + 2).unwrap_or(0);
        let sentence = format!(" {}", &t[sentence_start..i]);
        [" it can", " they can", " ally can", " target can", " wielder can", " mount can", " creature can", " allies can", " familiar must"].iter().any(|k| b.contains(k))
            || [" on its turn", " on their turn", " the target can", " each target", " the creature can"].iter().any(|k| sentence.contains(k))
    };
    let own: Vec<&(usize, ActionKind, &'static str)> =
        found.iter().filter(|(i, _, p)| !is_command(*i, p) && !is_minor(*i, p) && !is_condition(*i) && !is_others(*i)).collect();
    let primary = own.first().map(|f| f.1);
    let mut also: Vec<ActionKind> = Vec::new();
    for f in &own {
        if Some(f.1) != primary && !also.contains(&f.1) {
            also.push(f.1);
        }
    }
    let command = found.iter().find(|(i, kind, p)| is_command(*i, p) && Some(*kind) != primary && !also.contains(kind)).map(|f| f.1);
    Activation { primary, command, also, found }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uses(text: &str) -> Option<(u16, String, String)> {
        uses_at(text, 20)
    }

    fn uses_at(text: &str, level: u8) -> Option<(u16, String, String)> {
        uses_from_text(text, 3, level, |_| 2, |c| if c == "paladin" { 5 } else { 0 }).map(|u| (u.uses, u.unit, u.recharge))
    }

    #[test]
    fn uses_from_feature_text() {
        let b = "You can use this feature a number of times equal to your Proficiency Bonus, and you regain all expended uses when you finish a Long Rest.";
        assert_eq!(uses(b), Some((3, String::new(), "LR".into())));
        let once = "Once you use this feature, you can't do so again until you finish a Short or Long Rest.";
        assert_eq!(uses(once), Some((1, String::new(), "SR".into())));
        let bard = "You can confer a Bardic Inspiration die a number of times equal to your Charisma modifier (minimum of once), and you regain all expended uses when you finish a Long Rest.";
        assert_eq!(uses(bard), Some((2, String::new(), "LR".into())));
        let loh = "Your blessed touch can heal wounds. You have a pool of healing power that replenishes when you finish a Long Rest. With that pool, you can restore a total number of Hit Points equal to five times your Paladin level.";
        assert_eq!(uses(loh), Some((25, "HP".into(), "LR".into())));
        let sup = "You have four Superiority Dice, which are d8s. A Superiority Die is expended when you use it. You regain all expended Superiority Dice when you finish a Short or Long Rest.";
        assert_eq!(uses(sup), Some((4, "d8".into(), "SR".into())));
        // Counts that grow with level.
        let indomitable = "You must use the new roll, and you can't use this feature again until you finish a Long Rest. You can use this feature twice before a Long Rest starting at level 13 and three times before a Long Rest starting at level 17.";
        assert_eq!(uses_at(indomitable, 9).map(|u| u.0), Some(1));
        assert_eq!(uses_at(indomitable, 13).map(|u| u.0), Some(2));
        assert_eq!(uses_at(indomitable, 20).map(|u| u.0), Some(3));
        let surge = "Once you use this feature, you can't do so again until you finish a Short or Long Rest. Starting at level 17, you can use it twice before a rest but only once on a turn.";
        assert_eq!(uses_at(surge, 2), Some((1, String::new(), "SR".into())));
        assert_eq!(uses_at(surge, 17).map(|u| u.0), Some(2));
        // 2014 wording.
        let indomitable14 = "Once you do so, you can't use this feature again until you finish a long rest. You can use this feature twice between long rests starting at 13th level and three times between long rests starting at 17th level.";
        assert_eq!(uses_at(indomitable14, 10).map(|u| u.0), Some(1));
        assert_eq!(uses_at(indomitable14, 17).map(|u| u.0), Some(3));
        let loh14 = "You have a pool of healing power that replenishes when you take a long rest. With that pool, you can restore a total number of hit points equal to your paladin level × 5.";
        assert_eq!(uses_at(loh14, 20), Some((25, "HP".into(), "LR".into())));
        let sense = "You can use this feature a number of times equal to 1 + your Charisma modifier. When you finish a long rest, you regain all expended uses.";
        assert_eq!(uses_at(sense, 20).map(|u| u.0), Some(3));
        let cd = "You can use this class's Channel Divinity twice. You regain one of its expended uses when you finish a Short Rest, and you regain all expended uses when you finish a Long Rest.";
        assert_eq!(recharge(cd).as_deref(), Some("1 SR · all LR"));
        let talisman = "This benefit can be used a number of times equal to your proficiency bonus, and all expended uses are restored when you finish a long rest. This ceremony can be performed during a short or long rest.";
        assert_eq!(recharge(talisman).as_deref(), Some("LR"));
        // Mentions a rest but has no limited uses.
        assert_eq!(uses("The familiar disappears when you finish a Long Rest."), None);
    }

    #[test]
    fn wild_shape_style_recharge() {
        let t = "You regain one expended use when you finish a Short Rest, and you regain all expended uses when you finish a Long Rest.";
        assert_eq!(recharge(t).as_deref(), Some("1 SR · all LR"));
    }

    #[test]
    fn innate_spell_uses() {
        assert_eq!(innate_uses("1/long rest", 3, |_| 2, None), Some((1, "LR".into())));
        assert_eq!(innate_uses("pb/long rest", 3, |_| 2, None), Some((3, "LR".into())));
        assert_eq!(innate_uses("int/long rest", 3, |_| 4, None), Some((4, "LR".into())));
        assert_eq!(innate_uses("at will", 3, |_| 2, None), None);
    }

    #[test]
    fn action_types_from_text() {
        let p = |t: &str| activation(t).primary;
        assert_eq!(p("You can enter your Rage as a Bonus Action"), Some(ActionKind::Bonus));
        assert_eq!(p("When an attacker hits you, you can take a Reaction to halve the damage"), Some(ActionKind::Reaction));
        assert_eq!(p("As a Magic action, you present your Holy Symbol"), Some(ActionKind::Action));
        assert_eq!(p("You have Resistance to Poison damage"), None);
        let di = "As a Magic action, choose any Cleric spell of level 5 or lower that doesn't require a Reaction to cast.";
        assert_eq!(p(di), Some(ActionKind::Action));
        // The first phrase is the feature's own; a later one commands a summon.
        let spirit = "As a Magic action, you can expend a use of your Wild Shape to summon your Wildfire Spirit. In combat, the spirit takes its turn immediately after yours. It can move and use its Reaction on its own, but the only action it takes is the Dodge action, unless you take a Bonus Action on your turn to command it to take another action.";
        let a = activation(spirit);
        assert_eq!(a.primary, Some(ActionKind::Action));
        assert_eq!(a.command, Some(ActionKind::Bonus));
        // A condition on another action is not the feature's own action.
        let war = "When you use your action to cast a cantrip, you can make one weapon attack as a bonus action.";
        assert_eq!(p(war), Some(ActionKind::Bonus));
        // Several real uses: listed in each column.
        let polearm = "You can use a bonus action to make a melee attack with the opposite end of the weapon. While you are wielding a Quarterstaff, you can take a Reaction to make one melee attack against a creature that enters your reach.";
        let a = activation(polearm);
        assert_eq!((a.primary, a.also.clone()), (Some(ActionKind::Bonus), vec![ActionKind::Reaction]));
        // Someone else's reaction doesn't count.
        let ally = "As a Bonus Action, choose up to three creatures. Each target can immediately take a Reaction to right itself.";
        assert_eq!(activation(ally).also, vec![]);
        // A summon you command.
        let defender = "It acts during your turn. The only action it takes is the Dodge action unless you take a Bonus Action to command it to take an action. If the defender has died within the last hour, you can take a Magic action to touch it.";
        let a = activation(defender);
        assert_eq!((a.primary, a.command), (None, Some(ActionKind::Bonus)));
    }
}
