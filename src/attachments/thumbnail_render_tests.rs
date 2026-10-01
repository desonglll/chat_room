use std::io::Cursor;

use image::{DynamicImage, ImageFormat, Rgb, RgbImage, Rgba, RgbaImage};

use super::*;

fn encoded(image: DynamicImage, format: ImageFormat) -> Vec<u8> {
    let mut bytes = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut bytes), format)
        .unwrap();
    bytes
}

fn decoded(bytes: &[u8]) -> DynamicImage {
    image::load_from_memory(bytes).unwrap()
}

#[test]
fn a_large_photo_fits_the_edge_and_keeps_its_aspect() {
    let photo = DynamicImage::ImageRgb8(RgbImage::from_pixel(4000, 3000, Rgb([200, 30, 30])));
    let source = encoded(photo, ImageFormat::Jpeg);
    let rendered = render_thumbnail(&source).unwrap();
    assert_eq!(stored_content_type(&rendered), "image/jpeg");
    let thumb = decoded(&rendered);
    assert_eq!((thumb.width(), thumb.height()), (640, 480));
    assert!(
        rendered.len() * 10 < source.len(),
        "{} vs {}",
        rendered.len(),
        source.len()
    );
}

#[test]
fn small_images_are_not_upscaled_and_transparency_stays_png() {
    let sticker = DynamicImage::ImageRgba8(RgbaImage::from_pixel(100, 60, Rgba([0, 0, 0, 0])));
    let rendered = render_thumbnail(&encoded(sticker, ImageFormat::Png)).unwrap();
    assert_eq!(stored_content_type(&rendered), "image/png");
    let thumb = decoded(&rendered);
    assert_eq!((thumb.width(), thumb.height()), (100, 60));
    assert_eq!(thumb.to_rgba8().get_pixel(0, 0)[3], 0, "alpha survives");
}

#[test]
fn gif_and_webp_sources_render_their_first_frame() {
    let tall = DynamicImage::ImageRgb8(RgbImage::from_pixel(300, 1200, Rgb([0, 90, 200])));
    for format in [ImageFormat::Gif, ImageFormat::WebP] {
        let rendered = render_thumbnail(&encoded(tall.clone(), format)).unwrap();
        let thumb = decoded(&rendered);
        assert_eq!((thumb.width(), thumb.height()), (160, 640), "{format:?}");
    }
}

#[test]
fn exif_orientation_is_applied() {
    // A 40×20 JPEG whose EXIF says "rotate 90° clockwise" must come out 20×40.
    let wide = DynamicImage::ImageRgb8(RgbImage::from_pixel(40, 20, Rgb([10, 200, 10])));
    let jpeg = encoded(wide, ImageFormat::Jpeg);
    let rotated = with_exif_orientation(&jpeg, 6);
    let thumb = decoded(&render_thumbnail(&rotated).unwrap());
    assert_eq!((thumb.width(), thumb.height()), (20, 40));
}

#[test]
fn decompression_bombs_are_refused_before_decoding() {
    // Structurally valid PNGs that are tiny on disk but declare huge canvases. 20 000² is caught
    // by the decoder's own `Limits`; 8 000 × 7 000 fits that allocation cap and is caught by
    // `MAX_SOURCE_PIXELS`. Neither decodes a single row (the IDAT is empty).
    assert_eq!(
        render_thumbnail(&png_header_only(20_000, 20_000)).err(),
        Some(RenderError::TooLarge)
    );
    const { assert!(8_000u64 * 7_000 > MAX_SOURCE_PIXELS) };
    assert_eq!(
        render_thumbnail(&png_header_only(8_000, 7_000)).err(),
        Some(RenderError::TooLarge)
    );
}

#[test]
fn garbage_and_svg_are_undecodable() {
    assert_eq!(
        render_thumbnail(b"not an image").err(),
        Some(RenderError::Undecodable)
    );
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"/>"#;
    assert_eq!(render_thumbnail(svg).err(), Some(RenderError::Undecodable));
}

fn png_header_only(width: u32, height: u32) -> Vec<u8> {
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8-bit RGB
    chunk(&mut png, b"IHDR", &ihdr);
    chunk(
        &mut png,
        b"IDAT",
        &[0x78, 0x9C, 0x03, 0x00, 0x00, 0x00, 0x00, 0x01],
    ); // empty zlib
    chunk(&mut png, b"IEND", &[]);
    png
}

fn chunk(png: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    png.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let mut covered = kind.to_vec();
    covered.extend_from_slice(data);
    png.extend_from_slice(&covered);
    png.extend_from_slice(&crc32(&covered).to_be_bytes());
}

/// Insert an APP1 Exif segment carrying only the Orientation tag right after the JPEG SOI.
fn with_exif_orientation(jpeg: &[u8], orientation: u16) -> Vec<u8> {
    let mut tiff = Vec::new();
    tiff.extend_from_slice(b"MM\x00\x2a\x00\x00\x00\x08"); // big-endian TIFF, IFD at 8
    tiff.extend_from_slice(&1u16.to_be_bytes()); // one entry
    tiff.extend_from_slice(&0x0112u16.to_be_bytes()); // Orientation
    tiff.extend_from_slice(&3u16.to_be_bytes()); // SHORT
    tiff.extend_from_slice(&1u32.to_be_bytes()); // count
    tiff.extend_from_slice(&orientation.to_be_bytes());
    tiff.extend_from_slice(&[0, 0]); // value padding
    tiff.extend_from_slice(&0u32.to_be_bytes()); // no next IFD
    let mut payload = b"Exif\x00\x00".to_vec();
    payload.extend_from_slice(&tiff);
    let mut out = jpeg[..2].to_vec(); // SOI
    out.extend_from_slice(&[0xFF, 0xE1]);
    out.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
    out.extend_from_slice(&payload);
    out.extend_from_slice(&jpeg[2..]);
    out
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}
