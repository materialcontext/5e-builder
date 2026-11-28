//! The "Custom" step: free control over the character, outside the rules
//! as written (house rules, DM rewards, homebrew). Everything is stored as
//! ordinary grants in `Character::custom`, so the engine applies it to every
//! number and it prints on the sheet.

use crate::editor::{icon, EditorCtx, StepChoices};
use crate::entries::Entries;
use crate::picker::{option_list, ListState};
use compendium::*;
use leptos::prelude::*;
use rules::{ChoiceState, Step};

const DAMAGE: [&str; 13] = ["acid", "bludgeoning", "cold", "fire", "force", "lightning", "necrotic", "piercing", "poison", "psychic", "radiant", "slashing", "thunder"];

/// What a custom grant is, in words, for the list of additions.
fn describe_custom(comp: &Compendium, g: &Grant) -> String {
    match g {
        Grant::Feat(k) => format!("Feat: {}", comp.feats.get(k).map(|f| f.name.clone()).unwrap_or_else(|| title_case(k.name_part()))),
        Grant::Spells(sg) => {
            let names: Vec<String> = sg
                .spells
                .iter()
                .map(|s| {
                    let name = match &s.pick {
                        SpellPick::Fixed(k) => comp.spells.get(k).map(|x| x.name.clone()).unwrap_or_else(|| title_case(k.name_part())),
                        _ => "spell".into(),
                    };
                    let how = match &s.kind {
                        SpellKind::Prepared => "always prepared".to_string(),
                        SpellKind::Known => "known".to_string(),
                        SpellKind::Innate(u) => u.clone(),
                        SpellKind::Expanded => "on your spell list".to_string(),
                    };
                    format!("{name} ({how})")
                })
                .collect();
            format!("Spell: {}", names.join(", "))
        }
        Grant::Feature(f) => format!("Feature: {}", f.name),
        Grant::Prof(p) => {
            let kind = match p {
                Prof::Skill(_) => "Skill proficiency",
                Prof::Save(_) => "Saving throw proficiency",
                Prof::Armor(_) => "Armor training",
                Prof::Weapon(_) => "Weapon proficiency",
                Prof::Tool(_) => "Tool proficiency",
                Prof::Language(_) => "Language",
            };
            format!("{kind}: {}", p.label())
        }
        Grant::Expertise(sk) => format!("Expertise: {}", sk.name()),
        Grant::Resist(d) => format!("Resistance: {}", title_case(d)),
        Grant::Darkvision(r) => format!("Darkvision {r} ft."),
        Grant::Ability { ability, amount } => format!("{} {}", ability.name(), signed(*amount as i32)),
        Grant::HpBonus(n) => format!("Hit point maximum {}", signed(*n)),
        Grant::AcBonus { amount, .. } => format!("Armor Class {}", signed(*amount as i32)),
        Grant::SpeedBonus { mode, amount } => format!("{} speed +{amount} ft.", title_case(mode)),
        other => describe(std::slice::from_ref(other)),
    }
}

/// Replace (or remove, when `None`) the single custom grant matching `is`.
fn set_single(ctx: EditorCtx, is: fn(&Grant) -> bool, g: Option<Grant>) {
    ctx.edit(|c| {
        c.custom.retain(|x| !is(x));
        if let Some(g) = g {
            c.custom.push(g);
        }
    });
}

#[component]
pub fn CustomStep() -> impl IntoView {
    let ctx = expect_context::<EditorCtx>();
    let comp = ctx.comp.get_value();

    // ---- ability adjustments
    let ability_rows = Ability::ALL
        .iter()
        .map(|&a| {
            let current = move || {
                ctx.ch.with(|c| {
                    c.custom.iter().filter_map(|g| if let Grant::Ability { ability, amount } = g { (*ability == a).then_some(*amount as i32) } else { None }).sum::<i32>()
                })
            };
            view! {
                <label class="field inline custom-num">
                    <span>{a.name()}</span>
                    <input class="num" type="number" min="-10" max="10" prop:value=move || current().to_string()
                        aria-label=format!("{} adjustment", a.name())
                        on:change=move |ev| {
                            let v: i8 = event_target_value(&ev).parse().unwrap_or(0);
                            ctx.edit(|c| {
                                c.custom.retain(|g| !matches!(g, Grant::Ability { ability, .. } if *ability == a));
                                if v != 0 {
                                    c.custom.push(Grant::Ability { ability: a, amount: v.clamp(-10, 10) });
                                }
                            });
                        } />
                    <span class="muted small">{move || ctx.build.with(|b| format!("now {}", b.score(a)))}</span>
                </label>
            }
        })
        .collect_view();

    // ---- single-value bonuses
    let bonus = move |label: &'static str, help: &'static str, get: fn(&Grant) -> Option<i32>, is: fn(&Grant) -> bool, make: fn(i32) -> Grant| {
        view! {
            <label class="field inline custom-num">
                <span>{label}</span>
                <input class="num" type="number" min="-50" max="200" aria-label=label
                    prop:value=move || ctx.ch.with(|c| c.custom.iter().find_map(get).unwrap_or(0).to_string())
                    on:change=move |ev| {
                        let v: i32 = event_target_value(&ev).parse().unwrap_or(0);
                        set_single(ctx, is, (v != 0).then(|| make(v)));
                    } />
                <span class="muted small">{help}</span>
            </label>
        }
    };

    // ---- proficiencies, expertise, languages, resistances
    let kind = RwSignal::new("skill".to_string());
    let value = RwSignal::new(String::new());
    let c_tools = comp.clone();
    let tool_names: Vec<String> = {
        let mut v: Vec<String> = c_tools
            .items
            .iter()
            .filter(|i| i.rarity.is_none() && matches!(i.kind.as_str(), "Artisan's Tools" | "Tools" | "Gaming Set" | "Musical Instrument"))
            .map(|i| i.name.clone())
            .collect();
        v.sort();
        v.dedup();
        v
    };
    let lang_names: Vec<String> = {
        let mut v: Vec<String> = comp.languages.iter().map(|l| l.name.clone()).collect();
        v.sort();
        v.dedup();
        v
    };
    let value_input = move || {
        let k = kind.get();
        let opts: Vec<String> = match k.as_str() {
            "skill" | "expertise" => Skill::ALL.iter().map(|s| s.name().to_string()).collect(),
            "save" => Ability::ALL.iter().map(|a| a.name().to_string()).collect(),
            "armor" => ["Light", "Medium", "Heavy", "Shield"].iter().map(|s| s.to_string()).collect(),
            "resist" => DAMAGE.iter().map(|d| title_case(d)).collect(),
            _ => vec![],
        };
        if opts.is_empty() {
            let list_id = format!("custom-{k}-list");
            let names = match k.as_str() {
                "tool" => tool_names.clone(),
                "language" => lang_names.clone(),
                _ => vec!["Simple".into(), "Martial".into(), "Longsword".into(), "Longbow".into()],
            };
            view! {
                <input type="text" list=list_id.clone() aria-label="Which one" placeholder="Type or pick a name"
                    prop:value=move || value.get() on:input=move |ev| value.set(event_target_value(&ev)) />
                <datalist id=list_id>{names.into_iter().map(|n| view! { <option value=n /> }).collect_view()}</datalist>
            }
            .into_any()
        } else {
            view! {
                <select aria-label="Which" on:change=move |ev| value.set(event_target_value(&ev))>
                    <option value="">"Choose…"</option>
                    {opts.into_iter().map(|o| view! { <option value=o.clone()>{o.clone()}</option> }).collect_view()}
                </select>
            }
            .into_any()
        }
    };
    let add_prof = move |_| {
        let v = value.get().trim().to_string();
        if v.is_empty() {
            return;
        }
        let g = match kind.get().as_str() {
            "skill" => Skill::parse(&v).map(|s| Grant::Prof(Prof::Skill(s))),
            "expertise" => Skill::parse(&v).map(Grant::Expertise),
            "save" => Ability::parse(&v).map(|a| Grant::Prof(Prof::Save(a))),
            "armor" => Some(Grant::Prof(Prof::Armor(v.to_lowercase()))),
            "weapon" => Some(Grant::Prof(Prof::Weapon(v.to_lowercase()))),
            "tool" => Some(Grant::Prof(Prof::Tool(v.to_lowercase()))),
            "language" => Some(Grant::Prof(Prof::Language(v.clone()))),
            "resist" => Some(Grant::Resist(v.to_lowercase())),
            _ => None,
        };
        if let Some(g) = g {
            ctx.edit(|c| c.custom.push(g.clone()));
            value.set(String::new());
        }
    };

    // ---- feats and spells (any, prerequisites not checked)
    let pseudo = |options: Options| ChoiceState {
        path: "custom/add".into(),
        label: String::new(),
        origin: "Custom".into(),
        step: Step::Custom,
        count: 1,
        options,
        selected: vec![],
        level: 1,
        class: None,
    };
    let c_feats = comp.clone();
    let feat_items = Memo::new(move |_| {
        let choice = pseudo(Options::Feats { categories: vec![] });
        let mut items = ctx.build.with(|b| ctx.ch.with(|c| rules::options(&choice, b, c, &c_feats)));
        for o in &mut items {
            // Custom additions ignore prerequisites; keep the reason visible as a note.
            if let Some(d) = o.disabled.take() {
                if d != "Already taken" {
                    o.warning = Some(format!("Prerequisite not met here: {d}"));
                } else {
                    o.warning = Some(d);
                }
            }
        }
        items
    });
    let add_feat = Callback::new(move |v: Vec<String>| {
        if let Some(k) = v.into_iter().next() {
            ctx.edit(|c| c.custom.push(Grant::Feat(Key(k.clone()))));
        }
    });
    let spell_kind = RwSignal::new("prepared".to_string());
    let c_spells = comp.clone();
    let spell_items = Memo::new(move |_| {
        let choice = pseudo(Options::Spells(SpellFilter::default()));
        ctx.build.with(|b| ctx.ch.with(|c| rules::options(&choice, b, c, &c_spells)))
    });
    let add_spell = Callback::new(move |v: Vec<String>| {
        if let Some(k) = v.into_iter().next() {
            let kind = match spell_kind.get_untracked().as_str() {
                "known" => SpellKind::Known,
                "1/long rest" => SpellKind::Innate("1/long rest".into()),
                "at will" => SpellKind::Innate("at will".into()),
                _ => SpellKind::Prepared,
            };
            let g = Grant::Spells(SpellGroup {
                name: Some("Custom".into()),
                ability: SpellAbility::Inherit,
                spells: vec![SpellGrant { level: 0, kind, pick: SpellPick::Fixed(Key(k)) }],
            });
            ctx.edit(|c| c.custom.push(g.clone()));
        }
    });
    let feat_state = ListState::new();
    let spell_state = ListState::new();

    // ---- free-form features
    let f_name = RwSignal::new(String::new());
    let f_text = RwSignal::new(String::new());

    // ---- the list of everything added
    let c_list = comp.clone();
    let added = move || {
        let list: Vec<(usize, Grant)> = ctx.ch.with(|c| c.custom.iter().cloned().enumerate().collect());
        if list.is_empty() {
            return view! { <p class="muted">"Nothing added yet."</p> }.into_any();
        }
        view! {
            <ul class="custom-list">
                {list.into_iter().map(|(i, g)| {
                    let text = describe_custom(&c_list, &g);
                    let body = match &g {
                        Grant::Feature(f) => f.body.clone(),
                        Grant::Feat(k) => c_list.feats.get(k).map(|f| f.body.clone()).unwrap_or_default(),
                        _ => vec![],
                    };
                    let label = text.clone();
                    view! {
                        <li>
                            {if body.is_empty() {
                                view! { <span>{text.clone()}</span> }.into_any()
                            } else {
                                view! { <details><summary>{text.clone()}</summary><Entries entries=body /></details> }.into_any()
                            }}
                            <button class="link-button" aria-label=format!("Remove {label}") on:click=move |_| ctx.edit(|c| { if i < c.custom.len() { c.custom.remove(i); } })>{icon("x")}"Remove"</button>
                        </li>
                    }
                }).collect_view()}
            </ul>
        }
        .into_any()
    };

    // ---- which spells go in the sheet's Actions box
    let action_spells = move || {
        let comp = ctx.comp.get_value();
        let prefs = ctx.ch.with(|c| c.action_spells.clone());
        // (spell key, name, "Level 2")
        type Row = (String, String, String);
        let mut groups: [(&str, Vec<Row>); 3] = [("Actions", vec![]), ("Bonus actions", vec![]), ("Reactions", vec![])];
        ctx.build.with(|b| {
            let mut spells: Vec<_> = b
                .spells
                .iter()
                .filter(|k| k.mode != rules::build::SpellMode::Spellbook)
                .filter_map(|k| comp.spells.get(&k.key).map(|s| (s, k.mode.clone())))
                .collect();
            spells.sort_by_key(|(s, mode)| rules::play::spell_rank(s, mode));
            spells.dedup_by(|a, b| a.0.key == b.0.key);
            for (s, _) in spells {
                let i = match rules::play::spell_column(&s.time) {
                    Some(rules::play::ActionKind::Action) => 0,
                    Some(rules::play::ActionKind::Bonus) => 1,
                    Some(rules::play::ActionKind::Reaction) => 2,
                    None => continue,
                };
                let level = if s.level == 0 { "Cantrip".to_string() } else { format!("Level {}", s.level) };
                groups[i].1.push((s.key.0.clone(), s.name.clone(), level));
            }
        });
        if groups.iter().all(|(_, v)| v.is_empty()) {
            return view! { <p class="muted">"No spells cast with an action, bonus action or reaction yet."</p> }.into_any();
        }
        groups
            .into_iter()
            .filter(|(_, v)| !v.is_empty())
            .map(|(title, rows)| {
                let rows = rows
                    .into_iter()
                    .map(|(key, name, level)| {
                        let state = match prefs.get(&key) {
                            Some(true) => "always",
                            Some(false) => "never",
                            None => "auto",
                        };
                        let k = key.clone();
                        view! {
                            <li class="action-spell">
                                <span class="action-spell-name">{name.clone()}<span class="muted small">{format!(" · {level}")}</span></span>
                                <select aria-label=format!("{name} in the Actions box") prop:value=state on:change=move |ev| {
                                    let v = event_target_value(&ev);
                                    let k = k.clone();
                                    ctx.edit(move |c| match v.as_str() {
                                        "always" => { c.action_spells.insert(k, true); }
                                        "never" => { c.action_spells.insert(k, false); }
                                        _ => { c.action_spells.remove(&k); }
                                    });
                                }>
                                    <option value="auto" selected=state == "auto">"Automatic"</option>
                                    <option value="always" selected=state == "always">"Always show"</option>
                                    <option value="never" selected=state == "never">"Never show"</option>
                                </select>
                            </li>
                        }
                    })
                    .collect_view();
                view! { <h3 class="action-spell-group">{title}</h3><ul class="action-spells">{rows}</ul> }
            })
            .collect_view()
            .into_any()
    };

    view! {
        <div class="note custom-intro">
            <p><strong>"This page is for anything the rules as written don't cover."</strong></p>
            <p>"Use it for house rules, rewards from your DM, homebrew, or any change you want to make by hand: adjust ability scores, add proficiencies, languages, feats, spells or your own features, or give bonuses to hit points, Armor Class or speed. Nothing here is checked against the rules — what you add is applied to your character as is, and printed on your sheet."</p>
        </div>

        <h2>"Added so far"</h2>
        {added}

        <h2>"Ability scores"</h2>
        <p class="muted">"Raise or lower any score. Adjustments here can go past 20."</p>
        <div class="custom-grid">{ability_rows}</div>

        <h2>"Hit points, Armor Class and movement"</h2>
        <div class="custom-grid">
            {bonus("Hit point maximum", "added to your maximum",
                |g| if let Grant::HpBonus(n) = g { Some(*n) } else { None },
                |g| matches!(g, Grant::HpBonus(_)),
                Grant::HpBonus)}
            {bonus("Armor Class", "added to your AC",
                |g| if let Grant::AcBonus { amount, when: AcWhen::Always } = g { Some(*amount as i32) } else { None },
                |g| matches!(g, Grant::AcBonus { when: AcWhen::Always, .. }),
                |v| Grant::AcBonus { amount: v.clamp(-20, 20) as i8, when: AcWhen::Always })}
            {bonus("Walking speed (ft.)", "added to your speed",
                |g| if let Grant::SpeedBonus { mode, amount } = g { (mode == "walk").then_some(*amount as i32) } else { None },
                |g| matches!(g, Grant::SpeedBonus { mode, .. } if mode == "walk"),
                |v| Grant::SpeedBonus { mode: "walk".into(), amount: v.max(0) as u16 })}
            {bonus("Darkvision (ft.)", "range, if better than your own",
                |g| if let Grant::Darkvision(n) = g { Some(*n as i32) } else { None },
                |g| matches!(g, Grant::Darkvision(_)),
                |v| Grant::Darkvision(v.max(0) as u16))}
        </div>

        <h2>"Proficiencies, languages and resistances"</h2>
        <div class="row custom-add">
            <select aria-label="Kind of addition" on:change=move |ev| { kind.set(event_target_value(&ev)); value.set(String::new()); }>
                <option value="skill">"Skill proficiency"</option>
                <option value="expertise">"Skill expertise"</option>
                <option value="save">"Saving throw proficiency"</option>
                <option value="armor">"Armor training"</option>
                <option value="weapon">"Weapon proficiency"</option>
                <option value="tool">"Tool proficiency"</option>
                <option value="language">"Language"</option>
                <option value="resist">"Damage resistance"</option>
            </select>
            {value_input}
            <button class="button" on:click=add_prof>{icon("plus")}"Add"</button>
        </div>

        <h2>"Feats"</h2>
        <p class="muted">"Any feat, whatever its prerequisites. Choices the feat asks for appear at the bottom of this page."</p>
        {move || view! { <div class="record-list">{option_list(feat_items, vec![], 1, add_feat, feat_state, "custom-feats")}</div> }}

        <h2>"Spells"</h2>
        <label class="field inline">
            <span>"Add spells as"</span>
            <select on:change=move |ev| spell_kind.set(event_target_value(&ev))>
                <option value="prepared">"Always prepared"</option>
                <option value="known">"Known"</option>
                <option value="1/long rest">"Castable once per Long Rest"</option>
                <option value="at will">"Castable at will"</option>
            </select>
        </label>
        {move || view! { <div class="record-list">{option_list(spell_items, vec![], 1, add_spell, spell_state, "custom-spells")}</div> }}

        <h2>"Spells in the Actions box"</h2>
        <p class="muted">"Page one lists what you can do with an action, a bonus action and a reaction, spells included. Space is limited, so each column fills in order: spells you set to always show, then your features, then other spells (free casts first, then spells that attack, force a save, deal damage or heal, strongest first, then the rest). Spells that don't fit are counted, and all of them are on the spell page. They're listed here in that order."</p>
        {action_spells}

        <h2>"Your own features"</h2>
        <p class="muted">"Anything else — a boon, a curse, a magic tattoo, a house rule. It's printed with your feats and traits."</p>
        <div class="custom-feature">
            <label class="field"><span>"Name"</span>
                <input type="text" prop:value=move || f_name.get() on:input=move |ev| f_name.set(event_target_value(&ev)) />
            </label>
            <label class="field"><span>"What it does"</span>
                <textarea rows="3" prop:value=move || f_text.get() on:input=move |ev| f_text.set(event_target_value(&ev))></textarea>
            </label>
            <button class="button" disabled=move || f_name.get().trim().is_empty() on:click=move |_| {
                let name = f_name.get().trim().to_string();
                let text = f_text.get().trim().to_string();
                let body = text.split("\n\n").filter(|p| !p.trim().is_empty()).map(|p| Entry::P(p.trim().to_string())).collect();
                ctx.edit(|c| c.custom.push(Grant::Feature(Feature { name: name.clone(), body })));
                f_name.set(String::new());
                f_text.set(String::new());
            }>{icon("plus")}"Add feature"</button>
        </div>

        <StepChoices step=Step::Custom />
    }
}
