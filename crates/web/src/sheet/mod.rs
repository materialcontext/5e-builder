//! Printable character sheet: a replica of the two-page 2024 sheet
//! (US Letter / A4), followed by optional pages with full feature and spell
//! text and a continuation page for anything that did not fit.

mod art;
mod replica;

use crate::app::logo;
use crate::editor::icon;
use crate::entries::Entries;
use crate::store::Store;
use compendium::{Compendium, Entry};
use leptos::prelude::*;
use replica::Overflow;
use rules::build::FeatureKind;
use rules::sheet::{Sheet, SpellLine};
use std::sync::Arc;

#[derive(Clone, Copy)]
struct Opts {
    feature_text: RwSignal<bool>,
    spell_text: RwSignal<bool>,
    a4: RwSignal<bool>,
    tint: RwSignal<bool>,
}

#[component]
pub fn SheetPage(id: String) -> impl IntoView {
    let comp = expect_context::<Arc<Compendium>>();
    let store = expect_context::<Store>();
    let Some(ch) = store.with_untracked(|s| s.iter().find(|c| c.id == id).cloned()) else {
        return view! {
            <main class="loading"><div class="loading-card"><h1>"Character not found"</h1><a class="button primary" href="#/">"Back to characters"</a></div></main>
        }
        .into_any();
    };
    let o = Opts {
        feature_text: RwSignal::new(true),
        spell_text: RwSignal::new(false),
        a4: RwSignal::new(false),
        // Off by default: a full-page grey background wastes ink.
        tint: RwSignal::new(false),
    };
    let b = rules::build(&ch, &comp);
    let s = rules::sheet(&ch, &b, &comp);
    let pending = rules::issues(&ch, &b, &comp).len();
    let edit_href = format!("#/edit/{id}/review");
    let toggle = move |label: &'static str, sig: RwSignal<bool>| {
        view! {
            <label class="switch">
                <input type="checkbox" prop:checked=move || sig.get() on:change=move |ev| sig.set(event_target_checked(&ev)) />
                <span>{label}</span>
            </label>
        }
    };

    // Pages 1–2 depend on the paper: on longer paper the boxes nearest the
    // bottom grow so the margins stay even all around.
    let (ch1, s1) = (ch.clone(), s.clone());
    let replica_pages = move || {
        let ext = replica::extra_height(o.a4.get());
        let mut overflow = Overflow::default();
        let p1 = replica::page1(&ch1, &s1, &mut overflow, ext);
        let p2 = replica::page2(&ch1, &s1, &mut overflow, ext);
        let page = |p: replica::Page, label: &'static str| {
            view! {
                <section class="page rp-page" aria-label=label>
                    <div class="rp" style=format!("height: calc({} * var(--u))", 774.0 + ext)>
                        <div class="rp-art-wrap" inner_html=p.svg></div>
                        <div class="rp-overlay" inner_html=p.html></div>
                    </div>
                </section>
            }
        };
        let more = (!overflow.is_empty()).then(|| continuation_page(&s1, overflow));
        view! { {page(p1, "Page 1: core statistics")}{page(p2, "Page 2: spells, story and equipment")}{more} }
    };
    let (s2, s3) = (s.clone(), s.clone());

    view! {
        <div class="sheet-view">
            <header class="sheet-toolbar no-print">
                <a class="brand" href="#/" aria-label="5e Builder — all characters">{logo()}</a>
                <a class="button ghost small" href="#/">{icon("back")}"Characters"</a>
                <a class="button ghost small" href=edit_href>"Edit"</a>
                {(pending > 0).then(|| view! { <span class="chip warn">{format!("{pending} unfinished")}</span> })}
                <span class="spacer"></span>
                <div class="switches" role="group" aria-label="Sheet options">
                    {toggle("Feature text", o.feature_text)}
                    {toggle("Spell descriptions", o.spell_text)}
                    {toggle("Grey background (uses more ink)", o.tint)}
                    {toggle("A4 paper", o.a4)}
                </div>
                <button class="button primary" on:click=move |_| { if let Some(w) = web_sys::window() { let _ = w.print(); } }>{icon("print")}"Print or save PDF"</button>
            </header>
            {move || view! {
                <style>{
                    // Sheet pages are positioned edge to edge on the paper; text pages
                    // (features, spells, continuation) flow over several sheets, so
                    // they get real margins on every sheet.
                    if o.a4.get() {
                        "@page { size: A4; margin: 0; } @page text { size: A4; margin: 0.3in; }"
                    } else {
                        "@page { size: letter; margin: 0; } @page text { size: letter; margin: 0.3in; }"
                    }
                }</style>
            }}
            <main class="sheet" class:a4=move || o.a4.get() class:tint=move || o.tint.get() id="main" aria-label=format!("Character sheet for {}", s.name)>
                <h1 class="visually-hidden">{format!("{} — character sheet", s.name)}</h1>
                {replica_pages}
                {move || o.feature_text.get().then(|| features_page(&s2))}
                {move || o.spell_text.get().then(|| spell_text_page(&s3))}
            </main>
        </div>
    }
    .into_any()
}

/// A full page in the sheet's style holding flowing text.
/// `columns`: flow text in two columns. Pages made of tables use one column,
/// because a table can't break across columns or, inside columns, pages.
fn text_page(title: String, label: &'static str, body: AnyView, columns: bool) -> impl IntoView {
    view! {
        <section class="page rp-page rp-text-page" aria-label=label>
            <div class="rp-panel">
                <h2 class="rp-title">{title}</h2>
                <p class="no-print rp-flow-note">"On paper this section continues over as many pages as it needs, each with its own margins."</p>
                <div class:rp-columns=columns class:rp-single=!columns>{body}</div>
            </div>
        </section>
    }
}

fn features_page(s: &Sheet) -> impl IntoView {
    let groups = [
        (FeatureKind::Class, "Class Features"),
        (FeatureKind::Species, "Species Traits"),
        (FeatureKind::Feat, "Feats"),
        (FeatureKind::Background, "Background"),
        (FeatureKind::Other, "Other Features"),
    ];
    let sections = groups
        .into_iter()
        .filter_map(|(kind, title)| {
            // Identical repeats (e.g. several Ability Score Improvement entries) print once.
            let mut items: Vec<rules::build::FeatureEntry> = Vec::new();
            for f in s.features.iter().filter(|f| f.kind == kind) {
                if !items.iter().any(|x| x.feature == f.feature) {
                    items.push(f.clone());
                }
            }
            if items.is_empty() {
                return None;
            }
            Some(view! {
                <section class="rp-group">
                    <h3 class="rp-group-title">{title}</h3>
                    {items.into_iter().map(|f| {
                        let body: Vec<Entry> = f.feature.body.clone();
                        view! {
                            <article class="rp-entry">
                                <h4>{f.feature.name.clone()}<span class="rp-origin">{f.origin.clone()}</span></h4>
                                <Entries entries=body />
                            </article>
                        }
                    }).collect_view()}
                </section>
            })
        })
        .collect_view();
    text_page(
        format!("{} — Features & Traits", s.name),
        "Features and traits",
        sections.into_any(),
        true,
    )
}

fn spell_entry(x: &SpellLine) -> impl IntoView {
    let lvl = if x.level == 0 {
        format!("{} cantrip", x.school)
    } else {
        format!("Level {} {}", x.level, x.school)
    };
    view! {
        <article class="rp-entry">
            <h4>{x.name.clone()}<span class="rp-origin">{lvl}</span></h4>
            <p class="rp-meta">{format!("{} · {} · {} · {}", x.time, x.range, x.components, x.duration)}</p>
            <Entries entries=x.body.clone() />
        </article>
    }
}

fn spell_text_page(s: &Sheet) -> impl IntoView {
    if s.spells.is_empty() {
        return ().into_any();
    }
    let items = s.spells.iter().map(spell_entry).collect_view();
    text_page(
        format!("{} — Spell Descriptions", s.name),
        "Spell descriptions",
        items.into_any(),
        true,
    )
    .into_any()
}

fn spell_rows(spells: &[SpellLine]) -> impl IntoView {
    spells
        .iter()
        .map(|x| {
            let tags = [x.concentration.then_some("C"), x.ritual.then_some("R")].into_iter().flatten().collect::<Vec<_>>().join(" ");
            view! {
                <tr>
                    <td class="c">{if x.level == 0 { "Cantrip".to_string() } else { x.level.to_string() }}</td>
                    <td class="strong">{x.name.clone()}</td>
                    <td>{x.time.split(',').next().unwrap_or("").to_string()}</td>
                    <td>{x.range.clone()}</td>
                    <td class="c">{tags}</td>
                    <td>{x.duration.clone()}</td>
                </tr>
            }
        })
        .collect_view()
}

fn spell_table(title: &'static str, spells: Vec<SpellLine>) -> impl IntoView {
    (!spells.is_empty()).then(|| {
        view! {
            <section class="rp-group">
                <h3 class="rp-group-title">{title}</h3>
                <table class="rp-table">
                    <thead><tr><th class="c">"Level"</th><th>"Name"</th><th>"Casting Time"</th><th>"Range"</th><th class="c"><abbr title="Concentration / Ritual">"C/R"</abbr></th><th>"Duration"</th></tr></thead>
                    <tbody>{spell_rows(&spells)}</tbody>
                </table>
            </section>
        }
    })
}

/// Everything that did not fit in its box on pages 1–2.
fn continuation_page(s: &Sheet, of: Overflow) -> impl IntoView {
    let attacks = (!of.attacks.is_empty()).then(|| {
        let rows = of
            .attacks
            .iter()
            .map(|a| view! { <tr><td class="strong">{a.name.clone()}</td><td class="c">{a.to_hit()}</td><td>{a.damage.clone()}</td><td>{a.notes.clone()}</td></tr> })
            .collect_view();
        view! {
            <section class="rp-group">
                <h3 class="rp-group-title">"Weapons & Damage Cantrips (continued)"</h3>
                <table class="rp-table">
                    <thead><tr><th>"Name"</th><th class="c">"Atk Bonus / DC"</th><th>"Damage & Type"</th><th>"Notes"</th></tr></thead>
                    <tbody>{rows}</tbody>
                </table>
            </section>
        }
    });
    let attuned = (!of.attuned.is_empty()).then(|| {
        view! {
            <section class="rp-group">
                <h3 class="rp-group-title">"Magic Item Attunement (continued)"</h3>
                <ul class="rp-list">{of.attuned.iter().map(|a| view! { <li>{a.clone()}</li> }).collect_view()}</ul>
            </section>
        }
    });
    let sections = of
        .sections
        .into_iter()
        .map(|(title, html)| {
            view! {
                <section class="rp-group">
                    <h3 class="rp-group-title">{format!("{title} (continued)")}</h3>
                    <div class="rp-flow" inner_html=html></div>
                </section>
            }
        })
        .collect_view();
    let body = view! {
        {attacks}
        {spell_table("Cantrips & Prepared Spells (continued)", of.spells)}
        {spell_table("Spellbook — not prepared", of.spellbook)}
        {attuned}
        {sections}
    };
    text_page(
        format!("{} — Continued", s.name),
        "Continuation page",
        body.into_any(),
        false,
    )
}
