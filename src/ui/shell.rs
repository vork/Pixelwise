//! Top-level layout: header, left image list, center viewport, right panel.

use leptos::prelude::*;

use crate::state::store::use_store;
use crate::state::view::ViewMode;
use crate::ui::compare::CompareControls;
use crate::ui::controls::{ChannelClipControls, ChannelMapPanel, InspectControls, LutPanel};
use crate::ui::drop_zone::DropZone;
use crate::ui::histogram::HistogramPanel;
use crate::ui::metrics_panel::MetricsPanel;
use crate::ui::pixel_probe::PixelProbe;
use crate::ui::sanity_panel::SanityPanel;
use crate::ui::shortcuts::KeyboardShortcuts;
use crate::ui::viewport::Viewport;

#[component]
pub fn AppShell() -> impl IntoView {
    let store = use_store();

    let mode_label = Signal::derive(move || store.mode.get().label().to_string());

    // The right-panel conditionals below need to fire only when the *boolean*
    // they depend on changes — not every time `images` or `primary` mutates.
    // Without Memo, a channel-mapping update or any image swap would tear down
    // and remount the entire ChannelMap/InspectControls/SanityPanel subtree,
    // which disposes their internal Effects mid-frame and panics the next time
    // those Effects fire. The disposed-Effect panic crashes the wgpu render
    // pass mid-recording, freezing the canvas on whatever was last drawn.
    let has_primary = Memo::new(move |_| store.primary_image().is_some());
    let mode_is_diff = Memo::new(move |_| matches!(store.mode.get(), ViewMode::Difference));
    let mode_needs_two = Memo::new(move |_| store.mode.get().needs_two_images());
    let mode_shows_metrics = Memo::new(move |_| {
        matches!(store.mode.get(), ViewMode::Metrics | ViewMode::Difference)
    });

    // Drawer state for the small-screen layout. On lg+ the panels are static
    // columns (CSS handles that); these only drive the mobile slide-overs.
    let left_open = RwSignal::new(false);
    let right_open = RwSignal::new(false);

    let left_class = move || {
        let base = "panel drawer drawer-left flex flex-col min-h-0 overflow-hidden \
                    lg:w-[260px] lg:flex-shrink-0 lg:translate-x-0";
        if left_open.get() { format!("{base} translate-x-0") } else { format!("{base} -translate-x-full") }
    };
    let right_class = move || {
        let base = "panel drawer drawer-right flex flex-col min-h-0 overflow-hidden scroll-thin \
                    overflow-y-auto lg:w-[320px] lg:flex-shrink-0 lg:translate-x-0";
        if right_open.get() { format!("{base} translate-x-0") } else { format!("{base} translate-x-full") }
    };
    let backdrop_open = move || left_open.get() || right_open.get();

    view! {
        <div class="h-screen w-screen flex flex-col text-text bg-bg overflow-hidden">
            <TopBar left_open=left_open right_open=right_open />
            <div class="flex-1 min-h-0 flex gap-0 lg:gap-2 lg:p-2 relative">
                // Backdrop behind open drawers (mobile only).
                <div
                    class="fixed left-0 right-0 top-12 bottom-7 z-30 bg-black/50 backdrop-blur-[1px] lg:hidden"
                    class:hidden=move || !backdrop_open()
                    on:click=move |_| { left_open.set(false); right_open.set(false); }
                />

                <aside class=left_class>
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

                <main class="panel relative flex flex-col min-h-0 overflow-hidden flex-1 min-w-0">
                    <ModeBar />
                    <div class="relative flex-1 min-h-0">
                        <Viewport />
                        <PixelProbe />
                        {move || if store.gpu_available.get() == Some(false) {
                            view! { <NoGpuFallback /> }.into_any()
                        } else {
                            view! { <div /> }.into_any()
                        }}
                    </div>
                </main>

                <aside class=right_class>
                    {move || if has_primary.get() {
                        view! {
                            <div class="p-2 space-y-2">
                                // Exposure / tonemap / channel / clipping only apply to the
                                // per-image display path; Difference mode renders a diff
                                // heatmap that ignores them, so hide them there.
                                {move || if mode_is_diff.get() {
                                    view! { <div /> }.into_any()
                                } else {
                                    view! {
                                        <>
                                            <ChannelMapPanel />
                                            <InspectControls />
                                            <LutPanel />
                                            <ChannelClipControls />
                                        </>
                                    }.into_any()
                                }}
                                {move || if mode_needs_two.get() {
                                    view! { <CompareControls /> }.into_any()
                                } else {
                                    view! { <div /> }.into_any()
                                }}
                                <HistogramPanel />
                                {move || if mode_shows_metrics.get() {
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
            <KeyboardShortcuts />
        </div>
    }
}

#[component]
fn TopBar(left_open: RwSignal<bool>, right_open: RwSignal<bool>) -> impl IntoView {
    let store = use_store();
    let hdr = move || store.hdr_active.get();
    let gpu = move || store.gpu_available.get();

    view! {
        <header class="h-12 flex items-center justify-between px-2 sm:px-3 gap-2 border-b border-border bg-panel">
            <div class="flex items-center gap-2 min-w-0">
                // Mobile: toggle the Images drawer.
                <button
                    class="btn px-2 lg:hidden"
                    title="Images"
                    on:click=move |_| { right_open.set(false); left_open.update(|v| *v = !*v); }>
                    "☰"
                </button>
                <img src="icon-1024.png" class="w-7 h-7 rounded-md shrink-0" alt="" />
                <div class="flex items-baseline gap-2 min-w-0">
                    <span class="brand-text font-semibold text-[15px] tracking-tight">"Pixelwise"</span>
                    <span class="text-muted text-xs hidden md:inline">"HDR image inspection"</span>
                </div>
            </div>
            <div class="flex items-center gap-2">
                {move || match gpu() {
                    Some(true) => view! { <span class="chip-ok hidden sm:inline-flex">"WebGPU"</span> }.into_any(),
                    Some(false) => view! { <span class="chip-err">"No WebGPU"</span> }.into_any(),
                    None => view! { <span class="chip">"Booting…"</span> }.into_any(),
                }}
                {move || if hdr() {
                    // HDR canvas available — let the user flip to an SDR
                    // preview (tone-mapped) to inspect the non-HDR look.
                    let on = move || store.hdr_enabled.get();
                    view! {
                        <button
                            class=move || if on() { "chip-ok cursor-pointer" } else { "chip-warn cursor-pointer" }
                            title="Toggle HDR output vs. SDR preview"
                            on:click=move |_| store.hdr_enabled.update(|v| *v = !*v)>
                            {move || if on() { "HDR · P3" } else { "SDR preview" }}
                        </button>
                    }.into_any()
                } else {
                    view! { <span class="chip hidden sm:inline-flex">"SDR"</span> }.into_any()
                }}
                <a class="btn hidden lg:inline-flex" href="https://github.com/vork/Pixelwise" target="_blank" rel="noreferrer">"Repo"</a>
                // Mobile: toggle the controls drawer.
                <button
                    class="btn px-2 lg:hidden"
                    title="Controls"
                    on:click=move |_| { left_open.set(false); right_open.update(|v| *v = !*v); }>
                    "⚙"
                </button>
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
                                    on:click=move |_| store.toggle_primary(idx)
                                    title="Toggle as primary (A)">
                                    "A"
                                </button>
                                <button
                                    class=move || if secondary.get() { "btn btn-pri" } else { "btn" }
                                    on:click=move |_| store.toggle_secondary(idx)
                                    title="Toggle as secondary (B)">
                                    "B"
                                </button>
                            </div>
                            <button
                                class="btn text-muted hover:text-err px-1.5"
                                on:click=move |_| store.remove_image(idx)
                                title="Unload image">
                                "×"
                            </button>
                        </li>
                    }
                }).collect_view()
            }}
        </ul>
    }
}

#[component]
fn NoGpuFallback() -> impl IntoView {
    view! {
        <div class="absolute inset-0 flex items-center justify-center p-6 pointer-events-none">
            <div class="panel-inset max-w-md p-5 text-sm space-y-3 pointer-events-auto">
                <div class="brand-text font-semibold text-base">"Browser doesn't support WebGPU"</div>
                <p class="text-muted">
                    "Pixelwise needs WebGPU for image inspection and rendering. \
                     Your browser couldn't provide a GPU device."
                </p>
                <div>
                    <div class="label mb-1">"What to try"</div>
                    <ul class="list-disc pl-5 text-muted space-y-1">
                        <li>"Update to Chrome or Edge 129 or newer."</li>
                        <li>"On Safari, use 26 or newer (macOS Sequoia / iOS 18)."</li>
                        <li>"On Firefox, enable WebGPU in about:config (141+ supports it)."</li>
                        <li>"On Linux, Chrome/Edge need a recent driver and a non-headless session."</li>
                    </ul>
                </div>
                <a class="btn inline-flex" href="https://caniuse.com/webgpu" target="_blank" rel="noreferrer">
                    "Compatibility table"
                </a>
            </div>
        </div>
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
