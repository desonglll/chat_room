//! TG-1301: WAV (RIFF/WAVE) duration — the `data` chunk's byte length over the `fmt ` chunk's
//! byte rate. Chunks are walked in order (each is a 4-byte id, a little-endian u32 size, the body
//! and a pad byte when the size is odd), so `LIST`/`fact` chunks before `data` are skipped.

pub(super) fn wav_duration_ms(bytes: &[u8]) -> Option<u32> {
    let mut at = 12;
    let mut byte_rate = None;
    while let Some(header) = bytes.get(at..at + 8) {
        let size = u32::from_le_bytes(header[4..8].try_into().ok()?) as usize;
        let body = at + 8;
        match &header[..4] {
            b"fmt " => {
                // audio_format u16, channels u16, sample_rate u32, byte_rate u32, …
                let rate = bytes.get(body + 8..body + 12)?;
                byte_rate = Some(u32::from_le_bytes(rate.try_into().ok()?));
            }
            b"data" => {
                let rate = u128::from(byte_rate.filter(|rate| *rate > 0)?);
                // A streaming writer may leave the size unset (0 / u32::MAX): use what is there.
                let present = bytes.len().saturating_sub(body);
                let length = if size == 0 || size == u32::MAX as usize {
                    present
                } else {
                    size.min(present)
                };
                return u32::try_from(length as u128 * 1000 / rate).ok();
            }
            _ => {}
        }
        at = body.checked_add(size)?.checked_add(size & 1)?;
    }
    None
}
