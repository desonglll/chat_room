/**
 * Waveform maths for TG-401, pure and DOM-free.
 *
 * Recording: the recorder samples the live PCM peak every few tens of milliseconds and
 * timestamps each sample. `buildWaveform` buckets those samples by TIME (not by count) into
 * Telegram's 100 × 5-bit waveform, so sample 37 always describes 37–38 % of the recording
 * even when a background tab throttled the sampling timer. Buckets that received no sample
 * hold the previous sample (sample-and-hold).
 *
 * Playback: `resampleWaveform` turns the stored 100 samples into however many bars the bubble
 * has room for; `waveformBarCount` grows the bubble with the duration like Telegram does.
 */

export const WAVEFORM_SAMPLES = 100
export const WAVEFORM_MAX = 31

export interface LevelSample {
  /** Milliseconds since recording started. */
  atMs: number
  /** Absolute PCM peak in 0..1. */
  peak: number
}

export function buildWaveform(samples: readonly LevelSample[], durationMs: number, count = WAVEFORM_SAMPLES): number[] {
  const buckets: Array<number | null> = new Array<number | null>(count).fill(null)
  if (durationMs > 0) {
    for (const sample of samples) {
      const index = Math.min(count - 1, Math.max(0, Math.floor((sample.atMs / durationMs) * count)))
      const peak = Math.min(1, Math.max(0, Number.isFinite(sample.peak) ? sample.peak : 0))
      buckets[index] = Math.max(buckets[index] ?? 0, peak)
    }
  }
  const filled = fillGaps(buckets)
  const loudest = Math.max(0, ...filled)
  // Telegram normalises to the loudest peak, so a quiet recording still draws a readable
  // shape; true silence stays flat.
  if (loudest < 0.01) return filled.map(() => 0)
  return filled.map((peak) => Math.min(WAVEFORM_MAX, Math.round((peak / loudest) * WAVEFORM_MAX)))
}

function fillGaps(buckets: ReadonlyArray<number | null>): number[] {
  // Sample-and-hold: a level sample describes the audio from its moment until the next one,
  // so an empty bucket takes the last sample before it (leading gaps take the first one).
  const first = buckets.find((value) => value !== null) ?? 0
  let held = first
  return buckets.map((value) => {
    if (value !== null) held = value
    return held
  })
}

/** `bars` bars from a stored waveform: each bar is the max of the samples it covers. */
export function resampleWaveform(waveform: readonly number[], bars: number): number[] {
  if (bars <= 0 || waveform.length === 0) return []
  const out: number[] = []
  for (let bar = 0; bar < bars; bar += 1) {
    const start = Math.floor((bar * waveform.length) / bars)
    const end = Math.max(start + 1, Math.floor(((bar + 1) * waveform.length) / bars))
    let peak = 0
    for (let index = start; index < end && index < waveform.length; index += 1)
      peak = Math.max(peak, waveform[index] ?? 0)
    out.push(peak)
  }
  return out
}

/** Telegram widens a voice bubble with its length: 1 s → 24 bars, 30 s and longer → 48. */
export function waveformBarCount(durationMs: number): number {
  const seconds = Math.max(0, durationMs / 1000)
  return Math.round(24 + (Math.min(seconds, 30) / 30) * 24)
}

/** `0:07`, `1:05`, `12:30`, `1:02:03`. */
export function formatVoiceDuration(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000))
  const hours = Math.floor(total / 3600)
  const minutes = Math.floor((total % 3600) / 60)
  const seconds = String(total % 60).padStart(2, '0')
  return hours > 0 ? `${hours}:${String(minutes).padStart(2, '0')}:${seconds}` : `${minutes}:${seconds}`
}

/** The recording timer: `0:03,4` — Telegram shows tenths while recording. */
export function formatRecordingTime(ms: number): string {
  const tenths = Math.floor(Math.max(0, ms) / 100) % 10
  return `${formatVoiceDuration(ms)},${tenths}`
}
