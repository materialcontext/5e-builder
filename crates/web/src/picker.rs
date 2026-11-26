//! Generic renderer for any rules choice: lists, searchable lists and
//! ordered ability-bonus slots, all driven by `rules::options`.

use crate::editor::{choice_anchor, icon, EditorCtx};
use crate::entries::Entries;
use compendium::{Ability, Options};
use leptos::prelude::*;
use rules::OptionItem;

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
    let search = RwSignal::new(String::new());
    let inspect = RwSignal::new(None::<String>);
    let group = RwSignal::new(None::<String>);

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
            let selected = c.selected.clone();
            let chips = (many && n > 0).then(|| {
                let labels: Vec<(String, String)> = opts.with(|o| {
                    selected.iter().filter_map(|v| o.iter().find(|x| &x.value == v).map(|x| (v.clone(), x.label.clone()))).collect()
                });
                let sel = selected.clone();
                let p = path.clone();
                view! {
                    <div class="chips">
                        {labels.into_iter().map(|(v, l)| {
                            let (sel, p) = (sel.clone(), p.clone());
                            view! {
                                <button class="chip removable" aria-label=format!("Remove {l}") on:click=move |_| {
                                    let next: Vec<String> = sel.iter().filter(|x| **x != v).cloned().collect();
                                    ctx.edit(|ch| ch.set_choice(&p, next));
                                }>{l.clone()}{icon("x")}</button>
                            }
                        }).collect_view()}
                    </div>
                }
            });
            let group_bar = (many && groups.len() > 1).then(|| {
                view! {
                    <div class="group-bar" role="group" aria-label="Filter options">
                        <button class="tab small" class:active=move || group.get().is_none() aria-pressed=move || group.get().is_none().to_string() on:click=move |_| group.set(None)>"All"</button>
                        {groups.into_iter().map(|g| {
                            let g2 = g.clone();
                            let g3 = g.clone();
                            {
                            let g4 = g.clone();
                            view! { <button class="tab small" class:active=move || group.get().as_deref() == Some(g2.as_str()) aria-pressed=move || (group.get().as_deref() == Some(g4.as_str())).to_string() on:click=move |_| group.set(Some(g3.clone()))>{g}</button> }
                        }
                        }).collect_view()}
                    </div>
                }
            });
            let p2 = path.clone();
            let list = move || {
                let q = search.get().to_lowercase();
                let g = group.get();
                let selected = selected.clone();
                opts.get()
                    .into_iter()
                    .filter(|o| g.is_none() || o.group == g)
                    .filter(|o| q.is_empty() || o.label.to_lowercase().contains(&q) || o.detail.to_lowercase().contains(&q))
                    .map(|o| option_row(o, &selected, count, p2.clone(), inspect, ctx))
                    .collect_view()
            };
            view! {
                {chips}
                {many.then(|| view! {
                    <input class="search" type="search" placeholder="Search options" aria-label="Search options"
                        prop:value=move || search.get()
                        on:input=move |ev| search.set(event_target_value(&ev)) />
                })}
                {group_bar}
                <div class="options" class:scroll=many>{list}</div>
            }
            .into_any()
        };
        view! { <section class="choice" class:complete=done class:fresh=is_new id=anchor.clone() tabindex="-1" role="group" aria-labelledby=format!("{title_id} {anchor}-origin")>{header}{body}</section> }.into_any()
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

fn option_row(o: OptionItem, selected: &[String], count: usize, path: String, inspect: RwSignal<Option<String>>, ctx: EditorCtx) -> impl IntoView {
    let is_sel = selected.contains(&o.value);
    let full = selected.len() >= count && count > 1;
    let blocked = o.disabled.is_some() && !is_sel;
    let value = o.value.clone();
    let sel = selected.to_vec();
    let on_pick = move |_| {
        if blocked {
            return;
        }
        let next = toggle(sel.clone(), &value, count);
        ctx.edit(|ch| ch.set_choice(&path, next));
    };
    let has_body = !o.body.is_empty();
    let v2 = o.value.clone();
    let v2b = o.value.clone();
    let v3 = o.value.clone();
    let body = o.body.clone();
    view! {
        <div class="option" class:selected=is_sel class:disabled=blocked class:dim=full && !is_sel>
            <button class="option-main" on:click=on_pick disabled=blocked aria-pressed=is_sel.to_string()
                aria-disabled=(full && !is_sel).then_some("true") title=(full && !is_sel).then_some("Limit reached — remove a selection first")>
                <span class="check" aria-hidden="true">{is_sel.then(|| icon("check"))}</span>
                <span class="option-label">{o.label.clone()}</span>
                <span class="option-detail">{o.detail.clone()}</span>
                {o.warning.clone().map(|w| view! { <span class="badge warn">{w}</span> })}
                {o.disabled.clone().filter(|_| !is_sel).map(|d| view! { <span class="badge">{d}</span> })}
            </button>
            {has_body.then(|| view! {
                <button class="info" aria-label=format!("Details: {}", o.label) aria-expanded=move || (inspect.get().as_deref() == Some(v2b.as_str())).to_string()
                    on:click=move |_| inspect.update(|i| *i = if i.as_deref() == Some(&v2) { None } else { Some(v2.clone()) })>{icon("info")}</button>
            })}
            {move || (inspect.get().as_deref() == Some(v3.as_str())).then(|| view! {
                <div class="option-body"><Entries entries=body.clone() /></div>
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
