// TG-401: waveform maths — time-aligned buckets, gap filling, normalisation, display helpers.
import { describe, expect, test } from 'bun:test'
import {
  buildWaveform,
  formatRecordingTime,
  formatVoiceDuration,
  resampleWaveform,
  waveformBarCount,
  WAVEFORM_MAX,
  WAVEFORM_SAMPLES,
  type LevelSample,
} from '../waveform'

/** 40 ms sampling, like the recorder: quiet for `quietMs`, then loud until `durationMs`. */
function samples(durationMs: number, quietMs: number, stepMs = 40): LevelSample[] {
  const out: LevelSample[] = []
  for (let at = 0; at < durationMs; at += stepMs) out.push({ atMs: at, peak: at < quietMs ? 0.05 : 0.8 })
  return out
}

describe('buildWaveform', () => {
  test('is exactly 100 five-bit samples', () => {
    const waveform = buildWaveform(samples(3_000, 1_000), 3_000)
    expect(waveform).toHaveLength(WAVEFORM_SAMPLES)
    expect(waveform.every((value) => Number.isInteger(value) && value >= 0 && value <= WAVEFORM_MAX)).toBeTrue()
  })

  test('is aligned with the audio: the loud half is the second half, to the bucket', () => {
    const waveform = buildWaveform(samples(10_000, 5_000), 10_000)
    expect(waveform.slice(0, 50).every((value) => value <= 2)).toBeTrue()
    expect(waveform.slice(50).every((value) => value === WAVEFORM_MAX)).toBeTrue()
  })

  test('is aligned by time even when sampling was throttled (a background tab)', () => {
    // One sample per second (1000 ms timer clamp), loud from 7 s of 10 s.
    const waveform = buildWaveform(samples(10_000, 7_000, 1_000), 10_000)
    expect(waveform.slice(0, 70).every((value) => value <= 2)).toBeTrue()
    expect(waveform.slice(70).every((value) => value === WAVEFORM_MAX)).toBeTrue()
  })

  test('normalises to the loudest peak; silence stays flat', () => {
    expect(
      Math.max(
        ...buildWaveform(
          samples(2_000, 0).map((s) => ({ ...s, peak: 0.1 })),
          2_000,
        ),
      ),
    ).toBe(WAVEFORM_MAX)
    expect(buildWaveform([{ atMs: 0, peak: 0 }], 1_000)).toEqual(new Array(100).fill(0))
    expect(buildWaveform([], 1_000)).toEqual(new Array(100).fill(0))
  })
})

describe('display helpers', () => {
  test('resample takes the max of the samples each bar covers', () => {
    const ramp = Array.from({ length: 100 }, (_, index) => Math.floor(index / 4))
    const bars = resampleWaveform(ramp, 25)
    expect(bars).toHaveLength(25)
    expect(bars[0]).toBe(0)
    expect(bars[24]).toBe(24)
    expect(resampleWaveform(ramp, 0)).toEqual([])
  })

  test('the bubble grows with the duration, within Telegram-like bounds', () => {
    expect(waveformBarCount(0)).toBe(24)
    expect(waveformBarCount(15_000)).toBe(36)
    expect(waveformBarCount(30_000)).toBe(48)
    expect(waveformBarCount(600_000)).toBe(48)
  })

  test('durations read like Telegram', () => {
    expect(formatVoiceDuration(7_900)).toBe('0:07')
    expect(formatVoiceDuration(65_000)).toBe('1:05')
    expect(formatVoiceDuration(3_723_000)).toBe('1:02:03')
    expect(formatRecordingTime(3_450)).toBe('0:03,4')
  })
})
