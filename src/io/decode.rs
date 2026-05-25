//! Magic-byte sniffer + dispatcher.

use std::sync::Arc;

use thiserror::Error;

use super::HdrImage;

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("unrecognized format")]
    UnknownFormat,
    #[error("EXR decode: {0}")]
    Exr(String),
    #[error("HDR decode: {0}")]
    Hdr(String),
    #[error("PNG decode: {0}")]
    Png(String),
    #[error("TIFF decode: {0}")]
    Tiff(String),
    #[error("image decode: {0}")]
    Image(String),
}

pub fn decode(bytes: Vec<u8>, name: &str) -> Result<Arc<HdrImage>, DecodeError> {
    let kind = sniff(&bytes, name);
    log::info!("decode: {name} ({kind:?}) {} bytes", bytes.len());
    let img = match kind {
        Format::Exr => super::exr::decode(&bytes, name)?,
        Format::Hdr => super::hdr::decode(&bytes, name)?,
        Format::Png => super::png::decode(&bytes, name)?,
        Format::Tiff => super::tiff::decode(&bytes, name)?,
        Format::Jpeg => super::png::decode_via_image(&bytes, name, "JPEG")?,
        Format::Unknown => return Err(DecodeError::UnknownFormat),
    };
    Ok(Arc::new(img))
}

#[derive(Debug, Clone, Copy)]
enum Format {
    Exr,
    Hdr,
    Png,
    Tiff,
    Jpeg,
    Unknown,
}

fn sniff(b: &[u8], name: &str) -> Format {
    if b.len() < 4 {
        return Format::Unknown;
    }
    // EXR magic: 0x76 0x2f 0x31 0x01
    if b.starts_with(&[0x76, 0x2f, 0x31, 0x01]) {
        return Format::Exr;
    }
    if b.starts_with(b"#?RADIANCE") || b.starts_with(b"#?RGBE") {
        return Format::Hdr;
    }
    if b.starts_with(&[0x89, b'P', b'N', b'G']) {
        return Format::Png;
    }
    if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Format::Jpeg;
    }
    // TIFF: II*\0 or MM\0*
    if b.starts_with(&[b'I', b'I', 0x2A, 0x00]) || b.starts_with(&[b'M', b'M', 0x00, 0x2A]) {
        return Format::Tiff;
    }
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".exr") {
        return Format::Exr;
    }
    if lower.ends_with(".hdr") || lower.ends_with(".pic") {
        return Format::Hdr;
    }
    if lower.ends_with(".png") {
        return Format::Png;
    }
    if lower.ends_with(".tif") || lower.ends_with(".tiff") {
        return Format::Tiff;
    }
    if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        return Format::Jpeg;
    }
    Format::Unknown
}
