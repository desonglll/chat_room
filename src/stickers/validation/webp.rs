//! Static WebP: RIFF container, first chunk `VP8 ` (lossy), `VP8L` (lossless) or `VP8X`
//! (extended). Animated WebP is refused — animation belongs to TGS and WebM stickers.

use super::{StickerRejection, ValidatedSticker};
use crate::stickers::models::StickerFormat;

pub(super) fn validate(bytes: &[u8]) -> Result<ValidatedSticker, StickerRejection> {
    let invalid = StickerRejection::UnsupportedFormat;
    let riff_size = le_u32(bytes, 4).ok_or(invalid)? as usize;
    if riff_size.checked_add(8) != Some(bytes.len()) {
        return Err(invalid);
    }
    let mut offset = 12;
    let mut dimensions = None;
    let mut first = true;
    while offset + 8 <= bytes.len() {
        let fourcc = &bytes[offset..offset + 4];
        let size = le_u32(bytes, offset + 4).ok_or(invalid)? as usize;
        let start = offset + 8;
        let payload = bytes.get(start..start.checked_add(size).ok_or(invalid)?);
        let payload = payload.ok_or(invalid)?;
        match fourcc {
            b"ANIM" | b"ANMF" => return Err(StickerRejection::AnimatedWebp),
            b"VP8X" if first => {
                if payload.first().ok_or(invalid)? & 0x02 != 0 {
                    return Err(StickerRejection::AnimatedWebp);
                }
                let width = le_u24(payload, 4).ok_or(invalid)? + 1;
                let height = le_u24(payload, 7).ok_or(invalid)? + 1;
                dimensions = Some((width, height));
            }
            b"VP8 " if dimensions.is_none() => {
                if payload.get(3..6) != Some(&[0x9d, 0x01, 0x2a][..]) {
                    return Err(invalid);
                }
                let width = u32::from(le_u16(payload, 6).ok_or(invalid)? & 0x3fff);
                let height = u32::from(le_u16(payload, 8).ok_or(invalid)? & 0x3fff);
                dimensions = Some((width, height));
            }
            b"VP8L" if dimensions.is_none() => {
                if payload.first() != Some(&0x2f) {
                    return Err(invalid);
                }
                let bits = le_u32(payload, 1).ok_or(invalid)?;
                dimensions = Some(((bits & 0x3fff) + 1, ((bits >> 14) & 0x3fff) + 1));
            }
            _ if first => return Err(invalid),
            _ => {}
        }
        first = false;
        offset = start + size + (size & 1);
    }
    let (width, height) = dimensions.ok_or(invalid)?;
    Ok(ValidatedSticker {
        format: StickerFormat::Webp,
        width,
        height,
        duration_ms: None,
    })
}

fn le_u16(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn le_u24(bytes: &[u8], at: usize) -> Option<u32> {
    let raw = bytes.get(at..at + 3)?;
    Some(u32::from(raw[0]) | u32::from(raw[1]) << 8 | u32::from(raw[2]) << 16)
}

fn le_u32(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}
