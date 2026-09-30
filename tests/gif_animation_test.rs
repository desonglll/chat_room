//! TG-305: header-only validation of uploaded GIFs and GIF-format videos.

mod gif_support;
mod sticker_support;

use chat_room::stickers::gifs::animation::{
    validate_animation, AnimationFormat, AnimationRejection, MAX_ANIMATION_BYTES,
};
use gif_support::{gif, mp4, valid_mp4, Mp4Spec};
use sticker_support::{webm, WebmSpec};

#[test]
fn a_real_gif_is_accepted_with_its_logical_screen() {
    let animation = validate_animation(&gif(320, 240)).unwrap();
    assert_eq!(animation.format, AnimationFormat::Gif);
    assert_eq!((animation.width, animation.height), (320, 240));
    assert_eq!(animation.duration_ms, None);
    assert_eq!(animation.format.mime_type(), "image/gif");
}

#[test]
fn a_silent_h264_mp4_is_accepted_with_geometry_and_duration() {
    let animation = validate_animation(&valid_mp4()).unwrap();
    assert_eq!(animation.format, AnimationFormat::Mp4);
    assert_eq!((animation.width, animation.height), (480, 270));
    assert_eq!(animation.duration_ms, Some(2_000));
}

#[test]
fn an_mp4_with_sound_or_another_codec_is_refused() {
    let audio = mp4(Mp4Spec {
        audio: true,
        ..Mp4Spec::default()
    });
    assert_eq!(
        validate_animation(&audio),
        Err(AnimationRejection::AudioNotAllowed)
    );
    let hevc = mp4(Mp4Spec {
        codec: b"hvc1",
        ..Mp4Spec::default()
    });
    assert_eq!(
        validate_animation(&hevc),
        Err(AnimationRejection::UnsupportedCodec)
    );
    let long = mp4(Mp4Spec {
        duration_ms: 61_000,
        ..Mp4Spec::default()
    });
    assert_eq!(validate_animation(&long), Err(AnimationRejection::TooLong));
}

#[test]
fn a_silent_webm_is_accepted_and_one_with_audio_is_refused() {
    let animation = validate_animation(&webm(WebmSpec {
        width: 640,
        height: 360,
        duration_ms: Some(4_000.0),
        ..WebmSpec::default()
    }))
    .unwrap();
    assert_eq!(animation.format, AnimationFormat::Webm);
    assert_eq!((animation.width, animation.height), (640, 360));
    assert_eq!(animation.duration_ms, Some(4_000));
    let unknown_size = webm(WebmSpec {
        unknown_size_segment: true,
        ..WebmSpec::default()
    });
    assert!(validate_animation(&unknown_size).is_ok());
    let audio = webm(WebmSpec {
        audio: true,
        ..WebmSpec::default()
    });
    assert_eq!(
        validate_animation(&audio),
        Err(AnimationRejection::AudioNotAllowed)
    );
}

#[test]
fn anything_else_is_refused_without_panicking() {
    assert_eq!(
        validate_animation(b"\x89PNG\r\n\x1a\n"),
        Err(AnimationRejection::UnsupportedFormat)
    );
    assert_eq!(
        validate_animation(&gif(0, 10)),
        Err(AnimationRejection::InvalidDimensions)
    );
    assert_eq!(
        validate_animation(b"GIF89a\x01"),
        Err(AnimationRejection::InvalidFile)
    );
    let mut oversized = gif(10, 10);
    oversized.resize(MAX_ANIMATION_BYTES + 1, 0);
    assert_eq!(
        validate_animation(&oversized),
        Err(AnimationRejection::FileTooLarge)
    );
    // Every truncation of a valid file is refused or accepted, never a panic.
    let full = valid_mp4();
    for length in 0..full.len() {
        let _ = validate_animation(&full[..length]);
    }
    let full = webm(WebmSpec::default());
    for length in 0..full.len() {
        let _ = validate_animation(&full[..length]);
    }
}
