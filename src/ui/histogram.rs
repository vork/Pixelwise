//! Histogram panel — uses a `<canvas>` 2D context for rendering.

use leptos::html::Canvas;
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

use crate::metrics::histogram::{self, Histogram};
use crate::state::store::use_store;

#[component]
pub fn HistogramPanel() -> impl IntoView {
    let store = use_store();
    let canvas_ref = NodeRef::<Canvas>::new();
    let hist: RwSignal<Option<Histogram>> = RwSignal::new(None);

    // Recompute when image changes.
    Effect::new(move |_| {
        let _ = store.render_epoch.get();
        let Some(img) = store.primary_image() else {
            hist.set(None);
            return;
        };
        let h = histogram::compute(&img, -10.0, 10.0);
        hist.set(Some(h));
    });

    // Render whenever histogram or exposure changes.
    Effect::new(move |_| {
        let _ev = store.exposure.get();
        let Some(canvas_el) = canvas_ref.get() else { return };
        let Some(h) = hist.get() else { return };
        let canvas: HtmlCanvasElement = canvas_el.unchecked_into();
        let dpr = web_sys::window().map(|w| w.device_pixel_ratio()).unwrap_or(1.0);
        let cw = canvas.client_width() as f64 * dpr;
        let ch = canvas.client_height() as f64 * dpr;
        canvas.set_width(cw as u32);
        canvas.set_height(ch as u32);
        let ctx: CanvasRenderingContext2d = canvas
            .get_context("2d")
            .ok()
            .flatten()
            .and_then(|c| c.dyn_into().ok())
            .expect("2d ctx");
        draw_histogram(&ctx, cw, ch, &h, _ev);
    });

    view! {
        <section class="panel-inset p-2 space-y-1">
            <header class="flex items-center justify-between">
                <span class="label">"Log-luminance histogram"</span>
                <span class="text-[10px] text-muted nums">{move || hist.with(|h| h.as_ref()
                    .map(|h| format!("P50 {:+.1} · P90 {:+.1} · P99 {:+.1} · P99.9 {:+.1} EV",
                        h.percentiles.0, h.percentiles.1, h.percentiles.2, h.percentiles.3))
                    .unwrap_or_default())}</span>
            </header>
            <canvas node_ref=canvas_ref class="block w-full h-[100px]" />
        </section>
    }
}

fn draw_histogram(ctx: &CanvasRenderingContext2d, w: f64, h: f64, hist: &Histogram, exposure: f32) {
    ctx.set_fill_style_str("#16161e");
    ctx.fill_rect(0.0, 0.0, w, h);

    let max = *hist.bins.iter().max().unwrap_or(&1) as f64;
    let n = hist.bins.len();
    let bw = w / n as f64;

    // Bars.
    ctx.set_fill_style_str("#dd86ff");
    ctx.set_global_alpha(0.65);
    for (i, &b) in hist.bins.iter().enumerate() {
        let bh = (b as f64 / max) * (h - 6.0);
        ctx.fill_rect(i as f64 * bw, h - bh, bw.max(1.0), bh);
    }
    ctx.set_global_alpha(1.0);

    // Percentile markers.
    let draw_marker = |ev: f32, color: &str, label: &str| {
        let t = ((ev - hist.min_ev) / (hist.max_ev - hist.min_ev)).clamp(0.0, 1.0);
        let x = t as f64 * w;
        ctx.set_stroke_style_str(color);
        ctx.set_line_width(1.0);
        ctx.begin_path();
        ctx.move_to(x, 0.0);
        ctx.line_to(x, h);
        let _ = ctx.stroke();
        ctx.set_fill_style_str(color);
        ctx.set_font("10px JetBrains Mono, monospace");
        let _ = ctx.fill_text(label, x + 2.0, 10.0);
    };
    draw_marker(hist.percentiles.0, "#7ef7c0", "P50");
    draw_marker(hist.percentiles.1, "#f7d97e", "P90");
    draw_marker(hist.percentiles.2, "#f7a543", "P99");
    draw_marker(hist.percentiles.3, "#ff7b8a", "P99.9");

    // Current exposure marker = vertical brand line at 0 EV shifted.
    let t = ((0.0 - exposure - hist.min_ev) / (hist.max_ev - hist.min_ev)).clamp(0.0, 1.0);
    let x = t as f64 * w;
    ctx.set_stroke_style_str("#e9e6ff");
    ctx.set_line_width(2.0);
    ctx.begin_path();
    ctx.move_to(x, 0.0);
    ctx.line_to(x, h);
    let _ = ctx.stroke();
}
