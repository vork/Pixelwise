//! Right-side panel: exposure / tonemap / channel / clipping flag controls.

use leptos::ev;
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;

use crate::color::tonemap as tm;
use crate::io::ChannelSelection;
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
                title="Double-click to reset to 0 EV"
                prop:value=move || store.exposure.get().to_string()
                on:input=on_exposure
                on:dblclick=move |_| store.exposure.set(0.0)
            />
            <div class="flex justify-between text-[10px] text-muted nums">
                <button class="btn" on:click=move |_| store.exposure.set(0.0)>"0 EV"</button>
                <button class="btn" on:click=move |_| store.exposure.update(|v| *v -= 1.0)>"-1"</button>
                <button class="btn" on:click=move |_| store.exposure.update(|v| *v += 1.0)>"+1"</button>
            </div>

            <header class="flex items-center justify-between pt-1">
                <span class="label">"Tone mapping"</span>
            </header>
            <div class="grid grid-cols-4 gap-1">
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

            <TonemapParams />
        </section>
    }
}

/// Standalone LUT stage panel — chains a 3D LUT after the tonemap pipeline.
/// Identity until the user drops a `.cube` / `.3dl` file, then sample the
/// loaded LUT for every pixel post-tonemap.
#[component]
pub fn LutPanel() -> impl IntoView {
    let store = use_store();
    view! {
        <section class="panel-inset p-2 space-y-2">
            <header class="flex items-center justify-between">
                <span class="label">"LUT"</span>
                <span class="text-[10px] text-muted nums">"post-tonemap"</span>
            </header>
            {move || match store.lut.get() {
                Some(l) => view! {
                    <div class="text-[11px]">
                        <div class="text-text truncate" title=l.name.clone()>{l.name.clone()}</div>
                        <div class="text-[10px] text-muted nums">
                            {format!("{0}³ · {1}", l.size, l.source_label)}
                        </div>
                    </div>
                    <button
                        class="btn w-full text-[10px]"
                        on:click=move |_| store.lut.set(None)>
                        "Unload LUT"
                    </button>
                }.into_any(),
                None => view! {
                    <div class="text-[11px] text-muted">
                        "No LUT loaded. Drop a "
                        <span class="nums text-text">".cube"</span>" or "
                        <span class="nums text-text">".3dl"</span>
                        " onto the page to chain it after the tonemap."
                    </div>
                }.into_any(),
            }}
        </section>
    }
}

#[component]
fn TonemapParams() -> impl IntoView {
    let store = use_store();
    view! {
        {move || match store.tonemap.get() {
            Tonemap::Gamma => view! {
                <div class="space-y-1.5 pt-1 border-t border-border">
                    <TonemapCurve mode=Tonemap::Gamma />
                    <ParamSlider
                        label="Gamma" min=1.0 max=3.0 step=0.05
                        signal=store.tonemap_gamma default=2.2 fmt="{:.2}"
                    />
                </div>
            }.into_any(),
            Tonemap::Reinhard => view! {
                <div class="space-y-1.5 pt-1 border-t border-border">
                    <TonemapCurve mode=Tonemap::Reinhard />
                    <ParamSlider
                        label="White point" min=1.0 max=16.0 step=0.1
                        signal=store.reinhard_white default=4.0 fmt="{:.2}"
                    />
                </div>
            }.into_any(),
            Tonemap::Hable => view! {
                <div class="space-y-1.5 pt-1 border-t border-border">
                    <TonemapCurve mode=Tonemap::Hable />
                    <ParamSlider
                        label="Toe length" min=0.0 max=1.0 step=0.01
                        signal=store.piecewise_toe_length default=0.5 fmt="{:.2}"
                    />
                    <ParamSlider
                        label="Toe strength" min=0.0 max=1.0 step=0.01
                        signal=store.piecewise_toe_strength default=0.5 fmt="{:.2}"
                    />
                    <ParamSlider
                        label="Shldr length" min=0.0 max=1.0 step=0.01
                        signal=store.piecewise_shoulder_length default=0.5 fmt="{:.2}"
                    />
                    <ParamSlider
                        label="Shldr strength" min=0.0 max=4.0 step=0.05
                        signal=store.piecewise_shoulder_strength default=2.0 fmt="{:.2}"
                    />
                    <button
                        class="btn w-full text-[10px]"
                        on:click=move |_| {
                            store.piecewise_toe_length.set(0.5);
                            store.piecewise_toe_strength.set(0.5);
                            store.piecewise_shoulder_length.set(0.5);
                            store.piecewise_shoulder_strength.set(2.0);
                        }>
                        "Reset to defaults"
                    </button>
                </div>
            }.into_any(),
            _ => view! { <div /> }.into_any(),
        }}
    }
}

#[component]
fn ParamSlider(
    label: &'static str,
    min: f32,
    max: f32,
    step: f32,
    signal: RwSignal<f32>,
    default: f32,
    fmt: &'static str,
) -> impl IntoView {
    let on_input = move |ev: ev::Event| {
        if let Some(t) = ev.target().and_then(|t| t.dyn_into::<HtmlInputElement>().ok()) {
            if let Ok(v) = t.value().parse::<f32>() {
                signal.set(v);
            }
        }
    };
    let title = format!("Double-click to reset to {default}");
    view! {
        <div class="flex items-center gap-2 text-[11px]">
            <span class="text-muted w-20 truncate" title=label>{label}</span>
            <input
                type="range"
                min=min.to_string()
                max=max.to_string()
                step=step.to_string()
                class="brand flex-1"
                title=title
                prop:value=move || signal.get().to_string()
                on:input=on_input
                on:dblclick=move |_| signal.set(default)
            />
            <span class="nums w-12 text-right">
                {move || format_param(signal.get(), fmt)}
            </span>
        </div>
    }
}

fn format_param(v: f32, fmt: &str) -> String {
    match fmt {
        "{:.2}" => format!("{:.2}", v),
        "{:.3}" => format!("{:.3}", v),
        _ => format!("{}", v),
    }
}

/// Live plot of the tonemap function. X is scene-linear input, Y is the
/// display-referred output (0..1). The view re-renders whenever the parameter
/// signals it depends on change.
#[component]
fn TonemapCurve(mode: Tonemap) -> impl IntoView {
    let store = use_store();
    const W: f32 = 280.0;
    const H: f32 = 140.0;
    const PAD_L: f32 = 28.0;
    const PAD_R: f32 = 8.0;
    const PAD_T: f32 = 8.0;
    const PAD_B: f32 = 18.0;
    let inner_w = W - PAD_L - PAD_R;
    let inner_h = H - PAD_T - PAD_B;

    // Derived piecewise segments from the user-facing Hable knobs.
    let hable_derived = move || {
        tm::piecewise_segments_from_user(
            store.piecewise_toe_strength.get(),
            store.piecewise_toe_length.get(),
            store.piecewise_shoulder_strength.get(),
            store.piecewise_shoulder_length.get(),
        )
    };

    // Domain for the X axis. Keep it tight to where the curve has visible
    // shape — for Hable / Reinhard the white-point is often huge (12+) and
    // showing the whole way to W just shows a flat asymptote at y=1.
    let x_max = move || match mode {
        Tonemap::Hable => {
            let (_, w) = hable_derived();
            (w * 0.5).clamp(2.0, 6.0)
        }
        Tonemap::Reinhard => (store.reinhard_white.get() * 1.2).clamp(2.0, 8.0),
        _ => 1.2,
    };

    // Curve evaluator — pure operator shape, no extra display encoding for
    // the modes whose encoding is applied separately downstream. Matches
    // what Framewise's pwEvalChannel returns: a single-channel value that
    // shows the actual tone-map curve, not the display-encoded result.
    let sample = move |x: f32| -> f32 {
        match mode {
            Tonemap::Linear => tm::curve::linear(x),
            Tonemap::Srgb => tm::curve::srgb(x),
            Tonemap::Reinhard => tm::curve::reinhard(x, store.reinhard_white.get()),
            Tonemap::Aces => tm::curve::aces(x),
            Tonemap::Filmic => tm::curve::filmic_hejl(x),
            Tonemap::Hable => {
                let (pw, w) = hable_derived();
                tm::curve::hable(x, pw, w)
            }
            Tonemap::Gamma => tm::curve::gamma(x, store.tonemap_gamma.get()),
        }
    };

    // SVG y-axis grows downward, so we flip y when projecting.
    let project = move |x: f32, y: f32, xmax: f32| -> (f32, f32) {
        let nx = (x / xmax).clamp(0.0, 1.0);
        let ny = (y / 1.1).clamp(0.0, 1.0);
        (PAD_L + nx * inner_w, PAD_T + (1.0 - ny) * inner_h)
    };

    let path = move || {
        let xm = x_max();
        let n = 128;
        let mut s = String::with_capacity(n * 14);
        for i in 0..=n {
            let t = i as f32 / n as f32;
            let x = t * xm;
            let y = sample(x);
            let (px, py) = project(x, y, xm);
            if i == 0 {
                s.push_str(&format!("M{:.1},{:.1}", px, py));
            } else {
                s.push_str(&format!(" L{:.1},{:.1}", px, py));
            }
        }
        s
    };

    // Reference lines: y=1 (display white) and x=W (input white point).
    let y_one = move || {
        let xm = x_max();
        let (_, py) = project(0.0, 1.0, xm);
        (py, PAD_L, PAD_L + inner_w)
    };
    let x_white = move || {
        let xm = x_max();
        let w = match mode {
            Tonemap::Hable => hable_derived().1,
            Tonemap::Reinhard => store.reinhard_white.get(),
            _ => 1.0,
        };
        let (px, _) = project(w, 0.0, xm);
        (px, PAD_T, PAD_T + inner_h)
    };
    let show_white = matches!(mode, Tonemap::Hable | Tonemap::Reinhard);

    let toe_marker = move || {
        if !matches!(mode, Tonemap::Hable) {
            return None;
        }
        let xm = x_max();
        let (pw, _) = hable_derived();
        let (px, py) = project(pw[0], pw[1], xm);
        Some((px, py))
    };
    let shoulder_marker = move || {
        if !matches!(mode, Tonemap::Hable) {
            return None;
        }
        let xm = x_max();
        let (pw, _) = hable_derived();
        let (px, py) = project(pw[2], pw[3], xm);
        Some((px, py))
    };

    let x_axis_label = move || {
        let xm = x_max();
        format!("0   →   {:.1}", xm)
    };

    view! {
        <svg
            viewBox=format!("0 0 {} {}", W, H)
            class="w-full panel-inset"
            style="aspect-ratio: 2 / 1;"
        >
            // Plot frame
            <rect
                x=PAD_L y=PAD_T width=inner_w height=inner_h
                fill="#0b0b0f" stroke="#2a2a37" stroke-width="1"
            />

            // y = 1 reference line (display white)
            <line
                x1=move || y_one().1.to_string()
                y1=move || y_one().0.to_string()
                x2=move || y_one().2.to_string()
                y2=move || y_one().0.to_string()
                stroke="#3a3a47" stroke-width="0.75" stroke-dasharray="3 3"
            />

            // x = white-point reference (only when meaningful)
            {move || if show_white {
                view! {
                    <line
                        x1=move || x_white().0.to_string()
                        y1=move || x_white().1.to_string()
                        x2=move || x_white().0.to_string()
                        y2=move || x_white().2.to_string()
                        stroke="#3a3a47" stroke-width="0.75" stroke-dasharray="3 3"
                    />
                }.into_any()
            } else {
                view! { <g /> }.into_any()
            }}

            // The curve itself, brand-gradient stroke
            <defs>
                <linearGradient id="tm-curve" x1="0%" y1="0%" x2="100%" y2="0%">
                    <stop offset="0%" stop-color="#dd86ff"/>
                    <stop offset="100%" stop-color="#f7a543"/>
                </linearGradient>
            </defs>
            <path
                fill="none"
                stroke="url(#tm-curve)"
                stroke-width="1.75"
                d=path
            />

            // Toe / shoulder transition markers (piecewise only)
            {move || toe_marker().map(|(cx, cy)| view! {
                <circle r="3" fill="#dd86ff" stroke="#0b0b0f" stroke-width="1"
                    cx=cx.to_string() cy=cy.to_string() />
            })}
            {move || shoulder_marker().map(|(cx, cy)| view! {
                <circle r="3" fill="#f7a543" stroke="#0b0b0f" stroke-width="1"
                    cx=cx.to_string() cy=cy.to_string() />
            })}

            // Axis labels
            <text x=PAD_L y=H - 4.0 fill="#6b6c80" font-size="9" font-family="JetBrains Mono, monospace">
                {x_axis_label}
            </text>
            <text x="4" y=PAD_T + 8.0 fill="#6b6c80" font-size="9" font-family="JetBrains Mono, monospace">
                "1.0"
            </text>
            <text x="4" y=PAD_T + inner_h fill="#6b6c80" font-size="9" font-family="JetBrains Mono, monospace">
                "0.0"
            </text>
        </svg>
    }
}

/// Source-channel mapping for multichannel images (e.g. multilayer EXR AOVs).
/// Lets the user choose which channels drive R/G/B and an *optional* alpha,
/// plus one-click presets for each detected layer.
#[component]
pub fn ChannelMapPanel() -> impl IntoView {
    let store = use_store();
    view! {
        {move || {
            let idx = store.primary.get()?;
            let img = store.images.with(|v| v.get(idx).cloned())?;
            let mc = img.multichannel.clone()?;
            let channels = mc.channels.clone();
            let sel = mc.selection;

            // Options shared by every selector: (index, full channel name).
            let options: Vec<(usize, String)> =
                channels.iter().enumerate().map(|(i, c)| (i, c.name.clone())).collect();

            // Layer presets (only worth showing when there's more than one).
            let layers = mc.layers();
            let layer_presets: Vec<(String, ChannelSelection)> = layers
                .iter()
                .map(|l| (l.name.clone(), mc.selection_for_layer(l)))
                .collect();
            let show_layers = layer_presets.len() > 1;

            // Default channel to use when the user enables alpha: a real alpha
            // channel if one exists, otherwise the last channel.
            let alpha_default = channels
                .iter()
                .position(|c| matches!(c.leaf().to_ascii_lowercase().as_str(), "a" | "alpha"))
                .unwrap_or(channels.len().saturating_sub(1));
            let alpha_on = sel.a.is_some();

            Some(view! {
                <section class="panel-inset p-2 space-y-2">
                    <header class="flex items-center justify-between">
                        <span class="label">"Channels"</span>
                        <span class="text-[10px] text-muted nums">
                            {format!("{} · {} ch", img.format_label, channels.len())}
                        </span>
                    </header>

                    {show_layers.then(|| view! {
                        <div class="flex flex-wrap gap-1">
                            {layer_presets.into_iter().map(|(name, lsel)| {
                                let active = lsel == sel;
                                view! {
                                    <button
                                        class=move || if active { "btn btn-pri text-[10px] px-2 py-0.5" }
                                                      else { "btn text-[10px] px-2 py-0.5" }
                                        title="Map this layer to RGB(A)"
                                        on:click=move |_| store.set_channel_selection(idx, lsel)>
                                        {name}
                                    </button>
                                }
                            }).collect_view()}
                        </div>
                    })}

                    <ChanRow tag="R" dot="#ff8a8a" options=options.clone() current=sel.r
                        on_pick=Callback::new(move |i| store.set_channel_selection(idx, ChannelSelection { r: i, ..sel })) />
                    <ChanRow tag="G" dot="#7ddc8e" options=options.clone() current=sel.g
                        on_pick=Callback::new(move |i| store.set_channel_selection(idx, ChannelSelection { g: i, ..sel })) />
                    <ChanRow tag="B" dot="#8aa9ff" options=options.clone() current=sel.b
                        on_pick=Callback::new(move |i| store.set_channel_selection(idx, ChannelSelection { b: i, ..sel })) />

                    <div class="flex items-center gap-2 text-[11px] pt-0.5 border-t border-border mt-1">
                        <button
                            class=move || if alpha_on { "btn btn-pri text-[10px] w-12" } else { "btn text-[10px] w-12" }
                            title="Alpha is optional — toggle whether an alpha channel is used"
                            on:click=move |_| {
                                let next = if alpha_on { None } else { Some(alpha_default) };
                                store.set_channel_selection(idx, ChannelSelection { a: next, ..sel });
                            }>
                            "Alpha"
                        </button>
                        {if alpha_on {
                            view! {
                                <ChanRow tag="A" dot="#cfd0e8" options=options.clone() current=sel.a.unwrap_or(0)
                                    on_pick=Callback::new(move |i| store.set_channel_selection(idx, ChannelSelection { a: Some(i), ..sel })) />
                            }.into_any()
                        } else {
                            view! { <span class="text-muted text-[10px]">"none (opaque)"</span> }.into_any()
                        }}
                    </div>
                </section>
            })
        }}
    }
}

/// One R/G/B/A assignment row: a colored tag and a channel dropdown.
#[component]
fn ChanRow(
    tag: &'static str,
    dot: &'static str,
    options: Vec<(usize, String)>,
    current: usize,
    on_pick: Callback<usize>,
) -> impl IntoView {
    view! {
        <div class="flex items-center gap-2 text-[11px] flex-1">
            <span class="w-3 text-center font-semibold" style=format!("color:{dot}")>{tag}</span>
            <select
                class="select flex-1"
                on:change=move |ev: ev::Event| {
                    if let Some(t) = ev.target() {
                        if let Ok(i) = t.unchecked_into::<HtmlInputElement>().value().parse::<usize>() {
                            on_pick.run(i);
                        }
                    }
                }>
                // Mark the active option `selected` directly: setting the
                // select's `value` prop before its <option> children mount
                // leaves selectedIndex at 0. The panel re-renders wholesale on
                // every change, so a plain (non-reactive) flag is sufficient.
                {options.into_iter().map(|(i, name)| {
                    view! { <option value=i.to_string() selected=i == current>{name}</option> }
                }).collect_view()}
            </select>
        </div>
    }
}

#[component]
pub fn ChannelClipControls() -> impl IntoView {
    let store = use_store();
    // Alpha is optional everywhere: when the active image has no alpha, the
    // "A" view is meaningless (it's a constant 1.0), so disable it and bounce
    // the selection back to RGB if it was on Alpha.
    let has_alpha = Signal::derive(move || store.primary_image().map(|i| i.has_alpha).unwrap_or(false));
    Effect::new(move |_| {
        if !has_alpha.get() && store.channel.get() == Channel::Alpha {
            store.channel.set(Channel::Rgb);
        }
    });
    view! {
        <section class="panel-inset p-2 space-y-2">
            <header class="label">"Channel"</header>
            <div class="grid grid-cols-5 gap-1">
                {Channel::ALL.iter().copied().map(|c| {
                    let active = Signal::derive(move || store.channel.get() == c);
                    let disabled = Signal::derive(move || c == Channel::Alpha && !has_alpha.get());
                    view! {
                        <button
                            class=move || if active.get() { "btn btn-pri" } else { "btn" }
                            disabled=move || disabled.get()
                            title=move || if disabled.get() { "No alpha channel in this image" } else { "" }
                            on:click=move |_| if !disabled.get() { store.channel.set(c) }>
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
