//! TGS: a gzip-compressed Lottie JSON document. Telegram's rules: 512×512 canvas, at most
//! 60 fps, at most 3 seconds. The inflated size is capped so a small gzip bomb cannot make
//! the server allocate without bound.

use std::io::Read;

use flate2::read::GzDecoder;
use serde_json::Value;

use super::{StickerRejection, ValidatedSticker, MAX_DURATION_MS};
use crate::stickers::models::StickerFormat;

/// Inflated Lottie JSON cap.
pub const MAX_INFLATED_BYTES: u64 = 1024 * 1024;
const CANVAS_SIDE: f64 = 512.0;
const MAX_FRAME_RATE: f64 = 60.0;

pub(super) fn validate(bytes: &[u8]) -> Result<ValidatedSticker, StickerRejection> {
    let mut json = Vec::new();
    GzDecoder::new(bytes)
        .take(MAX_INFLATED_BYTES + 1)
        .read_to_end(&mut json)
        .map_err(|_| StickerRejection::InvalidGzip)?;
    if json.len() as u64 > MAX_INFLATED_BYTES {
        return Err(StickerRejection::FileTooLarge);
    }
    let document: Value =
        serde_json::from_slice(&json).map_err(|_| StickerRejection::InvalidJson)?;
    let lottie = document
        .as_object()
        .ok_or(StickerRejection::InvalidLottie)?;
    let number = |key: &str| lottie.get(key).and_then(Value::as_f64);
    let (Some(frame_rate), Some(in_point), Some(out_point), Some(width), Some(height)) = (
        number("fr"),
        number("ip"),
        number("op"),
        number("w"),
        number("h"),
    ) else {
        return Err(StickerRejection::InvalidLottie);
    };
    let has_version = lottie.get("v").is_some_and(Value::is_string);
    let has_layers = lottie.get("layers").is_some_and(Value::is_array);
    if !has_version || !has_layers || frame_rate <= 0.0 || out_point <= in_point {
        return Err(StickerRejection::InvalidLottie);
    }
    if width != CANVAS_SIDE || height != CANVAS_SIDE {
        return Err(StickerRejection::InvalidDimensions);
    }
    if frame_rate > MAX_FRAME_RATE {
        return Err(StickerRejection::FrameRateTooHigh);
    }
    let duration_ms = ((out_point - in_point) / frame_rate * 1000.0).round();
    if duration_ms > MAX_DURATION_MS as f64 {
        return Err(StickerRejection::TooLong);
    }
    Ok(ValidatedSticker {
        format: StickerFormat::Tgs,
        width: CANVAS_SIDE as u32,
        height: CANVAS_SIDE as u32,
        duration_ms: Some(duration_ms as u32),
    })
}
