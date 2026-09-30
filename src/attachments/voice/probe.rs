//! Container sniffing and duration probing for recorded voice files, without decoding audio.
//!
//! The recorder produces one of three containers: WebM/Opus (Chromium, Firefox), Ogg/Opus
//! (Firefox) or MP4/AAC (Safari). Each carries timing in its framing, so the duration can be
//! read by walking headers — no codec, no new dependency:
//!
//! - **Ogg**: the last page's granule position minus the OpusHead pre-skip, at 48 kHz.
//! - **WebM**: `Info/Duration` when present; MediaRecorder never writes it (it streams with
//!   unknown sizes), so otherwise the newest `Cluster/Timecode + Block` timestamp, i.e. the
//!   start of the last frame (the result is short by at most one frame, 20–60 ms).
//! - **MP4**: `mvhd`, else the track's `mdhd`, else the sum of fragment sample durations
//!   (`trun`, defaulted by `tfhd`/`trex`), which is what a fragmented recording carries.
//!
//! The sniffed container also decides the stored MIME type, so a client can never label an
//! arbitrary file as audio.

/// A recognised voice container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Container {
    Ogg,
    WebM,
    Mp4,
}

impl Container {
    pub fn mime_type(self) -> &'static str {
        match self {
            Container::Ogg => "audio/ogg",
            Container::WebM => "audio/webm",
            Container::Mp4 => "audio/mp4",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Container::Ogg => "ogg",
            Container::WebM => "webm",
            Container::Mp4 => "m4a",
        }
    }
}

/// What a probe learned: the container always, the duration when the framing carries it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Probe {
    pub container: Container,
    pub duration_ms: Option<u32>,
}

pub fn probe(bytes: &[u8]) -> Option<Probe> {
    let container = sniff(bytes)?;
    let duration_ms = match container {
        Container::Ogg => ogg_duration_ms(bytes),
        Container::WebM => webm_duration_ms(bytes),
        Container::Mp4 => mp4_duration_ms(bytes),
    }
    .filter(|ms| *ms > 0);
    Some(Probe {
        container,
        duration_ms,
    })
}

pub fn sniff(bytes: &[u8]) -> Option<Container> {
    if bytes.starts_with(b"OggS") {
        Some(Container::Ogg)
    } else if bytes.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        Some(Container::WebM)
    } else if bytes.get(4..8) == Some(b"ftyp") {
        Some(Container::Mp4)
    } else {
        None
    }
}

fn to_ms(ticks: u128, per_second: u128) -> Option<u32> {
    if per_second == 0 {
        return None;
    }
    u32::try_from(ticks * 1000 / per_second).ok()
}

// ── Ogg ─────────────────────────────────────────────────────────────────────────────────

fn ogg_duration_ms(bytes: &[u8]) -> Option<u32> {
    // First page: 27-byte header, segment table, then the OpusHead packet.
    let segments = usize::from(*bytes.get(26)?);
    let head = bytes.get(27 + segments..)?;
    if !head.starts_with(b"OpusHead") {
        return None;
    }
    let pre_skip = u64::from(u16::from_le_bytes([*head.get(10)?, *head.get(11)?]));
    let mut end = bytes.len();
    while let Some(start) = find_last(&bytes[..end], b"OggS") {
        if let Some(granule) = bytes.get(start + 6..start + 14) {
            let granule = i64::from_le_bytes(granule.try_into().ok()?);
            // -1: no packet finishes on this page; keep looking further back.
            if granule >= 0 && bytes.get(start + 4) == Some(&0) {
                let samples = u64::try_from(granule).ok()?.saturating_sub(pre_skip);
                return to_ms(u128::from(samples), 48_000);
            }
        }
        end = start;
    }
    None
}

fn find_last(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .rposition(|window| window == needle)
}

// ── WebM (EBML) ─────────────────────────────────────────────────────────────────────────

const SEGMENT: u32 = 0x1853_8067;
const CLUSTER: u32 = 0x1F43_B675;
const INFO: u32 = 0x1549_A966;
const BLOCK_GROUP: u32 = 0xA0;
const TIMECODE_SCALE: u32 = 0x2A_D7B1;
const DURATION: u32 = 0x4489;
const CLUSTER_TIMECODE: u32 = 0xE7;
const SIMPLE_BLOCK: u32 = 0xA3;
const BLOCK: u32 = 0xA1;

/// One EBML variable-length integer: `(value, length)`. `keep_marker` for element ids.
fn vint(bytes: &[u8], keep_marker: bool) -> Option<(u64, usize)> {
    let first = *bytes.first()?;
    let length = first.leading_zeros() as usize + 1;
    if length > 8 {
        return None;
    }
    let mut value = if keep_marker {
        u64::from(first)
    } else {
        u64::from(first) & ((1u64 << (8 - length)) - 1)
    };
    for byte in bytes.get(1..length)? {
        value = value << 8 | u64::from(*byte);
    }
    Some((value, length))
}

fn uint(bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .fold(0, |value, byte| value << 8 | u64::from(*byte))
}

fn webm_duration_ms(bytes: &[u8]) -> Option<u32> {
    let mut scale: u64 = 1_000_000;
    let mut declared: Option<f64> = None;
    let mut cluster_time: i64 = 0;
    let mut newest: Option<i64> = None;
    let mut position = 0;
    // A flat walk: master elements that matter are entered (their children follow inline),
    // everything else is skipped by size. Unknown sizes only ever occur on Segment/Cluster.
    while position < bytes.len() {
        let (id, id_length) = vint(&bytes[position..], true)?;
        let (size, size_length) = vint(bytes.get(position + id_length..)?, false)?;
        let data = position + id_length + size_length;
        let unknown = size == (1u64 << (7 * size_length)) - 1;
        let end = usize::try_from(size)
            .ok()
            .and_then(|size| data.checked_add(size));
        let body = || end.and_then(|end| bytes.get(data..end));
        match id as u32 {
            SEGMENT | CLUSTER | INFO | BLOCK_GROUP => {
                position = data;
                continue;
            }
            TIMECODE_SCALE => scale = uint(body()?),
            DURATION => {
                let raw = body()?;
                declared = match raw.len() {
                    4 => Some(f64::from(f32::from_be_bytes(raw.try_into().ok()?))),
                    8 => Some(f64::from_be_bytes(raw.try_into().ok()?)),
                    _ => None,
                };
            }
            CLUSTER_TIMECODE => cluster_time = i64::try_from(uint(body()?)).ok()?,
            SIMPLE_BLOCK | BLOCK => {
                if let Some(block) = body() {
                    let (_, track_length) = vint(block, false)?;
                    let relative = block.get(track_length..track_length + 2)?;
                    let at =
                        cluster_time + i64::from(i16::from_be_bytes([relative[0], relative[1]]));
                    newest = Some(newest.map_or(at, |current| current.max(at)));
                }
            }
            _ => {}
        }
        if unknown {
            break;
        }
        match end {
            Some(end) if end <= bytes.len() => position = end,
            _ => break,
        }
    }
    let nanos_per_tick = u128::from(scale);
    match declared.filter(|value| value.is_finite() && *value > 0.0) {
        Some(ticks) => to_ms((ticks * scale as f64) as u128, 1_000_000_000),
        None => to_ms(
            u128::try_from(newest?).ok()? * nanos_per_tick,
            1_000_000_000,
        ),
    }
}

// ── MP4 (ISO BMFF) ──────────────────────────────────────────────────────────────────────

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

fn mp4_duration_ms(bytes: &[u8]) -> Option<u32> {
    let moov = child(bytes, b"moov")?;
    if let Some(ms) = child(moov, b"mvhd")
        .and_then(header_duration)
        .and_then(usable)
    {
        return Some(ms);
    }
    let mdhd = child(moov, b"trak")
        .and_then(|trak| child(trak, b"mdia"))
        .and_then(|mdia| child(mdia, b"mdhd"))
        .and_then(header_duration)?;
    if let Some(ms) = usable(mdhd) {
        return Some(ms);
    }
    let default_duration = child(moov, b"mvex")
        .and_then(|mvex| child(mvex, b"trex"))
        .and_then(|trex| u32_at(trex, 12))
        .unwrap_or(0);
    let mut ticks: u128 = 0;
    for (kind, moof) in boxes(bytes) {
        if kind != b"moof" {
            continue;
        }
        for (kind, traf) in boxes(moof) {
            if kind == b"traf" {
                ticks += traf_ticks(traf, default_duration)?;
            }
        }
    }
    (ticks > 0)
        .then(|| to_ms(ticks, u128::from(mdhd.0)))
        .flatten()
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

#[cfg(test)]
#[path = "probe_tests.rs"]
mod tests;
