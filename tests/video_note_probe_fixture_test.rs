//! TG-402: container probing of REAL round-video recordings, checked against ffprobe.
//!
//! Both were recorded by headless Chromium 153 through the recorder's own pipeline: a fake
//! 640×360 camera (`--use-file-for-fake-video-capture`, a generated Y4M), centre-cropped into a
//! 384×384 canvas, `canvas.captureStream()` + the fake microphone, ~2.5 s.
//!
//! - `chromium-video-note.webm` — `video/webm;codecs=vp9,opus`, live WebM (no `Duration`);
//!   ffprobe: 384×384 VP9, last packets at 2.434 s (video) and 2.409 s (audio).
//! - `chromium-video-note-fragmented.mp4` — `video/mp4` (the Safari branch; this Chromium has
//!   no H.264 encoder, so VP9 + Opus in fragmented MP4): two tracks on different clocks
//!   (video 1/30000, audio 1/48000); ffprobe: video 2.4741 s, audio 2.421 s.

use chat_room::attachments::voice::probe::{has_video_track, probe, Container};

const WEBM: &[u8] = include_bytes!("fixtures/video_note/chromium-video-note.webm");
const MP4: &[u8] = include_bytes!("fixtures/video_note/chromium-video-note-fragmented.mp4");
const VOICE_WEBM: &[u8] = include_bytes!("fixtures/voice/chromium-recorder.webm");
const VOICE_MP4: &[u8] = include_bytes!("fixtures/voice/chromium-fragmented.m4a");

#[test]
fn chromium_video_webm_duration_is_the_newest_block_of_any_track() {
    let probed = probe(WEBM).unwrap();
    assert_eq!(probed.container, Container::WebM);
    assert_eq!(probed.duration_ms, Some(2_434));
}

#[test]
fn two_track_fragmented_mp4_reads_each_track_on_its_own_clock() {
    // Before TG-402 every fragment was read on the first track's clock, so the 48 kHz audio
    // track counted at 30 kHz claimed 3.87 s.
    let probed = probe(MP4).unwrap();
    assert_eq!(probed.container, Container::Mp4);
    assert_eq!(probed.duration_ms, Some(2_474));
}

#[test]
fn only_files_with_a_video_track_are_video_notes() {
    assert!(has_video_track(WEBM, Container::WebM));
    assert!(has_video_track(MP4, Container::Mp4));
    assert!(!has_video_track(VOICE_WEBM, Container::WebM));
    assert!(!has_video_track(VOICE_MP4, Container::Mp4));
    assert!(!has_video_track(b"OggS", Container::Ogg));
}
