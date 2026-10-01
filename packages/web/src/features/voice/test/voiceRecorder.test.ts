// TG-401: the recorder lifecycle with fakes — Opus/WebM default, the Safari MP4 branch,
// waveform alignment from the live levels, permission errors and cancel.
import { describe, expect, test } from 'bun:test'
import { createVoiceRecorder, RecorderError } from '../voiceRecorder'
import { fakeEnv } from './fakes'

describe('voice recorder', () => {
  test('records Opus/WebM and hands back an aligned 100-sample waveform', async () => {
    const fake = fakeEnv()
    const recorder = createVoiceRecorder(fake.env)
    const levels: number[] = []
    recorder.onLevel((peak) => levels.push(peak))
    await recorder.start()
    expect(fake.recorders[0]!.mimeType).toBe('audio/webm;codecs=opus')
    expect(fake.recorders[0]!.started).toBeTrue()
    fake.advance(2_000, 0.02)
    fake.advance(2_000, 0.9)
    const recording = await recorder.stop()
    expect(recording.container).toBe('webm')
    expect(recording.blob.type).toBe('audio/webm')
    expect(recording.blob.size).toBe(3)
    expect(recording.durationMs).toBe(4_000)
    expect(recording.waveform).toHaveLength(100)
    expect(Math.max(...recording.waveform.slice(0, 49))).toBeLessThanOrEqual(1)
    expect(Math.min(...recording.waveform.slice(51))).toBe(31)
    expect(levels.length).toBe(100)
    expect(fake.stopped).toEqual(['tap', 'track'])
    expect(fake.intervalActive()).toBeFalse()
  })

  test('Safari: no Opus support → AAC in MP4, uploaded as .m4a-shaped container', async () => {
    const fake = fakeEnv({ supported: ['audio/mp4;codecs=mp4a.40.2', 'audio/mp4'], recorderMime: 'audio/mp4' })
    const recorder = createVoiceRecorder(fake.env)
    expect(recorder.format).toEqual({ mimeType: 'audio/mp4;codecs=mp4a.40.2', container: 'mp4' })
    await recorder.start()
    expect(fake.recorders[0]!.mimeType).toBe('audio/mp4')
    fake.advance(1_000, 0.5)
    const recording = await recorder.stop()
    expect(recording.container).toBe('mp4')
    expect(recording.blob.type).toBe('audio/mp4')
  })

  test('no recordable type throws `unsupported` up front', () => {
    expect(() => createVoiceRecorder(fakeEnv({ supported: [] }).env)).toThrow(RecorderError)
  })

  test('a denied microphone is a permission error; a missing one is "no device" (TG-1301)', async () => {
    const denied = createVoiceRecorder(fakeEnv({ denied: 'NotAllowedError' }).env)
    expect(await denied.start().catch((error: RecorderError) => error.reason)).toBe('permission')
    const missing = createVoiceRecorder(fakeEnv({ denied: 'NotFoundError' }).env)
    expect(await missing.start().catch((error: RecorderError) => error.reason)).toBe('nodevice')
  })

  test('cancel releases the microphone and discards the data', async () => {
    const fake = fakeEnv()
    const recorder = createVoiceRecorder(fake.env)
    await recorder.start()
    fake.advance(500, 0.4)
    recorder.cancel()
    expect(fake.recorders[0]!.stopped).toBeTrue()
    expect(fake.stopped).toEqual(['tap', 'track'])
    expect(fake.intervalActive()).toBeFalse()
  })
})
