/** Fakes for the TG-401 recorder tests: MediaRecorder, microphone, level tap and clock. */
import type { LevelTap, MediaRecorderLike, MediaStreamLike, RecorderEnv } from '../voiceRecorder'

export class FakeMediaRecorder implements MediaRecorderLike {
  started = false
  stopped = false
  ondataavailable: ((event: { data: Blob }) => void) | null = null
  onstop: (() => void) | null = null
  onerror: (() => void) | null = null
  constructor(readonly mimeType: string) {}
  start(): void {
    this.started = true
  }
  stop(): void {
    this.stopped = true
    this.ondataavailable?.({ data: new Blob([new Uint8Array([1, 2, 3])], { type: this.mimeType }) })
    this.onstop?.()
  }
}

export interface FakeEnvOptions {
  supported?: string[] | undefined
  /** `NotAllowedError` etc. to make getUserMedia reject. */
  denied?: string
  /** What the recorder reports as its mimeType (browsers may normalise the request). */
  recorderMime?: string
}

export function fakeEnv(options: FakeEnvOptions = {}) {
  const supported = options.supported ?? ['audio/webm;codecs=opus']
  let now = 1_000
  let interval: (() => void) | null = null
  let level = 0
  const stopped: string[] = []
  const recorders: FakeMediaRecorder[] = []
  const stream: MediaStreamLike = { getTracks: () => [{ stop: () => stopped.push('track') }] }
  const tap: LevelTap = { read: () => level, close: () => stopped.push('tap') }
  const env: RecorderEnv = {
    isTypeSupported: (type) => supported.includes(type),
    getUserMedia: async () => {
      if (options.denied) throw Object.assign(new Error('denied'), { name: options.denied })
      return stream
    },
    createRecorder: (_stream, mimeType) => {
      const recorder = new FakeMediaRecorder(options.recorderMime ?? mimeType)
      recorders.push(recorder)
      return recorder
    },
    createLevelTap: () => tap,
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
    recorders,
    stopped,
    /** Advance the clock by `ms`, reading `peak` at every 40 ms tick. */
    advance(ms: number, peak: number) {
      level = peak
      for (let elapsed = 0; elapsed < ms; elapsed += 40) {
        now += 40
        interval?.()
      }
    },
    intervalActive: () => interval !== null,
  }
}
