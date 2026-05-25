use leptos::prelude::*;

use crate::state::store::{provide_store, Store};
use crate::ui::shell::AppShell;

#[component]
pub fn App() -> impl IntoView {
    provide_store(Store::new());

    view! {
        <AppShell />
    }
}
