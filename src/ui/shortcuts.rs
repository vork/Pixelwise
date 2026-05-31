//! Global keyboard shortcuts + an (desktop-only) help overlay.
//!
//! Adapted from Framewise's bindings, keeping the ones that map onto a still-
//! image inspector (no playback/frame-stepping). A window-level `keydown`
//! listener drives the store; shortcuts are ignored while a form control is
//! focused or when a browser modifier (Ctrl/Cmd/Alt) is held.

use gloo_events::{EventListener, EventListenerOptions};
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::KeyboardEvent;

use crate::state::store::{use_store, Store};
use crate::state::view::{Channel, DiffMode, Tonemap, ViewMode};

/// Bindings shown in the help overlay: (keys, description).
const BINDINGS: &[(&str, &str)] = &[
    ("↑ / ↓", "Zoom in / out"),
    ("1 2 4 8", "Zoom 1:1 … 8:1"),
    ("R", "Reset · fit to viewport"),
    ("F", "Cycle view mode"),
    ("E", "Toggle Difference mode"),
    ("M", "Cycle diff metric"),
    ("C", "Cycle channel view"),
    ("T", "Cycle tone map"),
    ("Space", "Flicker: swap A / B"),
    ("[ / ]", "Exposure −/+ 0.5 EV"),
    ("{ / }", "Gamma −/+ 0.1"),
    ("0", "Reset exposure & gamma"),
    ("H", "Toggle HDR / SDR preview"),
    ("?", "Toggle this help"),
];

#[component]
pub fn KeyboardShortcuts() -> impl IntoView {
    let store = use_store();
    let help_open = RwSignal::new(false);

    Effect::new(move |_| {
        let win = web_sys::window().expect("window");
        let opts = EventListenerOptions::enable_prevent_default();
        let listener = EventListener::new_with_options(&win, "keydown", opts, move |ev| {
            let kev: &KeyboardEvent = ev.unchecked_ref();
            // Leave browser/OS combos and in-field typing alone.
            if kev.ctrl_key() || kev.meta_key() || kev.alt_key() {
                return;
            }
            if target_is_editable(kev) {
                return;
            }
            if handle_key(&store, help_open, kev) {
                ev.prevent_default();
            }
        });
        listener.forget();
    });

    view! {
        // Desktop only — there's no keyboard on touch devices.
        <button
            class="hidden lg:flex fixed bottom-2 right-2 z-30 btn w-7 h-7 p-0 items-center justify-center text-sm"
            title="Keyboard shortcuts (?)"
            on:click=move |_| help_open.update(|v| *v = !*v)>
            "?"
        </button>
        {move || help_open.get().then(|| view! {
            <div
                class="hidden lg:flex fixed inset-0 z-50 items-center justify-center bg-black/60 backdrop-blur-sm"
                on:click=move |_| help_open.set(false)>
                <div
                    class="panel p-4 max-w-md w-[28rem] shadow-soft"
                    on:click=|ev| ev.stop_propagation()>
                    <div class="flex items-center justify-between mb-3">
                        <span class="brand-text font-semibold text-base">"Keyboard shortcuts"</span>
                        <button class="btn px-2" on:click=move |_| help_open.set(false)>"Esc"</button>
                    </div>
                    <div class="grid grid-cols-1 gap-1">
                        {BINDINGS.iter().map(|(keys, desc)| view! {
                            <div class="flex items-center justify-between text-[12px] py-0.5">
                                <span class="text-muted">{*desc}</span>
                                <kbd class="nums chip text-text">{*keys}</kbd>
                            </div>
                        }).collect_view()}
                    </div>
                </div>
            </div>
        })}
    }
}

fn target_is_editable(ev: &KeyboardEvent) -> bool {
    let Some(t) = ev.target() else { return false };
    let Ok(el) = t.dyn_into::<web_sys::Element>() else { return false };
    matches!(el.tag_name().as_str(), "INPUT" | "SELECT" | "TEXTAREA")
}

/// Returns true if the key was handled (so the caller can `preventDefault`).
fn handle_key(store: &Store, help_open: RwSignal<bool>, ev: &KeyboardEvent) -> bool {
    let dpr = web_sys::window().map(|w| w.device_pixel_ratio()).unwrap_or(1.0) as f32;
    let zoom = |factor: f32| {
        store.camera.update(|c| c.zoom = (c.zoom * factor).clamp(0.01, 4096.0));
    };
    let preset = |n: f32| {
        store.camera.update(|c| c.zoom = (n * dpr).clamp(0.01, 4096.0));
    };

    match ev.key().as_str() {
        "ArrowUp" => zoom(1.25),
        "ArrowDown" => zoom(0.8),
        "1" => preset(1.0),
        "2" => preset(2.0),
        "4" => preset(4.0),
        "8" => preset(8.0),
        "r" | "R" => store.fit_request.update(|n| *n += 1),
        "f" | "F" => cycle_mode(store),
        "e" | "E" => toggle_diff(store),
        "m" | "M" => cycle(&store.diff_mode, DiffMode::ALL),
        "c" | "C" => cycle_channel(store),
        "t" | "T" => cycle(&store.tonemap, Tonemap::ALL),
        " " => store.flicker_a.update(|v| *v = !*v),
        "]" => store.exposure.update(|v| *v = (*v + 0.5).clamp(-10.0, 10.0)),
        "[" => store.exposure.update(|v| *v = (*v - 0.5).clamp(-10.0, 10.0)),
        "}" => store.tonemap_gamma.update(|v| *v = (*v + 0.1).clamp(1.0, 3.0)),
        "{" => store.tonemap_gamma.update(|v| *v = (*v - 0.1).clamp(1.0, 3.0)),
        "0" => {
            store.exposure.set(0.0);
            store.tonemap_gamma.set(2.2);
        }
        "h" | "H" => {
            if store.hdr_active.get_untracked() {
                store.hdr_enabled.update(|v| *v = !*v);
            }
        }
        "?" => help_open.update(|v| *v = !*v),
        "Escape" => help_open.set(false),
        _ => return false,
    }
    true
}

/// Cycle a `repr(u32)`-style enum signal through its `ALL` list.
fn cycle<T: Copy + PartialEq + Send + Sync + 'static>(sig: &RwSignal<T>, all: &[T]) {
    if all.is_empty() {
        return;
    }
    let cur = sig.get_untracked();
    let idx = all.iter().position(|m| *m == cur).unwrap_or(0);
    sig.set(all[(idx + 1) % all.len()]);
}

fn cycle_mode(store: &Store) {
    let has_b = store.secondary_image().is_some();
    let modes: Vec<ViewMode> = ViewMode::ALL
        .iter()
        .copied()
        .filter(|m| has_b || !m.needs_two_images())
        .collect();
    let cur = store.mode.get_untracked();
    let idx = modes.iter().position(|m| *m == cur).unwrap_or(0);
    store.mode.set(modes[(idx + 1) % modes.len()]);
}

fn toggle_diff(store: &Store) {
    if store.secondary_image().is_none() {
        return;
    }
    let next = if store.mode.get_untracked() == ViewMode::Difference {
        ViewMode::Split
    } else {
        ViewMode::Difference
    };
    store.mode.set(next);
}

fn cycle_channel(store: &Store) {
    let has_alpha = store.primary_image().map(|i| i.has_alpha).unwrap_or(false);
    let chans: Vec<Channel> = Channel::ALL
        .iter()
        .copied()
        .filter(|c| has_alpha || *c != Channel::Alpha)
        .collect();
    let cur = store.channel.get_untracked();
    let idx = chans.iter().position(|c| *c == cur).unwrap_or(0);
    store.channel.set(chans[(idx + 1) % chans.len()]);
}
