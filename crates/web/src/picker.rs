//! Generic renderer for any rules choice: lists, searchable lists and
//! ordered ability-bonus slots, all driven by `rules::options`.

use crate::editor::EditorCtx;
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

    move || {
        let Some(c) = choice.get() else { return ().into_any() };
        let count = c.count as usize;
        let n = c.picked();
        let done = n >= count;
        let path = c.path.clone();
        let header = view! {
            <div class="choice-head">
                <div>
                    <span class="choice-label">{c.label.clone()}</span>
                    <span class="choice-origin">{c.origin.clone()}</span>
                </div>
                <span class="counter" class:done=done class:over={n > count}>{format!("{n} / {count}")}</span>
            </div>
        };

        let body = if c.ordered() {
            ability_slots(&c.options, c.selected.clone(), path.clone(), ctx).into_any()
        } else {
            let many = opts.with(|o| o.len()) > 14;
            let selected = c.selected.clone();
            let p2 = path.clone();
            let list = move || {
                let q = search.get().to_lowercase();
                let selected = selected.clone();
                opts.get()
                    .into_iter()
                    .filter(|o| q.is_empty() || o.label.to_lowercase().contains(&q) || o.detail.to_lowercase().contains(&q) || selected.contains(&o.value))
                    .map(|o| option_row(o, &selected, count, p2.clone(), inspect, ctx))
                    .collect_view()
            };
            view! {
                {many.then(|| view! {
                    <input class="search" type="search" placeholder="Search…"
                        prop:value=move || search.get()
                        on:input=move |ev| search.set(event_target_value(&ev)) />
                })}
                <div class="options" class:scroll=many>{list}</div>
            }
            .into_any()
        };
        view! { <section class="choice" class:complete=done>{header}{body}</section> }.into_any()
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
    let v3 = o.value.clone();
    let body = o.body.clone();
    view! {
        <div class="option" class:selected=is_sel class:disabled=blocked class:dim=full && !is_sel>
            <button class="option-main" on:click=on_pick disabled=blocked>
                <span class="check">{if is_sel { "✓" } else { "" }}</span>
                <span class="option-label">{o.label.clone()}</span>
                <span class="option-detail">{o.detail.clone()}</span>
                {o.warning.clone().map(|w| view! { <span class="badge warn">{w}</span> })}
                {o.disabled.clone().filter(|_| !is_sel).map(|d| view! { <span class="badge">{d}</span> })}
            </button>
            {has_body.then(|| view! {
                <button class="info" title="Details"
                    on:click=move |_| inspect.update(|i| *i = if i.as_deref() == Some(&v2) { None } else { Some(v2.clone()) })>"ⓘ"</button>
            })}
            {move || (inspect.get().as_deref() == Some(v3.as_str())).then(|| view! {
                <div class="option-body"><Entries entries=body.clone() /></div>
            })}
        </div>
    }
}

/// Positional picks for unequal ability bonuses (+2 to one, +1 to another).
fn ability_slots(options: &Options, selected: Vec<String>, path: String, ctx: EditorCtx) -> impl IntoView {
    let Options::Abilities { from, amounts } = options.clone() else { return ().into_any() };
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
                    <select on:change=move |ev| {
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
