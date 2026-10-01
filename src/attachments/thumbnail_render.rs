//! TG-1302: the pure, blocking half of thumbnailing — bytes in, small image out. No I/O and no
//! async, so it runs inside `spawn_blocking` and is tested directly with generated images.
//!
//! Defences against hostile input: the declared dimensions are checked before any pixel is
//! decoded (a 1 KB PNG can claim 60 000 × 60 000), and `image::Limits` caps the decoder's own
//! allocations as a second line.

use std::io::Cursor;

use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits};

/// Longest edge of a thumbnail. Bubbles are at most ~320 CSS px wide; 640 keeps them sharp on
/// 2x phone screens while staying a few tens of KB.
pub const THUMBNAIL_EDGE: u32 = 640;
/// Refuse sources that would decode to more pixels than this (~200 MB as RGBA).
pub const MAX_SOURCE_PIXELS: u64 = 50_000_000;
const MAX_DECODER_ALLOC: u64 = 256 * 1024 * 1024;
const JPEG_QUALITY: u8 = 80;

#[derive(Debug, PartialEq, Eq)]
pub enum RenderError {
    /// Not a decodable PNG/JPEG/GIF/WebP.
    Undecodable,
    /// Declared dimensions exceed `MAX_SOURCE_PIXELS`.
    TooLarge,
}

/// Decode `source` (first frame for GIF), apply its EXIF orientation, fit it inside
/// `THUMBNAIL_EDGE`, and encode JPEG — or PNG when the image has transparency, so a sticker-like
/// PNG does not turn black around the edges.
pub fn render_thumbnail(source: &[u8]) -> Result<Vec<u8>, RenderError> {
    let mut reader = ImageReader::new(Cursor::new(source))
        .with_guessed_format()
        .map_err(|_| RenderError::Undecodable)?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(65_535);
    limits.max_image_height = Some(65_535);
    limits.max_alloc = Some(MAX_DECODER_ALLOC);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(refusal)?;
    let (width, height) = decoder.dimensions();
    if u64::from(width) * u64::from(height) > MAX_SOURCE_PIXELS {
        return Err(RenderError::TooLarge);
    }
    let orientation = decoder
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut image = DynamicImage::from_decoder(decoder).map_err(refusal)?;
    image.apply_orientation(orientation);
    let thumbnail = if image.width() > THUMBNAIL_EDGE || image.height() > THUMBNAIL_EDGE {
        image.thumbnail(THUMBNAIL_EDGE, THUMBNAIL_EDGE)
    } else {
        image
    };
    encode(thumbnail)
}

/// The decoder's own `Limits` refusal is the same verdict as our pixel cap.
fn refusal(error: image::ImageError) -> RenderError {
    match error {
        image::ImageError::Limits(_) => RenderError::TooLarge,
        _ => RenderError::Undecodable,
    }
}

/// The served type is read back from these bytes (`stored_content_type`), cached or fresh.
fn encode(image: DynamicImage) -> Result<Vec<u8>, RenderError> {
    let mut bytes = Vec::new();
    if image.color().has_alpha() {
        image
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .map_err(|_| RenderError::Undecodable)?;
        return Ok(bytes);
    }
    let rgb = DynamicImage::ImageRgb8(image.into_rgb8());
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, JPEG_QUALITY);
    rgb.write_with_encoder(encoder)
        .map_err(|_| RenderError::Undecodable)?;
    Ok(bytes)
}

/// The stored thumbnail's type, from its first bytes (we only ever write PNG or JPEG).
pub fn stored_content_type(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"\x89PNG") {
        "image/png"
    } else {
        "image/jpeg"
    }
}

#[cfg(test)]
#[path = "thumbnail_render_tests.rs"]
mod tests;
