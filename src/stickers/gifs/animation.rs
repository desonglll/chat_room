//! Header-only validation of an uploaded "GIF": a real GIF, or a short silent MP4 (H.264)
//! or WebM video — the format Telegram actually stores GIFs in.
//!
//! The format is sniffed from the bytes, never taken from the client. Only container
//! headers are walked (no decoding, no media dependency), which is enough to read the
//! geometry for the panel's masonry layout and to refuse a video that carries sound: a
//! GIF is always played muted, so an audio track would be silently lost.

/// Largest accepted animation. Telegram converts GIFs up to 20 MB into MP4s far smaller.
pub const MAX_ANIMATION_BYTES: usize = 10 * 1024 * 1024;
/// Longest accepted animation when the container states a duration.
pub const MAX_ANIMATION_DURATION_MS: u64 = 60_000;
/// Longest accepted side, in pixels.
pub const MAX_ANIMATION_SIDE: u64 = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationFormat {
    Gif,
    Mp4,
    Webm,
}

impl AnimationFormat {
    pub const fn mime_type(self) -> &'static str {
        match self {
            AnimationFormat::Gif => "image/gif",
            AnimationFormat::Mp4 => "video/mp4",
            AnimationFormat::Webm => "video/webm",
        }
    }

    pub const fn extension(self) -> &'static str {
        match self {
            AnimationFormat::Gif => "gif",
            AnimationFormat::Mp4 => "mp4",
            AnimationFormat::Webm => "webm",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Animation {
    pub format: AnimationFormat,
    pub width: u32,
    pub height: u32,
    pub duration_ms: Option<u32>,
}

/// Why an upload is not an acceptable GIF; `code` is the stable wire string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationRejection {
    UnsupportedFormat,
    FileTooLarge,
    InvalidFile,
    InvalidDimensions,
    UnsupportedCodec,
    AudioNotAllowed,
    TooLong,
}

impl AnimationRejection {
    pub const fn code(self) -> &'static str {
        match self {
            AnimationRejection::UnsupportedFormat => "unsupported_format",
            AnimationRejection::FileTooLarge => "file_too_large",
            AnimationRejection::InvalidFile => "invalid_file",
            AnimationRejection::InvalidDimensions => "invalid_dimensions",
            AnimationRejection::UnsupportedCodec => "unsupported_codec",
            AnimationRejection::AudioNotAllowed => "audio_not_allowed",
            AnimationRejection::TooLong => "too_long",
        }
    }
}

mod mp4;
mod webm;

use AnimationRejection::InvalidFile;

pub fn validate_animation(bytes: &[u8]) -> Result<Animation, AnimationRejection> {
    if bytes.len() > MAX_ANIMATION_BYTES {
        return Err(AnimationRejection::FileTooLarge);
    }
    let animation = if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        gif(bytes)?
    } else if bytes.len() >= 8 && &bytes[4..8] == b"ftyp" {
        mp4::mp4(bytes)?
    } else if bytes.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]) {
        webm::webm(bytes)?
    } else {
        return Err(AnimationRejection::UnsupportedFormat);
    };
    let sides = [u64::from(animation.width), u64::from(animation.height)];
    if sides
        .iter()
        .any(|side| *side == 0 || *side > MAX_ANIMATION_SIDE)
    {
        return Err(AnimationRejection::InvalidDimensions);
    }
    if animation
        .duration_ms
        .is_some_and(|ms| u64::from(ms) > MAX_ANIMATION_DURATION_MS)
    {
        return Err(AnimationRejection::TooLong);
    }
    Ok(animation)
}

fn gif(bytes: &[u8]) -> Result<Animation, AnimationRejection> {
    let screen = bytes.get(6..10).ok_or(InvalidFile)?;
    Ok(Animation {
        format: AnimationFormat::Gif,
        width: u32::from(u16::from_le_bytes([screen[0], screen[1]])),
        height: u32::from(u16::from_le_bytes([screen[2], screen[3]])),
        duration_ms: None,
    })
}
