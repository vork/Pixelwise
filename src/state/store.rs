//! Global reactive store, provided once at the app root.

use leptos::prelude::*;
use std::sync::Arc;

use super::view::{Camera, Channel, ClipFlags, DiffMode, Tonemap, ViewMode};
use crate::io::HdrImage;

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
            tonemap: RwSignal::new(Tonemap::Aces),
            diff_mode: RwSignal::new(DiffMode::Abs),
            exposure: RwSignal::new(0.0),
            clip: RwSignal::new(ClipFlags::empty()),
            camera: RwSignal::new(Camera::default()),
            split_pos: RwSignal::new(0.5),
            onion_alpha: RwSignal::new(0.5),
            flicker_a: RwSignal::new(true),
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
