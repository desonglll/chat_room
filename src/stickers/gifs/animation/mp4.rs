//! MP4 (ISO BMFF): walk `moov` for the video track's geometry and codec and refuse any
//! sound track. Only box headers are read.

use super::{Animation, AnimationFormat, AnimationRejection};
use AnimationRejection::{InvalidFile, UnsupportedCodec};

/// `(type, payload start, payload end)` of every box directly inside `start..end`.
fn boxes(
    bytes: &[u8],
    start: usize,
    end: usize,
) -> Result<Vec<([u8; 4], usize, usize)>, AnimationRejection> {
    let mut found = Vec::new();
    let mut at = start;
    while at + 8 <= end {
        let size = u64::from(be32(bytes, at)?);
        let kind: [u8; 4] = bytes[at + 4..at + 8].try_into().map_err(|_| InvalidFile)?;
        let (header, size) = match size {
            0 => (8, (end - at) as u64),
            1 => (16, be64(bytes, at + 8)?),
            size => (8, size),
        };
        let box_end = at.checked_add(usize::try_from(size).map_err(|_| InvalidFile)?);
        let box_end = box_end.filter(|box_end| *box_end <= end && size >= header as u64);
        let box_end = box_end.ok_or(InvalidFile)?;
        found.push((kind, at + header, box_end));
        at = box_end;
    }
    Ok(found)
}

fn child(
    bytes: &[u8],
    parent: (usize, usize),
    kind: &[u8; 4],
) -> Result<Option<(usize, usize)>, AnimationRejection> {
    Ok(boxes(bytes, parent.0, parent.1)?
        .into_iter()
        .find(|(found, _, _)| found == kind)
        .map(|(_, start, end)| (start, end)))
}

pub(super) fn mp4(bytes: &[u8]) -> Result<Animation, AnimationRejection> {
    let moov = child(bytes, (0, bytes.len()), b"moov")?.ok_or(InvalidFile)?;
    let mut duration_ms = None;
    if let Some((start, _)) = child(bytes, moov, b"mvhd")? {
        let (timescale, duration) = match *bytes.get(start).ok_or(InvalidFile)? {
            1 => (be32(bytes, start + 20)?, be64(bytes, start + 24)?),
            _ => (
                be32(bytes, start + 12)?,
                u64::from(be32(bytes, start + 16)?),
            ),
        };
        if timescale > 0 {
            duration_ms = u32::try_from(duration.saturating_mul(1000) / u64::from(timescale)).ok();
        }
    }
    let mut geometry = None;
    for (kind, start, end) in boxes(bytes, moov.0, moov.1)? {
        if &kind != b"trak" {
            continue;
        }
        let mdia = child(bytes, (start, end), b"mdia")?.ok_or(InvalidFile)?;
        let hdlr = child(bytes, mdia, b"hdlr")?.ok_or(InvalidFile)?;
        match bytes.get(hdlr.0 + 8..hdlr.0 + 12).ok_or(InvalidFile)? {
            b"soun" => return Err(AnimationRejection::AudioNotAllowed),
            b"vide" if geometry.is_none() => {
                let codec = mp4_codec(bytes, mdia)?;
                if !matches!(&codec, b"avc1" | b"avc3") {
                    return Err(UnsupportedCodec);
                }
                let tkhd = child(bytes, (start, end), b"tkhd")?.ok_or(InvalidFile)?;
                // Width and height are the last two 16.16 fixed-point fields of `tkhd`.
                let width = be32(bytes, tkhd.1.checked_sub(8).ok_or(InvalidFile)?)? >> 16;
                let height = be32(bytes, tkhd.1 - 4)? >> 16;
                geometry = Some((width, height));
            }
            _ => {}
        }
    }
    let (width, height) = geometry.ok_or(InvalidFile)?;
    Ok(Animation {
        format: AnimationFormat::Mp4,
        width,
        height,
        duration_ms,
    })
}

/// The first sample entry's four-character code in `mdia/minf/stbl/stsd`.
fn mp4_codec(bytes: &[u8], mdia: (usize, usize)) -> Result<[u8; 4], AnimationRejection> {
    let minf = child(bytes, mdia, b"minf")?.ok_or(InvalidFile)?;
    let stbl = child(bytes, minf, b"stbl")?.ok_or(InvalidFile)?;
    let stsd = child(bytes, stbl, b"stsd")?.ok_or(InvalidFile)?;
    // version/flags (4) + entry_count (4), then the first entry's size (4) and type (4).
    bytes
        .get(stsd.0 + 12..stsd.0 + 16)
        .and_then(|kind| kind.try_into().ok())
        .ok_or(InvalidFile)
}

fn be32(bytes: &[u8], at: usize) -> Result<u32, AnimationRejection> {
    let slice = bytes.get(at..at + 4).ok_or(InvalidFile)?;
    Ok(u32::from_be_bytes(
        slice.try_into().map_err(|_| InvalidFile)?,
    ))
}

fn be64(bytes: &[u8], at: usize) -> Result<u64, AnimationRejection> {
    let slice = bytes.get(at..at + 8).ok_or(InvalidFile)?;
    Ok(u64::from_be_bytes(
        slice.try_into().map_err(|_| InvalidFile)?,
    ))
}
