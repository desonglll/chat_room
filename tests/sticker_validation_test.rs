//! TG-302: server-side validation of the three sticker formats — each valid shape and each
//! rejection, through the public validator, plus the HTTP mapping of a rejection.

mod sticker_support;

use chat_room::{
    config::AppConfig,
    stickers::{
        models::{SetType, StickerFormat},
        validation::{validate_sticker_file, StickerRejection},
    },
};
use flate2::Compression;
use reqwest::StatusCode;
use sticker_support::{
    gzip, http, lottie, tgs, valid_tgs, valid_webm, webm, webp_extended, webp_lossless, webp_lossy,
    WebmSpec,
};

fn regular(bytes: &[u8]) -> Result<(StickerFormat, u32, u32, Option<u32>), StickerRejection> {
    validate_sticker_file(bytes, SetType::Regular)
        .map(|valid| (valid.format, valid.width, valid.height, valid.duration_ms))
}

#[test]
fn static_webp_in_all_three_encodings_is_accepted() {
    assert_eq!(
        regular(&webp_lossless(512, 512)),
        Ok((StickerFormat::Webp, 512, 512, None))
    );
    assert_eq!(
        regular(&webp_lossy(512, 300)),
        Ok((StickerFormat::Webp, 512, 300, None))
    );
    assert_eq!(
        regular(&webp_extended(200, 512, false)),
        Ok((StickerFormat::Webp, 200, 512, None))
    );
}

#[test]
fn webp_rejections() {
    assert_eq!(
        regular(&webp_lossless(511, 511)),
        Err(StickerRejection::InvalidDimensions)
    );
    assert_eq!(
        regular(&webp_extended(600, 512, false)),
        Err(StickerRejection::InvalidDimensions)
    );
    assert_eq!(
        regular(&webp_extended(512, 512, true)),
        Err(StickerRejection::AnimatedWebp)
    );
    let mut truncated = webp_lossless(512, 512);
    truncated.truncate(truncated.len() - 4);
    assert_eq!(
        regular(&truncated),
        Err(StickerRejection::UnsupportedFormat)
    );
    let mut oversized = webp_lossless(512, 512);
    oversized.resize(512 * 1024 + 1, 0);
    assert_eq!(regular(&oversized), Err(StickerRejection::FileTooLarge));
}

#[test]
fn custom_emoji_raster_stickers_must_be_100_square() {
    let custom = |bytes: &[u8]| validate_sticker_file(bytes, SetType::CustomEmoji).map(|v| v.width);
    assert_eq!(custom(&webp_lossless(100, 100)), Ok(100));
    assert_eq!(
        custom(&webp_lossless(512, 512)),
        Err(StickerRejection::InvalidDimensions)
    );
    let video = webm(WebmSpec {
        width: 100,
        height: 100,
        ..WebmSpec::default()
    });
    assert_eq!(custom(&video), Ok(100));
    // TGS keeps its 512 canvas for custom emoji too.
    assert_eq!(custom(&valid_tgs()), Ok(512));
}

#[test]
fn tgs_is_accepted_with_its_duration() {
    assert_eq!(
        regular(&valid_tgs()),
        Ok((StickerFormat::Tgs, 512, 512, Some(2_000)))
    );
    // Exactly 3 s at 30 fps is still allowed.
    assert_eq!(
        regular(&tgs(&lottie(512, 512, 30.0, 90.0))),
        Ok((StickerFormat::Tgs, 512, 512, Some(3_000)))
    );
}

#[test]
fn tgs_rejections() {
    let mut not_gzip = valid_tgs();
    not_gzip.truncate(20);
    assert_eq!(regular(&not_gzip), Err(StickerRejection::InvalidGzip));
    assert_eq!(
        regular(&gzip(b"{not json", Compression::best())),
        Err(StickerRejection::InvalidJson)
    );
    assert_eq!(
        regular(&gzip(br#"{"hello":"world"}"#, Compression::best())),
        Err(StickerRejection::InvalidLottie)
    );
    let mut no_layers = lottie(512, 512, 60.0, 60.0);
    no_layers.as_object_mut().unwrap().remove("layers");
    assert_eq!(
        regular(&tgs(&no_layers)),
        Err(StickerRejection::InvalidLottie)
    );
    assert_eq!(
        regular(&tgs(&lottie(512, 256, 60.0, 60.0))),
        Err(StickerRejection::InvalidDimensions)
    );
    assert_eq!(
        regular(&tgs(&lottie(512, 512, 120.0, 60.0))),
        Err(StickerRejection::FrameRateTooHigh)
    );
    assert_eq!(
        regular(&tgs(&lottie(512, 512, 60.0, 181.0))),
        Err(StickerRejection::TooLong)
    );
    // Over 64 KiB on the wire.
    let padded = serde_json::json!({ "padding": (0..70_000).map(|i| char::from(b'a' + (i * 7 % 26) as u8)).collect::<String>() });
    assert_eq!(
        regular(&gzip(padded.to_string().as_bytes(), Compression::none())),
        Err(StickerRejection::FileTooLarge)
    );
    // A small gzip that inflates past the 1 MiB cap.
    let bomb = format!(
        "{{\"v\":\"5\",\"pad\":\"{}\"}}",
        " ".repeat(2 * 1024 * 1024)
    );
    let bomb = gzip(bomb.as_bytes(), Compression::best());
    assert!(bomb.len() < 64 * 1024);
    assert_eq!(regular(&bomb), Err(StickerRejection::FileTooLarge));
}

#[test]
fn vp9_webm_is_accepted() {
    assert_eq!(
        regular(&valid_webm()),
        Ok((StickerFormat::Webm, 512, 512, Some(2_500)))
    );
    let live = webm(WebmSpec {
        unknown_size_segment: true,
        ..WebmSpec::default()
    });
    assert_eq!(
        regular(&live),
        Ok((StickerFormat::Webm, 512, 512, Some(2_500)))
    );
    let undeclared_rate = webm(WebmSpec {
        frame_duration_ns: None,
        height: 320,
        ..WebmSpec::default()
    });
    assert_eq!(
        regular(&undeclared_rate),
        Ok((StickerFormat::Webm, 512, 320, Some(2_500)))
    );
}

#[test]
fn webm_rejections() {
    let reject = |spec: WebmSpec| regular(&webm(spec)).unwrap_err();
    assert_eq!(
        reject(WebmSpec {
            codec: "V_VP8",
            ..WebmSpec::default()
        }),
        StickerRejection::UnsupportedCodec
    );
    assert_eq!(
        reject(WebmSpec {
            audio: true,
            ..WebmSpec::default()
        }),
        StickerRejection::AudioNotAllowed
    );
    assert_eq!(
        reject(WebmSpec {
            duration_ms: Some(3_500.0),
            ..WebmSpec::default()
        }),
        StickerRejection::TooLong
    );
    assert_eq!(
        reject(WebmSpec {
            duration_ms: None,
            ..WebmSpec::default()
        }),
        StickerRejection::InvalidWebm
    );
    assert_eq!(
        reject(WebmSpec {
            frame_duration_ns: Some(16_666_667),
            ..WebmSpec::default()
        }),
        StickerRejection::FrameRateTooHigh
    );
    assert_eq!(
        reject(WebmSpec {
            width: 640,
            ..WebmSpec::default()
        }),
        StickerRejection::InvalidDimensions
    );
    assert_eq!(
        reject(WebmSpec {
            doc_type: b"matroska",
            ..WebmSpec::default()
        }),
        StickerRejection::InvalidWebm
    );
    let mut truncated = valid_webm();
    truncated.truncate(truncated.len() - 3);
    assert_eq!(regular(&truncated), Err(StickerRejection::InvalidWebm));
    let mut oversized = valid_webm();
    oversized.resize(256 * 1024 + 1, 0);
    assert_eq!(regular(&oversized), Err(StickerRejection::FileTooLarge));
}

#[test]
fn unknown_bytes_are_refused_whatever_the_client_claims() {
    assert_eq!(
        regular(b"\x89PNG\r\n\x1a\n...."),
        Err(StickerRejection::UnsupportedFormat)
    );
    assert_eq!(regular(b""), Err(StickerRejection::UnsupportedFormat));
}

#[tokio::test]
async fn http_upload_accepts_each_format_and_reports_rejection_codes() {
    let server = http::start(AppConfig::default()).await;
    let owner = server.token("sticker-validation-owner").await;
    server
        .create_set(&owner, "Validation_Pack", "regular")
        .await;

    for (bytes, format, mime) in [
        (webp_lossless(512, 512), "webp", "image/webp"),
        (valid_tgs(), "tgs", "application/x-tgsticker"),
        (valid_webm(), "webm", "video/webm"),
    ] {
        let (status, sticker) = server
            .upload(&owner, "validation_pack", bytes.clone(), "😀 🎉")
            .await;
        assert_eq!(status, StatusCode::CREATED, "{format}: {sticker}");
        assert_eq!(sticker["format"], format);
        assert_eq!(sticker["mime_type"], mime);
        assert_eq!(sticker["emojis"], serde_json::json!(["😀", "🎉"]));
        let (status, body) = server.fetch(sticker["file_url"].as_str().unwrap()).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, bytes, "{format} catalogue file round-trips");
    }

    for (bytes, emoji, code) in [
        (webp_extended(512, 512, true), "😀", "animated_webp"),
        (tgs(&lottie(512, 512, 60.0, 600.0)), "😀", "too_long"),
        (
            webm(WebmSpec {
                audio: true,
                ..WebmSpec::default()
            }),
            "😀",
            "audio_not_allowed",
        ),
        (b"plain text".to_vec(), "😀", "unsupported_format"),
        (webp_lossless(512, 512), "hello", "invalid_emoji"),
    ] {
        let (status, body) = server.upload(&owner, "validation_pack", bytes, emoji).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"], code);
    }

    let stranger = server.token("sticker-validation-stranger").await;
    let (status, _) = server
        .upload(&stranger, "validation_pack", webp_lossless(512, 512), "😀")
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "only the owner edits a set");
    let set: serde_json::Value = server
        .client
        .get(server.url("/api/sticker-sets/VALIDATION_PACK"))
        .bearer_auth(&stranger)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(set["stickers"].as_array().unwrap().len(), 3);
}
