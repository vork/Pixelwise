//! Pixelwise — HDR image inspection in the browser.

pub mod app;
pub mod color;
pub mod io;
pub mod metrics;
pub mod render;
pub mod state;
pub mod ui;

use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(log::Level::Info);
    log::info!("pixelwise starting");

    leptos::mount::mount_to(
        leptos::prelude::document()
            .get_element_by_id("app-root")
            .expect("#app-root not found")
            .unchecked_into(),
        app::App,
    )
    .forget();
}
