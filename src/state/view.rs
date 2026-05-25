//! View-state types: pan/zoom, exposure, tonemap, channel, clipping flags.
//!
//! These are POD-shaped structs that get mirrored into shader uniforms.

use bytemuck::{Pod, Zeroable};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ViewMode {
    Single = 0,
    Split = 1,
    Flicker = 2,
    OnionSkin = 3,
    Difference = 4,
    Metrics = 5,
}

impl ViewMode {
    pub const ALL: &'static [Self] = &[
        Self::Single,
        Self::Split,
        Self::Flicker,
        Self::OnionSkin,
        Self::Difference,
        Self::Metrics,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Single => "Single",
            Self::Split => "Split",
            Self::Flicker => "Flicker",
            Self::OnionSkin => "Onion",
            Self::Difference => "Diff",
            Self::Metrics => "Metrics",
        }
    }

    pub fn needs_two_images(self) -> bool {
        !matches!(self, Self::Single)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Channel {
    Rgb = 0,
    Red = 1,
    Green = 2,
    Blue = 3,
    Alpha = 4,
    Luminance = 5,
    LogLuminance = 6,
    Saturation = 7,
    Hue = 8,
    FalseColor = 9,
}

impl Channel {
    pub const ALL: &'static [Self] = &[
        Self::Rgb,
        Self::Red,
        Self::Green,
        Self::Blue,
        Self::Alpha,
        Self::Luminance,
        Self::LogLuminance,
        Self::Saturation,
        Self::Hue,
        Self::FalseColor,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Rgb => "RGB",
            Self::Red => "R",
            Self::Green => "G",
            Self::Blue => "B",
            Self::Alpha => "A",
            Self::Luminance => "Lum",
            Self::LogLuminance => "Log L",
            Self::Saturation => "Sat",
            Self::Hue => "Hue",
            Self::FalseColor => "False",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Tonemap {
    Linear = 0,
    Reinhard = 1,
    Aces = 2,
    Filmic = 3,
    Gamma = 4,
}

impl Tonemap {
    pub const ALL: &'static [Self] =
        &[Self::Linear, Self::Reinhard, Self::Aces, Self::Filmic, Self::Gamma];
    pub fn label(self) -> &'static str {
        match self {
            Self::Linear => "Linear",
            Self::Reinhard => "Reinhard",
            Self::Aces => "ACES",
            Self::Filmic => "Filmic",
            Self::Gamma => "Gamma",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum DiffMode {
    Abs = 0,
    Signed = 1,
    Log = 2,
    Relative = 3,
}

impl DiffMode {
    pub const ALL: &'static [Self] = &[Self::Abs, Self::Signed, Self::Log, Self::Relative];
    pub fn label(self) -> &'static str {
        match self {
            Self::Abs => "|a-b|",
            Self::Signed => "a-b",
            Self::Log => "|log a − log b|",
            Self::Relative => "rel",
        }
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub struct ClipFlags: u32 {
        const OVER       = 1 << 0;
        const UNDER      = 1 << 1;
        const NEGATIVE   = 1 << 2;
        const NAN        = 1 << 3;
        const INF        = 1 << 4;
        const OUT_GAMUT  = 1 << 5;
    }
}

/// Camera/transform state for the viewport.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    /// Image-space coordinate at the viewport center, in pixels.
    pub center: [f32; 2],
    /// Zoom: screen px per image px. 1.0 = 1:1, larger = zoomed in.
    pub zoom: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self { center: [0.0, 0.0], zoom: 1.0 }
    }
}

/// Display parameters bound as a uniform to the display shader.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct DisplayUniform {
    /// Transformation from clip-space (-1..1) UV to image-space UV (0..1).
    /// xy = scale, zw = offset.
    pub uv_xform: [f32; 4],
    pub exposure: f32,
    pub tonemap: u32,
    pub channel: u32,
    pub clip_flags: u32,
    pub output_is_hdr: u32,
    pub width: u32,
    pub height: u32,
    pub false_color_min: f32,
    pub false_color_max: f32,
    pub _pad0: f32,
    pub _pad1: f32,
    pub _pad2: f32,
}

impl Default for DisplayUniform {
    fn default() -> Self {
        Self {
            uv_xform: [1.0, 1.0, 0.0, 0.0],
            exposure: 0.0,
            tonemap: Tonemap::Aces as u32,
            channel: Channel::Rgb as u32,
            clip_flags: 0,
            output_is_hdr: 0,
            width: 1,
            height: 1,
            false_color_min: 0.0,
            false_color_max: 1.0,
            _pad0: 0.0,
            _pad1: 0.0,
            _pad2: 0.0,
        }
    }
}
