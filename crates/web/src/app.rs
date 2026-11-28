//! App shell: compendium loading, hash routing and the character manager.

use crate::editor::{icon, Editor};
use crate::sheet::SheetPage;
use crate::store::{self, Store};
use compendium::Compendium;
use leptos::prelude::*;
use rules::Character;
use std::sync::Arc;
use wasm_bindgen::JsCast;

#[derive(Clone, Debug, PartialEq)]
pub enum Route {
    Home,
    Edit(String, String),
    Sheet(String),
}

impl Route {
    fn parse(hash: &str) -> Route {
        let parts: Vec<&str> = hash.trim_start_matches('#').trim_start_matches('/').split('/').collect();
        match parts.as_slice() {
            ["edit", id] => Route::Edit(id.to_string(), "basics".into()),
            ["edit", id, step, ..] => Route::Edit(id.to_string(), step.to_string()),
            ["sheet", id, ..] => Route::Sheet(id.to_string()),
            _ => Route::Home,
        }
    }
    fn href(&self) -> String {
        match self {
            Route::Home => "#/".into(),
            Route::Edit(id, step) => format!("#/edit/{id}/{step}"),
            Route::Sheet(id) => format!("#/sheet/{id}"),
        }
    }
}

fn current_hash() -> String {
    web_sys::window().and_then(|w| w.location().hash().ok()).unwrap_or_default()
}

pub fn go(r: Route) {
    if let Some(w) = web_sys::window() {
        let _ = w.location().set_hash(&r.href());
    }
}

async fn fetch_compendium() -> Result<Compendium, String> {
    let window = web_sys::window().ok_or("no window")?;
    let resp = wasm_bindgen_futures::JsFuture::from(window.fetch_with_str("compendium.json")).await.map_err(|_| "Network error loading compendium.json")?;
    let resp: web_sys::Response = resp.dyn_into().map_err(|_| "Bad response")?;
    if !resp.ok() {
        return Err(format!("compendium.json: HTTP {}. Run the importer first (see README).", resp.status()));
    }
    let text = wasm_bindgen_futures::JsFuture::from(resp.text().map_err(|_| "Bad body")?).await.map_err(|_| "Bad body")?;
    Compendium::from_json(&text.as_string().unwrap_or_default()).map_err(|e| format!("Invalid compendium.json: {e}"))
}

#[component]
pub fn App() -> impl IntoView {
    let compendium = LocalResource::new(fetch_compendium);
    let store: Store = RwSignal::new(store::load());
    provide_context(store);
    Effect::new(move |_| store.with(|s| store::save(s)));

    let route = RwSignal::new(Route::parse(&current_hash()));
    let handle = window_event_listener(leptos::ev::hashchange, move |_| {
        route.set(Route::parse(&current_hash()));
        if let Some(w) = web_sys::window() {
            w.scroll_to_with_x_and_y(0.0, 0.0);
        }
    });
    on_cleanup(move || handle.remove());

    move || match compendium.get() {
        None => view! {
            <main class="loading" aria-busy="true">
                <div class="loading-card">
                    {logo()}
                    <h1>"5e Builder"</h1>
                    <div class="progress-indeterminate" role="progressbar" aria-label="Loading rules compendium"><span></span></div>
                    <p class="muted">"Loading the rules compendium (about 1.3 MB)…"</p>
                </div>
            </main>
        }
        .into_any(),
        Some(Err(e)) => view! {
            <main class="loading">
                <div class="loading-card" role="alert">
                    {logo()}
                    <h1>"Couldn't load the rules"</h1>
                    <p>{e}</p>
                    <button class="button primary" on:click=move |_| compendium.refetch()>"Try again"</button>
                </div>
            </main>
        }
        .into_any(),
        Some(Ok(c)) => {
            let comp = Arc::new(c);
            provide_context(comp.clone());
            view! { <Routed route=route /> }.into_any()
        }
    }
}

#[component]
fn Routed(route: RwSignal<Route>) -> impl IntoView {
    move || match route.get() {
        Route::Home => view! { <Home /> }.into_any(),
        Route::Edit(id, step) => view! { <Editor id=id step=step /> }.into_any(),
        Route::Sheet(id) => view! { <SheetPage id=id /> }.into_any(),
    }
}

/// Brand mark: a stylised d20.
pub fn logo() -> impl IntoView {
    view! {
        <svg class="logo" viewBox="0 0 32 32" aria-hidden="true" focusable="false">
            <path d="M16 2 28 9v14l-12 7-12-7V9z" fill="var(--accent)" />
            <path d="M16 2 28 9 16 13 4 9zM16 13v17M4 9l12 4 12-4M4 23l12-10 12 10" fill="none" stroke="var(--accent-ink)" stroke-width="1.4" stroke-linejoin="round" opacity=".85" />
        </svg>
    }
}

/// Stable hue per class for avatars (decorative).
fn avatar_hue(s: &str) -> u32 {
    s.bytes().fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32)) % 360
}

fn initials(name: &str) -> String {
    let i: String = name.split_whitespace().filter_map(|w| w.chars().next()).take(2).collect();
    if i.is_empty() { "?".into() } else { i.to_uppercase() }
}

#[component]
fn Home() -> impl IntoView {
    let store = expect_context::<Store>();
    let comp = expect_context::<Arc<Compendium>>();
    let message = RwSignal::new(None::<String>);

    let create = move |_| {
        let mut c = Character::new(store::new_id());
        c.created = store::now();
        c.updated = c.created.clone();
        let id = c.id.clone();
        store.update(|s| s.push(c));
        go(Route::Edit(id, "basics".into()));
    };

    let import = move |ev: leptos::ev::Event| {
        let input: web_sys::HtmlInputElement = event_target(&ev);
        let Some(file) = input.files().and_then(|f| f.get(0)) else { return };
        input.set_value("");
        leptos::task::spawn_local(async move {
            match store::read_characters(file).await {
                Ok(list) => {
                    let n = list.len();
                    store.update(|s| {
                        for mut c in list {
                            if c.id.is_empty() || s.iter().any(|x| x.id == c.id) {
                                c.id = store::new_id();
                            }
                            s.push(c);
                        }
                    });
                    message.set(Some(format!("Imported {n} character{}.", if n == 1 { "" } else { "s" })));
                }
                Err(e) => message.set(Some(e)),
            }
        });
    };

    let c1 = comp.clone();
    let list = move || {
        let comp = c1.clone();
        let mut chars = store.get();
        chars.sort_by(|a, b| b.updated.cmp(&a.updated));
        if chars.is_empty() {
            return view! {
                <div class="empty">
                    <svg class="empty-art" viewBox="0 0 120 90" aria-hidden="true">
                        <rect x="20" y="10" width="62" height="76" rx="6" fill="var(--surface)" stroke="var(--line-strong)" stroke-width="2" />
                        <path d="M32 28h38M32 40h30M32 52h34M32 64h22" stroke="var(--line-strong)" stroke-width="3" stroke-linecap="round" />
                        <path d="M86 26l12 7v14l-12 7-12-7V33z" fill="var(--accent)" />
                    </svg>
                    <h2>"No characters yet"</h2>
                    <p class="muted">"Create one with the step-by-step builder. It takes about ten minutes, and you can save, edit and print it any time."</p>
                    <button class="button primary big" on:click=create>{icon("plus")}"Create your first character"</button>
                </div>
            }
            .into_any();
        }
        view! {
            <ul class="char-grid" role="list">
                {chars
                    .into_iter()
                    .map(|c| {
                        let b = rules::build(&c, &comp);
                        let s = rules::sheet(&c, &b, &comp);
                        let pending = rules::issues(&c, &b, &comp).len();
                        let (id1, id2, id3, id4) = (c.id.clone(), c.id.clone(), c.id.clone(), c.id.clone());
                        let c_export = c.clone();
                        let (c_foundry, comp_foundry) = (c.clone(), comp.clone());
                        let name = s.name.clone();
                        let hue = avatar_hue(&s.class_line);
                        let level_up = (s.level < 20 && !c.levels.is_empty()).then(|| {
                            let id = c.id.clone();
                            let last = c.levels.last().map(|l| l.class.clone());
                            view! {
                                <button class="menu-item" on:click=move |_| {
                                    if let Some(k) = last.clone() {
                                        store::update(store, &id, |c| c.level_up(k));
                                        go(Route::Edit(id.clone(), "class".into()));
                                    }
                                }>"Level up"</button>
                            }
                        });
                        view! {
                            <li class="char-card">
                                <a class="char-main" href=format!("#/edit/{id1}/basics")>
                                    <span class="avatar" style=format!("--hue: {hue}") aria-hidden="true">{initials(&s.name)}</span>
                                    <span class="char-text">
                                        <span class="char-name">{s.name.clone()}</span>
                                        <span class="char-line">{if s.summary.is_empty() { "Not started".to_string() } else { format!("Level {} · {}", s.level, s.summary) }}</span>
                                        <span class="char-meta">{s.background.clone()}</span>
                                    </span>
                                </a>
                                <div class="char-foot">
                                    {if pending == 0 {
                                        view! { <span class="chip ok">{icon("check")}"Ready"</span> }.into_any()
                                    } else {
                                        view! { <span class="chip warn">{format!("{pending} to do")}</span> }.into_any()
                                    }}
                                    <span class="spacer"></span>
                                    <a class="button small" href=format!("#/sheet/{id2}")>"Sheet"</a>
                                    <a class="button small ghost" href=format!("#/edit/{id3}/class")>"Edit"</a>
                                    <details class="menu">
                                        <summary class="icon-button" aria-label=format!("More actions for {name}")>"⋯"</summary>
                                        <div class="menu-list">
                                            {level_up}
                                            <button class="menu-item" on:click=move |_| {
                                                store.update(|all| {
                                                    if let Some(orig) = all.iter().find(|x| x.id == id4).cloned() {
                                                        let mut copy = orig;
                                                        copy.id = store::new_id();
                                                        copy.name = format!("{} (copy)", copy.name);
                                                        copy.updated = store::now();
                                                        all.push(copy);
                                                    }
                                                })
                                            }>"Duplicate"</button>
                                            <button class="menu-item" on:click=move |_| store::download(&store::file_name(&c_export), &serde_json::to_string_pretty(&c_export).unwrap_or_default())>"Export JSON"</button>
                                            <button class="menu-item" title="Actor file for Foundry VTT's D&D 5e system: right-click an actor, Import Data" on:click=move |_| store::download_foundry(&c_foundry, &comp_foundry)>"Export for Foundry VTT"</button>
                                            <button class="menu-item danger" on:click=move |_| {
                                                let ok = web_sys::window().and_then(|w| w.confirm_with_message(&format!("Delete {name}? This cannot be undone.")).ok()).unwrap_or(false);
                                                if ok {
                                                    let id = c.id.clone();
                                                    store.update(|all| all.retain(|x| x.id != id));
                                                }
                                            }>"Delete…"</button>
                                        </div>
                                    </details>
                                </div>
                            </li>
                        }
                    })
                    .collect_view()}
            </ul>
        }
        .into_any()
    };

    let version = comp.data_version.clone();
    view! {
        <div class="home">
            <header class="site-head">
                <a class="brand" href="#/" aria-label="5e Builder home">{logo()}<span class="brand-name" aria-hidden="true">"5e Builder"</span></a>
            </header>
            <main id="main" class="page">
                <section class="hero">
                    <h1>"Build, manage and print D&D characters"</h1>
                    <p class="lede">"A guided builder for the 2024 rules, with legacy options. Every choice is tracked, every number is calculated, and the sheet is ready to print."</p>
                    <div class="row">
                        <button class="button primary big" on:click=create>{icon("plus")}"New character"</button>
                        <label class="button ghost big">{icon("upload")}"Import JSON"<input type="file" accept=".json,application/json" class="visually-hidden" on:change=import /></label>
                    </div>
                </section>
                {move || message.get().map(|m| view! { <p class="note" role="status">{m}</p> })}
                <section aria-labelledby="chars-title">
                    <div class="section-head">
                        <h2 id="chars-title">"Your characters"</h2>
                        <button class="button ghost small" disabled=move || store.with(|s| s.is_empty())
                            on:click=move |_| store::download("characters.json", &serde_json::to_string_pretty(&store.get_untracked()).unwrap_or_default())>{icon("download")}"Export all"</button>
                    </div>
                    {list}
                </section>
            </main>
            <footer class="site-foot">
                <p>{format!("Rules data: 5etools {version}. Characters are saved in this browser only — export them to keep a backup.")}</p>
                <p>"Unofficial fan content. Not approved or endorsed by Wizards of the Coast. Dungeons & Dragons and D&D are trademarks of Wizards of the Coast LLC."</p>
            </footer>
        </div>
    }
}
