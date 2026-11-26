//! The creation wizard / character editor.

use crate::app::{go, Route};
use crate::entries::Entries;
use crate::picker::ChoicePicker;
use crate::store::{self, Store};
use compendium::*;
use leptos::prelude::*;
use rules::{AbilityMethod, Build, Character, Step};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Everything a wizard component needs; cheap to copy.
#[derive(Clone, Copy)]
pub struct EditorCtx {
    pub comp: StoredValue<Arc<Compendium>>,
    pub store: Store,
    pub id: StoredValue<String>,
    pub ch: Memo<Character>,
    pub build: Memo<Build>,
}

impl EditorCtx {
    pub fn edit(&self, f: impl FnOnce(&mut Character)) {
        store::update(self.store, &self.id.get_value(), f);
    }
}

pub const STEPS: [(Step, &str); 9] = [
    (Step::Basics, "Basics"),
    (Step::Class, "Class"),
    (Step::Background, "Background"),
    (Step::Species, "Species"),
    (Step::Abilities, "Abilities"),
    (Step::Spells, "Spells"),
    (Step::Equipment, "Equipment"),
    (Step::Details, "Details"),
    (Step::Basics, "Review"),
];

pub fn step_slug(i: usize) -> &'static str {
    ["basics", "class", "background", "species", "abilities", "spells", "equipment", "details", "review"][i]
}

pub fn step_index(slug: &str) -> usize {
    (0..STEPS.len()).find(|&i| step_slug(i) == slug).unwrap_or(0)
}

#[component]
pub fn Editor(id: String, step: String) -> impl IntoView {
    let comp = expect_context::<Arc<Compendium>>();
    let store = expect_context::<Store>();
    let id_v = StoredValue::new(id.clone());
    let exists = store.with_untracked(|s| s.iter().any(|c| c.id == id));
    if !exists {
        return view! { <div class="page"><p>"Character not found. "</p><a href="#/">"Back to characters"</a></div> }.into_any();
    }
    let ch = Memo::new(move |_| store.with(|s| s.iter().find(|c| c.id == id_v.get_value()).cloned().unwrap_or_default()));
    let c2 = comp.clone();
    let build = Memo::new(move |_| ch.with(|c| rules::build(c, &c2)));
    let ctx = EditorCtx { comp: StoredValue::new(comp.clone()), store, id: id_v, ch, build };
    provide_context(ctx);

    let c3 = comp.clone();
    let issues = Memo::new(move |_| ch.with(|c| build.with(|b| rules::issues(c, b, &c3))));
    let current = step_index(&step);

    // Re-sync starting equipment only when the equipment *choices* change,
    // so manual inventory edits are never undone.
    let c4 = comp.clone();
    Effect::new(move |_| {
        let pending_eq = build.with(|b| b.pending().any(|c| c.step == Step::Equipment));
        let sig = ch.with(|c| build.with(|b| rules::starting_signature(c, b)));
        let stale = ch.with(|c| c.starting_signature != sig && !c.levels.is_empty());
        if !pending_eq && stale {
            let comp = c4.clone();
            ctx.edit(|c| rules::apply_starting_equipment(c, &comp));
        }
    });

    let nav = move || {
        let iss = issues.get();
        STEPS
            .iter()
            .enumerate()
            .map(|(i, (step, label))| {
                let n = if i == STEPS.len() - 1 { iss.len() } else { iss.iter().filter(|x| x.step == *step).count() };
                let href = format!("#/edit/{}/{}", id_v.get_value(), step_slug(i));
                view! {
                    <a class="step" class:active=i == current class:ok=n == 0 href=href>
                        <span class="step-num">{i + 1}</span>
                        <span class="step-label">{*label}</span>
                        {(n > 0).then(|| view! { <span class="step-badge">{n}</span> })}
                    </a>
                }
            })
            .collect_view()
    };

    let body = match current {
        0 => view! { <BasicsStep /> }.into_any(),
        1 => view! { <ClassStep /> }.into_any(),
        2 => view! { <BackgroundStep /> }.into_any(),
        3 => view! { <SpeciesStep /> }.into_any(),
        4 => view! { <AbilitiesStep /> }.into_any(),
        5 => view! { <SpellsStep /> }.into_any(),
        6 => view! { <EquipmentStep /> }.into_any(),
        7 => view! { <DetailsStep /> }.into_any(),
        _ => view! { <ReviewStep issues=issues /> }.into_any(),
    };
    let prev = (current > 0).then(|| format!("#/edit/{}/{}", id, step_slug(current - 1)));
    let next = (current + 1 < STEPS.len()).then(|| format!("#/edit/{}/{}", id, step_slug(current + 1)));
    let sheet_href = format!("#/sheet/{id}");
    let c5 = comp.clone();

    view! {
        <div class="editor">
            <header class="editor-head">
                <a class="brand" href="#/">"⚔ 5e Builder"</a>
                <div class="who">
                    <strong>{move || { let n = ch.with(|c| c.name.clone()); if n.is_empty() { "New character".to_string() } else { n } }}</strong>
                    <span class="muted">{move || ch.with(|c| build.with(|b| rules::sheet(c, b, &c5).summary))}</span>
                </div>
                <a class="button" href=sheet_href>"Sheet & print"</a>
            </header>
            <nav class="steps">{nav}</nav>
            <main class="step-body">{body}</main>
            <footer class="step-foot">
                {prev.map(|h| view! { <a class="button ghost" href=h>"← Back"</a> })}
                <span class="spacer"></span>
                {next.map(|h| view! { <a class="button primary" href=h>"Next →"</a> })}
            </footer>
        </div>
    }
    .into_any()
}

/// Choice pickers for every choice of a step, in build order.
#[component]
fn StepChoices(step: Step, #[prop(optional)] filter: Option<fn(&rules::ChoiceState) -> bool>) -> impl IntoView {
    let ctx = expect_context::<EditorCtx>();
    let paths = Memo::new(move |_| {
        ctx.build.with(|b| b.choices.iter().filter(|c| c.step == step && filter.is_none_or(|f| f(c))).map(|c| c.path.clone()).collect::<Vec<_>>())
    });
    view! {
        <For each=move || paths.get() key=|p| p.clone() children=move |p| view! { <ChoicePicker path=p /> } />
    }
}

// ------------------------------------------------------------------ Basics

#[component]
fn BasicsStep() -> impl IntoView {
    let ctx = expect_context::<EditorCtx>();
    let comp = ctx.comp.get_value();
    let mut groups: BTreeMap<SourceGroup, Vec<SourceInfo>> = BTreeMap::new();
    for s in &comp.sources {
        groups.entry(s.group).or_default().push(s.clone());
    }
    let groups_view = groups
        .into_iter()
        .map(|(g, list)| {
            let ids: Vec<String> = list.iter().map(|s| s.id.clone()).collect();
            let ids2 = ids.clone();
            view! {
                <fieldset class="source-group">
                    <legend>{g.label()}
                        <button class="link" on:click=move |_| { let ids = ids.clone(); ctx.edit(|c| ids.iter().for_each(|i| { c.settings.disabled_sources.remove(i); })) }>"all"</button>
                        <button class="link" on:click=move |_| { let ids = ids2.clone(); ctx.edit(|c| c.settings.disabled_sources.extend(ids)) }>"none"</button>
                    </legend>
                    <div class="source-list">
                        {list.into_iter().map(|s| {
                            let id = s.id.clone();
                            let id2 = s.id.clone();
                            view! {
                                <label class="source" title=s.id.clone()>
                                    <input type="checkbox"
                                        prop:checked=move || ctx.ch.with(|c| c.settings.source_enabled(&id))
                                        on:change=move |ev| { let on = event_target_checked(&ev); let id = id2.clone(); ctx.edit(|c| { if on { c.settings.disabled_sources.remove(&id); } else { c.settings.disabled_sources.insert(id); } }) } />
                                    {s.name.clone()}
                                </label>
                            }
                        }).collect_view()}
                    </div>
                </fieldset>
            }
        })
        .collect_view();

    view! {
        <h2>"Basics"</h2>
        <label class="field big">
            <span>"Character name"</span>
            <input type="text" placeholder="e.g. Elowen Brightwater"
                prop:value=move || ctx.ch.with(|c| c.name.clone())
                on:input=move |ev| { let v = event_target_value(&ev); ctx.edit(|c| c.name = v) } />
        </label>
        <label class="field">
            <span>"Player"</span>
            <input type="text" prop:value=move || ctx.ch.with(|c| c.details.player.clone())
                on:input=move |ev| { let v = event_target_value(&ev); ctx.edit(|c| c.details.player = v) } />
        </label>
        <h3>"Rules"</h3>
        <p class="muted">"The wizard follows the 2024 Player's Handbook. Older (legacy) species, backgrounds, classes and subclasses are available too and are adapted to the 2024 rules."</p>
        <label class="toggle">
            <input type="checkbox" prop:checked=move || ctx.ch.with(|c| c.settings.legacy_species_asi)
                on:change=move |ev| { let on = event_target_checked(&ev); ctx.edit(|c| c.settings.legacy_species_asi = on) } />
            "Use legacy species ability score increases (instead of the background's)"
        </label>
        <label class="toggle">
            <input type="checkbox" prop:checked=move || ctx.ch.with(|c| c.settings.average_hp)
                on:change=move |ev| { let on = event_target_checked(&ev); ctx.edit(|c| c.settings.average_hp = on) } />
            "Take average hit points when gaining levels"
        </label>
        <h3>"Sources"</h3>
        <p class="muted">"Content from unchecked sources is hidden from every list."</p>
        {groups_view}
    }
}

// ------------------------------------------------------------------ Pickers for top-level content

#[derive(Clone, PartialEq)]
struct Card {
    key: Key,
    name: String,
    source: String,
    legacy: bool,
    detail: String,
}

/// Searchable grid of cards; `on_pick` receives the chosen key.
#[component]
fn CardGrid(cards: Signal<Vec<Card>>, selected: Signal<Option<Key>>, on_pick: Callback<Key>) -> impl IntoView {
    let search = RwSignal::new(String::new());
    let hide_legacy = RwSignal::new(false);
    let shown = move || {
        let q = search.get().to_lowercase();
        let sel = selected.get();
        cards
            .get()
            .into_iter()
            .filter(|c| q.is_empty() || c.name.to_lowercase().contains(&q) || c.source.to_lowercase().contains(&q))
            .filter(|c| !hide_legacy.get() || !c.legacy || Some(&c.key) == sel.as_ref())
            .map(|c| {
                let k = c.key.clone();
                let is_sel = sel.as_ref() == Some(&c.key);
                view! {
                    <button class="card" class:selected=is_sel on:click=move |_| on_pick.run(k.clone())>
                        <span class="card-name">{c.name.clone()}</span>
                        <span class="card-source">{c.source.clone()}{c.legacy.then_some(" · legacy")}</span>
                        <span class="card-detail">{c.detail.clone()}</span>
                    </button>
                }
            })
            .collect_view()
    };
    view! {
        <div class="grid-tools">
            <input class="search" type="search" placeholder="Search…" prop:value=move || search.get() on:input=move |ev| search.set(event_target_value(&ev)) />
            <label class="toggle small"><input type="checkbox" prop:checked=move || hide_legacy.get() on:change=move |ev| hide_legacy.set(event_target_checked(&ev)) />"Hide legacy"</label>
        </div>
        <div class="cards">{shown}</div>
    }
}

fn sort_cards(mut v: Vec<Card>) -> Vec<Card> {
    v.sort_by(|a, b| (a.legacy, &a.name, &a.source).cmp(&(b.legacy, &b.name, &b.source)));
    v
}

// ------------------------------------------------------------------ Class

#[component]
fn ClassStep() -> impl IntoView {
    let ctx = expect_context::<EditorCtx>();
    let comp = ctx.comp.get_value();
    let browsing = RwSignal::new(ctx.ch.with_untracked(|c| c.levels.is_empty()));
    let c1 = comp.clone();
    let cards = Signal::derive(move || {
        let enabled = ctx.ch.with(|c| c.settings.clone());
        sort_cards(
            c1.classes
                .iter()
                .filter(|c| enabled.source_enabled(&c.source))
                .map(|c| Card {
                    key: c.key.clone(),
                    name: c.name.clone(),
                    source: c1.source_name(&c.source),
                    legacy: c.legacy,
                    detail: format!(
                        "d{} hit die · {}{}",
                        c.hit_die,
                        c.primary.iter().map(|a| a.abbr()).collect::<Vec<_>>().join("/"),
                        if c.spellcasting.is_caster() { " · spellcaster" } else { "" }
                    ),
                })
                .collect(),
        )
    });
    let selected = Signal::derive(move || ctx.ch.with(|c| c.levels.first().map(|l| l.class.clone())));
    let on_pick = Callback::new(move |k: Key| {
        ctx.edit(|c| c.set_starting_class(k));
        browsing.set(false);
    });

    let c2 = comp.clone();
    let summary = move || {
        let class = selected.get().and_then(|k| c2.classes.get(&k).cloned())?;
        let level = ctx.ch.with(|c| c.class_level(&class.key));
        let features: Vec<(u8, String)> = class.features.iter().map(|f| (f.level, f.name.clone())).collect();
        Some(view! {
            <div class="summary-card">
                <div class="summary-head">
                    <div>
                        <h3>{class.name.clone()}</h3>
                        <span class="muted">{c2.source_name(&class.source)}{class.legacy.then_some(" · legacy")}</span>
                    </div>
                    <button class="button ghost" on:click=move |_| browsing.set(true)>"Change class"</button>
                </div>
                <p>
                    {format!("Hit die d{} · Saves: {}", class.hit_die, class.saves.iter().map(|a| a.name()).collect::<Vec<_>>().join(", "))}
                    {class.multiclass_req.clone().filter(|r| !r.is_empty()).map(|r| format!(" · Multiclass requirement: {r}"))}
                </p>
                <details>
                    <summary>"Class features by level"</summary>
                    <ol class="feature-levels">
                        {features.into_iter().map(|(l, n)| view! { <li class:locked={l > level}><b>{l}</b>" "{n}</li> }).collect_view()}
                    </ol>
                </details>
            </div>
        })
    };

    view! {
        <h2>"Class"</h2>
        <Show when=move || browsing.get() || selected.get().is_none()>
            <p class="muted">"Pick the class your character starts in. You can add levels in other classes below."</p>
            <CardGrid cards=cards selected=selected on_pick=on_pick />
        </Show>
        <Show when=move || !browsing.get() && selected.get().is_some()>
            {summary.clone()}
            <LevelsPanel />
            <h3>"Class choices"</h3>
            <StepChoices step=Step::Class />
        </Show>
    }
}

#[component]
fn LevelsPanel() -> impl IntoView {
    let ctx = expect_context::<EditorCtx>();
    let comp = ctx.comp.get_value();
    let add_class = RwSignal::new(String::new());
    let c1 = comp.clone();
    let levels_view = move || {
        let ch = ctx.ch.get();
        let rows = ch
            .levels
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let class = c1.classes.get(&l.class);
                let name = class.map(|c| c.name.clone()).unwrap_or_else(|| l.class.0.clone());
                let hd = class.map(|c| c.hit_die).unwrap_or(8);
                let n = ch.levels[..=i].iter().filter(|x| x.class == l.class).count();
                let roll = (!ch.settings.average_hp && i > 0).then(|| {
                    let v = l.hp_roll.map(|r| r.to_string()).unwrap_or_default();
                    view! {
                        <label class="hp-roll">"HP roll (d"{hd}") "
                            <input type="number" min="1" max=hd value=v
                                on:change=move |ev| { let r = event_target_value(&ev).parse::<u8>().ok().map(|r| r.clamp(1, hd)); ctx.edit(|c| c.levels[i].hp_roll = r) } />
                        </label>
                    }
                });
                view! { <li><b>{format!("Level {}", i + 1)}</b>" "{format!("{name} {n}")}{roll}</li> }
            })
            .collect_view();
        view! { <ol class="levels">{rows}</ol> }
    };
    let options = comp
        .classes
        .iter()
        .map(|c| (c.key.0.clone(), format!("{} ({})", c.name, c.source)))
        .collect::<Vec<_>>();
    view! {
        <div class="levels-panel">
            <div class="row">
                <label class="field inline">
                    <span>"Starting level"</span>
                    <select on:change=move |ev| { let n: u8 = event_target_value(&ev).parse().unwrap_or(1); ctx.edit(|c| c.set_starting_level(n)) }>
                        {(1..=20u8).map(|n| view! { <option value=n.to_string() selected=move || ctx.ch.with(|c| c.levels.first().map(|f| c.class_level(&f.class)) == Some(n))>{n}</option> }).collect_view()}
                    </select>
                </label>
                <span class="muted">{move || format!("Character level {}", ctx.ch.with(|c| c.total_level()))}</span>
            </div>
            <details>
                <summary>"Levels & multiclassing"</summary>
                {levels_view}
                <div class="row">
                    <select on:change=move |ev| add_class.set(event_target_value(&ev))>
                        <option value="">"Add a level in…"</option>
                        {options.into_iter().map(|(k, n)| view! { <option value=k>{n}</option> }).collect_view()}
                    </select>
                    <button class="button" disabled=move || add_class.get().is_empty() || ctx.ch.with(|c| c.total_level() >= 20)
                        on:click=move |_| { let k = add_class.get(); if !k.is_empty() { ctx.edit(|c| c.level_up(Key(k))) } }>"Level up"</button>
                    <button class="button ghost" disabled=move || ctx.ch.with(|c| c.levels.len() <= 1)
                        on:click=move |_| { let comp = ctx.comp.get_value(); ctx.edit(|c| { c.level_down(); c.prune_choices(&comp) }) }>"Remove last level"</button>
                </div>
            </details>
        </div>
    }
}

// ------------------------------------------------------------------ Background

fn grants_summary(comp: &Compendium, grants: &[Grant]) -> String {
    let named: Vec<String> = grants
        .iter()
        .filter_map(|g| match g {
            Grant::Feat(k) => Some(comp.feats.get(k).map(|f| format!("{} feat", f.name)).unwrap_or_else(|| title_case(k.name_part()))),
            Grant::Choose(c) if c.id.contains("equipment") => None,
            Grant::Item { .. } | Grant::Coins(_) => None,
            other => Some(describe(std::slice::from_ref(other))),
        })
        .filter(|s| !s.is_empty())
        .collect();
    named.join(" · ")
}

#[component]
fn BackgroundStep() -> impl IntoView {
    let ctx = expect_context::<EditorCtx>();
    let comp = ctx.comp.get_value();
    let browsing = RwSignal::new(ctx.ch.with_untracked(|c| c.background.is_none()));
    let c1 = comp.clone();
    let cards = Signal::derive(move || {
        let s = ctx.ch.with(|c| c.settings.clone());
        sort_cards(
            c1.backgrounds
                .iter()
                .filter(|b| s.source_enabled(&b.source))
                .map(|b| Card { key: b.key.clone(), name: b.name.clone(), source: c1.source_name(&b.source), legacy: b.legacy, detail: grants_summary(&c1, &b.grants) })
                .collect(),
        )
    });
    let selected = Signal::derive(move || ctx.ch.with(|c| c.background.clone()));
    let on_pick = Callback::new(move |k: Key| {
        ctx.edit(|c| c.set_background(Some(k)));
        browsing.set(false);
    });
    let c2 = comp.clone();
    let summary = move || {
        let b = selected.get().and_then(|k| c2.backgrounds.get(&k).cloned())?;
        let note = (!b.has_asi || !b.has_feat).then(|| {
            view! { <p class="note">"Legacy background: under the 2024 rules it also grants ability score increases and an origin feat of your choice."</p> }
        });
        Some(view! {
            <div class="summary-card">
                <div class="summary-head">
                    <div><h3>{b.name.clone()}</h3><span class="muted">{c2.source_name(&b.source)}</span></div>
                    <button class="button ghost" on:click=move |_| browsing.set(true)>"Change background"</button>
                </div>
                <p>{grants_summary(&c2, &b.grants)}</p>
                {note}
                {b.features.iter().map(|f| view! { <details><summary>{f.name.clone()}</summary><Entries entries=f.body.clone() /></details> }).collect_view()}
            </div>
        })
    };
    view! {
        <h2>"Background"</h2>
        <Show when=move || browsing.get() || selected.get().is_none()>
            <CardGrid cards=cards selected=selected on_pick=on_pick />
        </Show>
        <Show when=move || !browsing.get() && selected.get().is_some()>
            {summary.clone()}
            <StepChoices step=Step::Background />
        </Show>
    }
}

// ------------------------------------------------------------------ Species

#[component]
fn SpeciesStep() -> impl IntoView {
    let ctx = expect_context::<EditorCtx>();
    let comp = ctx.comp.get_value();
    let browsing = RwSignal::new(ctx.ch.with_untracked(|c| c.species.is_none()));
    let c1 = comp.clone();
    let cards = Signal::derive(move || {
        let s = ctx.ch.with(|c| c.settings.clone());
        sort_cards(
            c1.species
                .iter()
                .filter(|x| s.source_enabled(&x.source))
                .map(|x| Card {
                    key: x.key.clone(),
                    name: x.name.clone(),
                    source: c1.source_name(&x.source),
                    legacy: x.legacy,
                    detail: format!(
                        "{} · speed {} ft.{}{}",
                        x.sizes.iter().map(|s| s.name()).collect::<Vec<_>>().join("/"),
                        x.speed.walk,
                        x.darkvision.map(|d| format!(" · darkvision {d} ft.")).unwrap_or_default(),
                        if x.lineages.is_empty() { String::new() } else { format!(" · {} lineages", x.lineages.len()) }
                    ),
                })
                .collect(),
        )
    });
    let selected = Signal::derive(move || ctx.ch.with(|c| c.species.clone()));
    let on_pick = Callback::new(move |k: Key| {
        ctx.edit(|c| c.set_species(Some(k)));
        browsing.set(false);
    });
    let c2 = comp.clone();
    let summary = move || {
        let s = selected.get().and_then(|k| c2.species.get(&k).cloned())?;
        Some(view! {
            <div class="summary-card">
                <div class="summary-head">
                    <div><h3>{s.name.clone()}</h3><span class="muted">{c2.source_name(&s.source)}{s.legacy.then_some(" · legacy")}</span></div>
                    <button class="button ghost" on:click=move |_| browsing.set(true)>"Change species"</button>
                </div>
                <p>{format!("{} {} · Speed {} ft.", s.sizes.iter().map(|x| x.name()).collect::<Vec<_>>().join(" or "), s.creature_type, s.speed.walk)}</p>
                {s.features.iter().map(|f| view! { <details><summary>{f.name.clone()}</summary><Entries entries=f.body.clone() /></details> }).collect_view()}
            </div>
        })
    };
    view! {
        <h2>"Species"</h2>
        <Show when=move || browsing.get() || selected.get().is_none()>
            <CardGrid cards=cards selected=selected on_pick=on_pick />
        </Show>
        <Show when=move || !browsing.get() && selected.get().is_some()>
            {summary.clone()}
            <StepChoices step=Step::Species />
        </Show>
    }
}

// ------------------------------------------------------------------ Abilities

#[component]
fn AbilitiesStep() -> impl IntoView {
    let ctx = expect_context::<EditorCtx>();
    let method = move || ctx.ch.with(|c| c.abilities.method);
    let set_method = move |m: AbilityMethod| {
        ctx.edit(|c| {
            c.abilities.method = m;
            c.abilities.scores = match m {
                AbilityMethod::PointBuy => [8; 6],
                AbilityMethod::StandardArray => [0; 6],
                AbilityMethod::Manual => c.abilities.scores.map(|s| if s == 0 { 10 } else { s }),
            };
        })
    };
    let spent = move || ctx.ch.with(|c| c.abilities.scores.iter().filter_map(|s| rules::point_cost(*s)).map(|x| x as u32).sum::<u32>());
    let problem = move || ctx.ch.with(|c| rules::ability_problem(&c.abilities));

    let rows = move || {
        let (base, m) = ctx.ch.with(|c| (c.abilities.scores, c.abilities.method));
        let bonuses: Vec<(Ability, i8, String)> = ctx.build.with(|b| {
            b.grants
                .iter()
                .filter_map(|g| match g.grant {
                    Grant::Ability { ability, amount } => Some((ability, amount, g.origin.clone())),
                    _ => None,
                })
                .collect()
        });
        Ability::ALL
            .iter()
            .map(|&a| {
                let i = a.index();
                let mine: Vec<String> = bonuses.iter().filter(|b| b.0 == a).map(|b| format!("{} {}", signed(b.1 as i32), b.2)).collect();
                let total = ctx.build.with(|b| b.score(a));
                let input = match m {
                    AbilityMethod::PointBuy => view! {
                        <div class="stepper">
                            <button disabled=base[i] <= 8 on:click=move |_| ctx.edit(|c| c.abilities.scores[i] = c.abilities.scores[i].saturating_sub(1).max(8))>"−"</button>
                            <span class="score">{base[i]}</span>
                            <button disabled=base[i] >= 15 on:click=move |_| ctx.edit(|c| c.abilities.scores[i] = (c.abilities.scores[i] + 1).min(15))>"+"</button>
                            <span class="muted small">{format!("cost {}", rules::point_cost(base[i]).unwrap_or(0))}</span>
                        </div>
                    }
                    .into_any(),
                    AbilityMethod::StandardArray => {
                        let used: Vec<u8> = base.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, v)| *v).collect();
                        view! {
                            <select on:change=move |ev| { let v: u8 = event_target_value(&ev).parse().unwrap_or(0); ctx.edit(|c| c.abilities.scores[i] = v) }>
                                <option value="0" selected=base[i] == 0>"—"</option>
                                {rules::STANDARD_ARRAY.iter().map(|&v| {
                                    let taken = used.iter().filter(|u| **u == v).count() >= rules::STANDARD_ARRAY.iter().filter(|x| **x == v).count();
                                    view! { <option value=v.to_string() selected=base[i] == v disabled=taken && base[i] != v>{v}</option> }
                                }).collect_view()}
                            </select>
                        }
                        .into_any()
                    }
                    AbilityMethod::Manual => view! {
                        <input class="num" type="number" min="1" max="30" prop:value=base[i].to_string()
                            on:change=move |ev| { let v: u8 = event_target_value(&ev).parse().unwrap_or(10); ctx.edit(|c| c.abilities.scores[i] = v.clamp(1, 30)) } />
                    }
                    .into_any(),
                };
                view! {
                    <tr>
                        <th>{a.name()}</th>
                        <td>{input}</td>
                        <td class="small">{if mine.is_empty() { "—".to_string() } else { mine.join(", ") }}</td>
                        <td class="total">{total}</td>
                        <td class="mod">{signed(modifier(total))}</td>
                    </tr>
                }
            })
            .collect_view()
    };
    let tab = move |m: AbilityMethod, label: &'static str| {
        view! { <button class="tab" class:active=move || method() == m on:click=move |_| set_method(m)>{label}</button> }
    };
    view! {
        <h2>"Ability scores"</h2>
        <div class="tabs">
            {tab(AbilityMethod::PointBuy, "Point buy")}
            {tab(AbilityMethod::StandardArray, "Standard array")}
            {tab(AbilityMethod::Manual, "Manual / rolled")}
        </div>
        <Show when=move || method() == AbilityMethod::PointBuy>
            <p class="points">{move || format!("{} / {} points spent", spent(), rules::POINT_BUY_BUDGET)}</p>
        </Show>
        {move || problem().map(|p| view! { <p class="note">{p}</p> })}
        <div class="table-wrap">
            <table class="abilities">
                <thead><tr><th></th><th>"Base"</th><th>"Bonuses"</th><th>"Score"</th><th>"Mod"</th></tr></thead>
                <tbody>{rows}</tbody>
            </table>
        </div>
        <h3>"Ability score increases"</h3>
        <StepChoices step=Step::Background filter=is_ability_choice />
        <StepChoices step=Step::Species filter=is_ability_choice />
    }
}

fn is_ability_choice(c: &rules::ChoiceState) -> bool {
    matches!(&c.options, Options::Abilities { amounts, .. } if amounts.iter().any(|a| *a != 0)) || c.path.ends_with("/asi")
}

// ------------------------------------------------------------------ Spells

#[component]
fn SpellsStep() -> impl IntoView {
    let ctx = expect_context::<EditorCtx>();
    let c1 = ctx.comp.get_value();
    let casting = move || {
        let s = ctx.ch.with(|c| ctx.build.with(|b| rules::sheet(c, b, &c1)));
        if s.casting.is_empty() && s.spells.is_empty() {
            return view! { <p class="muted">"No spellcasting yet. Spell choices from your class, species, background or feats appear here."</p> }.into_any();
        }
        let slots: Vec<String> = s.slots.iter().enumerate().filter(|(_, n)| **n > 0).map(|(i, n)| format!("{}: {n}", ordinal(i as u8 + 1))).collect();
        view! {
            <div class="casting">
                {s.casting.iter().map(|c| view! {
                    <div class="stat"><span class="stat-label">{format!("{} ({})", c.label, c.ability.abbr())}</span>
                    <span>{format!("Save DC {} · Attack {}", c.save_dc, signed(c.attack))}</span></div>
                }).collect_view()}
                {(!slots.is_empty()).then(|| view! { <div class="stat"><span class="stat-label">"Spell slots"</span><span>{slots.join(" · ")}</span></div> })}
                {s.pact_slots.map(|(n, l)| view! { <div class="stat"><span class="stat-label">"Pact slots"</span><span>{format!("{n} × level {l}")}</span></div> })}
            </div>
        }
        .into_any()
    };
    view! {
        <h2>"Spells"</h2>
        {casting}
        <StepChoices step=Step::Spells />
    }
}

pub fn ordinal(n: u8) -> String {
    match n {
        1 => "1st".into(),
        2 => "2nd".into(),
        3 => "3rd".into(),
        n => format!("{n}th"),
    }
}

// ------------------------------------------------------------------ Equipment

#[component]
fn EquipmentStep() -> impl IntoView {
    let ctx = expect_context::<EditorCtx>();
    let comp = ctx.comp.get_value();
    let query = RwSignal::new(String::new());
    let custom = RwSignal::new(String::new());
    let c1 = comp.clone();
    let results = move || {
        let q = query.get().to_lowercase();
        if q.len() < 2 {
            return ().into_any();
        }
        let s = ctx.ch.with(|c| c.settings.clone());
        let mut hits: Vec<&Item> = c1.items.iter().filter(|i| s.source_enabled(&i.source) && i.name.to_lowercase().contains(&q)).collect();
        hits.sort_by_key(|i| (!i.name.to_lowercase().starts_with(&q), rules::options::source_rank(&i.source), i.name.len()));
        hits.truncate(30);
        view! {
            <div class="options scroll">
                {hits.into_iter().map(|i| {
                    let key = i.key.clone();
                    let name = i.name.clone();
                    view! {
                        <div class="option">
                            <button class="option-main" on:click=move |_| {
                                let (key, name) = (key.clone(), name.clone());
                                ctx.edit(|c| c.inventory.push(rules::InventoryItem { item: Some(key), name, qty: 1, equipped: false, attuned: false, starting: false, notes: String::new() }));
                                query.set(String::new());
                            }>
                                <span class="check">"+"</span>
                                <span class="option-label">{i.name.clone()}</span>
                                <span class="option-detail">{format!("{} · {}{}", i.kind, i.source, i.rarity.as_ref().map(|r| format!(" · {r}")).unwrap_or_default())}</span>
                            </button>
                        </div>
                    }
                }).collect_view()}
            </div>
        }
        .into_any()
    };
    let inventory = move || {
        let inv = ctx.ch.with(|c| c.inventory.clone());
        if inv.is_empty() {
            return view! { <p class="muted">"Inventory is empty. Choose starting equipment above or add items."</p> }.into_any();
        }
        view! {
            <table class="inventory">
                <thead><tr><th>"Item"</th><th>"Qty"</th><th>"Equipped"</th><th></th></tr></thead>
                <tbody>
                    {inv.into_iter().enumerate().map(|(i, it)| view! {
                        <tr>
                            <td>{it.name.clone()}{it.starting.then(|| view! { <span class="badge">"starting"</span> })}</td>
                            <td><input class="num" type="number" min="1" prop:value=it.qty.to_string()
                                on:change=move |ev| { let q: u16 = event_target_value(&ev).parse().unwrap_or(1); ctx.edit(|c| { c.inventory[i].qty = q.max(1); c.inventory[i].starting = false; }) } /></td>
                            <td><input type="checkbox" prop:checked=it.equipped on:change=move |ev| { let on = event_target_checked(&ev); ctx.edit(|c| c.inventory[i].equipped = on) } /></td>
                            <td><button class="link" on:click=move |_| ctx.edit(|c| { c.inventory.remove(i); })>"remove"</button></td>
                        </tr>
                    }).collect_view()}
                </tbody>
            </table>
        }
        .into_any()
    };
    let coin = move |label: &'static str, get: fn(&rules::Coins) -> u32, set: fn(&mut rules::Coins, u32)| {
        view! {
            <label class="coin"><span>{label}</span>
                <input class="num" type="number" min="0" prop:value=move || ctx.ch.with(|c| get(&c.coins).to_string())
                    on:change=move |ev| { let v: u32 = event_target_value(&ev).parse().unwrap_or(0); ctx.edit(|c| set(&mut c.coins, v)) } />
            </label>
        }
    };
    view! {
        <h2>"Equipment"</h2>
        <StepChoices step=Step::Equipment />
        <h3>"Inventory"</h3>
        {inventory}
        <div class="add-item">
            <input class="search" type="search" placeholder="Add an item from the compendium…" prop:value=move || query.get() on:input=move |ev| query.set(event_target_value(&ev)) />
            {results}
            <div class="row">
                <input type="text" placeholder="…or a custom item" prop:value=move || custom.get() on:input=move |ev| custom.set(event_target_value(&ev)) />
                <button class="button" disabled=move || custom.get().trim().is_empty() on:click=move |_| {
                    let name = custom.get().trim().to_string();
                    ctx.edit(|c| c.inventory.push(rules::InventoryItem { item: None, name, qty: 1, equipped: false, attuned: false, starting: false, notes: String::new() }));
                    custom.set(String::new());
                }>"Add"</button>
            </div>
        </div>
        <h3>"Coins"</h3>
        <div class="coins">
            {coin("CP", |c| c.cp, |c, v| c.cp = v)}
            {coin("SP", |c| c.sp, |c, v| c.sp = v)}
            {coin("EP", |c| c.ep, |c, v| c.ep = v)}
            {coin("GP", |c| c.gp, |c, v| c.gp = v)}
            {coin("PP", |c| c.pp, |c, v| c.pp = v)}
        </div>
    }
}

// ------------------------------------------------------------------ Details

#[component]
fn DetailsStep() -> impl IntoView {
    let ctx = expect_context::<EditorCtx>();
    type Get = fn(&rules::Details) -> String;
    type Set = fn(&mut rules::Details, String);
    let field = move |label: &'static str, get: Get, set: Set| {
        view! {
            <label class="field"><span>{label}</span>
                <input type="text" prop:value=move || ctx.ch.with(|c| get(&c.details))
                    on:input=move |ev| { let v = event_target_value(&ev); ctx.edit(|c| set(&mut c.details, v)) } />
            </label>
        }
    };
    let area = move |label: &'static str, get: Get, set: Set| {
        view! {
            <label class="field"><span>{label}</span>
                <textarea rows="3" prop:value=move || ctx.ch.with(|c| get(&c.details))
                    on:input=move |ev| { let v = event_target_value(&ev); ctx.edit(|c| set(&mut c.details, v)) }></textarea>
            </label>
        }
    };
    view! {
        <h2>"Details"</h2>
        <label class="field big"><span>"Character name"</span>
            <input type="text" prop:value=move || ctx.ch.with(|c| c.name.clone()) on:input=move |ev| { let v = event_target_value(&ev); ctx.edit(|c| c.name = v) } />
        </label>
        <div class="field-grid">
            {field("Alignment", |d| d.alignment.clone(), |d, v| d.alignment = v)}
            {field("Pronouns", |d| d.pronouns.clone(), |d, v| d.pronouns = v)}
            {field("Age", |d| d.age.clone(), |d, v| d.age = v)}
            {field("Height", |d| d.height.clone(), |d, v| d.height = v)}
            {field("Weight", |d| d.weight.clone(), |d, v| d.weight = v)}
            {field("Eyes", |d| d.eyes.clone(), |d, v| d.eyes = v)}
            {field("Hair", |d| d.hair.clone(), |d, v| d.hair = v)}
            {field("Skin", |d| d.skin.clone(), |d, v| d.skin = v)}
            {field("Faith", |d| d.faith.clone(), |d, v| d.faith = v)}
        </div>
        {area("Personality traits", |d| d.personality.clone(), |d, v| d.personality = v)}
        {area("Ideals", |d| d.ideals.clone(), |d, v| d.ideals = v)}
        {area("Bonds", |d| d.bonds.clone(), |d, v| d.bonds = v)}
        {area("Flaws", |d| d.flaws.clone(), |d, v| d.flaws = v)}
        {area("Appearance", |d| d.appearance.clone(), |d, v| d.appearance = v)}
        {area("Backstory", |d| d.backstory.clone(), |d, v| d.backstory = v)}
        {area("Allies & organizations", |d| d.allies.clone(), |d, v| d.allies = v)}
        {area("Notes", |d| d.notes.clone(), |d, v| d.notes = v)}
    }
}

// ------------------------------------------------------------------ Review

#[component]
fn ReviewStep(issues: Memo<Vec<rules::Issue>>) -> impl IntoView {
    let ctx = expect_context::<EditorCtx>();
    let id = ctx.id.get_value();
    let sheet_href = format!("#/sheet/{id}");
    let list = move || {
        let iss = issues.get();
        if iss.is_empty() {
            return view! { <p class="ok-note">"✓ Your character is complete. Open the sheet to print it."</p> }.into_any();
        }
        view! {
            <ul class="issues">
                {iss.into_iter().map(|i| {
                    let idx = STEPS.iter().position(|(s, _)| *s == i.step).unwrap_or(0);
                    let href = format!("#/edit/{}/{}", ctx.id.get_value(), step_slug(idx));
                    view! { <li><a href=href>{STEPS[idx].1}</a>": "{i.text}</li> }
                }).collect_view()}
            </ul>
        }
        .into_any()
    };
    view! {
        <h2>"Review"</h2>
        {list}
        <div class="row">
            <a class="button primary" href=sheet_href>"Open character sheet"</a>
            <button class="button ghost" on:click=move |_| {
                let c = ctx.ch.get_untracked();
                store::download(&store::file_name(&c), &serde_json::to_string_pretty(&c).unwrap_or_default());
            }>"Export JSON"</button>
            <button class="button ghost" on:click=move |_| go(Route::Home)>"Back to characters"</button>
        </div>
    }
}
