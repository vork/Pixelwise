//! Top-level layout: header, left image list, center viewport, right panel.

use leptos::prelude::*;

use crate::state::store::use_store;
use crate::state::view::ViewMode;
use crate::ui::compare::CompareControls;
use crate::ui::controls::{ChannelClipControls, InspectControls};
use crate::ui::drop_zone::DropZone;
use crate::ui::histogram::HistogramPanel;
use crate::ui::metrics_panel::MetricsPanel;
use crate::ui::pixel_probe::PixelProbe;
use crate::ui::sanity_panel::SanityPanel;
use crate::ui::viewport::Viewport;

#[component]
pub fn AppShell() -> impl IntoView {
    let store = use_store();

    let mode_label = Signal::derive(move || store.mode.get().label().to_string());

    view! {
        <div class="h-screen w-screen flex flex-col text-text bg-bg overflow-hidden">
            <TopBar />
            <div class="flex-1 min-h-0 grid grid-cols-[260px_1fr_320px] gap-2 p-2">
                <aside class="panel flex flex-col min-h-0 overflow-hidden">
                    <div class="px-3 pt-2 pb-1 flex items-center justify-between">
                        <span class="label">"Images"</span>
                        <span class="text-[10px] text-muted nums">
                            {move || format!("{} loaded", store.images.with(|v| v.len()))}
                        </span>
                    </div>
                    <ImageList />
                    <div class="p-2 mt-auto">
                        <DropZone />
                    </div>
                </aside>

                <main class="panel relative flex flex-col min-h-0 overflow-hidden">
                    <ModeBar />
                    <div class="relative flex-1 min-h-0">
                        <Viewport />
                        <PixelProbe />
                    </div>
                </main>

                <aside class="panel flex flex-col min-h-0 overflow-hidden scroll-thin overflow-y-auto">
                    {move || if store.primary_image().is_some() {
                        view! {
                            <div class="p-2 space-y-2">
                                <InspectControls />
                                <ChannelClipControls />
                                {move || if store.mode.get().needs_two_images() {
                                    view! { <CompareControls /> }.into_any()
                                } else {
                                    view! { <div /> }.into_any()
                                }}
                                <HistogramPanel />
                                {move || if matches!(store.mode.get(), ViewMode::Metrics | ViewMode::Difference) {
                                    view! { <MetricsPanel /> }.into_any()
                                } else {
                                    view! { <div /> }.into_any()
                                }}
                                <SanityPanel />
                            </div>
                        }.into_any()
                    } else {
                        view! { <EmptyRight /> }.into_any()
                    }}
                </aside>
            </div>
            <StatusBar mode_label=mode_label />
        </div>
    }
}

#[component]
fn TopBar() -> impl IntoView {
    let store = use_store();
    let hdr = move || store.hdr_active.get();
    let gpu = move || store.gpu_available.get();

    view! {
        <header class="h-12 flex items-center justify-between px-3 border-b border-border bg-panel">
            <div class="flex items-center gap-3">
                <img src="icon-1024.png" class="w-7 h-7 rounded-md" alt="" />
                <div class="flex items-baseline gap-2">
                    <span class="brand-text font-semibold text-[15px] tracking-tight">"Pixelwise"</span>
                    <span class="text-muted text-xs">"HDR image inspection"</span>
                </div>
            </div>
            <div class="flex items-center gap-2">
                {move || match gpu() {
                    Some(true) => view! { <span class="chip-ok">"WebGPU"</span> }.into_any(),
                    Some(false) => view! { <span class="chip-err">"No WebGPU"</span> }.into_any(),
                    None => view! { <span class="chip">"Booting…"</span> }.into_any(),
                }}
                {move || if hdr() {
                    view! { <span class="chip-ok">"HDR · P3"</span> }.into_any()
                } else {
                    view! { <span class="chip">"SDR"</span> }.into_any()
                }}
                <a class="btn" href="https://github.com/vork/Pixelwise" target="_blank" rel="noreferrer">"Repo"</a>
            </div>
        </header>
    }
}

#[component]
fn StatusBar(mode_label: Signal<String>) -> impl IntoView {
    let store = use_store();
    view! {
        <footer class="h-7 px-3 flex items-center justify-between text-[11px] text-muted border-t border-border bg-panel">
            <div class="flex items-center gap-3 nums">
                <span>{move || mode_label.get()}</span>
                <span>{move || {
                    if let Some(img) = store.primary_image() {
                        format!("{} × {} · {} · {}", img.width, img.height, img.format_label, img.color_space.label())
                    } else { String::from("no image") }
                }}</span>
            </div>
            <div class="flex items-center gap-3 nums">
                <span>{move || format!("zoom {:.2}×", store.camera.get().zoom)}</span>
                <span>{move || format!("EV {:+.2}", store.exposure.get())}</span>
            </div>
        </footer>
    }
}

#[component]
fn EmptyRight() -> impl IntoView {
    view! {
        <div class="p-4 text-xs text-muted">
            "Load an image to see controls, histogram, metrics, and HDR sanity checks."
        </div>
    }
}

#[component]
fn ImageList() -> impl IntoView {
    let store = use_store();
    view! {
        <ul class="px-2 pb-2 space-y-1 overflow-y-auto scroll-thin">
            {move || {
                let images = store.images.get();
                images.iter().enumerate().map(|(idx, img)| {
                    let img = img.clone();
                    let name = img.name.clone();
                    let name_title = name.clone();
                    let format_label = img.format_label;
                    let w = img.width;
                    let h = img.height;
                    let primary = Signal::derive(move || store.primary.get() == Some(idx));
                    let secondary = Signal::derive(move || store.secondary.get() == Some(idx));
                    view! {
                        <li class="group panel-inset px-2 py-1.5 flex items-center justify-between gap-2">
                            <div class="min-w-0 flex-1">
                                <div class="text-xs truncate" title=name_title>{name}</div>
                                <div class="text-[10px] text-muted nums">
                                    {format!("{}×{} · {}", w, h, format_label)}
                                </div>
                            </div>
                            <div class="flex flex-col gap-0.5">
                                <button
                                    class=move || if primary.get() { "btn btn-pri" } else { "btn" }
                                    on:click=move |_| store.primary.set(Some(idx))
                                    title="Set as primary (A)">
                                    "A"
                                </button>
                                <button
                                    class=move || if secondary.get() { "btn btn-pri" } else { "btn" }
                                    on:click=move |_| store.secondary.set(Some(idx))
                                    title="Set as secondary (B)">
                                    "B"
                                </button>
                            </div>
                        </li>
                    }
                }).collect_view()
            }}
        </ul>
    }
}

#[component]
fn ModeBar() -> impl IntoView {
    let store = use_store();
    view! {
        <div class="h-10 px-2 border-b border-border flex items-center gap-2">
            <div class="seg">
                {ViewMode::ALL.iter().copied().map(|m| {
                    let active = Signal::derive(move || store.mode.get() == m);
                    let needs_two = m.needs_two_images();
                    let disabled = Signal::derive(move || needs_two && store.secondary_image().is_none());
                    view! {
                        <button
                            class=move || if active.get() { "active" } else { "" }
                            disabled=move || disabled.get()
                            on:click=move |_| store.mode.set(m)>
                            {m.label()}
                        </button>
                    }
                }).collect_view()}
            </div>
            <div class="ml-auto text-[11px] text-muted nums">
                {move || {
                    let p = store.primary_image().map(|i| i.name.clone()).unwrap_or_default();
                    let s = store.secondary_image().map(|i| i.name.clone()).unwrap_or_default();
                    if s.is_empty() { format!("A: {}", p) } else { format!("A: {}   B: {}", p, s) }
                }}
            </div>
        </div>
    }
}
