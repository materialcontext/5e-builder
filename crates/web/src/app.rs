//! App shell: compendium loading, hash routing and the character manager.

use crate::editor::Editor;
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
        None => view! { <div class="loading">"Loading compendium…"</div> }.into_any(),
        Some(Err(e)) => view! { <div class="page error"><h2>"Could not load data"</h2><p>{e}</p></div> }.into_any(),
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
                    <p>"No characters yet."</p>
                    <p class="muted">"Create one with the step-by-step wizard, or import a JSON file."</p>
                </div>
            }
            .into_any();
        }
        chars
            .into_iter()
            .map(|c| {
                let b = rules::build(&c, &comp);
                let s = rules::sheet(&c, &b, &comp);
                let pending = rules::issues(&c, &b, &comp).len();
                let (id1, id2, id3, id4) = (c.id.clone(), c.id.clone(), c.id.clone(), c.id.clone());
                let c_export = c.clone();
                let name = s.name.clone();
                view! {
                    <article class="char-card">
                        <div class="char-main">
                            <h3><a href=format!("#/edit/{id1}/basics")>{s.name.clone()}</a></h3>
                            <p>{if s.summary.is_empty() { "Not started".to_string() } else { format!("Level {} · {}", s.level, s.summary) }}</p>
                            <p class="muted small">{s.background.clone()}{(pending > 0).then(|| format!(" · {pending} unfinished"))}</p>
                        </div>
                        <div class="char-actions">
                            <a class="button primary" href=format!("#/sheet/{id2}")>"Sheet"</a>
                            <a class="button ghost" href=format!("#/edit/{id3}/class")>"Edit"</a>
                            <button class="button ghost" on:click=move |_| {
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
                            <button class="button ghost" on:click=move |_| store::download(&store::file_name(&c_export), &serde_json::to_string_pretty(&c_export).unwrap_or_default())>"Export"</button>
                            <button class="button danger" on:click=move |_| {
                                let ok = web_sys::window().and_then(|w| w.confirm_with_message(&format!("Delete {name}? This cannot be undone.")).ok()).unwrap_or(false);
                                if ok {
                                    let id = c.id.clone();
                                    store.update(|all| all.retain(|x| x.id != id));
                                }
                            }>"Delete"</button>
                        </div>
                    </article>
                }
            })
            .collect_view()
            .into_any()
    };

    let version = comp.data_version.clone();
    view! {
        <div class="page home">
            <header class="home-head">
                <h1>"⚔ 5e Builder"</h1>
                <p class="muted">"Build, manage and print D&D 5e characters (2024 rules, with legacy options)."</p>
            </header>
            <div class="row">
                <button class="button primary big" on:click=create>"New character"</button>
                <label class="button ghost">"Import JSON"<input type="file" accept=".json,application/json" class="hidden" on:change=import /></label>
                <button class="button ghost" disabled=move || store.with(|s| s.is_empty())
                    on:click=move |_| store::download("characters.json", &serde_json::to_string_pretty(&store.get_untracked()).unwrap_or_default())>"Export all"</button>
            </div>
            {move || message.get().map(|m| view! { <p class="note">{m}</p> })}
            <div class="char-list">{list}</div>
            <footer class="muted small">{format!("Data: 5etools {version}. Characters are stored in this browser; export them to keep a backup.")}</footer>
        </div>
    }
}
