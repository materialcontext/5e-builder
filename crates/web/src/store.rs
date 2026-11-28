//! Browser persistence: characters in localStorage, JSON import/export.

use leptos::prelude::*;
use rules::Character;
use wasm_bindgen::{JsCast, JsValue};

const KEY: &str = "5e-builder.characters.v1";

pub type Store = RwSignal<Vec<Character>>;

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

pub fn load() -> Vec<Character> {
    storage()
        .and_then(|s| s.get_item(KEY).ok().flatten())
        .and_then(|txt| serde_json::from_str(&txt).ok())
        .unwrap_or_default()
}

pub fn save(chars: &[Character]) {
    if let (Some(s), Ok(txt)) = (storage(), serde_json::to_string(chars)) {
        let _ = s.set_item(KEY, &txt);
    }
}

pub fn now() -> String {
    js_sys::Date::new_0().to_iso_string().as_string().unwrap_or_default()
}

pub fn new_id() -> String {
    let r = (js_sys::Math::random() * 1e12) as u64;
    format!("{:x}{:x}", js_sys::Date::now() as u64, r)
}

/// Mutates one character in the store and stamps its update time.
pub fn update(store: Store, id: &str, f: impl FnOnce(&mut Character)) {
    store.update(|all| {
        if let Some(c) = all.iter_mut().find(|c| c.id == id) {
            f(c);
            c.updated = now();
        }
    });
}

/// Offers the character as a Foundry VTT (dnd5e) actor file. In Foundry:
/// create an actor, right-click it in the Actors sidebar, "Import Data".
pub fn download_foundry(c: &Character, comp: &compendium::Compendium) {
    download(&rules::foundry::file_name(c), &rules::foundry::actor_json(c, comp));
}

/// Offers `contents` as a file download.
pub fn download(filename: &str, contents: &str) {
    let Some(window) = web_sys::window() else { return };
    let Some(document) = window.document() else { return };
    let parts = js_sys::Array::of1(&JsValue::from_str(contents));
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type("application/json");
    let Ok(blob) = web_sys::Blob::new_with_str_sequence_and_options(&parts, &opts) else { return };
    let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) else { return };
    if let Ok(a) = document.create_element("a") {
        let a: web_sys::HtmlAnchorElement = a.unchecked_into();
        a.set_href(&url);
        a.set_download(filename);
        a.click();
    }
    let _ = web_sys::Url::revoke_object_url(&url);
}

/// Reads characters from an uploaded JSON file (a single character or a list).
pub async fn read_characters(file: web_sys::File) -> Result<Vec<Character>, String> {
    let text = wasm_bindgen_futures::JsFuture::from(file.text()).await.map_err(|_| "Could not read the file.".to_string())?;
    let text = text.as_string().unwrap_or_default();
    if let Ok(list) = serde_json::from_str::<Vec<Character>>(&text) {
        return Ok(list);
    }
    serde_json::from_str::<Character>(&text).map(|c| vec![c]).map_err(|e| format!("Not a character file: {e}"))
}

pub fn file_name(c: &Character) -> String {
    let base: String = c.name.chars().map(|ch| if ch.is_alphanumeric() { ch.to_ascii_lowercase() } else { '-' }).collect();
    let base = base.trim_matches('-');
    format!("{}.json", if base.is_empty() { "character" } else { base })
}
