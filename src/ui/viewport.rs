//! The wgpu-driven viewport: canvas + init effect + per-frame render loop.

use std::cell::RefCell;
use std::rc::Rc;

use leptos::ev;
use leptos::html::Canvas;
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{HtmlCanvasElement, WheelEvent};

use crate::render::context::{canvas_size, RenderContext};
use crate::render::pass::{render as render_frame, FrameResources};
use crate::state::store::use_store;

struct GfxState {
    ctx: RenderContext,
    frame: FrameResources,
    needs_redraw: bool,
}

#[component]
pub fn Viewport() -> impl IntoView {
    let store = use_store();
    let canvas_ref = NodeRef::<Canvas>::new();
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
            let _ = store.render_epoch.get();
            if let Some(g) = gfx.borrow_mut().as_mut() {
                g.needs_redraw = true;
            }
        });
    }

    // rAF render loop. Pumps frames continuously; the cost when idle is just
    // a uniform write + 3-vertex draw, so we keep it simple.
    {
        let gfx = gfx.clone();
        Effect::new(move |_| {
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
                    g.frame.sync_images(&g.ctx, &use_store());
                    if g.needs_redraw {
                        render_frame(&mut g.ctx, &mut g.frame, &use_store());
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
    let last_xy = RwSignal::new((0.0_f64, 0.0_f64));

    let on_pointer_down = move |ev: ev::PointerEvent| {
        dragging.set(true);
        last_xy.set((ev.client_x() as f64, ev.client_y() as f64));
        if let Some(t) = ev.target() {
            if let Some(el) = t.dyn_into::<web_sys::Element>().ok() {
                let _ = el.set_pointer_capture(ev.pointer_id());
            }
        }
    };
    let on_pointer_up = move |_ev: ev::PointerEvent| {
        dragging.set(false);
    };
    let on_pointer_move = move |ev: ev::PointerEvent| {
        let (px, py) = last_xy.get_untracked();
        let nx = ev.client_x() as f64;
        let ny = ev.client_y() as f64;
        last_xy.set((nx, ny));
        if dragging.get_untracked() {
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
        if let Some(img) = store.primary_image() {
            if let Some(canvas) = canvas_ref.get() {
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
                let ix = img_x.floor() as i32;
                let iy = img_y.floor() as i32;
                if ix >= 0 && iy >= 0 && (ix as u32) < img.width && (iy as u32) < img.height {
                    store.probe_px.set(Some((ix, iy)));
                } else {
                    store.probe_px.set(None);
                }
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

    view! {
        <canvas
            node_ref=canvas_ref
            class="absolute inset-0 w-full h-full block touch-none cursor-grab active:cursor-grabbing"
            on:pointerdown=on_pointer_down
            on:pointerup=on_pointer_up
            on:pointermove=on_pointer_move
            on:wheel=on_wheel
        />
    }
}
