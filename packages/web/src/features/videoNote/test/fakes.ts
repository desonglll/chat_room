/** Fakes for the TG-402 recorder tests: camera session, MediaRecorder and clock. */
import { FakeMediaRecorder } from '../../voice/test/fakes'
import type { CameraSession, VideoRecorderEnv } from '../videoRecorder'

export interface FakeVideoEnvOptions {
  supported?: string[] | undefined
  /** `NotAllowedError` / `NotFoundError` to make the camera fail. */
  denied?: string
  recorderMime?: string
  /** Frames the camera delivers nothing for (draw() false) before the first picture. */
  blankFrames?: number
}

export function fakeVideoEnv(options: FakeVideoEnvOptions = {}) {
  const supported = options.supported ?? ['video/webm;codecs=vp9,opus', 'video/mp4']
  let now = 1_000
  let interval: (() => void) | null = null
  let blank = options.blankFrames ?? 0
  const log: string[] = []
  const recorders: FakeMediaRecorder[] = []
  const preview = { kind: 'canvas' }
  const camera: CameraSession = {
    stream: { getTracks: () => [] },
    preview,
    draw() {
      if (blank > 0) {
        blank -= 1
        return false
      }
      log.push('draw')
      return true
    },
    thumbnail: async () => {
      log.push('thumbnail')
      return new Blob([new Uint8Array([0xff, 0xd8, 0xff])], { type: 'image/jpeg' })
    },
    close: () => log.push('close'),
  }
  const env: VideoRecorderEnv = {
    isTypeSupported: (type) => supported.includes(type),
    openCamera: async () => {
      if (options.denied) throw Object.assign(new Error('denied'), { name: options.denied })
      log.push('open')
      return camera
    },
    createRecorder: (_stream, mimeType) => {
      const recorder = new FakeMediaRecorder(options.recorderMime ?? mimeType)
      recorders.push(recorder)
      return recorder
    },
    now: () => now,
    setInterval: (callback) => {
      interval = callback
      return 1
    },
    clearInterval: () => {
      interval = null
    },
  }
  return {
    env,
    log,
    preview,
    recorders,
    /** Advance the clock by `ms` in 33 ms frames. */
    advance(ms: number) {
      for (let elapsed = 0; elapsed < ms; elapsed += 33) {
        now += 33
        interval?.()
      }
    },
    intervalActive: () => interval !== null,
  }
}
