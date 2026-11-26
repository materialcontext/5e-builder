//! Renders compendium rich text.

use compendium::Entry;
use leptos::prelude::*;

#[component]
pub fn Entries(entries: Vec<Entry>) -> impl IntoView {
    entries.into_iter().map(entry).collect_view()
}

fn entry(e: Entry) -> AnyView {
    match e {
        Entry::P(text) => view! { <p>{text}</p> }.into_any(),
        Entry::Section { name, body } => view! {
            <div class="entry-section">
                {name.map(|n| view! { <strong class="entry-name">{n}". "</strong> })}
                {body.into_iter().map(entry).collect_view()}
            </div>
        }
        .into_any(),
        Entry::List(items) => view! { <ul>{items.into_iter().map(|i| view! { <li>{entry(i)}</li> }).collect_view()}</ul> }.into_any(),
        Entry::Table { caption, head, rows } => view! {
            <table class="entry-table">
                {caption.map(|c| view! { <caption>{c}</caption> })}
                <thead><tr>{head.into_iter().map(|h| view! { <th>{h}</th> }).collect_view()}</tr></thead>
                <tbody>
                    {rows.into_iter().map(|r| view! { <tr>{r.into_iter().map(|c| view! { <td>{c}</td> }).collect_view()}</tr> }).collect_view()}
                </tbody>
            </table>
        }
        .into_any(),
    }
}
