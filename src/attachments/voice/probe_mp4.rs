//! MP4 (ISO BMFF) duration: see the parent module's docs for the rules.

use super::to_ms;

/// Iterate the boxes directly inside `bytes`: `(type, body)`.
fn boxes(bytes: &[u8]) -> impl Iterator<Item = (&[u8], &[u8])> {
    let mut position = 0usize;
    std::iter::from_fn(move || {
        let header = bytes.get(position..position + 8)?;
        let mut size = u64::from(u32::from_be_bytes(header[0..4].try_into().ok()?));
        let kind = &header[4..8];
        let mut offset = 8;
        if size == 1 {
            size = u64::from_be_bytes(bytes.get(position + 8..position + 16)?.try_into().ok()?);
            offset = 16;
        } else if size == 0 {
            size = (bytes.len() - position) as u64;
        }
        let end = position.checked_add(usize::try_from(size).ok()?)?;
        let body = bytes.get(position + offset..end.min(bytes.len()))?;
        position = end;
        Some((kind, body))
    })
}

fn child<'a>(bytes: &'a [u8], kind: &[u8]) -> Option<&'a [u8]> {
    boxes(bytes)
        .find(|(found, _)| *found == kind)
        .map(|(_, body)| body)
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

/// `(timescale, duration)` of an `mvhd`/`mdhd` body (full box, version 0 or 1).
fn header_duration(body: &[u8]) -> Option<(u32, u64)> {
    if body.first()? == &1 {
        let timescale = u32_at(body, 20)?;
        let duration = u64::from_be_bytes(body.get(24..32)?.try_into().ok()?);
        Some((timescale, duration))
    } else {
        Some((u32_at(body, 12)?, u64::from(u32_at(body, 16)?)))
    }
}

fn usable((timescale, duration): (u32, u64)) -> Option<u32> {
    if duration == 0 || duration == u64::from(u32::MAX) || duration == u64::MAX {
        return None;
    }
    to_ms(u128::from(duration), u128::from(timescale))
}

pub(super) fn mp4_duration_ms(bytes: &[u8]) -> Option<u32> {
    let moov = child(bytes, b"moov")?;
    let mdhd = child(moov, b"trak")
        .and_then(|trak| child(trak, b"mdia"))
        .and_then(|mdia| child(mdia, b"mdhd"))
        .and_then(header_duration);
    // A fragmented file (`mvex`) keeps its samples in `moof`s; its `mvhd`/`mdhd` durations
    // describe only the (empty) initial segment — Chromium writes a few ticks there.
    if let Some(mvex) = child(moov, b"mvex") {
        let timescale = mdhd?.0;
        let default_duration = child(mvex, b"trex")
            .and_then(|trex| u32_at(trex, 12))
            .unwrap_or(0);
        // Each fragment starts at its `tfdt` decode time when it has one: Chromium writes a
        // short last-sample duration per fragment, so summing `trun`s alone undercounts.
        let (mut cursor, mut ticks): (u128, u128) = (0, 0);
        for (kind, moof) in boxes(bytes) {
            if kind != b"moof" {
                continue;
            }
            for (kind, traf) in boxes(moof) {
                if kind == b"traf" {
                    let start = child(traf, b"tfdt").and_then(decode_time).unwrap_or(cursor);
                    cursor = start + traf_ticks(traf, default_duration)?;
                    ticks = ticks.max(cursor);
                }
            }
        }
        if ticks > 0 {
            return to_ms(ticks, u128::from(timescale));
        }
    }
    child(moov, b"mvhd")
        .and_then(header_duration)
        .and_then(usable)
        .or_else(|| mdhd.and_then(usable))
}

/// `tfdt.baseMediaDecodeTime` (full box, version 0: 32 bits, version 1: 64 bits).
fn decode_time(tfdt: &[u8]) -> Option<u128> {
    if tfdt.first()? == &1 {
        Some(u128::from(u64::from_be_bytes(
            tfdt.get(4..12)?.try_into().ok()?,
        )))
    } else {
        Some(u128::from(u32_at(tfdt, 4)?))
    }
}

fn traf_ticks(traf: &[u8], trex_default: u32) -> Option<u128> {
    let mut default_duration = trex_default;
    if let Some(tfhd) = child(traf, b"tfhd") {
        let flags = u32_at(tfhd, 0)? & 0x00FF_FFFF;
        let mut at = 8; // version/flags + track_ID
        if flags & 0x01 != 0 {
            at += 8;
        }
        if flags & 0x02 != 0 {
            at += 4;
        }
        if flags & 0x08 != 0 {
            default_duration = u32_at(tfhd, at)?;
        }
    }
    let mut ticks: u128 = 0;
    for (kind, trun) in boxes(traf) {
        if kind != b"trun" {
            continue;
        }
        let flags = u32_at(trun, 0)? & 0x00FF_FFFF;
        let count = u32_at(trun, 4)?;
        let mut at =
            8 + if flags & 0x01 != 0 { 4 } else { 0 } + if flags & 0x04 != 0 { 4 } else { 0 };
        let per_sample = [0x100, 0x200, 0x400, 0x800]
            .iter()
            .filter(|bit| flags & **bit != 0)
            .count()
            * 4;
        for _ in 0..count {
            let duration = if flags & 0x100 != 0 {
                u32_at(trun, at)?
            } else {
                default_duration
            };
            ticks += u128::from(duration);
            at += per_sample;
        }
    }
    Some(ticks)
}
