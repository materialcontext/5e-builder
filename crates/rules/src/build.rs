//! Grant expansion: walks every grant reachable from the character's
//! species, background, classes and selections, producing the list of
//! choices (resolved or pending) and the leaf grants that apply.
//!
//! Choice paths are stable strings: `<owner>/<choice id>`, and a choice made
//! inside a selected option lives under `<choice path>/<selected value>/...`.
//! Changing a selection therefore orphans (and [`Character::set_choice`]
//! clears) everything nested under the old value.

use crate::character::Character;
use crate::spellcasting::max_spell_level;
use compendium::*;
use std::collections::BTreeMap;

/// Wizard step a choice belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Step {
    Basics,
    Class,
    Background,
    Species,
    Abilities,
    Spells,
    Equipment,
    Details,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChoiceState {
    pub path: String,
    pub label: String,
    /// Where the choice comes from, e.g. "Wizard 1" or "Acolyte".
    pub origin: String,
    pub step: Step,
    pub count: u8,
    pub options: Options,
    pub selected: Vec<String>,
    /// Character level at which the choice became available.
    pub level: u8,
    /// Class whose spell list/ability applies (spell choices).
    pub class: Option<Key>,
}

impl ChoiceState {
    /// Number of filled picks (positional choices may hold empty placeholders).
    pub fn picked(&self) -> usize {
        self.selected.iter().filter(|s| !s.is_empty()).count()
    }
    pub fn is_complete(&self) -> bool {
        self.picked() >= self.count as usize
    }
    /// Picks are positional (e.g. +2 then +1).
    pub fn ordered(&self) -> bool {
        matches!(&self.options, Options::Abilities { amounts, .. } if amounts.windows(2).any(|w| w[0] != w[1]))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Granted {
    pub grant: Grant,
    pub origin: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FeatureEntry {
    pub feature: Feature,
    pub origin: String,
    pub kind: FeatureKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FeatureKind {
    Species,
    Background,
    Class,
    Feat,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpellMode {
    Cantrip,
    /// Prepared from the class list (counts against the prepared limit).
    Prepared,
    /// Always prepared (domain spells, feat spells, ...).
    AlwaysPrepared,
    Known,
    /// In the spellbook, not necessarily prepared.
    Spellbook,
    /// Castable without a slot ("1/long rest", "at will").
    Innate(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct KnownSpell {
    pub key: Key,
    pub origin: String,
    pub ability: Option<Ability>,
    pub mode: SpellMode,
    pub class: Option<Key>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expansion {
    Spell(Key),
    Filter(SpellFilter),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClassState {
    pub key: Key,
    pub level: u8,
    pub subclass: Option<Key>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Build {
    pub choices: Vec<ChoiceState>,
    pub grants: Vec<Granted>,
    pub features: Vec<FeatureEntry>,
    pub spells: Vec<KnownSpell>,
    pub feats: Vec<(Key, String)>,
    pub optional_features: Vec<(Key, String)>,
    pub classes: Vec<ClassState>,
    pub warnings: Vec<String>,
    /// Spells added to a class's spell list (class `None`: every list).
    pub expanded: Vec<(Option<Key>, Expansion)>,
    /// Final ability scores (base + bonuses), indexed by [`Ability::index`].
    pub scores: [i32; 6],
}

impl Build {
    pub fn choice(&self, path: &str) -> Option<&ChoiceState> {
        self.choices.iter().find(|c| c.path == path)
    }
    pub fn pending(&self) -> impl Iterator<Item = &ChoiceState> {
        self.choices.iter().filter(|c| !c.is_complete())
    }
    pub fn score(&self, a: Ability) -> i32 {
        self.scores[a.index()]
    }
    pub fn modifier(&self, a: Ability) -> i32 {
        modifier(self.score(a))
    }
    pub fn profs(&self) -> impl Iterator<Item = &Prof> {
        self.grants.iter().filter_map(|g| match &g.grant {
            Grant::Prof(p) => Some(p),
            _ => None,
        })
    }
    pub fn has_prof(&self, p: &Prof) -> bool {
        self.profs().any(|x| x == p)
    }
    pub fn has_feat_named(&self, name: &str) -> bool {
        self.feats.iter().any(|(k, _)| k.name_part() == name.to_lowercase())
    }
    pub fn has_feature_named(&self, name: &str) -> bool {
        self.features.iter().any(|f| f.feature.name.eq_ignore_ascii_case(name))
    }
}

#[derive(Clone)]
struct Ctx {
    path: String,
    /// Scope level: class level for class content, character level otherwise.
    level: u8,
    origin: String,
    step: Step,
    gained_at: u8,
    kind: FeatureKind,
    casting: Option<(Option<Ability>, Key)>,
    /// Ability raised by the granting feat; used by its "inherit" spells.
    inherit_ability: Option<Ability>,
}

impl Ctx {
    fn child(&self, segment: &str) -> Ctx {
        Ctx { path: format!("{}/{}", self.path, segment), ..self.clone() }
    }
}

struct Walker<'a> {
    ch: &'a Character,
    comp: &'a Compendium,
    out: Build,
    depth: u8,
    formulas: Vec<(usize, String, u8, Option<Ability>)>,
}

/// Expands a character against the compendium.
pub fn build(ch: &Character, comp: &Compendium) -> Build {
    let mut w = Walker { ch, comp, out: Build::default(), depth: 0, formulas: Vec::new() };
    w.classes();
    w.background();
    w.species();
    w.finish();
    w.out
}

impl<'a> Walker<'a> {
    fn char_level(&self) -> u8 {
        self.ch.total_level().max(1)
    }

    // ------------------------------------------------------------ roots

    fn species(&mut self) {
        let Some(key) = &self.ch.species else { return };
        let Some(sp) = self.comp.species.get(key) else {
            self.out.warnings.push(format!("Unknown species {key}"));
            return;
        };
        let ctx = Ctx {
            path: "species".into(),
            level: self.char_level(),
            origin: sp.name.clone(),
            step: Step::Species,
            gained_at: 1,
            kind: FeatureKind::Species,
            casting: None,
            inherit_ability: None,
        };
        let mut lineage = None;
        if !sp.lineages.is_empty() {
            let c = Choice {
                id: "lineage".into(),
                label: if sp.lineages.iter().all(|l| l.replaces_base) { "Variant".into() } else { "Lineage".into() },
                count: Count::Fixed(1),
                options: Options::Lineage { species: sp.key.clone() },
            };
            let path = self.register(&c, &ctx);
            lineage = self.ch.selected(&path).first().and_then(|v| sp.lineages.iter().find(|l| l.key.0 == *v));
        }
        if sp.sizes.len() > 1 {
            let c = Choice {
                id: "size".into(),
                label: "Size".into(),
                count: Count::Fixed(1),
                options: Options::Bundles(sp.sizes.iter().map(|s| Bundle { name: s.name().into(), grants: vec![] }).collect()),
            };
            self.register(&c, &ctx);
        }
        let legacy_asi = self.ch.settings.legacy_species_asi;
        let replaces = lineage.is_some_and(|l| l.replaces_base);
        let mut features: Vec<&Feature> = Vec::new();
        if !replaces {
            let overridden: Vec<&str> = lineage.map(|l| l.features.iter().map(|f| f.name.as_str()).collect()).unwrap_or_default();
            features.extend(sp.features.iter().filter(|f| !overridden.contains(&f.name.as_str())));
            let grants = lineage.and_then(|l| l.base_override.clone()).unwrap_or_else(|| sp.grants.clone());
            self.walk(&grants, &ctx);
            if legacy_asi && !lineage.is_some_and(|l| l.overrides_asi) {
                let g = sp.legacy_asi.clone();
                self.walk(&g, &ctx.child("asi"));
            }
        }
        if let Some(l) = lineage {
            features.extend(l.features.iter());
            let lctx = Ctx { origin: format!("{} ({})", sp.name, l.name), ..ctx.child(&format!("lineage/{}", l.key)) };
            let grants = l.grants.clone();
            self.walk(&grants, &lctx);
            if legacy_asi {
                let g = l.legacy_asi.clone();
                self.walk(&g, &lctx.child("asi"));
            }
        }
        // 2024 rules: Common plus two standard languages, unless the (legacy)
        // species already defines its languages.
        let species_langs = gives_languages(&sp.grants) || lineage.is_some_and(|l| gives_languages(&l.grants));
        if !species_langs {
            let langs = vec![
                Grant::Prof(Prof::Language("Common".into())),
                Grant::Choose(Choice {
                    id: "languages".into(),
                    label: "Languages".into(),
                    count: Count::Fixed(2),
                    options: Options::AnyStandardLanguage,
                }),
            ];
            self.walk(&langs, &Ctx { origin: "Character".into(), ..ctx.clone() });
        }
        for f in features {
            if legacy_asi || !is_asi_feature(&f.name) {
                self.feature(f.clone(), &ctx);
            }
        }
    }

    fn background(&mut self) {
        let Some(key) = &self.ch.background else { return };
        let Some(bg) = self.comp.backgrounds.get(key) else {
            self.out.warnings.push(format!("Unknown background {key}"));
            return;
        };
        let ctx = Ctx {
            path: "background".into(),
            level: self.char_level(),
            origin: bg.name.clone(),
            step: Step::Background,
            gained_at: 1,
            kind: FeatureKind::Background,
            casting: None,
            inherit_ability: None,
        };
        let legacy_asi = self.ch.settings.legacy_species_asi;
        let mut grants: Vec<Grant> = bg
            .grants
            .iter()
            .filter(|g| !(legacy_asi && matches!(g, Grant::Choose(c) if c.id.starts_with("background.asi"))))
            .filter(|g| !matches!(g, Grant::Choose(c) if c.id.starts_with("background.equipment")))
            .cloned()
            .collect();
        // 2024 rules for older backgrounds: ability scores and an origin feat of your choice.
        if !bg.has_asi && !legacy_asi {
            grants.push(Grant::Choose(standard_background_asi()));
        }
        if !bg.has_feat {
            grants.push(Grant::Choose(Choice {
                id: "origin-feat".into(),
                label: "Origin feat".into(),
                count: Count::Fixed(1),
                options: Options::Feats { categories: vec!["O".into()] },
            }));
        }
        self.walk(&grants, &ctx);
        let equipment: Vec<Grant> =
            bg.grants.iter().filter(|g| matches!(g, Grant::Choose(c) if c.id.starts_with("background.equipment"))).cloned().collect();
        self.walk(&equipment, &Ctx { step: Step::Equipment, ..ctx.clone() });
        for f in &bg.features {
            self.feature(f.clone(), &ctx);
        }
    }

    fn classes(&mut self) {
        // Character level at which each class reached each of its levels.
        let mut reached: BTreeMap<Key, Vec<u8>> = BTreeMap::new();
        for (i, l) in self.ch.levels.iter().enumerate() {
            reached.entry(l.class.clone()).or_default().push(i as u8 + 1);
        }
        for (idx, (key, level)) in self.ch.class_levels().into_iter().enumerate() {
            let Some(class) = self.comp.classes.get(&key) else {
                self.out.warnings.push(format!("Unknown class {key}"));
                continue;
            };
            let at = |cl: u8| reached[&key].get(cl.saturating_sub(1) as usize).copied().unwrap_or(1);
            let origin = format!("{} {}", class.name, level);
            let base = Ctx {
                path: format!("class:{key}"),
                level,
                origin: origin.clone(),
                step: Step::Class,
                gained_at: 1,
                kind: FeatureKind::Class,
                casting: Some((class.spellcasting.ability, key.clone())),
                inherit_ability: None,
            };
            let mut state = ClassState { key: key.clone(), level, subclass: None };

            let start = if idx == 0 { class.starting.clone() } else { class.multiclass.clone() };
            self.walk(&start, &base.child("start"));
            if idx == 0 && !class.equipment.is_empty() {
                let eq = class.equipment.clone();
                self.walk(&eq, &Ctx { step: Step::Equipment, ..base.child("equipment") });
            }
            let g = class.grants.clone();
            self.walk(&g, &base);

            for (fi, f) in class.features.iter().enumerate().filter(|(_, f)| f.level <= level) {
                let fctx = Ctx { gained_at: at(f.level), ..base.child(&format!("f{fi}")) };
                if f.optional {
                    // Optional class features (Tasha's) are opt-in.
                    let mut use_it = vec![Grant::Feature(Feature { name: f.name.clone(), body: f.body.clone() })];
                    use_it.extend(f.grants.iter().cloned());
                    let c = Choice {
                        id: "optional".into(),
                        label: format!("Optional feature: {}", f.name),
                        count: Count::Fixed(1),
                        options: Options::Bundles(vec![
                            Bundle { name: "Use it".into(), grants: use_it },
                            Bundle { name: "Don't use it".into(), grants: vec![] },
                        ]),
                    };
                    self.choose(&c, &fctx);
                    continue;
                }
                if !f.subclass_slot {
                    self.feature(Feature { name: f.name.clone(), body: f.body.clone() }, &fctx);
                }
                self.walk(&f.grants, &fctx);
            }

            for (pi, p) in class.progressions.iter().enumerate() {
                self.progression(p, &base.child(&format!("p{pi}")), level);
            }
            if !class.weapon_mastery.is_empty() {
                let c = Choice {
                    id: "mastery".into(),
                    label: "Weapon Mastery".into(),
                    count: Count::ByLevel(class.weapon_mastery.clone()),
                    options: Options::WeaponMastery,
                };
                self.choose(&c, &base);
            }
            self.class_spells(&class.spellcasting, class.legacy, &base, &key);

            if let Some(sub_level) = class.subclass_level() {
                if level >= sub_level {
                    let c = Choice {
                        id: "subclass".into(),
                        label: class.subclass_title.clone(),
                        count: Count::Fixed(1),
                        options: Options::Subclass { class: key.clone() },
                    };
                    let path = self.choose(&c, &Ctx { gained_at: at(sub_level), ..base.clone() });
                    let selected = self.ch.selected(&path).first().cloned();
                    if let Some(sub) = selected.and_then(|s| self.comp.subclasses.get(&Key(s.clone()))) {
                        if sub.class == key {
                            state.subclass = Some(sub.key.clone());
                            self.subclass(sub, &base, &path, level, class.legacy, &at);
                        }
                    }
                }
            }
            self.out.classes.push(state);
        }
    }

    fn subclass(&mut self, sub: &Subclass, base: &Ctx, choice_path: &str, level: u8, legacy: bool, at: &dyn Fn(u8) -> u8) {
        let sctx = Ctx {
            path: format!("{choice_path}/{}", sub.key),
            origin: format!("{} ({})", base.origin, sub.short_name),
            casting: if sub.spellcasting.ability.is_some() {
                Some((sub.spellcasting.ability, base.casting.as_ref().map(|c| c.1.clone()).unwrap_or_default()))
            } else {
                base.casting.clone()
            },
            ..base.clone()
        };
        let g = sub.grants.clone();
        self.walk(&g, &sctx);
        for (fi, f) in sub.features.iter().enumerate().filter(|(_, f)| f.level <= level) {
            let fctx = Ctx { gained_at: at(f.level), ..sctx.child(&format!("f{fi}")) };
            self.feature(Feature { name: f.name.clone(), body: f.body.clone() }, &fctx);
            self.walk(&f.grants, &fctx);
        }
        for (pi, p) in sub.progressions.iter().enumerate() {
            self.progression(p, &sctx.child(&format!("p{pi}")), level);
        }
        if sub.spellcasting.is_caster() {
            let key = base.casting.as_ref().map(|c| c.1.clone()).unwrap_or_default();
            self.class_spells(&sub.spellcasting, legacy, &sctx, &key);
        }
    }

    fn progression(&mut self, p: &Progression, ctx: &Ctx, level: u8) {
        let options = match &p.kind {
            ProgressionKind::Feats(c) => Options::Feats { categories: c.clone() },
            ProgressionKind::OptionalFeatures(t) => Options::OptionalFeatures { types: t.clone() },
        };
        let c = Choice { id: "pick".into(), label: p.name.clone(), count: Count::ByLevel(p.totals.clone()), options };
        let _ = level;
        self.choose(&c, ctx);
    }

    fn class_spells(&mut self, sc: &Spellcasting, legacy: bool, ctx: &Ctx, class: &Key) {
        let level = ctx.level;
        let max = max_spell_level(sc.caster, level, legacy);
        let list = vec![sc.list.clone()];
        let sctx = Ctx { step: Step::Spells, casting: Some((sc.ability, class.clone())), ..ctx.clone() };
        if !sc.cantrips.is_empty() {
            let c = Choice {
                id: "cantrips".into(),
                label: "Cantrips".into(),
                count: Count::ByLevel(sc.cantrips.clone()),
                options: Options::Spells(SpellFilter { levels: vec![0], classes: list.clone(), ..Default::default() }),
            };
            self.choose(&c, &sctx);
        }
        if max == 0 {
            return;
        }
        let leveled = SpellFilter { levels: (1..=9).collect(), max_level: Some(max), classes: list, ..Default::default() };
        if !sc.spellbook.is_empty() {
            let total: u8 = sc.spellbook.iter().take(level as usize).sum();
            let book = Choice { id: "spellbook".into(), label: "Spellbook".into(), count: Count::Fixed(total), options: Options::Spells(leveled.clone()) };
            let book_path = self.choose(&book, &sctx);
            let within = SpellFilter { within_choice: Some(book_path), ..leveled };
            self.prepared_choice(sc, &sctx, within, level);
        } else if !sc.known.is_empty() {
            let c = Choice { id: "known".into(), label: "Spells known".into(), count: Count::ByLevel(sc.known.clone()), options: Options::Spells(leveled) };
            self.choose(&c, &sctx);
        } else {
            self.prepared_choice(sc, &sctx, leveled, level);
        }
    }

    fn prepared_choice(&mut self, sc: &Spellcasting, ctx: &Ctx, filter: SpellFilter, level: u8) {
        let (count, formula) = if !sc.prepared.is_empty() {
            (Count::ByLevel(sc.prepared.clone()), None)
        } else if let Some(f) = &sc.prepared_formula {
            (Count::Fixed(0), Some(f.clone()))
        } else {
            return;
        };
        let c = Choice { id: "prepared".into(), label: "Prepared spells".into(), count, options: Options::Spells(filter) };
        self.choose(&c, ctx);
        if let Some(f) = formula {
            let idx = self.out.choices.len() - 1;
            self.formulas.push((idx, f, level, sc.ability));
        }
    }

    // ------------------------------------------------------------ walking

    fn feature(&mut self, feature: Feature, ctx: &Ctx) {
        self.out.features.push(FeatureEntry { feature, origin: ctx.origin.clone(), kind: ctx.kind });
    }

    fn walk(&mut self, grants: &[Grant], ctx: &Ctx) {
        if self.depth > 12 {
            return;
        }
        self.depth += 1;
        for (i, g) in grants.iter().enumerate() {
            match g {
                Grant::AtLevel { level, grants } => {
                    if ctx.level >= *level {
                        self.walk(grants, ctx);
                    }
                }
                Grant::Choose(c) => {
                    self.choose(c, ctx);
                }
                Grant::Feat(k) => self.feat(k, &ctx.child(&format!("feat:{k}"))),
                Grant::Spells(group) => self.spell_group(group, &ctx.child(&format!("g{i}"))),
                Grant::Feature(f) => self.feature(f.clone(), ctx),
                Grant::HpPerClassLevel(n) => {
                    let hp = Grant::HpBonus(*n as i32 * ctx.level as i32);
                    self.out.grants.push(Granted { grant: hp, origin: ctx.origin.clone() });
                }
                other => self.out.grants.push(Granted { grant: other.clone(), origin: ctx.origin.clone() }),
            }
        }
        self.depth -= 1;
    }

    /// Registers a choice without expanding its selections. Returns its path.
    fn register(&mut self, c: &Choice, ctx: &Ctx) -> String {
        let path = format!("{}/{}", ctx.path, c.id);
        self.out.choices.push(ChoiceState {
            path: path.clone(),
            label: c.label.clone(),
            origin: ctx.origin.clone(),
            step: ctx.step,
            count: pick_count(c, ctx.level),
            options: c.options.clone(),
            selected: self.ch.selected(&path).to_vec(),
            level: ctx.gained_at,
            class: None,
        });
        path
    }

    /// Registers a choice and expands its valid selections. Returns its path.
    fn choose(&mut self, c: &Choice, ctx: &Ctx) -> String {
        let path = format!("{}/{}", ctx.path, c.id);
        let count = pick_count(c, ctx.level);
        if count == 0 && !matches!(c.options, Options::Spells(_)) {
            return path;
        }
        let selected: Vec<String> = self.ch.selected(&path).to_vec();
        let step = match &c.options {
            Options::Spells(_) => Step::Spells,
            _ => ctx.step,
        };
        let class = ctx.casting.as_ref().map(|c| c.1.clone());
        self.out.choices.push(ChoiceState {
            path: path.clone(),
            label: c.label.clone(),
            origin: ctx.origin.clone(),
            step,
            count,
            options: c.options.clone(),
            selected: selected.clone(),
            level: ctx.gained_at,
            class,
        });
        // Formula counts are resolved later; expand everything selected for now.
        let limit = if count == 0 { selected.len() } else { count as usize };
        let picked: Vec<String> = selected.into_iter().take(limit).collect();
        let child = Ctx { path: path.clone(), ..ctx.clone() };
        self.expand_selection(&c.options, &picked, &child);
        path
    }

    fn expand_selection(&mut self, options: &Options, picked: &[String], ctx: &Ctx) {
        match options {
            Options::Bundles(bundles) => {
                for v in picked {
                    if let Some(b) = bundles.iter().find(|b| &b.name == v) {
                        let grants = b.grants.clone();
                        let child = Ctx { path: format!("{}/{}", ctx.path, v), ..ctx.clone() };
                        self.walk(&grants, &child);
                    }
                }
            }
            Options::Feats { .. } => {
                for v in picked {
                    self.feat(&Key(v.clone()), &ctx.child(v));
                }
            }
            Options::OptionalFeatures { .. } => {
                for v in picked {
                    let Some(of) = self.comp.optional_features.get(&Key(v.clone())) else { continue };
                    self.out.optional_features.push((of.key.clone(), ctx.origin.clone()));
                    let child = Ctx { kind: FeatureKind::Other, ..ctx.child(v) };
                    self.feature(Feature { name: of.name.clone(), body: of.body.clone() }, &child);
                    let g = of.grants.clone();
                    self.walk(&g, &child);
                }
            }
            Options::Abilities { amounts, max, .. } => {
                for (v, amount) in picked.iter().zip(amounts) {
                    if let Some(a) = Ability::parse(v) {
                        if *amount != 0 {
                            self.push(Grant::Ability { ability: a, amount: *amount }, ctx);
                        }
                        if let Some(m) = max {
                            self.push(Grant::AbilityMax { ability: a, max: *m }, ctx);
                        }
                    }
                }
            }
            Options::Spells(_) => {
                let mode = match ctx.path.rsplit('/').next().unwrap_or("") {
                    "cantrips" => SpellMode::Cantrip,
                    "spellbook" => SpellMode::Spellbook,
                    "prepared" => SpellMode::Prepared,
                    _ => SpellMode::Known,
                };
                for v in picked {
                    let key = Key(v.clone());
                    let mode = match (&mode, self.comp.spells.get(&key)) {
                        (SpellMode::Known, Some(s)) if s.level == 0 => SpellMode::Cantrip,
                        (m, _) => m.clone(),
                    };
                    self.out.spells.push(KnownSpell {
                        key,
                        origin: ctx.origin.clone(),
                        ability: ctx.casting.as_ref().and_then(|c| c.0),
                        mode,
                        class: ctx.casting.as_ref().map(|c| c.1.clone()),
                    });
                }
            }
            Options::Subclass { .. } | Options::Lineage { .. } => {}
            Options::Resist(_) => picked.iter().for_each(|v| self.push(Grant::Resist(v.clone()), ctx)),
            Options::Items { .. } => {
                picked.iter().for_each(|v| self.push(Grant::Item { item: ItemRef::Key(Key(v.clone())), qty: 1 }, ctx))
            }
            Options::WeaponMastery => picked.iter().for_each(|v| self.push(Grant::Feature(mastery_feature(self.comp, v)), ctx)),
            Options::ToolExpertise(_) => {
                for v in picked {
                    if let Some(Prof::Tool(t)) = decode_prof(v) {
                        self.push(Grant::ToolExpertise(t), ctx);
                    }
                }
            }
            Options::SkillExpertise | Options::ExpertiseFrom(_) => {
                for v in picked {
                    if let Some(s) = decode_prof(v).and_then(|p| match p {
                        Prof::Skill(s) => Some(s),
                        _ => None,
                    }) {
                        self.push(Grant::Expertise(s), ctx);
                    }
                }
            }
            _ => {
                for v in picked {
                    if let Some(p) = decode_prof(v) {
                        self.push(Grant::Prof(p), ctx);
                    }
                }
            }
        }
    }

    fn push(&mut self, g: Grant, ctx: &Ctx) {
        match g {
            Grant::Feature(f) => self.feature(f, ctx),
            other => self.out.grants.push(Granted { grant: other, origin: ctx.origin.clone() }),
        }
    }

    fn feat(&mut self, key: &Key, ctx: &Ctx) {
        let Some(feat) = self.comp.feats.get(key) else {
            self.out.warnings.push(format!("Unknown feat {key}"));
            return;
        };
        self.out.feats.push((feat.key.clone(), ctx.origin.clone()));
        let fctx = Ctx {
            origin: format!("Feat: {}", feat.name),
            level: self.char_level(),
            kind: FeatureKind::Feat,
            casting: None,
            ..ctx.clone()
        };
        // The ability this feat increases (fixed or chosen) is the spellcasting
        // ability of its spells when they don't name one (Fey-Touched, Ritual Caster).
        let raised = feat.grants.iter().find_map(|g| match g {
            Grant::Ability { ability, .. } if matches!(ability, Ability::Int | Ability::Wis | Ability::Cha) => Some(*ability),
            Grant::Choose(c) => match &c.options {
                Options::Abilities { amounts, .. } if amounts.iter().any(|a| *a != 0) => {
                    self.ch.selected(&format!("{}/{}", fctx.path, c.id)).first().and_then(|v| Ability::parse(v))
                }
                Options::Bundles(bs) => {
                    // ASI feat style: alternatives of ability choices.
                    let sel = self.ch.selected(&format!("{}/{}", fctx.path, c.id)).first()?.clone();
                    let b = bs.iter().find(|b| b.name == sel)?;
                    b.grants.iter().find_map(|g| match g {
                        Grant::Choose(inner) if matches!(inner.options, Options::Abilities { .. }) => self
                            .ch
                            .selected(&format!("{}/{}/{}/{}", fctx.path, c.id, sel, inner.id))
                            .first()
                            .and_then(|v| Ability::parse(v)),
                        _ => None,
                    })
                }
                _ => None,
            },
            _ => None,
        });
        // Otherwise an ability chosen for one of the feat's spell groups (Rune Shaper).
        let raised = raised.or_else(|| {
            feat.grants.iter().enumerate().find_map(|(i, g)| match g {
                Grant::Spells(SpellGroup { ability: SpellAbility::Choose(_), .. }) => {
                    self.ch.selected(&format!("{}/g{i}/ability", fctx.path)).first().and_then(|v| Ability::parse(v))
                }
                _ => None,
            })
        });
        let fctx = Ctx { inherit_ability: raised, ..fctx };
        self.feature(Feature { name: feat.name.clone(), body: feat.body.clone() }, &fctx);
        let g = feat.grants.clone();
        self.walk(&g, &fctx);
    }

    fn spell_group(&mut self, group: &SpellGroup, ctx: &Ctx) {
        let ability = match &group.ability {
            SpellAbility::Fixed(a) => Some(*a),
            SpellAbility::Inherit => ctx.inherit_ability.or_else(|| ctx.casting.as_ref().and_then(|c| c.0)),
            SpellAbility::Choose(from) => {
                let c = Choice {
                    id: "ability".into(),
                    label: "Spellcasting ability".into(),
                    count: Count::Fixed(1),
                    options: Options::Abilities { from: from.clone(), amounts: vec![0], max: None },
                };
                let path = self.choose(&c, ctx);
                self.ch.selected(&path).first().and_then(|s| Ability::parse(s))
            }
        };
        let class = ctx.casting.as_ref().map(|c| c.1.clone());
        for (i, sg) in group.spells.iter().enumerate() {
            if sg.level > ctx.level {
                continue;
            }
            let mode = match &sg.kind {
                SpellKind::Known => SpellMode::Known,
                SpellKind::Prepared => SpellMode::AlwaysPrepared,
                SpellKind::Innate(u) => SpellMode::Innate(u.clone()),
                SpellKind::Expanded => {
                    // Added to the owning class's spell list; level limits come from the class.
                    let filter = match &sg.pick {
                        SpellPick::Fixed(k) => Expansion::Spell(k.clone()),
                        SpellPick::All(f) => Expansion::Filter(f.clone()),
                        SpellPick::Choose { .. } => continue,
                    };
                    self.out.expanded.push((class.clone(), filter));
                    continue;
                }
            };
            let keys: Vec<Key> = match &sg.pick {
                SpellPick::Fixed(k) => vec![k.clone()],
                SpellPick::All(_) => continue,
                SpellPick::Choose { filter, count } => {
                    let c = Choice {
                        id: format!("spell{i}"),
                        label: if filter.levels == [0] { "Cantrip".into() } else { "Spell".into() },
                        count: Count::Fixed(*count),
                        options: Options::Spells(filter.clone()),
                    };
                    let sctx = Ctx { step: ctx.step, casting: None, ..ctx.clone() };
                    // Register without auto-expansion as class spells; record with our mode.
                    let path = format!("{}/{}", sctx.path, c.id);
                    self.out.choices.push(ChoiceState {
                        path: path.clone(),
                        label: c.label.clone(),
                        origin: ctx.origin.clone(),
                        step: Step::Spells,
                        count: *count,
                        options: c.options.clone(),
                        selected: self.ch.selected(&path).to_vec(),
                        level: ctx.gained_at,
                        class: None,
                    });
                    self.ch.selected(&path).iter().take(*count as usize).map(|s| Key(s.clone())).collect()
                }
            };
            for key in keys {
                let mode = match (&mode, self.comp.spells.get(&key)) {
                    (SpellMode::Known, Some(s)) if s.level == 0 => SpellMode::Cantrip,
                    (SpellMode::Known, _) => SpellMode::AlwaysPrepared,
                    (m, _) => m.clone(),
                };
                self.out.spells.push(KnownSpell { key, origin: ctx.origin.clone(), ability, mode, class: class.clone() });
            }
        }
    }

    // ------------------------------------------------------------ finishing

    fn finish(&mut self) {
        let mut scores = [0i32; 6];
        let mut max = [20i32; 6];
        for a in Ability::ALL {
            scores[a.index()] = self.ch.abilities.get(a) as i32;
        }
        for g in &self.out.grants {
            match &g.grant {
                Grant::Ability { ability, amount } => scores[ability.index()] += *amount as i32,
                Grant::AbilityMax { ability, max: m } => max[ability.index()] = max[ability.index()].max(*m as i32),
                _ => {}
            }
        }
        for a in Ability::ALL {
            let base = self.ch.abilities.get(a) as i32;
            scores[a.index()] = scores[a.index()].min(max[a.index()].max(base));
        }
        self.out.scores = scores;
        // Spells with no ability of their own (e.g. Cartomancer's cantrip) use the
        // character's first spellcasting class ability.
        let class_ability = self
            .out
            .classes
            .iter()
            .filter_map(|c| self.comp.classes.get(&c.key))
            .find_map(|c| c.spellcasting.ability.filter(|_| c.spellcasting.is_caster()));
        for s in self.out.spells.iter_mut().filter(|s| s.ability.is_none()) {
            s.ability = class_ability;
        }
        for (idx, formula, level, ability) in std::mem::take(&mut self.formulas) {
            let _ = ability;
            let n = eval_formula(&formula, level as i32, &scores).max(1);
            self.out.choices[idx].count = n as u8;
            // Prepared spells beyond the limit don't count.
            let path = self.out.choices[idx].path.clone();
            let class = self.out.choices[idx].class.clone();
            let keep: Vec<String> = self.ch.selected(&path).iter().take(n as usize).cloned().collect();
            self.out.spells.retain(|s| !(s.mode == SpellMode::Prepared && s.class == class) || keep.contains(&s.key.0));
        }
    }
}

/// Number of picks a choice needs at a scope level. Ability-bonus choices
/// take one pick per bonus amount.
pub fn pick_count(c: &Choice, level: u8) -> u8 {
    match &c.options {
        Options::Abilities { amounts, .. } if c.count.at(level) > 0 => amounts.len() as u8,
        _ => c.count.at(level),
    }
}

fn gives_languages(grants: &[Grant]) -> bool {
    grants.iter().any(|g| match g {
        Grant::Prof(Prof::Language(_)) => true,
        Grant::Choose(c) => match &c.options {
            Options::Languages(_) | Options::AnyLanguage | Options::AnyStandardLanguage | Options::AnyExoticLanguage => true,
            Options::Bundles(b) => b.iter().any(|b| gives_languages(&b.grants)),
            _ => false,
        },
        Grant::AtLevel { grants, .. } => gives_languages(grants),
        _ => false,
    })
}

fn is_asi_feature(name: &str) -> bool {
    name.starts_with("Ability Score Increase")
}

pub fn standard_background_asi() -> Choice {
    let all: Vec<Ability> = Ability::ALL.to_vec();
    let bundle = |name: &str, id: &str, amounts: Vec<i8>| Bundle {
        name: name.into(),
        grants: vec![Grant::Choose(Choice {
            id: id.into(),
            label: "Ability score increase".into(),
            count: Count::Fixed(1),
            options: Options::Abilities { from: all.clone(), amounts, max: None },
        })],
    };
    Choice {
        id: "asi".into(),
        label: "Ability score increases".into(),
        count: Count::Fixed(1),
        options: Options::Bundles(vec![bundle("+2 / +1", "pick", vec![2, 1]), bundle("+1 / +1 / +1", "pick", vec![1, 1, 1])]),
    }
}

fn mastery_feature(comp: &Compendium, item: &str) -> Feature {
    let it = comp.items.get(&Key(item.to_string()));
    let name = it.map(|i| i.name.clone()).unwrap_or_else(|| title_case(Key(item.to_string()).name_part()));
    let mastery = it.and_then(|i| i.weapon.as_ref()).and_then(|w| w.mastery.clone()).unwrap_or_default();
    Feature { name: format!("Weapon Mastery: {name} ({mastery})"), body: vec![] }
}

/// Proficiency option values are prefixed: `skill:athletics`, `tool:…`, `lang:…`, `weapon:…`.
pub fn encode_prof(p: &Prof) -> String {
    match p {
        Prof::Skill(s) => format!("skill:{}", s.id()),
        Prof::Save(a) => format!("save:{}", a.id()),
        Prof::Armor(x) => format!("armor:{x}"),
        Prof::Weapon(x) => format!("weapon:{x}"),
        Prof::Tool(x) => format!("tool:{x}"),
        Prof::Language(x) => format!("lang:{x}"),
    }
}

pub fn decode_prof(v: &str) -> Option<Prof> {
    let (kind, rest) = v.split_once(':')?;
    Some(match kind {
        "skill" => Prof::Skill(Skill::parse(rest)?),
        "save" => Prof::Save(Ability::parse(rest)?),
        "armor" => Prof::Armor(rest.into()),
        "weapon" => Prof::Weapon(rest.into()),
        "tool" => Prof::Tool(rest.into()),
        "lang" => Prof::Language(rest.into()),
        _ => return None,
    })
}

/// Evaluates 5etools formulas like `<$level$> / 2 + <$cha_mod$>` (floor division).
pub fn eval_formula(f: &str, level: i32, scores: &[i32; 6]) -> i32 {
    let mut s = f.replace("<$level$>", &level.to_string());
    for a in Ability::ALL {
        s = s.replace(&format!("<${}_mod$>", a.id()), &format!("({})", modifier(scores[a.index()])));
    }
    let tokens: Vec<String> = tokenize(&s);
    let mut pos = 0;
    expr(&tokens, &mut pos)
}

fn tokenize(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut num = String::new();
    for c in s.chars() {
        if c.is_ascii_digit() {
            num.push(c);
        } else {
            if !num.is_empty() {
                out.push(std::mem::take(&mut num));
            }
            if "+-*/()".contains(c) {
                out.push(c.to_string());
            }
        }
    }
    if !num.is_empty() {
        out.push(num);
    }
    out
}

fn expr(t: &[String], p: &mut usize) -> i32 {
    let mut v = term(t, p);
    while *p < t.len() && (t[*p] == "+" || t[*p] == "-") {
        let op = t[*p].clone();
        *p += 1;
        let r = term(t, p);
        v = if op == "+" { v + r } else { v - r };
    }
    v
}

fn term(t: &[String], p: &mut usize) -> i32 {
    let mut v = atom(t, p);
    while *p < t.len() && (t[*p] == "*" || t[*p] == "/") {
        let op = t[*p].clone();
        *p += 1;
        let r = atom(t, p);
        v = if op == "*" { v * r } else if r != 0 { v.div_euclid(r) } else { v };
    }
    v
}

fn atom(t: &[String], p: &mut usize) -> i32 {
    if *p >= t.len() {
        return 0;
    }
    let tok = t[*p].clone();
    *p += 1;
    match tok.as_str() {
        "(" => {
            let v = expr(t, p);
            if *p < t.len() && t[*p] == ")" {
                *p += 1;
            }
            v
        }
        "-" => -atom(t, p),
        n => n.parse().unwrap_or(0),
    }
}
