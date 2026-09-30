//! WebM (EBML): read `Segment/Info` and `Segment/Tracks` for geometry and duration and
//! refuse any audio track. Clusters (the frames) are never read.

use super::{Animation, AnimationFormat, AnimationRejection};
use AnimationRejection::{InvalidFile, UnsupportedCodec};

const EBML: u32 = 0x1a45_dfa3;
const DOC_TYPE: u32 = 0x4282;
const CLUSTER: u32 = 0x1f43_b675;
const SEGMENT: u32 = 0x1853_8067;
const INFO: u32 = 0x1549_a966;
const TIMECODE_SCALE: u32 = 0x2a_d7b1;
const DURATION: u32 = 0x4489;
const TRACKS: u32 = 0x1654_ae6b;
const TRACK_ENTRY: u32 = 0xae;
const TRACK_TYPE: u32 = 0x83;
const VIDEO: u32 = 0xe0;
const PIXEL_WIDTH: u32 = 0xb0;
const PIXEL_HEIGHT: u32 = 0xba;

/// Reads one variable-length integer; `marker` keeps the length bit (element ids do).
fn vint(bytes: &[u8], at: usize, marker: bool) -> Result<(u64, usize, bool), AnimationRejection> {
    let first = *bytes.get(at).ok_or(InvalidFile)?;
    let length = first.leading_zeros() as usize + 1;
    if length > 8 {
        return Err(InvalidFile);
    }
    let mask = (0xff_u16 >> length) as u8;
    let mut value = u64::from(if marker { first } else { first & mask });
    let mut all_ones = !marker && first & mask == mask;
    for index in 1..length {
        let byte = *bytes.get(at + index).ok_or(InvalidFile)?;
        all_ones &= byte == 0xff;
        value = (value << 8) | u64::from(byte);
    }
    Ok((value, length, all_ones))
}

/// `(id, payload start, payload end)` of every element directly inside `start..end`.
fn elements(
    bytes: &[u8],
    start: usize,
    end: usize,
) -> Result<Vec<(u32, usize, usize)>, AnimationRejection> {
    let mut found = Vec::new();
    let mut at = start;
    while at < end {
        let (id, id_length, _) = vint(bytes, at, true)?;
        let (size, size_length, unknown) = vint(bytes, at + id_length, false)?;
        let payload = at + id_length + size_length;
        if payload > end {
            return Err(InvalidFile);
        }
        let payload_end = if unknown {
            end
        } else {
            payload
                .checked_add(usize::try_from(size).map_err(|_| InvalidFile)?)
                .ok_or(InvalidFile)?
        };
        if payload_end > end {
            return Err(InvalidFile);
        }
        found.push((
            u32::try_from(id).map_err(|_| InvalidFile)?,
            payload,
            payload_end,
        ));
        at = payload_end;
    }
    Ok(found)
}

fn ebml_uint(bytes: &[u8], start: usize, end: usize) -> Result<u64, AnimationRejection> {
    let slice = bytes
        .get(start..end)
        .filter(|slice| slice.len() <= 8)
        .ok_or(InvalidFile)?;
    Ok(slice
        .iter()
        .fold(0, |value, byte| (value << 8) | u64::from(*byte)))
}

fn ebml_float(bytes: &[u8], start: usize, end: usize) -> Result<f64, AnimationRejection> {
    let slice = bytes.get(start..end).ok_or(InvalidFile)?;
    match slice.len() {
        4 => Ok(f64::from(f32::from_be_bytes(
            slice.try_into().map_err(|_| InvalidFile)?,
        ))),
        8 => Ok(f64::from_be_bytes(
            slice.try_into().map_err(|_| InvalidFile)?,
        )),
        _ => Err(InvalidFile),
    }
}

pub(super) fn webm(bytes: &[u8]) -> Result<Animation, AnimationRejection> {
    let top = elements(bytes, 0, bytes.len())?;
    let header = top
        .first()
        .filter(|(id, _, _)| *id == EBML)
        .ok_or(InvalidFile)?;
    let doc_type = elements(bytes, header.1, header.2)?
        .into_iter()
        .find(|(id, _, _)| *id == DOC_TYPE)
        .ok_or(InvalidFile)?;
    if &bytes[doc_type.1..doc_type.2] != b"webm" {
        return Err(UnsupportedCodec);
    }
    let segment = top
        .iter()
        .find(|(id, _, _)| *id == SEGMENT)
        .ok_or(InvalidFile)?;
    let mut timecode_scale = 1_000_000_u64;
    let mut duration_ticks = None;
    let mut geometry = None;
    for (id, start, end) in elements(bytes, segment.1, segment.2)? {
        match id {
            INFO => {
                for (field, start, end) in elements(bytes, start, end)? {
                    match field {
                        TIMECODE_SCALE => timecode_scale = ebml_uint(bytes, start, end)?,
                        DURATION => duration_ticks = Some(ebml_float(bytes, start, end)?),
                        _ => {}
                    }
                }
            }
            TRACKS => {
                for (entry, start, end) in elements(bytes, start, end)? {
                    if entry == TRACK_ENTRY && geometry.is_none() {
                        geometry = webm_track(bytes, start, end)?;
                    } else if entry == TRACK_ENTRY {
                        webm_track(bytes, start, end)?;
                    }
                }
            }
            // Clusters hold the frames; everything needed is before them.
            CLUSTER => break,
            _ => {}
        }
    }
    let (width, height) = geometry.ok_or(InvalidFile)?;
    let duration_ms = duration_ticks
        .map(|ticks| ticks * timecode_scale as f64 / 1_000_000.0)
        .filter(|ms| ms.is_finite() && *ms >= 0.0)
        .map(|ms| ms.min(f64::from(u32::MAX)) as u32);
    Ok(Animation {
        format: AnimationFormat::Webm,
        width: u32::try_from(width).map_err(|_| AnimationRejection::InvalidDimensions)?,
        height: u32::try_from(height).map_err(|_| AnimationRejection::InvalidDimensions)?,
        duration_ms,
    })
}

/// A video track's geometry; an audio track refuses the whole file.
fn webm_track(
    bytes: &[u8],
    start: usize,
    end: usize,
) -> Result<Option<(u64, u64)>, AnimationRejection> {
    let fields = elements(bytes, start, end)?;
    let mut track_type = None;
    let mut geometry = None;
    for (id, start, end) in fields {
        match id {
            TRACK_TYPE => track_type = Some(ebml_uint(bytes, start, end)?),
            VIDEO => {
                let (mut width, mut height) = (0, 0);
                for (field, start, end) in elements(bytes, start, end)? {
                    match field {
                        PIXEL_WIDTH => width = ebml_uint(bytes, start, end)?,
                        PIXEL_HEIGHT => height = ebml_uint(bytes, start, end)?,
                        _ => {}
                    }
                }
                geometry = Some((width, height));
            }
            _ => {}
        }
    }
    match track_type {
        Some(2) => Err(AnimationRejection::AudioNotAllowed),
        Some(1) => Ok(geometry),
        _ => Ok(None),
    }
}
