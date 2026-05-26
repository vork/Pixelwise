//! Adobe/Resolve `.cube` 3D LUT parser. Tiny, permissive, and tolerant of
//! the small format dialect differences that show up in the wild
//! (mismatched whitespace, trailing comments, optional DOMAIN_MIN/MAX,
//! mixed line endings).

use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub enum LutError {
    #[error("file is not valid UTF-8")]
    NotUtf8,
    #[error("missing LUT_3D_SIZE header (only 3D LUTs are supported)")]
    MissingSize,
    #[error("invalid LUT_3D_SIZE: {0}")]
    InvalidSize(String),
    #[error("expected {expected} RGB samples, found {found}")]
    SampleMismatch { expected: usize, found: usize },
    #[error("invalid sample on line {line}: {text}")]
    InvalidSample { line: usize, text: String },
}

/// In-memory 3D LUT. `data` is `size × size × size × 3` floats, ordered with
/// R fastest, G next, B slowest — matching the `.cube` spec so the values
/// can be uploaded straight into a 3D texture without reshuffling.
#[derive(Debug)]
pub struct Lut {
    pub name: String,
    pub size: u32,
    pub data: Vec<f32>,
    pub domain_min: [f32; 3],
    pub domain_max: [f32; 3],
    pub source_label: &'static str,
}

impl Lut {
    /// 2×2×2 identity used by the renderer as a default before any LUT is
    /// loaded — passes through `[0,1]` linearly.
    pub fn identity() -> Arc<Self> {
        let mut data = Vec::with_capacity(2 * 2 * 2 * 3);
        for b in 0..2 {
            for g in 0..2 {
                for r in 0..2 {
                    data.push(r as f32);
                    data.push(g as f32);
                    data.push(b as f32);
                }
            }
        }
        Arc::new(Self {
            name: "identity".into(),
            size: 2,
            data,
            domain_min: [0.0; 3],
            domain_max: [1.0; 3],
            source_label: "identity",
        })
    }
}

/// Kinds of LUT files we recognize.
#[derive(Debug, Clone, Copy)]
pub enum LutKind {
    Cube,
    ThreeDl,
}

/// Sniff for a known LUT format. Cheap — checks the extension first, then
/// the first few KB of content for distinctive headers. Returns None if it
/// doesn't look like any LUT we know how to parse.
pub fn looks_like_lut(bytes: &[u8], name: &str) -> Option<LutKind> {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".cube") {
        return Some(LutKind::Cube);
    }
    if lower.ends_with(".3dl") {
        return Some(LutKind::ThreeDl);
    }
    let head = &bytes[..bytes.len().min(4096)];
    let text = std::str::from_utf8(head).ok()?;
    if text.contains("LUT_3D_SIZE") {
        return Some(LutKind::Cube);
    }
    // A bare grid line followed by integer-triplet samples is a strong sign
    // of a .3dl, but the heuristic isn't worth the false positives — only
    // trust the extension or the `LUT_3D_SIZE` marker.
    None
}

/// Dispatch to the right parser based on the sniffed kind.
pub fn parse_lut(bytes: &[u8], name: &str, kind: LutKind) -> Result<Lut, LutError> {
    match kind {
        LutKind::Cube => parse_cube(bytes, name),
        LutKind::ThreeDl => parse_3dl(bytes, name),
    }
}

/// Back-compat shim for the previous boolean-style API used by the drop-zone.
pub fn looks_like_cube(bytes: &[u8], name: &str) -> bool {
    looks_like_lut(bytes, name).is_some()
}

pub fn parse_cube(bytes: &[u8], file_name: &str) -> Result<Lut, LutError> {
    let text = std::str::from_utf8(bytes).map_err(|_| LutError::NotUtf8)?;
    let mut title = String::new();
    let mut size: u32 = 0;
    let mut domain_min = [0.0_f32; 3];
    let mut domain_max = [1.0_f32; 3];
    let mut data: Vec<f32> = Vec::new();

    for (lineno, raw) in text.lines().enumerate() {
        let line = match raw.find('#') {
            Some(i) => raw[..i].trim(),
            None => raw.trim(),
        };
        if line.is_empty() {
            continue;
        }
        let mut tokens = line.split_whitespace();
        let Some(head) = tokens.next() else { continue };
        match head {
            "TITLE" => {
                // Everything after TITLE, stripped of optional surrounding quotes.
                let rest = line.trim_start_matches("TITLE").trim();
                title = rest.trim_matches('"').to_string();
            }
            "LUT_3D_SIZE" => {
                let s = tokens.next().ok_or_else(|| LutError::InvalidSize(line.into()))?;
                size = s.parse().map_err(|_| LutError::InvalidSize(line.into()))?;
            }
            "LUT_1D_SIZE" => {
                // We don't support 1D LUTs yet; surface as a clear error.
                return Err(LutError::MissingSize);
            }
            "DOMAIN_MIN" => {
                domain_min = parse_triple(&mut tokens)?;
            }
            "DOMAIN_MAX" => {
                domain_max = parse_triple(&mut tokens)?;
            }
            t if is_numeric_start(t) => {
                // Data row. Re-parse with all three components on the line.
                let mut toks = line.split_whitespace();
                let triple = parse_triple(&mut toks)?;
                data.extend_from_slice(&triple);
                if data.len() % 3 != 0 {
                    return Err(LutError::InvalidSample {
                        line: lineno + 1,
                        text: line.into(),
                    });
                }
            }
            _ => {
                // Unknown header (TITLE variants, comments without `#`, etc.) —
                // be lenient and skip.
            }
        }
    }

    if size == 0 {
        return Err(LutError::MissingSize);
    }
    let expected = (size as usize).pow(3) * 3;
    if data.len() != expected {
        return Err(LutError::SampleMismatch {
            expected,
            found: data.len(),
        });
    }
    let name = if title.is_empty() {
        file_name.to_string()
    } else {
        title
    };
    Ok(Lut {
        name,
        size,
        data,
        domain_min,
        domain_max,
        source_label: "CUBE",
    })
}

/// Autodesk Lustre / IRIDAS `.3dl` 3D LUTs.
///
/// Format: optional `#` comments, then one line listing the N input grid
/// values (integer, e.g. `0 64 128 ... 1023` for a 17-point 10-bit LUT),
/// then N³ lines of three integer outputs. We infer the source bit depth
/// from the observed maximum sample so the file's integer codes get
/// normalized into the unit interval that the GPU LUT texture expects.
pub fn parse_3dl(bytes: &[u8], file_name: &str) -> Result<Lut, LutError> {
    let text = std::str::from_utf8(bytes).map_err(|_| LutError::NotUtf8)?;
    let mut size: Option<u32> = None;
    let mut data_int: Vec<u32> = Vec::new();
    for (lineno, raw) in text.lines().enumerate() {
        let line = match raw.find('#') {
            Some(i) => raw[..i].trim(),
            None => raw.trim(),
        };
        if line.is_empty() {
            continue;
        }
        let toks: Vec<&str> = line.split_whitespace().collect();
        // First non-blank, non-triplet line is the grid-points header.
        // Heuristic: 2..=256 integers on the line, none of them representing
        // a likely RGB sample on its own.
        if size.is_none() {
            let parsed: Option<Vec<u32>> =
                toks.iter().map(|s| s.parse::<u32>().ok()).collect();
            if let Some(grid) = parsed {
                if grid.len() >= 2 && grid.len() <= 256 {
                    size = Some(grid.len() as u32);
                    continue;
                }
            }
            // Bad header — try to keep going in case the file is malformed
            // but the data rows are still parseable. Fall through.
        }
        if toks.len() != 3 {
            return Err(LutError::InvalidSample {
                line: lineno + 1,
                text: line.into(),
            });
        }
        for t in toks {
            let v = t.parse::<u32>().map_err(|_| LutError::InvalidSample {
                line: lineno + 1,
                text: t.into(),
            })?;
            data_int.push(v);
        }
    }
    let size = size.ok_or(LutError::MissingSize)?;
    let expected_triples = (size as usize).pow(3);
    if data_int.len() != expected_triples * 3 {
        return Err(LutError::SampleMismatch {
            expected: expected_triples * 3,
            found: data_int.len(),
        });
    }
    // Infer bit depth from the max integer — `.3dl` doesn't store this.
    let actual_max = data_int.iter().copied().max().unwrap_or(0);
    let denom = match actual_max {
        0..=255 => 255.0_f32,
        256..=1023 => 1023.0,
        1024..=4095 => 4095.0,
        4096..=16383 => 16383.0,
        16384..=65535 => 65535.0,
        m => m as f32,
    };
    let inv = 1.0 / denom;
    let data: Vec<f32> = data_int.iter().map(|&v| v as f32 * inv).collect();
    Ok(Lut {
        name: file_name.to_string(),
        size,
        data,
        domain_min: [0.0; 3],
        domain_max: [1.0; 3],
        source_label: "3DL",
    })
}

fn is_numeric_start(s: &str) -> bool {
    matches!(s.chars().next(), Some(c) if c.is_ascii_digit() || c == '-' || c == '+' || c == '.')
}

fn parse_triple<'a, I: Iterator<Item = &'a str>>(it: &mut I) -> Result<[f32; 3], LutError> {
    let mut out = [0.0_f32; 3];
    for slot in out.iter_mut() {
        let tok = it.next().ok_or_else(|| LutError::InvalidSample {
            line: 0,
            text: "(short triple)".into(),
        })?;
        *slot = tok.parse().map_err(|_| LutError::InvalidSample {
            line: 0,
            text: tok.into(),
        })?;
    }
    Ok(out)
}
