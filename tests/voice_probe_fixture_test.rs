//! TG-401: container duration probing against REAL recordings, checked against ffprobe.
//!
//! - `chromium-recorder.webm` — Chromium 153 MediaRecorder, `audio/webm;codecs=opus`, the fake
//!   microphone held ~2.3 s. No `Duration` element (live WebM); ffprobe: last packet at
//!   2.280 s + 0.060 s. The probe reports the last block's start (documented: short by ≤ one
//!   frame).
//! - `chromium-fragmented.m4a` — the same browser with only `audio/mp4` allowed (the Safari
//!   fallback branch): fragmented MP4 whose `mvhd` claims 6 ms; ffprobe: 1.881 s.
//! - `ffmpeg-sine-1500ms.ogg` — `ffmpeg -f lavfi -i sine=duration=1.5 -c:a libopus`;
//!   ffprobe's format duration is 1.5065 s = 1.5 s + the 312-sample pre-skip, which a player
//!   discards; the playable duration is the 1.5 s that was encoded.

use chat_room::attachments::voice::probe::{probe, Container};

const WEBM: &[u8] = include_bytes!("fixtures/voice/chromium-recorder.webm");
const MP4: &[u8] = include_bytes!("fixtures/voice/chromium-fragmented.m4a");
const OGG: &[u8] = include_bytes!("fixtures/voice/ffmpeg-sine-1500ms.ogg");

#[test]
fn chromium_webm_duration_is_the_last_block() {
    let probed = probe(WEBM).unwrap();
    assert_eq!(probed.container, Container::WebM);
    assert_eq!(probed.duration_ms, Some(2_280));
}

#[test]
fn chromium_fragmented_mp4_duration_sums_its_fragments() {
    let probed = probe(MP4).unwrap();
    assert_eq!(probed.container, Container::Mp4);
    assert_eq!(probed.duration_ms, Some(1_881));
}

#[test]
fn ogg_opus_duration_is_the_playable_length() {
    let probed = probe(OGG).unwrap();
    assert_eq!(probed.container, Container::Ogg);
    assert_eq!(probed.duration_ms, Some(1_500));
}
