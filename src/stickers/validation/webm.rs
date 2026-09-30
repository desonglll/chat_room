//! WebM video stickers: VP9, no audio, at most 3 seconds, at most 30 fps.
//!
//! A minimal EBML walker reads only the header, `Segment/Info` and `Segment/Tracks`;
//! clusters (the frames) are skipped, never decoded.

use super::{StickerRejection, ValidatedSticker, MAX_DURATION_MS};
use crate::stickers::models::StickerFormat;

const EBML: u32 = 0x1a45_dfa3;
const DOC_TYPE: u32 = 0x4282;
const SEGMENT: u32 = 0x1853_8067;
const INFO: u32 = 0x1549_a966;
const TIMECODE_SCALE: u32 = 0x2a_d7b1;
const DURATION: u32 = 0x4489;
const TRACKS: u32 = 0x1654_ae6b;
const TRACK_ENTRY: u32 = 0xae;
const TRACK_TYPE: u32 = 0x83;
const CODEC_ID: u32 = 0x86;
const DEFAULT_DURATION: u32 = 0x23_e383;
const VIDEO: u32 = 0xe0;
const PIXEL_WIDTH: u32 = 0xb0;
const PIXEL_HEIGHT: u32 = 0xba;
const TRACK_TYPE_VIDEO: u64 = 1;
const TRACK_TYPE_AUDIO: u64 = 2;
/// 30 fps plus rounding slack for encoders that write 33.333 ms as 33 ms.
const MAX_FRAME_RATE: f64 = 30.5;

const INVALID: StickerRejection = StickerRejection::InvalidWebm;

#[derive(Clone, Copy)]
struct Element {
    id: u32,
    start: usize,
    end: usize,
}

#[derive(Default)]
struct VideoTrack {
    codec: Option<String>,
    width: Option<u64>,
    height: Option<u64>,
    frame_duration_ns: Option<u64>,
}

pub(super) fn validate(bytes: &[u8]) -> Result<ValidatedSticker, StickerRejection> {
    let top = children(bytes, 0, bytes.len())?;
    let header = top
        .first()
        .filter(|element| element.id == EBML)
        .ok_or(INVALID)?;
    let doc_type = children(bytes, header.start, header.end)?
        .into_iter()
        .find(|element| element.id == DOC_TYPE)
        .ok_or(INVALID)?;
    if &bytes[doc_type.start..doc_type.end] != b"webm" {
        return Err(INVALID);
    }
    let segment = top
        .iter()
        .find(|element| element.id == SEGMENT)
        .ok_or(INVALID)?;
    let mut timecode_scale = 1_000_000_u64;
    let mut duration_ticks = None;
    let mut video = None;
    for element in children(bytes, segment.start, segment.end)? {
        match element.id {
            INFO => {
                for field in children(bytes, element.start, element.end)? {
                    match field.id {
                        TIMECODE_SCALE => timecode_scale = uint(bytes, field)?,
                        DURATION => duration_ticks = Some(float(bytes, field)?),
                        _ => {}
                    }
                }
            }
            TRACKS => {
                for entry in children(bytes, element.start, element.end)? {
                    if entry.id != TRACK_ENTRY {
                        continue;
                    }
                    let (kind, track) = track_entry(bytes, entry)?;
                    match kind {
                        TRACK_TYPE_AUDIO => return Err(StickerRejection::AudioNotAllowed),
                        TRACK_TYPE_VIDEO if video.is_none() => video = Some(track),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    let video = video.ok_or(INVALID)?;
    if video.codec.as_deref() != Some("V_VP9") {
        return Err(StickerRejection::UnsupportedCodec);
    }
    let width = u32::try_from(video.width.ok_or(INVALID)?).map_err(|_| INVALID)?;
    let height = u32::try_from(video.height.ok_or(INVALID)?).map_err(|_| INVALID)?;
    let duration_ms = (duration_ticks.ok_or(INVALID)? * timecode_scale as f64 / 1e6).round();
    if !duration_ms.is_finite() || duration_ms <= 0.0 {
        return Err(INVALID);
    }
    if duration_ms > MAX_DURATION_MS as f64 {
        return Err(StickerRejection::TooLong);
    }
    if let Some(frame_ns) = video.frame_duration_ns {
        if frame_ns == 0 || 1e9 / frame_ns as f64 > MAX_FRAME_RATE {
            return Err(StickerRejection::FrameRateTooHigh);
        }
    }
    Ok(ValidatedSticker {
        format: StickerFormat::Webm,
        width,
        height,
        duration_ms: Some(duration_ms as u32),
    })
}

fn track_entry(bytes: &[u8], entry: Element) -> Result<(u64, VideoTrack), StickerRejection> {
    let mut kind = 0;
    let mut track = VideoTrack::default();
    for field in children(bytes, entry.start, entry.end)? {
        match field.id {
            TRACK_TYPE => kind = uint(bytes, field)?,
            CODEC_ID => {
                let raw = &bytes[field.start..field.end];
                let codec = String::from_utf8_lossy(raw);
                track.codec = Some(codec.trim_end_matches('\0').to_string());
            }
            DEFAULT_DURATION => track.frame_duration_ns = Some(uint(bytes, field)?),
            VIDEO => {
                for setting in children(bytes, field.start, field.end)? {
                    match setting.id {
                        PIXEL_WIDTH => track.width = Some(uint(bytes, setting)?),
                        PIXEL_HEIGHT => track.height = Some(uint(bytes, setting)?),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    Ok((kind, track))
}

/// The elements directly inside `[start, end)`. An unknown-size element (live-streamed
/// segments and clusters) extends to `end` and is necessarily the last one.
fn children(bytes: &[u8], start: usize, end: usize) -> Result<Vec<Element>, StickerRejection> {
    let mut elements = Vec::new();
    let mut offset = start;
    while offset < end {
        let (id, id_length) = element_id(bytes, offset)?;
        let (size, size_length) = vint(bytes, offset + id_length)?;
        let data_start = offset + id_length + size_length;
        let data_end = match size {
            Some(size) => usize::try_from(size)
                .ok()
                .and_then(|size| data_start.checked_add(size))
                .ok_or(INVALID)?,
            None => end,
        };
        if data_end > end {
            return Err(INVALID);
        }
        elements.push(Element {
            id,
            start: data_start,
            end: data_end,
        });
        offset = data_end;
    }
    Ok(elements)
}

fn element_id(bytes: &[u8], at: usize) -> Result<(u32, usize), StickerRejection> {
    let first = *bytes.get(at).ok_or(INVALID)?;
    let length = first.leading_zeros() as usize + 1;
    if length > 4 {
        return Err(INVALID);
    }
    let raw = bytes.get(at..at + length).ok_or(INVALID)?;
    Ok((
        raw.iter().fold(0, |id, byte| id << 8 | u32::from(*byte)),
        length,
    ))
}

/// An EBML size: `None` means "unknown" (all value bits set).
fn vint(bytes: &[u8], at: usize) -> Result<(Option<u64>, usize), StickerRejection> {
    let first = *bytes.get(at).ok_or(INVALID)?;
    let length = first.leading_zeros() as usize + 1;
    if length > 8 {
        return Err(INVALID);
    }
    let raw = bytes.get(at..at + length).ok_or(INVALID)?;
    let marker_cleared = u64::from(first) & ((1 << (8 - length)) - 1);
    let value = raw[1..]
        .iter()
        .fold(marker_cleared, |value, byte| value << 8 | u64::from(*byte));
    let unknown = value == (1_u64 << (7 * length)) - 1;
    Ok((if unknown { None } else { Some(value) }, length))
}

fn uint(bytes: &[u8], element: Element) -> Result<u64, StickerRejection> {
    let raw = &bytes[element.start..element.end];
    if raw.len() > 8 {
        return Err(INVALID);
    }
    Ok(raw
        .iter()
        .fold(0, |value, byte| value << 8 | u64::from(*byte)))
}

fn float(bytes: &[u8], element: Element) -> Result<f64, StickerRejection> {
    let raw = &bytes[element.start..element.end];
    match raw.len() {
        4 => Ok(f64::from(f32::from_be_bytes(
            raw.try_into().map_err(|_| INVALID)?,
        ))),
        8 => Ok(f64::from_be_bytes(raw.try_into().map_err(|_| INVALID)?)),
        _ => Err(INVALID),
    }
}
