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
//! - **MP4**: a fragmented recording (`mvex`, what MediaRecorder writes) ends where its last
//!   fragment does: that fragment's `tfdt` start plus its `trun` sample durations (defaulted
//!   by `tfhd`/`trex`), at the track's `mdhd` timescale;
//!   a plain file uses `mvhd`, else the track's `mdhd`.
//!
//! - **WAV** (TG-1301): the `data` chunk's size over the `fmt ` chunk's byte rate. The web
//!   client converts a system recorder's file to 16 kHz mono PCM WAV when it is in none of the
//!   three recorder containers (an MP3 from an Android recorder, say), which is how voice works
//!   over plain http, where the browser grants no microphone.
//!
//! The sniffed container also decides the stored MIME type, so a client can never label an
//! arbitrary file as audio.

/// A recognised voice container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Container {
    Ogg,
    WebM,
    Mp4,
    Wav,
}

impl Container {
    pub fn mime_type(self) -> &'static str {
        match self {
            Container::Ogg => "audio/ogg",
            Container::WebM => "audio/webm",
            Container::Mp4 => "audio/mp4",
            Container::Wav => "audio/wav",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Container::Ogg => "ogg",
            Container::WebM => "webm",
            Container::Mp4 => "m4a",
            Container::Wav => "wav",
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
        Container::Wav => wav::wav_duration_ms(bytes),
    }
    .filter(|ms| *ms > 0);
    Some(Probe {
        container,
        duration_ms,
    })
}

/// TG-402: whether the file declares a video track — a WebM `CodecID` starting `V_`
/// (`V_VP8`, `V_VP9`, `V_AV1`, `V_MPEG4/…`), or an MP4 `trak` whose handler is `vide`.
/// Ogg never carries the recorder's video.
pub fn has_video_track(bytes: &[u8], container: Container) -> bool {
    match container {
        Container::Ogg | Container::Wav => false,
        // `Tracks` precede the first `Cluster`: look for a `CodecID` (0x86) element there.
        Container::WebM => {
            let head = &bytes[..bytes.len().min(64 * 1024)];
            (0..head.len()).any(|at| {
                head[at] == 0x86
                    && vint(&head[at + 1..], false).is_some_and(|(size, length)| {
                        size <= 32 && head[at + 1 + length..].starts_with(b"V_")
                    })
            })
        }
        Container::Mp4 => mp4::mp4_has_video(bytes),
    }
}

pub fn sniff(bytes: &[u8]) -> Option<Container> {
    if bytes.starts_with(b"OggS") {
        Some(Container::Ogg)
    } else if bytes.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        Some(Container::WebM)
    } else if bytes.get(4..8) == Some(b"ftyp") {
        Some(Container::Mp4)
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WAVE") {
        Some(Container::Wav)
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

#[path = "probe_mp4.rs"]
mod mp4;

#[path = "probe_wav.rs"]
mod wav;
use mp4::mp4_duration_ms;

#[cfg(test)]
#[path = "probe_tests.rs"]
mod tests;
