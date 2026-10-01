// TG-402: the round video recorder — centre crop, container choice, lifecycle, errors.
import { describe, expect, test } from 'bun:test'
import { RecorderError } from '../../voice/voiceRecorder'
import { centerSquare, createVideoNoteRecorder, pickVideoFormat, videoNoteFileName } from '../videoRecorder'
import { fakeVideoEnv } from './fakes'

const settle = async () => {
  for (let i = 0; i < 12; i += 1) await Promise.resolve()
}

describe('centre crop', () => {
  test('a landscape camera loses equal strips left and right', () => {
    expect(centerSquare(640, 360)).toEqual({ sx: 140, sy: 0, side: 360 })
    expect(centerSquare(1920, 1080)).toEqual({ sx: 420, sy: 0, side: 1080 })
  })
  test('a portrait camera loses equal strips top and bottom', () => {
    expect(centerSquare(480, 640)).toEqual({ sx: 0, sy: 80, side: 480 })
  })
  test('a square camera is taken whole', () => {
    expect(centerSquare(384, 384)).toEqual({ sx: 0, sy: 0, side: 384 })
  })
})

describe('container', () => {
  test('WebM/VP9 first, MP4 as the Safari fallback, nothing when unsupported', () => {
    expect(pickVideoFormat(() => true)?.mimeType).toBe('video/webm;codecs=vp9,opus')
    expect(pickVideoFormat((type) => type.startsWith('video/mp4'))).toEqual({
      mimeType: 'video/mp4;codecs=avc1.42E01E,mp4a.40.2',
      container: 'mp4',
    })
    expect(pickVideoFormat(() => false)).toBeNull()
    expect(pickVideoFormat(undefined)).toBeNull()
    expect(videoNoteFileName('mp4')).toBe('video_note.mp4')
    expect(videoNoteFileName('webm')).toBe('video_note.webm')
  })
})

describe('video note recorder', () => {
  test('records the canvas, takes the first painted frame as thumbnail, releases the camera', async () => {
    const fake = fakeVideoEnv({ blankFrames: 3 })
    const recorder = createVideoNoteRecorder(fake.env)
    const elapsed: number[] = []
    recorder.onLevel((_peak, at) => elapsed.push(at))
    expect(recorder.preview()).toBeNull()
    await recorder.start()
    expect(recorder.preview()).toBe(fake.preview)
    expect(fake.recorders[0]!.started).toBe(true)
    fake.advance(1_650)
    expect(elapsed.at(-1)).toBe(1_650)
    // Blank frames produce no thumbnail; the first real one does, once.
    expect(fake.log.filter((entry) => entry === 'thumbnail')).toHaveLength(1)
    const recording = await recorder.stop()
    expect(recording.durationMs).toBe(1_650)
    expect(recording.container).toBe('webm')
    expect(recording.blob.type).toBe('video/webm')
    expect(recording.thumbnail?.type).toBe('image/jpeg')
    expect(fake.log.at(-1)).toBe('close')
    expect(fake.intervalActive()).toBe(false)
  })

  test('Safari: the recorder writes MP4 and the upload says so', async () => {
    const fake = fakeVideoEnv({ supported: ['video/mp4'], recorderMime: 'video/mp4;codecs=avc1,mp4a' })
    const recorder = createVideoNoteRecorder(fake.env)
    await recorder.start()
    fake.advance(900)
    const recording = await recorder.stop()
    expect(recording.container).toBe('mp4')
    expect(recording.blob.type).toBe('video/mp4')
  })

  test('a denied camera is a permission error; a missing one is "no device" (TG-1301)', async () => {
    const denied = createVideoNoteRecorder(fakeVideoEnv({ denied: 'NotAllowedError' }).env)
    expect(denied.start()).rejects.toEqual(new RecorderError('permission'))
    const missing = createVideoNoteRecorder(fakeVideoEnv({ denied: 'NotFoundError' }).env)
    expect(missing.start()).rejects.toEqual(new RecorderError('nodevice'))
    expect(() => createVideoNoteRecorder(fakeVideoEnv({ supported: [] }).env)).toThrow(RecorderError)
  })

  test('cancel discards the recording and closes the camera', async () => {
    const fake = fakeVideoEnv()
    const recorder = createVideoNoteRecorder(fake.env)
    await recorder.start()
    fake.advance(300)
    recorder.cancel()
    await settle()
    expect(fake.log.at(-1)).toBe('close')
    expect(fake.recorders[0]!.stopped).toBe(true)
    expect(recorder.preview()).toBeNull()
  })

  test('cancelled while the camera opens: it is closed as soon as it opens', async () => {
    const fake = fakeVideoEnv()
    const recorder = createVideoNoteRecorder(fake.env)
    const starting = recorder.start()
    recorder.cancel()
    await starting
    expect(fake.log).toEqual(['open', 'close'])
    expect(fake.recorders).toHaveLength(0)
  })
})
