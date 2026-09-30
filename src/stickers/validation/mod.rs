//! Server-side validation of uploaded sticker files.
//!
//! The format is decided by sniffing the bytes, never by the client's MIME type or file
//! name. Only headers are parsed — nothing is decoded or rendered — so validation is cheap
//! and needs no media dependency. Limits follow Telegram's sticker rules (see the table in
//! `docs/devlog/TG-302.md`).

mod tgs;
mod webm;
mod webp;

use super::models::{SetType, StickerFormat};

/// Longest side of a regular sticker; one side must be exactly this.
pub const REGULAR_SIDE: u32 = 512;
/// Both sides of a custom emoji image or video.
pub const CUSTOM_EMOJI_SIDE: u32 = 100;
/// Longest playback of an animated or video sticker.
pub const MAX_DURATION_MS: u64 = 3_000;

/// What a valid file turned out to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidatedSticker {
    pub format: StickerFormat,
    pub width: u32,
    pub height: u32,
    /// `None` for static WebP.
    pub duration_ms: Option<u32>,
}

/// Why a file was refused. [`StickerRejection::code`] is the stable wire string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StickerRejection {
    UnsupportedFormat,
    FileTooLarge,
    InvalidDimensions,
    AnimatedWebp,
    InvalidGzip,
    InvalidJson,
    InvalidLottie,
    TooLong,
    FrameRateTooHigh,
    InvalidWebm,
    UnsupportedCodec,
    AudioNotAllowed,
}

impl StickerRejection {
    pub const fn code(self) -> &'static str {
        match self {
            StickerRejection::UnsupportedFormat => "unsupported_format",
            StickerRejection::FileTooLarge => "file_too_large",
            StickerRejection::InvalidDimensions => "invalid_dimensions",
            StickerRejection::AnimatedWebp => "animated_webp",
            StickerRejection::InvalidGzip => "invalid_gzip",
            StickerRejection::InvalidJson => "invalid_json",
            StickerRejection::InvalidLottie => "invalid_lottie",
            StickerRejection::TooLong => "too_long",
            StickerRejection::FrameRateTooHigh => "frame_rate_too_high",
            StickerRejection::InvalidWebm => "invalid_webm",
            StickerRejection::UnsupportedCodec => "unsupported_codec",
            StickerRejection::AudioNotAllowed => "audio_not_allowed",
        }
    }
}

/// Validate one uploaded file for a set of `set_type`.
pub fn validate_sticker_file(
    bytes: &[u8],
    set_type: SetType,
) -> Result<ValidatedSticker, StickerRejection> {
    let format = sniff(bytes).ok_or(StickerRejection::UnsupportedFormat)?;
    if bytes.len() > format.max_bytes() {
        return Err(StickerRejection::FileTooLarge);
    }
    let validated = match format {
        StickerFormat::Webp => webp::validate(bytes)?,
        StickerFormat::Tgs => tgs::validate(bytes)?,
        StickerFormat::Webm => webm::validate(bytes)?,
    };
    // A TGS canvas is always 512×512 (checked in `tgs`), custom emoji included: the client
    // scales vector animation, so only raster formats carry the custom-emoji geometry.
    if format != StickerFormat::Tgs {
        check_geometry(validated.width, validated.height, set_type)?;
    }
    Ok(validated)
}

fn sniff(bytes: &[u8]) -> Option<StickerFormat> {
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some(StickerFormat::Webp)
    } else if bytes.starts_with(&[0x1f, 0x8b]) {
        Some(StickerFormat::Tgs)
    } else if bytes.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]) {
        Some(StickerFormat::Webm)
    } else {
        None
    }
}

fn check_geometry(width: u32, height: u32, set_type: SetType) -> Result<(), StickerRejection> {
    let valid = match set_type {
        SetType::Regular => {
            width <= REGULAR_SIDE
                && height <= REGULAR_SIDE
                && width.max(height) == REGULAR_SIDE
                && width.min(height) > 0
        }
        SetType::CustomEmoji => width == CUSTOM_EMOJI_SIDE && height == CUSTOM_EMOJI_SIDE,
    };
    valid
        .then_some(())
        .ok_or(StickerRejection::InvalidDimensions)
}
