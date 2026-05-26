//! Direct configuration of `GPUCanvasContext` for HDR output.
//!
//! wgpu's `SurfaceConfiguration` doesn't expose the WebGPU HDR canvas knobs
//! (`colorSpace` / `toneMapping`) consistently across releases, so we drive
//! the underlying `GPUCanvasContext.configure({...})` directly via js_sys.
//!
//! After wgpu has created its Surface from the canvas, we call this with the
//! GPUDevice handle wgpu used; the next acquire will pick up our config.

use js_sys::{Object, Reflect};
use wasm_bindgen::prelude::*;
use web_sys::{HtmlCanvasElement, window};

#[derive(Debug, Clone, Copy)]
pub struct HdrCapability {
    pub dynamic_range_high: bool,
    pub gamut_p3: bool,
    pub gamut_rec2020: bool,
}

impl HdrCapability {
    pub fn detect() -> Self {
        let w = window().expect("window");
        let q = |s: &str| {
            w.match_media(s)
                .ok()
                .flatten()
                .map(|m| m.matches())
                .unwrap_or(false)
        };
        Self {
            dynamic_range_high: q("(dynamic-range: high)"),
            gamut_p3: q("(color-gamut: p3)"),
            gamut_rec2020: q("(color-gamut: rec2020)"),
        }
    }
    pub fn can_hdr(self) -> bool {
        self.dynamic_range_high && self.gamut_p3
    }
    pub fn badge_label(self) -> &'static str {
        if self.can_hdr() {
            "HDR · P3 active"
        } else if self.gamut_p3 {
            "P3 · SDR"
        } else {
            "SDR fallback"
        }
    }
}

/// Try to configure the canvas's `GPUCanvasContext` for HDR output. Returns
/// `Ok(true)` when the HDR path was applied; `Ok(false)` when the SDR fallback
/// path was applied; `Err` when neither succeeded.
///
/// `device` should be the GPUDevice that wgpu created the surface on. We pass
/// it as a JsValue to avoid pulling specific wgpu internals into this module.
pub fn configure(
    canvas: &HtmlCanvasElement,
    device: &JsValue,
    prefer_hdr: bool,
) -> Result<bool, JsValue> {
    let ctx = canvas
        .get_context("webgpu")?
        .ok_or_else(|| JsValue::from_str("webgpu context unavailable"))?;

    let cap = HdrCapability::detect();
    let want_hdr = prefer_hdr && cap.can_hdr();

    // SDR: wgpu has already configured the canvas as bgra8unorm + srgb.
    // Re-running configure here is unnecessary and (when we don't have a
    // valid GPUDevice handle) actively destructive — a failed configure
    // call invalidates the swap chain so subsequent frames render nothing.
    if !want_hdr {
        return Ok(false);
    }

    // The HDR path requires the GPUDevice. If we couldn't reach it, leave
    // wgpu's SDR configuration alone rather than re-configure with undefined.
    if device.is_undefined() || device.is_null() {
        log::warn!("HDR requested but GPUDevice handle unavailable; staying on SDR");
        return Ok(false);
    }

    let cfg = Object::new();
    Reflect::set(&cfg, &"device".into(), device)?;
    Reflect::set(&cfg, &"format".into(), &"rgba16float".into())?;
    Reflect::set(&cfg, &"alphaMode".into(), &"opaque".into())?;
    Reflect::set(&cfg, &"colorSpace".into(), &"display-p3".into())?;
    let tm = Object::new();
    Reflect::set(&tm, &"mode".into(), &"extended".into())?;
    Reflect::set(&cfg, &"toneMapping".into(), &tm)?;

    let configure = Reflect::get(&ctx, &"configure".into())?;
    let configure_fn: js_sys::Function = configure.dyn_into()?;
    match configure_fn.call1(&ctx, &cfg) {
        Ok(_) => Ok(true),
        Err(e) => {
            log::warn!("HDR canvas configure failed, keeping wgpu's SDR config: {e:?}");
            Ok(false)
        }
    }
}
