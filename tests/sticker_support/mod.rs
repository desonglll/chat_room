//! TG-302 test fixtures: byte-exact sticker files built in code (headers only — the server
//! never decodes pixels), plus a scratch HTTP server.
#![allow(dead_code)]

pub mod http;

use std::io::Write;

use flate2::{write::GzEncoder, Compression};

// ── WebP ─────────────────────────────────────────────────────────────────────

fn riff(chunks: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
    let mut body = b"WEBP".to_vec();
    for (fourcc, payload) in chunks {
        body.extend_from_slice(*fourcc);
        body.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        body.extend_from_slice(payload);
        if payload.len() % 2 == 1 {
            body.push(0);
        }
    }
    let mut file = b"RIFF".to_vec();
    file.extend_from_slice(&(body.len() as u32).to_le_bytes());
    file.extend(body);
    file
}

fn le24(value: u32) -> [u8; 3] {
    let bytes = value.to_le_bytes();
    [bytes[0], bytes[1], bytes[2]]
}

/// Lossless static WebP.
pub fn webp_lossless(width: u32, height: u32) -> Vec<u8> {
    let bits = (width - 1) | (height - 1) << 14;
    let mut payload = vec![0x2f];
    payload.extend_from_slice(&bits.to_le_bytes());
    payload.extend_from_slice(&[0; 8]);
    riff(&[(b"VP8L", payload)])
}

/// Lossy static WebP.
pub fn webp_lossy(width: u16, height: u16) -> Vec<u8> {
    let mut payload = vec![0x10, 0x02, 0x00, 0x9d, 0x01, 0x2a];
    payload.extend_from_slice(&width.to_le_bytes());
    payload.extend_from_slice(&height.to_le_bytes());
    payload.extend_from_slice(&[0; 6]);
    riff(&[(b"VP8 ", payload)])
}

/// Extended WebP; `animated` sets the VP8X animation flag and adds an ANIM chunk.
pub fn webp_extended(width: u32, height: u32, animated: bool) -> Vec<u8> {
    let mut vp8x = vec![if animated { 0x02 } else { 0x00 }, 0, 0, 0];
    vp8x.extend_from_slice(&le24(width - 1));
    vp8x.extend_from_slice(&le24(height - 1));
    let mut chunks = vec![(b"VP8X", vp8x)];
    if animated {
        chunks.push((b"ANIM", vec![0; 6]));
    }
    let lossless = webp_lossless(width, height);
    chunks.push((b"VP8L", lossless[20..].to_vec()));
    riff(&chunks)
}

// ── TGS ──────────────────────────────────────────────────────────────────────

pub fn lottie(width: u32, height: u32, frame_rate: f64, frames: f64) -> serde_json::Value {
    serde_json::json!({
        "tgs": 1, "v": "5.5.2", "fr": frame_rate, "ip": 0, "op": frames,
        "w": width, "h": height, "nm": "fixture", "ddd": 0, "assets": [],
        "layers": [{ "ty": 4, "nm": "shape", "ip": 0, "op": frames, "st": 0, "shapes": [] }]
    })
}

pub fn gzip(bytes: &[u8], level: Compression) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), level);
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

pub fn tgs(document: &serde_json::Value) -> Vec<u8> {
    gzip(document.to_string().as_bytes(), Compression::best())
}

/// A valid 512×512, 60 fps, 2-second animation.
pub fn valid_tgs() -> Vec<u8> {
    tgs(&lottie(512, 512, 60.0, 120.0))
}

// ── WebM ─────────────────────────────────────────────────────────────────────

fn element(id: &[u8], payload: &[u8]) -> Vec<u8> {
    let mut bytes = id.to_vec();
    bytes.push(0x01);
    bytes.extend_from_slice(&(payload.len() as u64).to_be_bytes()[1..]);
    bytes.extend_from_slice(payload);
    bytes
}

fn uint(value: u64) -> Vec<u8> {
    let bytes = value.to_be_bytes();
    let first = bytes.iter().position(|byte| *byte != 0).unwrap_or(7);
    bytes[first..].to_vec()
}

pub struct WebmSpec {
    pub codec: &'static str,
    pub width: u64,
    pub height: u64,
    pub duration_ms: Option<f64>,
    pub frame_duration_ns: Option<u64>,
    pub audio: bool,
    pub doc_type: &'static [u8],
    pub unknown_size_segment: bool,
}

impl Default for WebmSpec {
    fn default() -> Self {
        WebmSpec {
            codec: "V_VP9",
            width: 512,
            height: 512,
            duration_ms: Some(2_500.0),
            frame_duration_ns: Some(33_333_333),
            audio: false,
            doc_type: b"webm",
            unknown_size_segment: false,
        }
    }
}

pub fn webm(spec: WebmSpec) -> Vec<u8> {
    let header = element(
        &[0x1a, 0x45, 0xdf, 0xa3],
        &element(&[0x42, 0x82], spec.doc_type),
    );
    let mut info = element(&[0x2a, 0xd7, 0xb1], &uint(1_000_000));
    if let Some(duration) = spec.duration_ms {
        info.extend(element(&[0x44, 0x89], &duration.to_be_bytes()));
    }
    let mut video = element(&[0xb0], &uint(spec.width));
    video.extend(element(&[0xba], &uint(spec.height)));
    let mut entry = element(&[0x83], &[1]);
    entry.extend(element(&[0x86], spec.codec.as_bytes()));
    if let Some(frame) = spec.frame_duration_ns {
        entry.extend(element(&[0x23, 0xe3, 0x83], &uint(frame)));
    }
    entry.extend(element(&[0xe0], &video));
    let mut tracks = element(&[0xae], &entry);
    if spec.audio {
        let mut audio = element(&[0x83], &[2]);
        audio.extend(element(&[0x86], b"A_OPUS"));
        tracks.extend(element(&[0xae], &audio));
    }
    let mut segment = element(&[0x15, 0x49, 0xa9, 0x66], &info);
    segment.extend(element(&[0x16, 0x54, 0xae, 0x6b], &tracks));
    segment.extend(element(
        &[0x1f, 0x43, 0xb6, 0x75],
        &[0xe7, 0x81, 0x00, 0xa3, 0x80],
    ));
    let mut file = header;
    if spec.unknown_size_segment {
        file.extend_from_slice(&[
            0x18, 0x53, 0x80, 0x67, 0x01, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        ]);
        file.extend(segment);
    } else {
        file.extend(element(&[0x18, 0x53, 0x80, 0x67], &segment));
    }
    file
}

pub fn valid_webm() -> Vec<u8> {
    webm(WebmSpec::default())
}
