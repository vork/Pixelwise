//! Global reactive store, provided once at the app root.

use leptos::prelude::*;
use std::sync::Arc;

use super::view::{Camera, Channel, ClipFlags, DiffMode, Tonemap, ViewMode};
use crate::io::lut::Lut;
use crate::io::{ChannelSelection, HdrImage};

#[derive(Clone, Copy)]
pub struct Store {
    pub images: RwSignal<Vec<Arc<HdrImage>>>,
    /// Index into `images` for the primary (A) image.
    pub primary: RwSignal<Option<usize>>,
    /// Index into `images` for the secondary (B) image.
    pub secondary: RwSignal<Option<usize>>,
    pub mode: RwSignal<ViewMode>,
    pub channel: RwSignal<Channel>,
    pub tonemap: RwSignal<Tonemap>,
    pub diff_mode: RwSignal<DiffMode>,
    pub exposure: RwSignal<f32>,
    pub clip: RwSignal<ClipFlags>,
    pub camera: RwSignal<Camera>,
    pub split_pos: RwSignal<f32>,
    pub onion_alpha: RwSignal<f32>,
    pub flicker_a: RwSignal<bool>,
    // Tunable tonemap parameters.
    pub tonemap_gamma: RwSignal<f32>,
    pub reinhard_white: RwSignal<f32>,
    // Hable user-facing piecewise filmic params (per filmicworlds.com).
    // These are inherently bounded; the curve is well-formed for ANY value
    // in their respective [0,1] / [0,W) ranges.
    pub piecewise_toe_strength: RwSignal<f32>,
    pub piecewise_toe_length: RwSignal<f32>,
    pub piecewise_shoulder_strength: RwSignal<f32>,
    pub piecewise_shoulder_length: RwSignal<f32>,
    /// Optional 3D LUT used by the LUT tone-map mode.
    pub lut: RwSignal<Option<Arc<Lut>>>,
    /// Cursor position in image pixel coordinates (None = outside).
    pub probe_px: RwSignal<Option<(i32, i32)>>,
    /// HDR canvas state, set after `render::context` boots.
    pub hdr_active: RwSignal<bool>,
    pub gpu_available: RwSignal<Option<bool>>,
    /// Bumped whenever the renderer should re-issue mip/upload work.
    pub render_epoch: RwSignal<u64>,
}

impl Store {
    pub fn new() -> Self {
        Self {
            images: RwSignal::new(Vec::new()),
            primary: RwSignal::new(None),
            secondary: RwSignal::new(None),
            mode: RwSignal::new(ViewMode::Single),
            channel: RwSignal::new(Channel::Rgb),
            // Default to sRGB: shows linear data correctly encoded without
            // a creative tone curve, and makes >1.0 clipping immediately
            // obvious — which is what an inspection tool should default to.
            // (HDR canvases skip tonemap entirely either way.)
            tonemap: RwSignal::new(Tonemap::Srgb),
            diff_mode: RwSignal::new(DiffMode::Abs),
            exposure: RwSignal::new(0.0),
            clip: RwSignal::new(ClipFlags::empty()),
            camera: RwSignal::new(Camera::default()),
            split_pos: RwSignal::new(0.5),
            onion_alpha: RwSignal::new(0.5),
            flicker_a: RwSignal::new(true),
            tonemap_gamma: RwSignal::new(2.2),
            reinhard_white: RwSignal::new(4.0),
            // Defaults from Hable's Uncharted-2 calibration.
            piecewise_toe_strength: RwSignal::new(0.5),
            piecewise_toe_length: RwSignal::new(0.5),
            piecewise_shoulder_strength: RwSignal::new(2.0),
            piecewise_shoulder_length: RwSignal::new(0.5),
            lut: RwSignal::new(None),
            probe_px: RwSignal::new(None),
            hdr_active: RwSignal::new(false),
            gpu_available: RwSignal::new(None),
            render_epoch: RwSignal::new(0),
        }
    }

    pub fn add_image(&self, img: Arc<HdrImage>) {
        let idx = self.images.with(|v| v.len());
        self.images.update(|v| v.push(img));
        if self.primary.get_untracked().is_none() {
            self.primary.set(Some(idx));
        } else if self.secondary.get_untracked().is_none() {
            self.secondary.set(Some(idx));
        }
        self.render_epoch.update(|n| *n += 1);
    }

    /// Remove an image from the list and fix up primary/secondary indices.
    ///
    /// Invariant maintained: A is set whenever any image exists. If the user
    /// unloads the image that's currently displayed (A), we promote B if it's
    /// set, otherwise pick the next remaining image — so the viewport never
    /// goes blank while there's still something to show.
    pub fn remove_image(&self, idx: usize) {
        let len = self.images.with_untracked(|v| v.len());
        if idx >= len {
            return;
        }
        self.images.update(|v| {
            v.remove(idx);
        });
        let new_len = len - 1;
        let fix = |opt: Option<usize>| match opt {
            Some(i) if i == idx => None,
            Some(i) if i > idx => Some(i - 1),
            other => other,
        };
        let mut p = fix(self.primary.get_untracked());
        let mut s = fix(self.secondary.get_untracked());

        // Keep something in A as long as any image remains.
        if p.is_none() && new_len > 0 {
            if let Some(b) = s.take() {
                // Promote B to A — matches toggle_primary's "swap-down" behavior
                // when A is turned off via the per-image button.
                p = Some(b);
            } else {
                // No B to promote — pick the "next in line": the slot the
                // removed image occupied (now holding the image that was
                // immediately after it), or the new last slot if we removed
                // the tail.
                p = Some(idx.min(new_len - 1));
            }
        }

        self.primary.set(p);
        self.secondary.set(s);
        self.render_epoch.update(|n| *n += 1);
    }

    /// Click handler for the "A" button on image `idx`. Toggles off when
    /// already A; swaps with B when the image is currently B; otherwise sets
    /// A to `idx`. Guarantees A != B, and never leaves B set with A empty
    /// (the renderer treats A as the canonical image).
    pub fn toggle_primary(&self, idx: usize) {
        let p = self.primary.get_untracked();
        let s = self.secondary.get_untracked();
        if p == Some(idx) {
            // Toggling A off — promote B to A if B is set, so we never end
            // up with B-only (which the viewport renders as nothing).
            self.primary.set(s);
            self.secondary.set(None);
        } else if s == Some(idx) {
            self.primary.set(Some(idx));
            self.secondary.set(p);
        } else {
            self.primary.set(Some(idx));
        }
    }

    pub fn toggle_secondary(&self, idx: usize) {
        let p = self.primary.get_untracked();
        let s = self.secondary.get_untracked();
        if s == Some(idx) {
            self.secondary.set(None);
        } else if p == Some(idx) {
            // Clicking B on the image that's currently A — swap, but only if
            // there's something to swap into A. Otherwise leave A alone and
            // refuse to make this the second copy.
            if let Some(other) = s {
                self.primary.set(Some(other));
                self.secondary.set(Some(idx));
            }
        } else {
            self.secondary.set(Some(idx));
        }
    }

    /// Remap which source channels feed R/G/B/A for a multichannel image.
    /// Rebuilds the displayed RGBA buffer in place (cheap — the planar source
    /// channels are shared) and bumps the render epoch so the GPU re-uploads.
    pub fn set_channel_selection(&self, image_idx: usize, sel: ChannelSelection) {
        let mut changed = false;
        self.images.update(|v| {
            if let Some(slot) = v.get_mut(image_idx) {
                if slot.multichannel.is_some() {
                    *slot = Arc::new(slot.with_selection(sel));
                    changed = true;
                }
            }
        });
        if changed {
            self.render_epoch.update(|n| *n += 1);
        }
    }

    pub fn primary_image(&self) -> Option<Arc<HdrImage>> {
        let i = self.primary.get()?;
        self.images.with(|v| v.get(i).cloned())
    }

    pub fn secondary_image(&self) -> Option<Arc<HdrImage>> {
        let i = self.secondary.get()?;
        self.images.with(|v| v.get(i).cloned())
    }
}

pub fn provide_store(store: Store) {
    provide_context(store);
}

pub fn use_store() -> Store {
    use_context::<Store>().expect("Store not provided")
}
