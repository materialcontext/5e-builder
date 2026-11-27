//! 5e Builder web app (Leptos, client-side rendered).

mod app;
mod custom;
mod editor;
mod entries;
mod picker;
mod sheet;
mod store;

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(app::App);
}
