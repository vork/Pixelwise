//! Right-side panel: exposure / tonemap / channel / clipping flag controls.

use leptos::ev;
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;

use crate::state::store::use_store;
use crate::state::view::{Channel, ClipFlags, Tonemap};

#[component]
pub fn InspectControls() -> impl IntoView {
    let store = use_store();

    let on_exposure = move |ev: ev::Event| {
        if let Some(t) = ev.target().and_then(|t| t.dyn_into::<HtmlInputElement>().ok()) {
            if let Ok(v) = t.value().parse::<f32>() {
                store.exposure.set(v);
            }
        }
    };

    view! {
        <section class="panel-inset p-2 space-y-2">
            <header class="flex items-center justify-between">
                <span class="label">"Exposure"</span>
                <span class="text-xs nums">{move || format!("{:+.2} EV", store.exposure.get())}</span>
            </header>
            <input
                type="range" min="-10" max="10" step="0.05"
                class="brand w-full"
                prop:value=move || store.exposure.get().to_string()
                on:input=on_exposure
            />
            <div class="flex justify-between text-[10px] text-muted nums">
                <button class="btn" on:click=move |_| store.exposure.set(0.0)>"0 EV"</button>
                <button class="btn" on:click=move |_| store.exposure.update(|v| *v -= 1.0)>"-1"</button>
                <button class="btn" on:click=move |_| store.exposure.update(|v| *v += 1.0)>"+1"</button>
            </div>

            <header class="flex items-center justify-between pt-1">
                <span class="label">"Tone mapping"</span>
            </header>
            <div class="grid grid-cols-5 gap-1">
                {Tonemap::ALL.iter().copied().map(|t| {
                    let active = Signal::derive(move || store.tonemap.get() == t);
                    view! {
                        <button
                            class=move || if active.get() { "btn btn-pri" } else { "btn" }
                            on:click=move |_| store.tonemap.set(t)>
                            {t.label()}
                        </button>
                    }
                }).collect_view()}
            </div>
        </section>
    }
}

#[component]
pub fn ChannelClipControls() -> impl IntoView {
    let store = use_store();
    view! {
        <section class="panel-inset p-2 space-y-2">
            <header class="label">"Channel"</header>
            <div class="grid grid-cols-5 gap-1">
                {Channel::ALL.iter().copied().map(|c| {
                    let active = Signal::derive(move || store.channel.get() == c);
                    view! {
                        <button
                            class=move || if active.get() { "btn btn-pri" } else { "btn" }
                            on:click=move |_| store.channel.set(c)>
                            {c.label()}
                        </button>
                    }
                }).collect_view()}
            </div>

            <header class="label pt-1">"Clipping overlays"</header>
            <div class="grid grid-cols-3 gap-1">
                <ClipBtn name="Over"  flag=ClipFlags::OVER />
                <ClipBtn name="Under" flag=ClipFlags::UNDER />
                <ClipBtn name="Neg"   flag=ClipFlags::NEGATIVE />
                <ClipBtn name="NaN"   flag=ClipFlags::NAN />
                <ClipBtn name="Inf"   flag=ClipFlags::INF />
                <ClipBtn name="Gamut" flag=ClipFlags::OUT_GAMUT />
            </div>
        </section>
    }
}

#[component]
fn ClipBtn(name: &'static str, flag: ClipFlags) -> impl IntoView {
    let store = use_store();
    let active = Signal::derive(move || store.clip.get().contains(flag));
    view! {
        <button
            class=move || if active.get() { "btn btn-pri" } else { "btn" }
            on:click=move |_| store.clip.update(|f| f.toggle(flag))>
            {name}
        </button>
    }
}
