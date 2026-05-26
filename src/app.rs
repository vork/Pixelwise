use leptos::prelude::*;

use crate::io::url::load_from_query;
use crate::state::store::{provide_store, use_store, Store};
use crate::ui::shell::AppShell;

#[component]
pub fn App() -> impl IntoView {
    provide_store(Store::new());

    // Kick off any `?a=&b=&img=` URL fetches on first mount.
    Effect::new(move |_| {
        let store = use_store();
        load_from_query(store);
    });

    view! {
        <AppShell />
    }
}
