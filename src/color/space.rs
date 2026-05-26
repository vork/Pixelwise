//! Minimal OCIO-lite: a fixed set of color spaces we recognize.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorSpace {
    /// Scene-linear with sRGB / Rec.709 primaries.
    #[default]
    LinearSRgb,
    /// sRGB encoded (gamma applied).
    SRgbEncoded,
    LinearRec709,
    LinearDisplayP3,
    LinearRec2020,
    AcesCg,
    /// Unknown — pass through, warn user.
    Unknown,
}

impl ColorSpace {
    pub fn label(self) -> &'static str {
        match self {
            Self::LinearSRgb => "Linear sRGB",
            Self::SRgbEncoded => "sRGB",
            Self::LinearRec709 => "Linear Rec.709",
            Self::LinearDisplayP3 => "Linear Display P3",
            Self::LinearRec2020 => "Linear Rec.2020",
            Self::AcesCg => "ACEScg",
            Self::Unknown => "Unknown",
        }
    }
    pub fn is_linear(self) -> bool {
        !matches!(self, Self::SRgbEncoded | Self::Unknown)
    }
}

/// Rec.709 luminance weights, used for Lum/LogLum/HDR-VDP-like math.
pub const LUM_WEIGHTS_REC709: [f32; 3] = [0.2126, 0.7152, 0.0722];

/// CIE D65 white point xy.
pub const D65_XY: [f32; 2] = [0.3127, 0.3290];
