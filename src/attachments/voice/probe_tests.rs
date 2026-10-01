use super::*;

fn ogg_page(granule: i64, payload: &[u8]) -> Vec<u8> {
    let mut page = b"OggS".to_vec();
    page.push(0); // version
    page.push(0); // header type
    page.extend_from_slice(&granule.to_le_bytes());
    page.extend_from_slice(&[0; 12]); // serial, sequence, crc
    page.push(1);
    page.push(payload.len() as u8);
    page.extend_from_slice(payload);
    page
}

fn opus_head(pre_skip: u16) -> Vec<u8> {
    let mut head = b"OpusHead".to_vec();
    head.push(1);
    head.push(1);
    head.extend_from_slice(&pre_skip.to_le_bytes());
    head.extend_from_slice(&48_000u32.to_le_bytes());
    head.extend_from_slice(&[0, 0, 0]);
    head
}

#[test]
fn ogg_opus_duration_is_the_last_granule_minus_pre_skip() {
    let mut file = ogg_page(0, &opus_head(312));
    file.extend(ogg_page(0, b"OpusTags"));
    file.extend(ogg_page(48_000, &[1; 10]));
    file.extend(ogg_page(48_000 * 3 + 312, &[1; 10]));
    file.extend(ogg_page(-1, &[1; 3]));
    assert_eq!(
        probe(&file),
        Some(Probe {
            container: Container::Ogg,
            duration_ms: Some(3_000)
        })
    );
}

fn ebml(id: &[u8], body: &[u8]) -> Vec<u8> {
    let mut element = id.to_vec();
    element.push(0x01); // 8-byte size: marker byte, then 7 bytes
    element.extend_from_slice(&(body.len() as u64).to_be_bytes()[1..]);
    element.extend_from_slice(body);
    element
}

fn unknown_size(id: &[u8]) -> Vec<u8> {
    let mut element = id.to_vec();
    element.extend_from_slice(&[0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
    element
}

fn simple_block(relative: i16) -> Vec<u8> {
    let mut body = vec![0x81];
    body.extend_from_slice(&relative.to_be_bytes());
    body.push(0x80);
    body.extend_from_slice(&[0xFC; 20]);
    ebml(&[0xA3], &body)
}

/// A MediaRecorder-shaped WebM: live (unknown-size) Segment and Clusters, no Duration.
fn recorder_webm(clusters: &[(u16, &[i16])]) -> Vec<u8> {
    let mut file = ebml(&[0x1A, 0x45, 0xDF, 0xA3], &ebml(&[0x42, 0x82], b"webm"));
    file.extend(unknown_size(&[0x18, 0x53, 0x80, 0x67]));
    file.extend(ebml(
        &[0x15, 0x49, 0xA9, 0x66],
        &ebml(&[0x2A, 0xD7, 0xB1], &[0x0F, 0x42, 0x40]),
    ));
    file.extend(ebml(&[0x16, 0x54, 0xAE, 0x6B], &[0; 16])); // Tracks, skipped by size
    for (timecode, blocks) in clusters {
        file.extend(unknown_size(&[0x1F, 0x43, 0xB6, 0x75]));
        file.extend(ebml(&[0xE7], &timecode.to_be_bytes()));
        for relative in *blocks {
            file.extend(simple_block(*relative));
        }
    }
    file
}

#[test]
fn recorder_webm_duration_comes_from_the_newest_block() {
    let file = recorder_webm(&[(0, &[0, 20, 40]), (2_000, &[0, 500, 1_480])]);
    assert_eq!(
        probe(&file),
        Some(Probe {
            container: Container::WebM,
            duration_ms: Some(3_480)
        })
    );
}

#[test]
fn webm_declared_duration_wins() {
    let mut info = ebml(&[0x2A, 0xD7, 0xB1], &[0x0F, 0x42, 0x40]);
    info.extend(ebml(&[0x44, 0x89], &1234.0f64.to_be_bytes()));
    let mut file = ebml(&[0x1A, 0x45, 0xDF, 0xA3], &[]);
    file.extend(ebml(
        &[0x18, 0x53, 0x80, 0x67],
        &ebml(&[0x15, 0x49, 0xA9, 0x66], &info),
    ));
    assert_eq!(probe(&file).unwrap().duration_ms, Some(1_234));
}

fn mp4_box(kind: &[u8], body: &[u8]) -> Vec<u8> {
    let mut element = ((body.len() + 8) as u32).to_be_bytes().to_vec();
    element.extend_from_slice(kind);
    element.extend_from_slice(body);
    element
}

fn mdhd(timescale: u32, duration: u32) -> Vec<u8> {
    let mut body = vec![0; 12];
    body.extend_from_slice(&timescale.to_be_bytes());
    body.extend_from_slice(&duration.to_be_bytes());
    body.extend_from_slice(&[0; 4]);
    body
}

#[test]
fn mp4_duration_uses_mvhd_when_set() {
    let mut file = mp4_box(b"ftyp", b"M4A \0\0\0\0");
    file.extend(mp4_box(b"moov", &mp4_box(b"mvhd", &mdhd(1_000, 2_500))));
    assert_eq!(probe(&file).unwrap().duration_ms, Some(2_500));
}

#[test]
fn fragmented_mp4_duration_sums_the_fragment_samples() {
    let trak = mp4_box(
        b"trak",
        &mp4_box(b"mdia", &mp4_box(b"mdhd", &mdhd(44_100, 0))),
    );
    let mut trex = vec![0; 12];
    trex.extend_from_slice(&1024u32.to_be_bytes());
    trex.extend_from_slice(&[0; 8]);
    let mut moov = mp4_box(b"mvhd", &mdhd(1_000, 0));
    moov.extend(trak);
    moov.extend(mp4_box(b"mvex", &mp4_box(b"trex", &trex)));
    let mut file = mp4_box(b"ftyp", b"iso5\0\0\0\0");
    file.extend(mp4_box(b"moov", &moov));
    // Fragment 1: 43 samples at the trex default (1024 ticks each).
    let mut trun = vec![0, 0, 0, 0];
    trun.extend_from_slice(&43u32.to_be_bytes());
    let mut traf = mp4_box(b"tfhd", &[0, 0, 0, 0, 0, 0, 0, 1]);
    traf.extend(mp4_box(b"trun", &trun));
    file.extend(mp4_box(b"moof", &mp4_box(b"traf", &traf)));
    // Fragment 2: 2 samples with explicit durations (flag 0x100) of 22_050 ticks.
    let mut trun = vec![0, 0, 0x01, 0x00];
    trun.extend_from_slice(&2u32.to_be_bytes());
    trun.extend_from_slice(&22_050u32.to_be_bytes());
    trun.extend_from_slice(&22_050u32.to_be_bytes());
    let mut traf = mp4_box(b"tfhd", &[0, 0, 0, 0, 0, 0, 0, 1]);
    traf.extend(mp4_box(b"trun", &trun));
    file.extend(mp4_box(b"moof", &mp4_box(b"traf", &traf)));
    // 43 * 1024 + 44_100 = 88_132 ticks at 44.1 kHz.
    assert_eq!(probe(&file).unwrap().duration_ms, Some(1_998));
}

#[test]
fn unknown_bytes_are_not_a_voice_container() {
    assert_eq!(probe(b"<html><script>"), None);
    assert_eq!(probe(b""), None);
    // A recognised header with garbage after it still sniffs, but has no duration.
    assert_eq!(
        probe(b"OggS\0\0garbage"),
        Some(Probe {
            container: Container::Ogg,
            duration_ms: None
        })
    );
}

/// TG-1301: a canonical PCM WAV — `fmt ` (16-byte PCM body), optional extra chunk, `data`.
fn wav(
    sample_rate: u32,
    channels: u16,
    pcm_bytes: usize,
    extra: Option<(&[u8; 4], &[u8])>,
) -> Vec<u8> {
    let byte_rate = sample_rate * u32::from(channels) * 2;
    let mut fmt = Vec::new();
    fmt.extend_from_slice(&1u16.to_le_bytes());
    fmt.extend_from_slice(&channels.to_le_bytes());
    fmt.extend_from_slice(&sample_rate.to_le_bytes());
    fmt.extend_from_slice(&byte_rate.to_le_bytes());
    fmt.extend_from_slice(&(channels * 2).to_le_bytes());
    fmt.extend_from_slice(&16u16.to_le_bytes());
    let chunk = |id: &[u8], body: &[u8]| {
        let mut out = id.to_vec();
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(body);
        if body.len() % 2 == 1 {
            out.push(0);
        }
        out
    };
    let mut body = b"WAVE".to_vec();
    body.extend(chunk(b"fmt ", &fmt));
    if let Some((id, payload)) = extra {
        body.extend(chunk(id, payload));
    }
    body.extend(chunk(b"data", &vec![0u8; pcm_bytes]));
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend(body);
    out
}

#[test]
fn wav_duration_is_the_data_size_over_the_byte_rate() {
    // 16 kHz mono 16-bit = 32 000 B/s; 2.5 s of PCM.
    assert_eq!(
        probe(&wav(16_000, 1, 80_000, None)),
        Some(Probe {
            container: Container::Wav,
            duration_ms: Some(2_500)
        })
    );
    // An odd-sized LIST chunk before `data` is skipped, pad byte included.
    let listed = wav(44_100, 2, 176_400, Some((b"LIST", b"INFOabc")));
    assert_eq!(probe(&listed).and_then(|p| p.duration_ms), Some(1_000));
    assert_eq!(Container::Wav.mime_type(), "audio/wav");
    // WAV never carries the round-video track.
    assert!(!has_video_track(&listed, Container::Wav));
}

#[test]
fn a_wav_without_fmt_or_data_sniffs_but_has_no_duration() {
    assert_eq!(
        probe(b"RIFF\x04\0\0\0WAVE"),
        Some(Probe {
            container: Container::Wav,
            duration_ms: None
        })
    );
    assert_eq!(probe(b"RIFF\x04\0\0\0AVI "), None);
}
