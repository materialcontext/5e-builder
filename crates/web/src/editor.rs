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
    pub history: RwSignal<History>,
}

/// Undo/redo snapshots. Rapid edits (typing) coalesce into one step.
#[derive(Clone, Default)]
pub struct History {
    undo: Vec<Character>,
    redo: Vec<Character>,
    last: f64,
}

impl EditorCtx {
    pub fn edit(&self, f: impl FnOnce(&mut Character)) {
        let before = self.ch.get_untracked();
        let now = js_sys::Date::now();
        self.history.update(|h| {
            if now - h.last > 700.0 {
                h.undo.push(before);
                if h.undo.len() > 100 {
                    h.undo.remove(0);
                }
            }
            h.last = now;
            h.redo.clear();
        });
        store::update(self.store, &self.id.get_value(), f);
    }

    fn restore(&self, c: Character) {
        let id = self.id.get_value();
        self.store.update(|all| {
            if let Some(x) = all.iter_mut().find(|x| x.id == id) {
                *x = c;
                x.updated = store::now();
            }
        });
    }

    pub fn undo(&self) {
        let cur = self.ch.get_untracked();
        let mut prev = None;
        self.history.update(|h| {
            prev = h.undo.pop();
            if prev.is_some() {
                h.redo.push(cur);
                h.last = 0.0;
            }
        });
        if let Some(p) = prev {
            self.restore(p);
        }
    }

    pub fn redo(&self) {
        let cur = self.ch.get_untracked();
        let mut next = None;
        self.history.update(|h| {
            next = h.redo.pop();
            if next.is_some() {
                h.undo.push(cur);
                h.last = 0.0;
            }
        });
        if let Some(n) = next {
            self.restore(n);
        }
    }

    /// Fills pending choices (of one step, or all) with suggested picks.
    pub fn fill(&self, step: Option<Step>) {
        let comp = self.comp.get_value();
        self.edit(|c| {
            rules::suggest::fill(c, &comp, step);
        });
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

/// One-line guidance shown under each step title (same place on every step).
pub const STEP_INTROS: [&str; 9] = [
    "Name your character and pick the rules options and books you want to use.",
    "Your class decides how you fight, what you're good at and how you grow. Pick your starting class and level.",
    "Your background is your life before adventuring. Under the 2024 rules it also sets your ability score increases and an origin feat.",
    "Your species gives you traits like size, speed, senses and special abilities.",
    "Assign your six ability scores, then see how your background or species raises them.",
    "Choose the cantrips and spells your character knows or prepares. Only casters and characters with magical traits have choices here.",
    "Pick your starting equipment, then add or remove anything you like. Equip armor and weapons so the sheet can use them.",
    "Give your character a personality, appearance and history. Everything here is optional except the name.",
    "Check that nothing is missing, then open the printable sheet.",
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
    let ctx = EditorCtx { comp: StoredValue::new(comp.clone()), store, id: id_v, ch, build, history: RwSignal::new(History::default()) };
    provide_context(ctx);
    let keys = window_event_listener(leptos::ev::keydown, move |ev| {
        let mac_or_ctrl = ev.ctrl_key() || ev.meta_key();
        if mac_or_ctrl && ev.key().eq_ignore_ascii_case("z") {
            let in_text = ev
                .target()
                .and_then(|t| wasm_bindgen::JsCast::dyn_into::<web_sys::Element>(t).ok())
                .is_some_and(|e| matches!(e.tag_name().as_str(), "INPUT" | "TEXTAREA"));
            if !in_text {
                ev.prevent_default();
                if ev.shift_key() { ctx.redo() } else { ctx.undo() }
            }
        }
    });
    on_cleanup(move || keys.remove());

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
    let prev = (current > 0).then(|| (format!("#/edit/{}/{}", id, step_slug(current - 1)), STEPS[current - 1].1));
    let next = (current + 1 < STEPS.len()).then(|| (format!("#/edit/{}/{}", id, step_slug(current + 1)), STEPS[current + 1].1));
    let sheet_href = format!("#/sheet/{id}");
    let c5 = comp.clone();
    let this_step = STEPS[current].0;
    let is_review = current == STEPS.len() - 1;

    let nav = move || {
        let iss = issues.get();
        STEPS
            .iter()
            .enumerate()
            .map(|(i, (step, label))| {
                let n = if i == STEPS.len() - 1 { iss.len() } else { iss.iter().filter(|x| x.step == *step).count() };
                let href = format!("#/edit/{}/{}", id_v.get_value(), step_slug(i));
                let status = if i == STEPS.len() - 1 {
                    if n == 0 { "ready".to_string() } else { format!("{n} to do") }
                } else if n == 0 {
                    "done".to_string()
                } else {
                    format!("{n} to do")
                };
                view! {
                    <li>
                        <a class="step" class:active=i == current class:ok=n == 0 href=href
                            aria-current=(i == current).then_some("step")>
                            <span class="step-num" aria-hidden="true">{if n == 0 && i != current { "✓".to_string() } else { (i + 1).to_string() }}</span>
                            <span class="step-label">{*label}</span>
                            <span class="visually-hidden">{format!(", {status}")}</span>
                            {(n > 0).then(|| view! { <span class="step-badge" aria-hidden="true">{n}</span> })}
                        </a>
                    </li>
                }
            })
            .collect_view()
    };

    view! {
        <div class="editor">
            <a class="skip-link" href="#main" on:click=|ev| { ev.prevent_default(); focus_id("main"); }>"Skip to step content"</a>
            <header class="editor-head">
                <a class="brand" href="#/" aria-label="5e Builder — all characters">{crate::app::logo()}<span class="brand-name">"5e Builder"</span></a>
                <div class="who">
                    <strong class="who-name">{move || { let n = ch.with(|c| c.name.clone()); if n.is_empty() { "New character".to_string() } else { n } }}</strong>
                    <span class="who-line">{move || ch.with(|c| build.with(|b| rules::sheet(c, b, &c5).summary))}</span>
                </div>
                <div class="head-actions">
                    <button class="icon-button" aria-label="Undo" title="Undo (Ctrl+Z)" disabled=move || ctx.history.with(|h| h.undo.is_empty()) on:click=move |_| ctx.undo()>{icon("undo")}</button>
                    <button class="icon-button" aria-label="Redo" title="Redo (Ctrl+Shift+Z)" disabled=move || ctx.history.with(|h| h.redo.is_empty()) on:click=move |_| ctx.redo()>{icon("redo")}</button>
                    <a class="button primary" href=sheet_href>{icon("print")}<span class="hide-sm">"Sheet & print"</span><span class="show-sm">"Sheet"</span></a>
                </div>
            </header>
            <nav class="steps" aria-label="Character creation steps"><ol>{nav}</ol></nav>
            <main class="step-body" id="main" tabindex="-1">
                <header class="step-header">
                    <p class="step-kicker">{format!("Step {} of {}", current + 1, STEPS.len())}</p>
                    <h1>{STEPS[current].1}</h1>
                    <p class="lede">{STEP_INTROS[current]}</p>
                </header>
                {(!is_review).then(|| view! { <StepTodo step=this_step issues=issues /> })}
                {body}
            </main>
            <aside class="rail" aria-label="Character summary"><SummaryRail issues=issues /></aside>
            <footer class="step-foot">
                {prev.map(|(h, label)| view! { <a class="button ghost" href=h>{icon("back")}<span>{label}</span></a> })}
                <span class="spacer"></span>
                {move || {
                    let iss = issues.get();
                    let target = STEPS.iter().position(|(s, _)| iss.iter().any(|i| i.step == *s))?;
                    (target != current).then(|| {
                        let href = format!("#/edit/{}/{}", id_v.get_value(), step_slug(target));
                        view! { <a class="button ghost hide-sm" href=href title="Go to the first step with something left to do">"Next unfinished"</a> }
                    })
                }}
                {next.map(|(h, label)| view! { <a class="button primary" href=h><span>{label}</span>{icon("next")}</a> })}
            </footer>
        </div>
    }
    .into_any()
}

/// Moves keyboard focus to an element (and scrolls it into view).
pub fn focus_id(id: &str) {
    if let Some(el) = web_sys::window().and_then(|w| w.document()).and_then(|d| d.get_element_by_id(id)) {
        el.scroll_into_view();
        if let Ok(h) = wasm_bindgen::JsCast::dyn_into::<web_sys::HtmlElement>(el) {
            let _ = h.focus();
        }
    }
}

/// DOM id for a choice section.
pub fn choice_anchor(path: &str) -> String {
    let slug: String = path.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    format!("choice-{slug}")
}

/// "Left on this step": every unfinished item, each a link to the choice (GOV.UK error-summary pattern).
#[component]
fn StepTodo(step: Step, issues: Memo<Vec<rules::Issue>>) -> impl IntoView {
    let ctx = expect_context::<EditorCtx>();
    let items = Memo::new(move |_| {
        let pending: Vec<(String, String)> = ctx.build.with(|b| {
            b.choices
                .iter()
                .filter(|c| c.step == step && !c.is_complete())
                .map(|c| (choice_anchor(&c.path), format!("{} — {} ({} of {} chosen)", c.label, c.origin, c.picked(), c.count)))
                .collect()
        });
        let other: Vec<String> = issues.with(|iss| {
            iss.iter()
                .filter(|i| i.step == step && !i.text.contains(" — "))
                .map(|i| i.text.clone())
                .collect()
        });
        (pending, other)
    });
    move || {
        let (pending, other) = items.get();
        if pending.is_empty() && other.is_empty() {
            return view! { <p class="todo done" role="status">{icon("check")}"Everything on this step is done."</p> }.into_any();
        }
        let n = pending.len() + other.len();
        view! {
            <section class="todo" aria-labelledby="todo-title">
                <h2 id="todo-title">{format!("Left on this step ({n})")}</h2>
                <ul>
                    {other.into_iter().map(|t| view! { <li>{t}</li> }).collect_view()}
                    {pending.into_iter().map(|(anchor, text)| {
                        let a2 = anchor.clone();
                        view! { <li><a href=format!("#{anchor}") on:click=move |ev| { ev.prevent_default(); focus_id(&a2); }>{text}</a></li> }
                    }).collect_view()}
                </ul>
            </section>
        }
        .into_any()
    }
}

/// Live character snapshot shown beside every step.
#[component]
fn SummaryRail(issues: Memo<Vec<rules::Issue>>) -> impl IntoView {
    let ctx = expect_context::<EditorCtx>();
    let comp = ctx.comp.get_value();
    let sheet = Memo::new(move |_| ctx.ch.with(|c| ctx.build.with(|b| rules::sheet(c, b, &comp))));
    let progress = Memo::new(move |_| {
        ctx.build.with(|b| {
            let total = b.choices.len().max(1);
            let done = b.choices.iter().filter(|c| c.is_complete()).count();
            (done, total)
        })
    });
    move || {
        let s = sheet.get();
        let (done, total) = progress.get();
        let left = issues.with(|i| i.len());
        let pct = (done * 100 / total).min(100);
        view! {
            <div class="rail-card">
                <p class="rail-title">{s.name.clone()}</p>
                <p class="rail-sub">{if s.summary.is_empty() { "Choose a class to begin".to_string() } else { format!("Level {} · {}", s.level, s.summary) }}</p>
                <div class="meter" role="img" aria-label=format!("{done} of {total} choices made")>
                    <div class="meter-fill" style=format!("width: {pct}%")></div>
                </div>
                <p class="rail-note">{if left == 0 { "Ready to print".to_string() } else { format!("{left} thing{} left to do", if left == 1 { "" } else { "s" }) }}</p>
                <dl class="rail-stats">
                    <div><dt>"AC"</dt><dd>{s.armor_class}</dd></div>
                    <div><dt>"HP"</dt><dd>{s.hp_max}</dd></div>
                    <div><dt>"Speed"</dt><dd>{s.speed.walk}</dd></div>
                    <div><dt>"Prof."</dt><dd>{compendium::signed(s.proficiency_bonus)}</dd></div>
                </dl>
                <dl class="rail-abilities">
                    {s.abilities.iter().map(|a| view! {
                        <div><dt>{a.ability.abbr()}</dt><dd><span class="mod">{compendium::signed(a.modifier)}</span><span class="score">{a.score}</span></dd></div>
                    }).collect_view()}
                </dl>
            </div>
        }
    }
}

/// Inline SVG icons (decorative; buttons carry their own labels).
pub fn icon(name: &str) -> impl IntoView {
    let d = match name {
        "undo" => "M9 14 4 9l5-5M4 9h10.5a5.5 5.5 0 0 1 0 11H11",
        "redo" => "m15 14 5-5-5-5M20 9H9.5a5.5 5.5 0 0 0 0 11H13",
        "print" => "M6 9V2h12v7M6 18H4a2 2 0 0 1-2-2v-5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2v5a2 2 0 0 1-2 2h-2M6 14h12v8H6z",
        "back" => "M15 18l-6-6 6-6",
        "next" => "M9 18l6-6-6-6",
        "check" => "M20 6 9 17l-5-5",
        "plus" => "M12 5v14M5 12h14",
        "sparkle" => "M12 3l1.9 5.1L19 10l-5.1 1.9L12 17l-1.9-5.1L5 10l5.1-1.9z",
        "dice" => "M12 2 3 7v10l9 5 9-5V7zM12 22V12M3 7l9 5 9-5",
        "info" => "M12 16v-4M12 8h.01M22 12a10 10 0 1 1-20 0 10 10 0 0 1 20 0z",
        "x" => "M18 6 6 18M6 6l12 12",
        "upload" => "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4M17 8l-5-5-5 5M12 3v12",
        "download" => "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4M7 10l5 5 5-5M12 15V3",
        _ => "",
    };
    view! {
        <svg class="icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false" fill="none" stroke="currentColor"
            stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d=d /></svg>
    }
}

/// Choice pickers for every choice of a step, in build order.
#[component]
fn StepChoices(step: Step, #[prop(optional)] filter: Option<fn(&rules::ChoiceState) -> bool>) -> impl IntoView {
    let ctx = expect_context::<EditorCtx>();
    let paths = Memo::new(move |_| {
        ctx.build.with(|b| b.choices.iter().filter(|c| c.step == step && filter.is_none_or(|f| f(c))).map(|c| c.path.clone()).collect::<Vec<_>>())
    });
    let pending = Memo::new(move |_| {
        ctx.build.with(|b| b.choices.iter().filter(|c| c.step == step && filter.is_none_or(|f| f(c)) && !c.is_complete()).count())
    });
    view! {
        <Show when=move || { pending.get() > 1 }>
            <div class="fill-bar">
                <span>{move || format!("{} choices left here.", pending.get())}</span>
                <button class="button ghost" on:click=move |_| ctx.fill(Some(step))
                    title="Picks sensible options for every unfinished choice on this step. You can change them afterwards.">"✨ Choose the rest for me"</button>
            </div>
        </Show>
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
                        <button class="link-button" aria-label=format!("Use all {} sources", g.label()) on:click=move |_| { let ids = ids.clone(); ctx.edit(|c| ids.iter().for_each(|i| { c.settings.disabled_sources.remove(i); })) }>"All"</button>
                        <button class="link-button" aria-label=format!("Turn off all {} sources", g.label()) on:click=move |_| { let ids = ids2.clone(); ctx.edit(|c| c.settings.disabled_sources.extend(ids)) }>"None"</button>
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
        <h2>"Rules"</h2>
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
        <h2>"Sources"</h2>
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
            <input class="search" type="search" placeholder="Search by name or book" aria-label="Search" prop:value=move || search.get() on:input=move |ev| search.set(event_target_value(&ev)) />
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
                        <h2>{class.name.clone()}</h2>
                        <span class="muted">{c2.source_name(&class.source)}{class.legacy.then_some(" · legacy")}</span>
                    </div>
                    <button class="button ghost" on:click=move |_| browsing.set(true)>"Change class"</button>
                </div>
                <p>
                    {format!("Hit die d{} · Saves: {}", class.hit_die, class.saves.iter().map(|a| a.name()).collect::<Vec<_>>().join(", "))}
                    {(!class.multiclass_abilities.is_empty()).then(|| format!(" · Multiclassing: {}", class.multiclass_abilities.iter().map(|g| g.iter().map(|(a, n)| format!("{} {n}", a.abbr())).collect::<Vec<_>>().join(" or ")).collect::<Vec<_>>().join(", ")))}
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
        <Show when=move || browsing.get() || selected.get().is_none()>
            <p class="muted">"Pick the class your character starts in. You can add levels in other classes below."</p>
            <CardGrid cards=cards selected=selected on_pick=on_pick />
        </Show>
        <Show when=move || !browsing.get() && selected.get().is_some()>
            {summary.clone()}
            <LevelsPanel />
            <h2>"Class choices"</h2>
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
                    <select aria-label="Class for the next level" on:change=move |ev| add_class.set(event_target_value(&ev))>
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
                    <div><h2>{b.name.clone()}</h2><span class="muted">{c2.source_name(&b.source)}</span></div>
                    <button class="button ghost" on:click=move |_| browsing.set(true)>"Change background"</button>
                </div>
                <p>{grants_summary(&c2, &b.grants)}</p>
                {note}
                {b.features.iter().map(|f| view! { <details><summary>{f.name.clone()}</summary><Entries entries=f.body.clone() /></details> }).collect_view()}
            </div>
        })
    };
    view! {
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
                    <div><h2>{s.name.clone()}</h2><span class="muted">{c2.source_name(&s.source)}{s.legacy.then_some(" · legacy")}</span></div>
                    <button class="button ghost" on:click=move |_| browsing.set(true)>"Change species"</button>
                </div>
                <p>{format!("{} {} · Speed {} ft.", s.sizes.iter().map(|x| x.name()).collect::<Vec<_>>().join(" or "), s.creature_type, s.speed.walk)}</p>
                {s.features.iter().map(|f| view! { <details><summary>{f.name.clone()}</summary><Entries entries=f.body.clone() /></details> }).collect_view()}
            </div>
        })
    };
    view! {
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
                AbilityMethod::StandardArray | AbilityMethod::Rolled => [0; 6],
                AbilityMethod::Manual => c.abilities.scores.map(|s| if s == 0 { 10 } else { s }),
            };
        })
    };
    let spent = move || ctx.ch.with(|c| c.abilities.scores.iter().filter_map(|s| rules::point_cost(*s)).map(|x| x as u32).sum::<u32>());
    let problem = move || ctx.ch.with(|c| rules::ability_problem(&c.abilities));

    let rows = move || {
        let (base, m, rolls) = ctx.ch.with(|c| (c.abilities.scores, c.abilities.method, c.abilities.rolls.clone()));
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
                            <button aria-label=format!("Decrease {}", a.name()) disabled=base[i] <= 8 on:click=move |_| ctx.edit(|c| c.abilities.scores[i] = c.abilities.scores[i].saturating_sub(1).max(8))>"−"</button>
                            <span class="score" aria-live="polite" aria-label=format!("{} base score {}", a.name(), base[i])>{base[i]}</span>
                            <button aria-label=format!("Increase {}", a.name()) disabled=base[i] >= 15 on:click=move |_| ctx.edit(|c| c.abilities.scores[i] = (c.abilities.scores[i] + 1).min(15))>"+"</button>
                            <span class="muted small">{format!("cost {}", rules::point_cost(base[i]).unwrap_or(0))}</span>
                        </div>
                    }
                    .into_any(),
                    AbilityMethod::StandardArray | AbilityMethod::Rolled => {
                        let pool: Vec<u8> = if m == AbilityMethod::Rolled { rolls.clone() } else { rules::STANDARD_ARRAY.to_vec() };
                        let mut values = pool.clone();
                        values.sort_unstable_by(|a, b| b.cmp(a));
                        values.dedup();
                        let used: Vec<u8> = base.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, v)| *v).collect();
                        view! {
                            <select aria-label=format!("{} base score", a.name()) disabled=pool.is_empty() on:change=move |ev| { let v: u8 = event_target_value(&ev).parse().unwrap_or(0); ctx.edit(|c| c.abilities.scores[i] = v) }>
                                <option value="0" selected=base[i] == 0>"—"</option>
                                {values.iter().map(|&v| {
                                    let taken = used.iter().filter(|u| **u == v).count() >= pool.iter().filter(|x| **x == v).count();
                                    view! { <option value=v.to_string() selected=base[i] == v disabled=taken && base[i] != v>{v}</option> }
                                }).collect_view()}
                            </select>
                        }
                        .into_any()
                    }
                    AbilityMethod::Manual => view! {
                        <input class="num" type="number" min="1" max="30" aria-label=format!("{} base score", a.name()) prop:value=base[i].to_string()
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
        <div class="tabs">
            {tab(AbilityMethod::PointBuy, "Point buy")}
            {tab(AbilityMethod::StandardArray, "Standard array")}
            {tab(AbilityMethod::Rolled, "Roll 4d6")}
            {tab(AbilityMethod::Manual, "Manual")}
        </div>
        <Show when=move || method() == AbilityMethod::Rolled>
            <div class="row">
                <button class="button" on:click=move |_| {
                    let rolls = rules::roll_ability_scores(js_sys::Math::random);
                    ctx.edit(|c| { c.abilities.rolls = rolls; c.abilities.scores = [0; 6]; });
                }>{move || if ctx.ch.with(|c| c.abilities.rolls.is_empty()) { "🎲 Roll six scores" } else { "🎲 Reroll" }}</button>
                <span class="rolls">{move || ctx.ch.with(|c| c.abilities.rolls.iter().map(|r| r.to_string()).collect::<Vec<_>>().join(" · "))}</span>
            </div>
        </Show>
        <Show when=move || method() != AbilityMethod::Manual>
            <div class="row">
                <button class="button ghost" on:click=move |_| {
                    let comp = ctx.comp.get_value();
                    ctx.edit(|c| auto_assign(c, &comp));
                } title="Highest scores go to your class's primary abilities, then Constitution.">"Assign for my class"</button>
            </div>
        </Show>
        <Show when=move || method() == AbilityMethod::PointBuy>
            <p class="points">{move || format!("{} / {} points spent", spent(), rules::POINT_BUY_BUDGET)}</p>
        </Show>
        {move || problem().map(|p| view! { <p class="note">{p}</p> })}
        <div class="table-wrap">
            <table class="abilities">
                <thead><tr><th>"Ability"</th><th>"Base"</th><th>"Bonuses"</th><th>"Score"</th><th>"Modifier"</th></tr></thead>
                <tbody>{rows}</tbody>
            </table>
        </div>
        <h2>"Ability score increases"</h2>
        <StepChoices step=Step::Background filter=is_ability_choice />
        <StepChoices step=Step::Species filter=is_ability_choice />
    }
}

/// Puts the highest available scores into the class's primary abilities, then CON, DEX, WIS.
fn auto_assign(c: &mut Character, comp: &Compendium) {
    let mut pool: Vec<u8> = match c.abilities.method {
        AbilityMethod::Rolled => c.abilities.rolls.clone(),
        _ => rules::STANDARD_ARRAY.to_vec(),
    };
    if pool.len() != 6 {
        return;
    }
    pool.sort_unstable_by(|a, b| b.cmp(a));
    let mut order: Vec<Ability> = c.levels.first().and_then(|l| comp.classes.get(&l.class)).map(|x| x.primary.clone()).unwrap_or_default();
    for a in [Ability::Con, Ability::Dex, Ability::Wis, Ability::Str, Ability::Cha, Ability::Int] {
        if !order.contains(&a) {
            order.push(a);
        }
    }
    for (a, v) in order.into_iter().zip(pool) {
        c.abilities.scores[a.index()] = v;
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
        let attuned = inv.iter().filter(|i| i.attuned).count();
        if inv.is_empty() {
            return view! { <p class="muted">"Inventory is empty. Choose starting equipment above or add items."</p> }.into_any();
        }
        view! {
            <table class="inventory">
                <thead><tr><th>"Item"</th><th>"Qty"</th><th>"Equipped"</th><th>"Attuned"</th><th><span class="visually-hidden">"Actions"</span></th></tr></thead>
                <tbody>
                    {inv.into_iter().enumerate().map(|(i, it)| view! {
                        <tr>
                            <td>{it.name.clone()}{it.starting.then(|| view! { <span class="badge">"starting"</span> })}</td>
                            <td><input class="num" type="number" min="1" aria-label=format!("Quantity of {}", it.name) prop:value=it.qty.to_string()
                                on:change=move |ev| { let q: u16 = event_target_value(&ev).parse().unwrap_or(1); ctx.edit(|c| { c.inventory[i].qty = q.max(1); c.inventory[i].starting = false; }) } /></td>
                            <td><input type="checkbox" aria-label=format!("{} equipped", it.name) prop:checked=it.equipped on:change=move |ev| { let on = event_target_checked(&ev); ctx.edit(|c| c.inventory[i].equipped = on) } /></td>
                            <td>{it.item.as_ref().and_then(|k| ctx.comp.get_value().items.get(k).map(|x| x.attunement)).unwrap_or(false).then(|| view! {
                                <input type="checkbox" aria-label="Attuned" prop:checked=it.attuned on:change=move |ev| { let on = event_target_checked(&ev); ctx.edit(|c| c.inventory[i].attuned = on) } />
                            })}</td>
                            <td><button class="link-button" aria-label=format!("Remove {}", it.name) on:click=move |_| ctx.edit(|c| { c.inventory.remove(i); })>{icon("x")}"Remove"</button></td>
                        </tr>
                    }).collect_view()}
                </tbody>
            </table>
            {(attuned > 0).then(|| view! { <p class="muted small" class:over-limit={attuned > 3}>{format!("Attuned to {attuned} of 3 items.")}</p> })}
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
        <StepChoices step=Step::Equipment />
        <h2>"Inventory"</h2>
        {inventory}
        <div class="add-item">
            <label class="field"><span>"Add an item from the compendium"</span>
                <input class="search" type="search" placeholder="e.g. rope, longsword, potion" prop:value=move || query.get() on:input=move |ev| query.set(event_target_value(&ev)) />
            </label>
            {results}
            <div class="row">
                <label class="field inline grow"><span>"Custom item"</span>
                    <input type="text" prop:value=move || custom.get() on:input=move |ev| custom.set(event_target_value(&ev)) />
                </label>
                <button class="button" disabled=move || custom.get().trim().is_empty() on:click=move |_| {
                    let name = custom.get().trim().to_string();
                    ctx.edit(|c| c.inventory.push(rules::InventoryItem { item: None, name, qty: 1, equipped: false, attuned: false, starting: false, notes: String::new() }));
                    custom.set(String::new());
                }>"Add"</button>
            </div>
        </div>
        <h2>"Coins"</h2>
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
        <label class="field big"><span>"Character name"</span>
            <input type="text" prop:value=move || ctx.ch.with(|c| c.name.clone()) on:input=move |ev| { let v = event_target_value(&ev); ctx.edit(|c| c.name = v) } />
        </label>
        <datalist id="alignments">
            {["Lawful Good", "Neutral Good", "Chaotic Good", "Lawful Neutral", "Neutral", "Chaotic Neutral", "Lawful Evil", "Neutral Evil", "Chaotic Evil", "Unaligned"]
                .into_iter().map(|a| view! { <option value=a></option> }).collect_view()}
        </datalist>
        <div class="field-grid">
            <label class="field"><span>"Alignment"</span>
                <input type="text" list="alignments" prop:value=move || ctx.ch.with(|c| c.details.alignment.clone())
                    on:input=move |ev| { let v = event_target_value(&ev); ctx.edit(|c| c.details.alignment = v) } />
            </label>
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
