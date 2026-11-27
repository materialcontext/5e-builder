//! Generic renderer for any rules choice: lists, searchable lists and
//! ordered ability-bonus slots, all driven by `rules::options`.

use crate::editor::{choice_anchor, icon, EditorCtx};
use crate::entries::Entries;
use compendium::{Ability, Options};
use leptos::prelude::*;
use rules::OptionItem;
use std::collections::{BTreeMap, BTreeSet};

/// Facet values in a sensible order: abilities in sheet order, "None" first,
/// everything else alphabetically.
fn facet_order(v: &str) -> (usize, String) {
    let abilities = ["Strength", "Dexterity", "Constitution", "Intelligence", "Wisdom", "Charisma"];
    if let Some(i) = abilities.iter().position(|a| *a == v) {
        return (i, String::new());
    }
    if v == "None" || v == "Cantrip" {
        return (0, String::new());
    }
    if let Some(n) = v.strip_prefix("Level ").and_then(|n| n.parse::<usize>().ok()) {
        return (n, String::new());
    }
    (10, v.to_string())
}

/// Facet → values (with counts) present in a list, in display order.
fn facets_of(opts: &[OptionItem]) -> Vec<(String, Vec<(String, usize)>)> {
    let order = ["Category", "Level", "On your list", "School", "Casting", "Ability increase", "Grants", "Prerequisite", "Other", "Edition", "Source"];
    let mut map: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    for o in opts {
        for (f, v) in &o.facets {
            *map.entry(f.clone()).or_default().entry(v.clone()).or_default() += 1;
        }
    }
    let mut out: Vec<(String, Vec<(String, usize)>)> = map
        .into_iter()
        .map(|(f, vals)| {
            let mut v: Vec<(String, usize)> = vals.into_iter().collect();
            v.sort_by_key(|(x, _)| facet_order(x));
            (f, v)
        })
        .collect();
    out.sort_by_key(|(f, _)| order.iter().position(|o| o == f).unwrap_or(order.len()));
    out
}

/// Does `o` pass the selected facet filters (any value within a facet, all facets)?
fn passes(o: &OptionItem, sel: &BTreeMap<String, BTreeSet<String>>) -> bool {
    sel.iter().filter(|(_, vals)| !vals.is_empty()).all(|(f, vals)| o.facets.iter().any(|(of, ov)| of == f && vals.contains(ov)))
}

/// Toggles `value` in a selection limited to `count` picks.
pub fn toggle(mut sel: Vec<String>, value: &str, count: usize) -> Vec<String> {
    if let Some(i) = sel.iter().position(|v| v == value) {
        sel.remove(i);
    } else if count == 1 {
        sel = vec![value.to_string()];
    } else if sel.len() < count {
        sel.push(value.to_string());
    }
    sel
}

#[component]
pub fn ChoicePicker(path: String) -> impl IntoView {
    let ctx = expect_context::<EditorCtx>();
    let comp = ctx.comp.get_value();
    let p = path.clone();
    let choice = Memo::new(move |_| ctx.build.with(|b| b.choice(&p).cloned()));
    let opts = Memo::new(move |_| {
        let Some(c) = choice.get() else { return vec![] };
        ctx.build.with(|b| ctx.ch.with(|ch| rules::options(&c, b, ch, &comp)))
    });
    let state = ListState::new();

    move || {
        let Some(c) = choice.get() else { return ().into_any() };
        let count = c.count as usize;
        let n = c.picked();
        let done = n >= count;
        let path = c.path.clone();
        let total_level = ctx.ch.with(|ch| ch.total_level());
        let is_new = total_level > 1 && c.level == total_level;
        let p_fill = path.clone();
        let anchor = choice_anchor(&path);
        let title_id = format!("{anchor}-title");
        let header = view! {
            <div class="choice-head">
                <div>
                    <h2 class="choice-label" id=title_id.clone()>{c.label.clone()}</h2>
                    <span class="choice-origin" id=format!("{anchor}-origin")>{c.origin.clone()}</span>
                    {is_new.then(|| view! { <span class="badge new">"New this level"</span> })}
                </div>
                <div class="choice-tools">
                    {(!done).then(|| view! {
                        <button class="link-button" on:click=move |_| {
                            let comp = ctx.comp.get_value();
                            let p = p_fill.clone();
                            ctx.edit(|ch| {
                                let b = rules::build(ch, &comp);
                                if let Some(c) = b.choice(&p).cloned() {
                                    pick_for(ch, &c, &b, &comp);
                                }
                            });
                        }>{icon("sparkle")}"Pick for me"</button>
                    })}
                    <span class="counter" class:done=done class:over={n > count} aria-live="polite">
                        <span class="visually-hidden">"Chosen: "</span>{format!("{n} / {count}")}
                    </span>
                </div>
            </div>
        };

        let body = if c.ordered() {
            ability_slots(&c.options, c.selected.clone(), path.clone(), ctx).into_any()
        } else {
            let selected = c.selected.clone();
            let p_set = path.clone();
            let on_set = Callback::new(move |next: Vec<String>| ctx.edit(|ch| ch.set_choice(&p_set, next.clone())));
            option_list(opts, selected, count, on_set, state, &anchor).into_any()
        };
        view! { <section class="choice" class:complete=done class:fresh=is_new id=anchor.clone() tabindex="-1" role="group" aria-labelledby=format!("{title_id} {anchor}-origin")>{header}{body}</section> }.into_any()
    }
}

/// Search, filter and expansion state for an option list. Created by the
/// owner of the list so it survives the list re-rendering after a pick.
#[derive(Clone, Copy)]
pub struct ListState {
    search: RwSignal<String>,
    inspect: RwSignal<Option<String>>,
    group: RwSignal<Option<String>>,
    facet_sel: RwSignal<BTreeMap<String, BTreeSet<String>>>,
    only_available: RwSignal<bool>,
}

impl ListState {
    pub fn new() -> Self {
        ListState {
            search: RwSignal::new(String::new()),
            inspect: RwSignal::new(None),
            group: RwSignal::new(None),
            facet_sel: RwSignal::new(BTreeMap::new()),
            only_available: RwSignal::new(false),
        }
    }
}

/// A searchable, filterable list of options. Every option expands to its full
/// description; the round control selects (`on_set` receives the new
/// selection, at most `count` values).
pub fn option_list(opts: Memo<Vec<OptionItem>>, selected: Vec<String>, count: usize, on_set: Callback<Vec<String>>, state: ListState, anchor: &str) -> impl IntoView {
    let ListState { search, inspect, group, facet_sel, only_available } = state;
    let n = selected.len();
    let many = opts.with(|o| o.len()) > 14;
    let groups: Vec<String> = opts.with(|o| {
        let mut g: Vec<String> = Vec::new();
        for x in o.iter().filter_map(|x| x.group.clone()) {
            if !g.contains(&x) {
                g.push(x);
            }
        }
        g
    });
    let chips = (many && n > 0 && count > 1).then(|| {
        let labels: Vec<(String, String)> =
            opts.with(|o| selected.iter().filter_map(|v| o.iter().find(|x| &x.value == v).map(|x| (v.clone(), x.label.clone()))).collect());
        let sel = selected.clone();
        view! {
            <div class="chips">
                {labels.into_iter().map(|(v, l)| {
                    let sel = sel.clone();
                    view! {
                        <button class="chip removable" aria-label=format!("Remove {l}") on:click=move |_| {
                            on_set.run(sel.iter().filter(|x| **x != v).cloned().collect());
                        }>{l.clone()}{icon("x")}</button>
                    }
                }).collect_view()}
            </div>
        }
    });
    let facets = opts.with(|o| facets_of(o));
    let filter_menu = (!facets.is_empty()).then(|| filter_menu(facets.clone(), facet_sel, only_available, opts, anchor));
    let group_bar = (many && groups.len() > 1 && facets.is_empty()).then(|| {
        view! {
            <div class="group-bar" role="group" aria-label="Filter options">
                <button class="tab small" class:active=move || group.get().is_none() aria-pressed=move || group.get().is_none().to_string() on:click=move |_| group.set(None)>"All"</button>
                {groups.into_iter().map(|g| {
                    let (g2, g3, g4) = (g.clone(), g.clone(), g.clone());
                    view! { <button class="tab small" class:active=move || group.get().as_deref() == Some(g2.as_str()) aria-pressed=move || (group.get().as_deref() == Some(g4.as_str())).to_string() on:click=move |_| group.set(Some(g3.clone()))>{g}</button> }
                }).collect_view()}
            </div>
        }
    });
    let list = move || {
        let q = search.get().to_lowercase();
        let g = group.get();
        let selected = selected.clone();
        let sel = facet_sel.get();
        let avail = only_available.get();
        let shown: Vec<OptionItem> = opts
            .get()
            .into_iter()
            .filter(|o| g.is_none() || o.group == g)
            .filter(|o| passes(o, &sel))
            .filter(|o| !avail || o.disabled.is_none() || selected.contains(&o.value))
            .filter(|o| q.is_empty() || o.label.to_lowercase().contains(&q) || o.detail.to_lowercase().contains(&q))
            .collect();
        // Older versions that were reprinted sit in a fold at the end; it opens
        // when one of them is the current selection.
        let (older, current): (Vec<OptionItem>, Vec<OptionItem>) = shown.into_iter().partition(|o| o.older.is_some());
        let n_older = older.len();
        let open = older.iter().any(|o| selected.contains(&o.value));
        let older_rows = older.into_iter().map(|o| option_row(o, &selected, count, on_set, inspect)).collect_view();
        // Section headings when the list has groups (language kinds, spell levels).
        let grouped = {
            let mut g: Vec<&str> = current.iter().filter_map(|o| o.group.as_deref()).collect();
            g.dedup();
            g.len() > 1
        };
        let mut last: Option<String> = None;
        let rows = current
            .into_iter()
            .map(|o| {
                let head = (grouped && o.group != last).then(|| {
                    last = o.group.clone();
                    view! { <h3 class="opt-group">{o.group.clone().unwrap_or_default()}</h3> }
                });
                view! { {head}{option_row(o, &selected, count, on_set, inspect)} }
            })
            .collect_view();
        view! {
            {rows}
            {(n_older > 0).then(|| view! {
                <details class="older-fold" open=open>
                    <summary>
                        {format!("Older versions replaced in newer books ({n_older})")}
                        <span class="muted small">" — the 2024 rules use the newer version; these are here for tables that allow them"</span>
                    </summary>
                    <div class="older-list">{older_rows}</div>
                </details>
            })}
        }
    };
    view! {
        {chips}
        {many.then(|| view! {
            <input class="search" type="search" placeholder="Search by name or detail" aria-label="Search options"
                prop:value=move || search.get()
                on:input=move |ev| search.set(event_target_value(&ev)) />
        })}
        {group_bar}
        {filter_menu}
        <div class="options" class:scroll=many>{list}</div>
    }
}

/// The filter menu for faceted lists (feats): checkbox groups per facet,
/// "only what I can take", a live result count and a reset.
fn filter_menu(
    facets: Vec<(String, Vec<(String, usize)>)>,
    sel: RwSignal<BTreeMap<String, BTreeSet<String>>>,
    only_available: RwSignal<bool>,
    opts: Memo<Vec<OptionItem>>,
    anchor: &str,
) -> impl IntoView {
    let active = move || sel.with(|m| m.values().map(BTreeSet::len).sum::<usize>()) + usize::from(only_available.get());
    let shown = move || {
        let m = sel.get();
        let avail = only_available.get();
        opts.with(|o| (o.iter().filter(|x| passes(x, &m) && (!avail || x.disabled.is_none())).count(), o.len()))
    };
    // A group is useful if it splits the list: several values, or one value
    // that only some options have ("On your list: Always prepared").
    let total = opts.with(|o| o.len());
    let groups = facets
        .into_iter()
        .filter(|(_, vals)| vals.len() > 1 || vals.first().is_some_and(|(_, n)| *n < total))
        .enumerate()
        .map(|(gi, (facet, vals))| {
            let f2 = facet.clone();
            let n_values = vals.len();
            let many_values = n_values > 10;
            let boxes = vals
                .into_iter()
                .enumerate()
                .map(|(vi, (v, n))| {
                    let (fa, fb, va, vb) = (facet.clone(), facet.clone(), v.clone(), v.clone());
                    let id = format!("{anchor}-f{gi}-{vi}");
                    let for_id = id.clone();
                    view! {
                        <label class="facet" for=for_id>
                            <input type="checkbox" id=id
                                prop:checked=move || sel.with(|m| m.get(&fa).is_some_and(|s| s.contains(&va)))
                                on:change=move |ev| {
                                    let on = event_target_checked(&ev);
                                    sel.update(|m| {
                                        let set = m.entry(fb.clone()).or_default();
                                        if on { set.insert(vb.clone()); } else { set.remove(&vb); }
                                    });
                                } />
                            <span>{v.clone()}</span>
                            <span class="facet-n">{n}</span>
                        </label>
                    }
                })
                .collect_view();
            if many_values {
                view! {
                    <details class="facet-group facet-more">
                        <summary>{format!("{f2} ({n_values})")}</summary>
                        <fieldset class="facet-group"><legend class="visually-hidden">{f2.clone()}</legend><div class="facet-list">{boxes}</div></fieldset>
                    </details>
                }
                .into_any()
            } else {
                view! { <fieldset class="facet-group"><legend>{f2}</legend><div class="facet-list">{boxes}</div></fieldset> }.into_any()
            }
        })
        .collect_view();
    let avail_id = format!("{anchor}-avail");
    let avail_for = avail_id.clone();
    view! {
        <details class="filters">
            <summary>
                {icon("filter")}"Filters"
                {move || (active() > 0).then(|| view! { <span class="badge">{active()}" active"</span> })}
                <span class="filters-count" aria-live="polite">{move || { let (a, b) = shown(); format!("{a} of {b} shown") }}</span>
            </summary>
            <div class="filters-body">
                <label class="facet wide" for=avail_for>
                    <input type="checkbox" id=avail_id prop:checked=move || only_available.get() on:change=move |ev| only_available.set(event_target_checked(&ev)) />
                    <span>"Only show options I can take now"</span>
                </label>
                {groups}
                <button class="link-button" on:click=move |_| { sel.set(BTreeMap::new()); only_available.set(false); }>"Clear filters"</button>
            </div>
        </details>
    }
}

/// Fills one choice with suggested picks (same heuristics as "choose the rest").
fn pick_for(ch: &mut rules::Character, c: &rules::ChoiceState, _b: &rules::Build, comp: &compendium::Compendium) {
    // Run the suggester restricted to this choice's step, but only keep this choice.
    let mut trial = ch.clone();
    rules::suggest::fill(&mut trial, comp, Some(c.step));
    if let Some(v) = trial.choices.get(&c.path).cloned() {
        ch.set_choice(&c.path, v);
    }
}

/// First real sentence of an option's description, for the card preview
/// (skips section titles and "you gain the following benefits" lead-ins).
fn preview(body: &[compendium::Entry]) -> String {
    use compendium::Entry;
    fn first(es: &[Entry]) -> Option<String> {
        for e in es {
            let found = match e {
                Entry::P(t) => {
                    let l = t.to_lowercase();
                    (!t.trim_end().ends_with(':') && !l.contains("the following benefit")).then(|| t.clone())
                }
                // The increase is already shown as a tag; preview the feat's own benefit.
                Entry::Section { name: Some(n), .. } if n == "Ability Score Increase" => None,
                Entry::Section { body, .. } => first(body),
                Entry::List(items) => first(items),
                Entry::Table { .. } => None,
            };
            if found.is_some() {
                return found;
            }
        }
        None
    }
    let text = first(body).unwrap_or_default();
    // A spell's casting line is a list of facts, not a sentence: keep it whole.
    if text.starts_with("Casting time:") {
        return text;
    }
    let end = text.find(". ").map(|i| i + 1).unwrap_or(text.len());
    let s = &text[..end];
    if s.chars().count() > 160 {
        let cut: String = s.chars().take(157).collect();
        format!("{}…", cut.rsplit_once(' ').map(|(a, _)| a).unwrap_or(&cut))
    } else {
        s.to_string()
    }
}

/// One option: a round select control on the left (the only thing that
/// selects), and a card that expands to the full description when clicked.
/// Options without a description have nothing to expand; there the card is
/// simply the select control's label.
fn option_row(o: OptionItem, selected: &[String], count: usize, on_set: Callback<Vec<String>>, inspect: RwSignal<Option<String>>) -> impl IntoView {
    let is_sel = selected.contains(&o.value);
    let full = selected.len() >= count && count > 1;
    let blocked = o.disabled.is_some() && !is_sel;
    let value = o.value.clone();
    let sel = selected.to_vec();
    let pick = move || {
        if blocked {
            return;
        }
        on_set.run(toggle(sel.clone(), &value, count));
    };
    let pick2 = pick.clone();
    let has_body = !o.body.is_empty();
    let body_id = format!("opt-{}", o.value.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect::<String>());
    let (v1, v2, v3) = (o.value.clone(), o.value.clone(), o.value.clone());
    let body = o.body.clone();
    let facet = |name: &str| -> Vec<String> { o.facets.iter().filter(|(f, _)| f == name).map(|(_, v)| v.clone()).collect() };
    let category = facet("Category").into_iter().next();
    let abilities = facet("Ability increase");
    let grants = facet("Grants");
    let spells = facet("Spells");
    let origin_feat = facet("Origin feat");
    let summary = if has_body && !o.facets.is_empty() { preview(&o.body) } else { String::new() };
    let label = o.label.clone();
    let action = if is_sel { format!("Remove {label}") } else { format!("Select {label}") };
    let limit = (full && !is_sel).then_some("Limit reached — remove a selection first");

    let content = view! {
        <span class="option-top">
            <span class="option-label">{o.label.clone()}</span>
            {category.map(|c| view! { <span class="tag cat">{c}</span> })}
            {(!abilities.is_empty()).then(|| {
                let mut ab = abilities.clone();
                ab.sort_by_key(|a| facet_order(a));
                let text = if ab.len() == 6 { "+ any ability".to_string() } else {
                    format!("+ {}", ab.iter().map(|a| a.chars().take(3).collect::<String>().to_uppercase()).collect::<Vec<_>>().join(" / "))
                };
                view! { <span class="tag asi" title="Ability score increase">{text}</span> }
            })}
        </span>
        <span class="option-detail">{o.detail.clone()}</span>
        {o.older.clone().map(|n| view! { <span class="badge">{n}</span> })}
        {(!grants.is_empty() || !spells.is_empty() || !origin_feat.is_empty()).then(|| view! {
            <span class="tags">
                {origin_feat.iter().map(|f| view! { <span class="tag cat">{format!("Feat: {f}")}</span> }).collect_view()}
                {spells.iter().map(|x| view! { <span class="tag asi">{x.clone()}</span> }).collect_view()}
                {grants.iter().filter(|g| !(g.as_str() == "Spells" && !spells.is_empty())).map(|g| view! { <span class="tag">{g.clone()}</span> }).collect_view()}
            </span>
        })}
        {(!summary.is_empty()).then(|| view! { <span class="option-summary">{summary.clone()}</span> })}
        {o.warning.clone().map(|w| view! { <span class="badge warn">{w}</span> })}
        {o.disabled.clone().filter(|_| !is_sel).map(|d| {
            // The prerequisite is already on the detail line; don't repeat it.
            let shown = if d.starts_with("Requires") { "Prerequisite not met".to_string() } else { d };
            view! { <span class="badge">{shown}</span> }
        })}
    };
    let main = if has_body {
        let open = move || inspect.get().as_deref() == Some(v1.as_str());
        view! {
            <button class="option-main" aria-expanded=move || open().to_string() aria-controls=body_id.clone()
                on:click=move |_| inspect.update(|i| *i = if i.as_deref() == Some(&v2) { None } else { Some(v2.clone()) })>
                {content}
                <span class="expand" aria-hidden="true">{icon("next")}</span>
            </button>
        }
        .into_any()
    } else {
        // Nothing to expand: the card is the select control's label (mouse only;
        // keyboard and screen readers use the select button itself).
        view! { <div class="option-main plain" on:click=move |_| pick2()>{content}</div> }.into_any()
    };
    view! {
        <div class="option" class:selected=is_sel class:disabled=blocked class:dim=full && !is_sel class:open=move || inspect.get().as_deref() == Some(v3.as_str())>
            <button class="select-dot" aria-pressed=is_sel.to_string() aria-label=action disabled=blocked
                aria-disabled=limit.map(|_| "true") title=limit on:click=move |_| pick()>
                <span class="dot" aria-hidden="true">{is_sel.then(|| icon("check"))}</span>
            </button>
            {main}
            {has_body.then(|| {
                let v = o.value.clone();
                let id = format!("opt-{}", o.value.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect::<String>());
                view! {
                    <div class="option-body" id=id hidden=move || inspect.get().as_deref() != Some(v.as_str())>
                        <Entries entries=body.clone() />
                    </div>
                }
            })}
        </div>
    }
}

/// Positional picks for unequal ability bonuses (+2 to one, +1 to another).
fn ability_slots(options: &Options, selected: Vec<String>, path: String, ctx: EditorCtx) -> impl IntoView {
    let Options::Abilities { from, amounts, .. } = options.clone() else { return ().into_any() };
    amounts
        .iter()
        .enumerate()
        .map(|(i, amount)| {
            let current = selected.get(i).cloned().unwrap_or_default();
            let sel = selected.clone();
            let path = path.clone();
            let from2 = from.clone();
            let n = amounts.len();
            view! {
                <label class="ability-slot">
                    <span class="amount">{compendium::signed(*amount as i32)}</span>
                    <select aria-label=format!("Ability for {}", compendium::signed(*amount as i32)) on:change=move |ev| {
                        let v = event_target_value(&ev);
                        let mut next = sel.clone();
                        next.resize(n, String::new());
                        if let Some(j) = next.iter().position(|x| x == &v) {
                            next[j] = next[i].clone();
                        }
                        next[i] = v;
                        ctx.edit(|ch| ch.set_choice(&path, next.clone()));
                    }>
                        <option value="" selected=current.is_empty()>"Choose ability…"</option>
                        {from2.iter().map(|a: &Ability| {
                            let id = a.id();
                            view! { <option value=id selected=current == id>{a.name()}</option> }
                        }).collect_view()}
                    </select>
                </label>
            }
        })
        .collect_view()
        .into_any()
}
