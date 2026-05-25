//! Metrics readout for A vs B (compute on demand).

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::metrics::pixel::{self, PixelMetrics};
use crate::state::store::use_store;

#[component]
pub fn MetricsPanel() -> impl IntoView {
    let store = use_store();
    let metrics: RwSignal<Option<PixelMetrics>> = RwSignal::new(None);
    let computing = RwSignal::new(false);

    let recompute = move |_| {
        let Some(a) = store.primary_image() else { return };
        let Some(b) = store.secondary_image() else { return };
        computing.set(true);
        spawn_local(async move {
            let m = pixel::compute(&a, &b);
            metrics.set(m);
            computing.set(false);
        });
    };

    view! {
        <section class="panel-inset p-2 space-y-2">
            <header class="flex items-center justify-between">
                <span class="label">"Metrics (A vs B)"</span>
                <button class="btn" on:click=recompute>"Compute"</button>
            </header>
            <Show when=move || computing.get()>
                <div class="text-[11px] text-muted">"Computing…"</div>
            </Show>
            {move || metrics.get().map(|m| view! {
                <table class="text-[11px] w-full nums">
                    <tbody>
                        <tr><td class="text-muted pr-2">"MAE"</td><td class="text-right">{format!("{:.5}", m.mae)}</td></tr>
                        <tr><td class="text-muted pr-2">"MSE"</td><td class="text-right">{format!("{:.5}", m.mse)}</td></tr>
                        <tr><td class="text-muted pr-2">"RMSE"</td><td class="text-right">{format!("{:.5}", m.rmse)}</td></tr>
                        <tr><td class="text-muted pr-2">"PSNR"</td><td class="text-right">{format!("{:.2} dB", m.psnr)}</td></tr>
                        <tr><td class="text-muted pr-2">"max |Δ|"</td><td class="text-right">{format!("{:.5}", m.max_abs)}</td></tr>
                        <tr><td class="text-muted pr-2">"rel err"</td><td class="text-right">{format!("{:.4}", m.relative_error)}</td></tr>
                        <tr><td class="text-muted pr-2">"log-L RMSE"</td><td class="text-right">{format!("{:.4}", m.log_lum_rmse)}</td></tr>
                        <tr><td class="text-muted pr-2">"SSIM (L)"</td><td class="text-right">{format!("{:.4}", m.ssim)}</td></tr>
                        <tr><td class="text-muted pr-2">"pixels"</td><td class="text-right">{format!("{}", m.pixels_compared)}</td></tr>
                    </tbody>
                </table>
            })}
        </section>
    }
}
