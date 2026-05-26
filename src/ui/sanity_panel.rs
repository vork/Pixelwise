//! HDR sanity / diagnostics: counts of NaN/Inf/negatives, dynamic range,
//! per-channel min/max/mean. Recomputes when the primary image changes.

use leptos::prelude::*;

use crate::metrics::stats::{self, Stats};
use crate::state::store::use_store;

#[component]
pub fn SanityPanel() -> impl IntoView {
    let store = use_store();
    let stats_a: RwSignal<Option<Stats>> = RwSignal::new(None);
    let stats_b: RwSignal<Option<Stats>> = RwSignal::new(None);

    Effect::new(move |_| {
        let _ = store.render_epoch.get();
        stats_a.set(store.primary_image().map(|i| stats::compute(&i)));
        stats_b.set(store.secondary_image().map(|i| stats::compute(&i)));
    });

    let dyn_range = move |img_idx: usize| -> Option<f32> {
        let images = store.images.get();
        let img = images.get(img_idx)?;
        img.dynamic_range_ev()
    };

    view! {
        <section class="panel-inset p-2 space-y-2">
            <header class="flex items-center justify-between">
                <span class="label">"HDR sanity"</span>
            </header>
            {move || stats_a.get().map(|s| view! { <SanityRow label="A" stats=s /> })}
            {move || stats_b.get().map(|s| view! { <SanityRow label="B" stats=s /> })}
            {move || {
                let pa = store.primary.get();
                let pb = store.secondary.get();
                match (pa, pb) {
                    (Some(a), Some(b)) => {
                        let la = stats_a.get().map(|s| s.mean_log_lum);
                        let lb = stats_b.get().map(|s| s.mean_log_lum);
                        let ev_diff = match (la, lb) {
                            (Some(la), Some(lb)) => Some((la - lb) / 2f32.ln()),
                            _ => None,
                        };
                        let dr_a = dyn_range(a);
                        let dr_b = dyn_range(b);
                        view! {
                            <div class="text-[11px] nums">
                                {ev_diff.map(|d| {
                                    let cls = if d.abs() > 0.25 { "text-warn" } else { "text-muted" };
                                    view! { <div class=cls>{format!("A is {:+.2} EV vs B (mean log-luminance)", d)}</div> }
                                })}
                                {match (dr_a, dr_b) {
                                    (Some(da), Some(db)) => view! {
                                        <div class="text-muted">
                                            {format!("dyn range: A={:.1} EV · B={:.1} EV", da, db)}
                                        </div>
                                    }.into_any(),
                                    _ => view! { <div /> }.into_any(),
                                }}
                            </div>
                        }.into_any()
                    }
                    _ => view! { <div /> }.into_any(),
                }
            }}
        </section>
    }
}

#[component]
fn SanityRow(label: &'static str, stats: Stats) -> impl IntoView {
    let total = stats.pixels.max(1);
    let pct = |n: u32| (n as f32 / total as f32) * 100.0;
    view! {
        <div class="text-[11px] nums">
            <div class="flex items-center gap-2 mb-0.5">
                <span class="chip">{label}</span>
                {if stats.nan_pixels > 0 {
                    view! { <span class="chip-err">{format!("{} NaN ({:.2}%)", stats.nan_pixels, pct(stats.nan_pixels))}</span> }.into_any()
                } else { view! { <span /> }.into_any() }}
                {if stats.inf_pixels > 0 {
                    view! { <span class="chip-err">{format!("{} Inf ({:.2}%)", stats.inf_pixels, pct(stats.inf_pixels))}</span> }.into_any()
                } else { view! { <span /> }.into_any() }}
                {if stats.negative_pixels > 0 {
                    view! { <span class="chip-warn">{format!("{} neg ({:.2}%)", stats.negative_pixels, pct(stats.negative_pixels))}</span> }.into_any()
                } else { view! { <span /> }.into_any() }}
            </div>
            <div class="text-muted">{format!("min RGB [{:.4}, {:.4}, {:.4}]", stats.min_rgb[0], stats.min_rgb[1], stats.min_rgb[2])}</div>
            <div class="text-muted">{format!("max RGB [{:.3}, {:.3}, {:.3}]", stats.max_rgb[0], stats.max_rgb[1], stats.max_rgb[2])}</div>
            <div class="text-muted">{format!("mean RGB [{:.4}, {:.4}, {:.4}]", stats.mean_rgb[0], stats.mean_rgb[1], stats.mean_rgb[2])}</div>
        </div>
    }
}
