//! The wgpu-driven viewport: canvas + init effect + per-frame render loop.

use std::cell::RefCell;
use std::rc::Rc;

use leptos::ev;
use leptos::html::Canvas;
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, WheelEvent};

use crate::render::context::{canvas_size, RenderContext};
use crate::render::pass::{render as render_frame, FrameResources};
use crate::state::store::{use_store, Store};
use crate::state::view::{Camera, ViewMode};

struct GfxState {
    ctx: RenderContext,
    frame: FrameResources,
    needs_redraw: bool,
}

#[component]
pub fn Viewport() -> impl IntoView {
    let store = use_store();
    let canvas_ref = NodeRef::<Canvas>::new();
    let overlay_ref = NodeRef::<Canvas>::new();
    let gfx: Rc<RefCell<Option<GfxState>>> = Rc::new(RefCell::new(None));

    // Init wgpu when canvas mounts.
    {
        let gfx = gfx.clone();
        Effect::new(move |_| {
            let Some(canvas_el) = canvas_ref.get() else {
                return;
            };
            let canvas: HtmlCanvasElement = canvas_el.unchecked_into();
            if gfx.borrow().is_some() {
                return;
            }
            let gfx = gfx.clone();
            spawn_local(async move {
                match RenderContext::new(canvas).await {
                    Ok(ctx) => {
                        let hdr = ctx.hdr_active;
                        store.hdr_active.set(hdr);
                        store.gpu_available.set(Some(true));
                        let frame = FrameResources::new(&ctx);
                        *gfx.borrow_mut() = Some(GfxState { ctx, frame, needs_redraw: true });
                    }
                    Err(e) => {
                        log::error!("wgpu init: {e}");
                        store.gpu_available.set(Some(false));
                    }
                }
            });
        });
    }

    // Reactive: any state change marks needs_redraw and the rAF loop picks it up.
    {
        let gfx = gfx.clone();
        Effect::new(move |_| {
            let _ = store.images.get();
            let _ = store.primary.get();
            let _ = store.secondary.get();
            let _ = store.mode.get();
            let _ = store.channel.get();
            let _ = store.tonemap.get();
            let _ = store.diff_mode.get();
            let _ = store.exposure.get();
            let _ = store.clip.get();
            let _ = store.camera.get();
            let _ = store.split_pos.get();
            let _ = store.onion_alpha.get();
            let _ = store.flicker_a.get();
            let _ = store.tonemap_gamma.get();
            let _ = store.reinhard_white.get();
            let _ = store.piecewise_toe_strength.get();
            let _ = store.piecewise_toe_length.get();
            let _ = store.piecewise_shoulder_strength.get();
            let _ = store.piecewise_shoulder_length.get();
            let _ = store.lut.get();
            let _ = store.hdr_enabled.get();
            let _ = store.render_epoch.get();
            if let Some(g) = gfx.borrow_mut().as_mut() {
                g.needs_redraw = true;
            }
        });
    }

    // rAF render loop. Mounted exactly once (the Effect closure doesn't track
    // any signals, so it doesn't re-run). Inside the rAF callback we don't
    // touch context-derived APIs — we already captured the store above.
    {
        let gfx = gfx.clone();
        let store = store;
        Effect::new(move |_: Option<()>| {
            let gfx = gfx.clone();
            let win = web_sys::window().expect("window");
            let cb_holder: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
            let cb_holder2 = cb_holder.clone();
            let cb = Closure::wrap(Box::new(move || {
                if let Some(g) = gfx.borrow_mut().as_mut() {
                    let (w, h) = canvas_size(&g.ctx.canvas);
                    if (w, h) != g.ctx.size {
                        g.ctx.resize(w, h);
                        g.needs_redraw = true;
                    }
                    g.frame.sync_images(&g.ctx, &store);
                    if g.needs_redraw {
                        render_frame(&mut g.ctx, &mut g.frame, &store);
                        if let Some(overlay) = overlay_ref.get() {
                            let overlay_canvas: HtmlCanvasElement = overlay.unchecked_into();
                            draw_pixel_overlay(&overlay_canvas, &store);
                        }
                        g.needs_redraw = false;
                    }
                }
                if let Some(cb) = cb_holder2.borrow().as_ref() {
                    let _ = web_sys::window()
                        .unwrap()
                        .request_animation_frame(cb.as_ref().unchecked_ref());
                }
            }) as Box<dyn FnMut()>);
            let _ = win.request_animation_frame(cb.as_ref().unchecked_ref());
            *cb_holder.borrow_mut() = Some(cb);
        });
    }

    // Mouse interactions: drag-pan + wheel-zoom + cursor probe.
    let dragging = RwSignal::new(false);
    let dragging_split = RwSignal::new(false);
    let last_xy = RwSignal::new((0.0_f64, 0.0_f64));

    let cursor_to_image = move |nx: f64, ny: f64| -> Option<(f32, f32, u32, u32)> {
        let img = store.primary_image()?;
        let canvas = canvas_ref.get()?;
        let canvas: HtmlCanvasElement = canvas.unchecked_into();
        let el: &web_sys::Element = canvas.unchecked_ref();
        let rect = el.get_bounding_client_rect();
        let cx = (nx - rect.left()) as f32;
        let cy = (ny - rect.top()) as f32;
        let dpr = web_sys::window().map(|w| w.device_pixel_ratio()).unwrap_or(1.0) as f32;
        let cw = rect.width() as f32;
        let ch = rect.height() as f32;
        let cam = store.camera.get_untracked();
        let visible_w = cw * dpr / cam.zoom;
        let visible_h = ch * dpr / cam.zoom;
        let img_x = cam.center[0] - visible_w * 0.5 + (cx / cw) * visible_w;
        let img_y = cam.center[1] - visible_h * 0.5 + (cy / ch) * visible_h;
        Some((img_x, img_y, img.width, img.height))
    };

    let set_split_from_cursor = move |nx: f64, ny: f64| {
        if let Some((img_x, _img_y, iw, _ih)) = cursor_to_image(nx, ny) {
            let u = (img_x / iw as f32).clamp(0.0, 1.0);
            store.split_pos.set(u);
        }
    };

    let on_pointer_down = move |ev: ev::PointerEvent| {
        let nx = ev.client_x() as f64;
        let ny = ev.client_y() as f64;
        last_xy.set((nx, ny));
        let split_mode = store.mode.get_untracked() == ViewMode::Split && ev.button() == 0;
        if split_mode {
            dragging_split.set(true);
            set_split_from_cursor(nx, ny);
        } else {
            dragging.set(true);
        }
        if let Some(t) = ev.target() {
            if let Some(el) = t.dyn_into::<web_sys::Element>().ok() {
                let _ = el.set_pointer_capture(ev.pointer_id());
            }
        }
    };
    let on_pointer_up = move |_ev: ev::PointerEvent| {
        dragging.set(false);
        dragging_split.set(false);
    };
    let on_pointer_move = move |ev: ev::PointerEvent| {
        let (px, py) = last_xy.get_untracked();
        let nx = ev.client_x() as f64;
        let ny = ev.client_y() as f64;
        last_xy.set((nx, ny));
        if dragging_split.get_untracked() {
            set_split_from_cursor(nx, ny);
        } else if dragging.get_untracked() {
            let cam = store.camera.get_untracked();
            let dpr = web_sys::window().map(|w| w.device_pixel_ratio()).unwrap_or(1.0);
            let dx = ((nx - px) * dpr) as f32 / cam.zoom;
            let dy = ((ny - py) * dpr) as f32 / cam.zoom;
            store.camera.update(|c| {
                c.center[0] -= dx;
                c.center[1] -= dy;
            });
        }
        // Update probe in image-space.
        if let Some((img_x, img_y, iw, ih)) = cursor_to_image(nx, ny) {
            let ix = img_x.floor() as i32;
            let iy = img_y.floor() as i32;
            if ix >= 0 && iy >= 0 && (ix as u32) < iw && (iy as u32) < ih {
                store.probe_px.set(Some((ix, iy)));
            } else {
                store.probe_px.set(None);
            }
        }
    };
    let on_wheel = move |ev: WheelEvent| {
        ev.prevent_default();
        let factor = (-ev.delta_y() * 0.001).exp() as f32;
        store.camera.update(|c| {
            c.zoom = (c.zoom * factor).clamp(0.01, 4096.0);
        });
    };

    let canvas_class = move || {
        let base = "absolute inset-0 w-full h-full block touch-none";
        if store.mode.get() == ViewMode::Split {
            format!("{base} cursor-col-resize")
        } else {
            format!("{base} cursor-grab active:cursor-grabbing")
        }
    };

    // "Fit to viewport": center on image and pick the largest zoom that
    // keeps the whole image visible (with a 2% margin so edges aren't flush).
    let fit_to_view = move || {
        let Some(img) = store.primary_image() else { return };
        let Some(canvas) = canvas_ref.get() else { return };
        let canvas: HtmlCanvasElement = canvas.unchecked_into();
        let el: &web_sys::Element = canvas.unchecked_ref();
        let rect = el.get_bounding_client_rect();
        let dpr = web_sys::window().map(|w| w.device_pixel_ratio()).unwrap_or(1.0) as f32;
        let cw_dev = rect.width() as f32 * dpr;
        let ch_dev = rect.height() as f32 * dpr;
        if cw_dev <= 0.0 || ch_dev <= 0.0 {
            return;
        }
        let zoom_x = cw_dev / img.width as f32;
        let zoom_y = ch_dev / img.height as f32;
        let zoom = zoom_x.min(zoom_y) * 0.98;
        store.camera.set(Camera {
            center: [img.width as f32 / 2.0, img.height as f32 / 2.0],
            zoom,
        });
    };
    let on_fit = move |_| fit_to_view();
    let on_one_to_one = move |_| {
        let dpr = web_sys::window().map(|w| w.device_pixel_ratio()).unwrap_or(1.0) as f32;
        let Some(img) = store.primary_image() else { return };
        store.camera.set(Camera {
            center: [img.width as f32 / 2.0, img.height as f32 / 2.0],
            zoom: dpr,
        });
    };

    // Auto-fit when the FIRST image is loaded so the user immediately sees
    // the whole picture instead of the top-left corner at 1× zoom. Tracks
    // a 0→non-zero transition of the image list length; later A/B swaps or
    // additions leave the camera alone.
    {
        let prev_count = StoredValue::new(0_usize);
        Effect::new(move |_| {
            let n = store.images.with(|v| v.len());
            let prev = prev_count.get_value();
            if prev == 0 && n > 0 {
                // The first fire may run before the canvas has settled into
                // its final size; queue for the next animation frame so the
                // bounding rect is reliable.
                let win = web_sys::window().expect("window");
                let cb = Closure::once_into_js(move || fit_to_view());
                let _ = win.request_animation_frame(cb.as_ref().unchecked_ref());
            }
            prev_count.set_value(n);
        });
    }

    view! {
        <>
            <canvas
                node_ref=canvas_ref
                class=canvas_class
                on:pointerdown=on_pointer_down
                on:pointerup=on_pointer_up
                on:pointermove=on_pointer_move
                on:wheel=on_wheel
            />
            <canvas
                node_ref=overlay_ref
                class="absolute inset-0 w-full h-full block pointer-events-none"
            />
            <div class="absolute top-2 right-2 flex gap-1 z-10">
                <button class="btn text-[11px]" on:click=on_fit title="Fit image to viewport">
                    "Fit"
                </button>
                <button class="btn text-[11px]" on:click=on_one_to_one title="Zoom to 1:1">
                    "1:1"
                </button>
            </div>
        </>
    }
}

/// Draws pixel grid + per-channel float values on top of the viewport canvas
/// when zoomed in far enough that the labels are legible. Skips entirely
/// otherwise, so the cost is near-zero at normal zoom levels.
fn draw_pixel_overlay(canvas: &HtmlCanvasElement, store: &Store) {
    let el: &web_sys::Element = canvas.unchecked_ref();
    let rect = el.get_bounding_client_rect();
    let cw = rect.width() as f32;
    let ch = rect.height() as f32;
    if cw <= 0.0 || ch <= 0.0 {
        return;
    }
    let dpr = web_sys::window().map(|w| w.device_pixel_ratio()).unwrap_or(1.0) as f32;

    // Resize the backing store on size change.
    let want_w = (cw * dpr).round().max(1.0) as u32;
    let want_h = (ch * dpr).round().max(1.0) as u32;
    if canvas.width() != want_w || canvas.height() != want_h {
        canvas.set_width(want_w);
        canvas.set_height(want_h);
    }

    let ctx_js = match canvas.get_context("2d").ok().flatten() {
        Some(c) => c,
        None => return,
    };
    let ctx: CanvasRenderingContext2d = match ctx_js.dyn_into() {
        Ok(c) => c,
        Err(_) => return,
    };
    // Draw in CSS-pixel coordinates regardless of DPR.
    let _ = ctx.reset_transform();
    let _ = ctx.scale(dpr as f64, dpr as f64);
    ctx.clear_rect(0.0, 0.0, cw as f64, ch as f64);

    let Some(img) = store.primary_image() else { return };
    let cam = store.camera.get_untracked();
    let zoom = cam.zoom.max(0.001);
    // Screen pixels (CSS) per image pixel.
    let cell_css = zoom / dpr;
    // The grid lines themselves are now drawn inside the WGSL fragment
    // shader (always-visible regardless of background brightness). This
    // overlay only handles the per-pixel numeric labels.
    if cell_css < 72.0 {
        return;
    }
    let show_alpha = cell_css >= 110.0 && img.has_alpha;

    // Visible image-pixel range (in image coords, can extend past edges).
    let visible_w_img = cw * dpr / zoom;
    let visible_h_img = ch * dpr / zoom;
    let x0_img = cam.center[0] - visible_w_img * 0.5;
    let y0_img = cam.center[1] - visible_h_img * 0.5;
    let px_min = (x0_img.floor() as i32).max(0);
    let py_min = (y0_img.floor() as i32).max(0);
    let px_max = ((x0_img + visible_w_img).ceil() as i32).min(img.width as i32);
    let py_max = ((y0_img + visible_h_img).ceil() as i32).min(img.height as i32);
    if px_min >= px_max || py_min >= py_max {
        return;
    }

    let img_to_css_x = |ix: f32| -> f32 { (ix - cam.center[0]) * zoom / dpr + cw * 0.5 };
    let img_to_css_y = |iy: f32| -> f32 { (iy - cam.center[1]) * zoom / dpr + ch * 0.5 };

    // Per-pixel float values. Measure a worst-case label at a base size,
    // then scale the font so the longest label fills the cell width (or
    // the stacked lines fill the cell height — whichever bound hits first).
    // This way labels always read as large as possible regardless of zoom.
    let n_lines = if show_alpha { 4 } else { 3 };
    const BASE: f32 = 16.0;
    const LINE_RATIO: f32 = 1.15;
    const PAD_RATIO: f32 = 0.12;
    let font_family = "JetBrains Mono, Menlo, monospace";
    ctx.set_font(&format!("{BASE:.1}px {font_family}"));
    ctx.set_text_baseline("top");
    ctx.set_text_align("left");
    let sample = "B -1.2345"; // 9 chars covers signed mid-range values
    let base_text_w = ctx
        .measure_text(sample)
        .ok()
        .map(|m: web_sys::TextMetrics| m.width() as f32)
        .unwrap_or(BASE * 6.0);
    let avail_w = cell_css * (1.0 - 2.0 * PAD_RATIO);
    let avail_h = cell_css * (1.0 - 2.0 * PAD_RATIO);
    let scale_w = avail_w / base_text_w.max(1.0);
    let scale_h = avail_h / (BASE * LINE_RATIO * n_lines as f32);
    let font_size = (BASE * scale_w.min(scale_h)).clamp(8.0, cell_css * 0.5);

    ctx.set_font(&format!("{font_size:.1}px {font_family}"));

    // Channel colors (Tailwind reds/greens/blues at high alpha).
    let row_colors = ["#ff8a8a", "#7ddc8e", "#8aa9ff", "#cfd0e8"];
    let labels = ["R", "G", "B", "A"];
    let pad = cell_css * PAD_RATIO;
    let line_h = font_size * LINE_RATIO;

    // Black outline behind colored fill so the text stays readable on any
    // pixel color (bright, saturated, or clipping-overlay backgrounds).
    ctx.set_stroke_style_str("rgba(0,0,0,0.85)");
    ctx.set_line_width((font_size * 0.22).max(2.0) as f64);
    ctx.set_line_join("round");
    for y in py_min..py_max {
        for x in px_min..px_max {
            let Some(rgba) = img.pixel(x as u32, y as u32) else { continue };
            let sx = img_to_css_x(x as f32) + pad as f32;
            let sy = img_to_css_y(y as f32) + pad as f32;
            let n_lines = if show_alpha { 4 } else { 3 };
            for ci in 0..n_lines {
                let v = rgba[ci];
                let text = format!("{} {}", labels[ci], fmt_pixel(v));
                let x = sx as f64;
                let y = (sy + ci as f32 * line_h) as f64;
                let _ = ctx.stroke_text(&text, x, y);
                ctx.set_fill_style_str(row_colors[ci]);
                let _ = ctx.fill_text(&text, x, y);
            }
        }
    }
}

fn fmt_pixel(v: f32) -> String {
    if v.is_nan() {
        "NaN".into()
    } else if v.is_infinite() {
        if v > 0.0 { "+Inf".into() } else { "-Inf".into() }
    } else if v.abs() >= 100.0 {
        format!("{v:.1}")
    } else if v.abs() >= 10.0 {
        format!("{v:.2}")
    } else if v.abs() >= 1.0 {
        format!("{v:.3}")
    } else {
        format!("{v:.4}")
    }
}
