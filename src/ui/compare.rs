//! Comparison-mode controls (slider, flicker rate, onion alpha, diff scale).

use std::cell::RefCell;
use std::rc::Rc;

use gloo_timers::callback::Interval;
use leptos::ev;
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;

use crate::state::store::use_store;
use crate::state::view::{DiffMode, ViewMode};

#[component]
pub fn CompareControls() -> impl IntoView {
    let store = use_store();

    // Flicker mode timer. `Interval` isn't Send, so we hold it in an Rc<RefCell>
    // rather than a Leptos StoredValue (which defaults to SyncStorage).
    let interval: Rc<RefCell<Option<Interval>>> = Rc::new(RefCell::new(None));
    {
        let interval = interval.clone();
        Effect::new(move |_| {
            let active = matches!(store.mode.get(), ViewMode::Flicker);
            if active && interval.borrow().is_none() {
                let s = store;
                *interval.borrow_mut() = Some(Interval::new(330, move || {
                    s.flicker_a.update(|v| *v = !*v);
                }));
            } else if !active {
                *interval.borrow_mut() = None;
            }
        });
    }

    let on_split = move |ev: ev::Event| {
        if let Some(t) = ev.target().and_then(|t| t.dyn_into::<HtmlInputElement>().ok()) {
            if let Ok(v) = t.value().parse::<f32>() {
                store.split_pos.set(v);
            }
        }
    };
    let on_onion = move |ev: ev::Event| {
        if let Some(t) = ev.target().and_then(|t| t.dyn_into::<HtmlInputElement>().ok()) {
            if let Ok(v) = t.value().parse::<f32>() {
                store.onion_alpha.set(v);
            }
        }
    };

    view! {
        <section class="panel-inset p-2 space-y-2">
            <header class="flex items-center justify-between">
                <span class="label">"Compare"</span>
                <span class="text-[10px] text-muted nums">{move || store.mode.get().label()}</span>
            </header>

            {move || match store.mode.get() {
                ViewMode::Split => view! {
                    <div class="space-y-1">
                        <div class="flex justify-between text-[10px] text-muted nums">
                            <span>"A"</span>
                            <span>{format!("{:.0}%", store.split_pos.get() * 100.0)}</span>
                            <span>"B"</span>
                        </div>
                        <input type="range" min="0" max="1" step="0.005" class="brand w-full"
                               prop:value=move || store.split_pos.get().to_string()
                               on:input=on_split />
                    </div>
                }.into_any(),
                ViewMode::OnionSkin => view! {
                    <div class="space-y-1">
                        <div class="flex justify-between text-[10px] text-muted nums">
                            <span>"A"</span>
                            <span>{format!("α {:.2}", store.onion_alpha.get())}</span>
                            <span>"B"</span>
                        </div>
                        <input type="range" min="0" max="1" step="0.005" class="brand w-full"
                               prop:value=move || store.onion_alpha.get().to_string()
                               on:input=on_onion />
                    </div>
                }.into_any(),
                ViewMode::Flicker => view! {
                    <div class="text-[11px] text-muted nums">
                        "Showing: "
                        <span class="text-text">{if store.flicker_a.get() { "A" } else { "B" }}</span>
                    </div>
                }.into_any(),
                ViewMode::Difference => view! {
                    <div class="grid grid-cols-4 gap-1">
                        {DiffMode::ALL.iter().copied().map(|m| {
                            let active = Signal::derive(move || store.diff_mode.get() == m);
                            view! {
                                <button
                                    class=move || if active.get() { "btn btn-pri" } else { "btn" }
                                    on:click=move |_| store.diff_mode.set(m)>
                                    {m.label()}
                                </button>
                            }
                        }).collect_view()}
                    </div>
                }.into_any(),
                _ => view! { <div /> }.into_any(),
            }}
        </section>
    }
}
