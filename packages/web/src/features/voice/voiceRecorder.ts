/**
 * One voice recording: microphone → MediaRecorder (container from `recorderFormat`) plus a
 * level tap that feeds the live waveform and, at the end, the stored 100-sample waveform.
 * Framework-free; every browser capability is injected (`RecorderEnv`), so tests drive the
 * whole lifecycle — including the Safari MP4 branch — with fakes.
 */
import { buildWaveform, type LevelSample } from './waveform'
import { containerOf, pickRecorderFormat, type RecorderFormat, type VoiceContainer } from './recorderFormat'

/** The subset of `MediaRecorder` used here. */
export interface MediaRecorderLike {
  readonly mimeType: string
  start(timesliceMs?: number): void
  stop(): void
  ondataavailable: ((event: { data: Blob }) => void) | null
  onstop: (() => void) | null
  onerror: (() => void) | null
}

export interface LevelTap {
  /** Current absolute PCM peak, 0..1. */
  read(): number
  close(): void
}

export interface MediaStreamLike {
  getTracks(): Array<{ stop(): void }>
}

export interface RecorderEnv {
  isTypeSupported: ((type: string) => boolean) | undefined
  getUserMedia(): Promise<MediaStreamLike>
  createRecorder(stream: MediaStreamLike, mimeType: string): MediaRecorderLike
  /** `null` when no AudioContext exists: recording still works, the waveform stays flat. */
  createLevelTap(stream: MediaStreamLike): LevelTap | null
  now(): number
  setInterval(callback: () => void, ms: number): unknown
  clearInterval(handle: unknown): void
}

export interface VoiceRecording {
  blob: Blob
  container: VoiceContainer
  durationMs: number
  waveform: number[]
}

/**
 * Why recording could not start. TG-1301 split two cases users could not tell apart:
 * `insecure` — the page is not a secure context (http on a LAN address), so the browser hides the
 * microphone/camera API entirely; `nodevice` — the API works but there is no such device.
 */
export type RecorderFailure = 'unsupported' | 'insecure' | 'permission' | 'nodevice' | 'device'

/** The failure for a browser that offers no recording API: is it the page, or the browser? */
export function unavailableFailure(scope: { isSecureContext?: boolean } = globalThis): RecorderFailure {
  return scope.isSecureContext === false ? 'insecure' : 'unsupported'
}

/** The failure for a `getUserMedia` rejection, by its DOMException name. */
export function mediaFailure(error: unknown): RecorderFailure {
  const name = (error as { name?: string } | null)?.name
  if (name === 'NotAllowedError' || name === 'SecurityError' || name === 'PermissionDeniedError') return 'permission'
  if (name === 'NotFoundError' || name === 'DevicesNotFoundError' || name === 'OverconstrainedError') return 'nodevice'
  return 'device'
}

export class RecorderError extends Error {
  constructor(readonly reason: RecorderFailure) {
    super(`voice recorder: ${reason}`)
    this.name = 'RecorderError'
  }
}

/** How often the level tap is read: ~24 per second, dense enough for 100 buckets per 4 s. */
export const LEVEL_INTERVAL_MS = 40

export interface VoiceRecorder {
  readonly format: RecorderFormat
  /** Resolves once the microphone is open and MediaRecorder runs. */
  start(): Promise<void>
  /** Live level + elapsed time, for the recording panel. */
  onLevel(listener: (peak: number, elapsedMs: number) => void): () => void
  elapsedMs(): number
  /** Finish and hand back the file; resolves after the recorder flushed its last chunk. */
  stop(): Promise<VoiceRecording>
  /** Discard everything and release the microphone. */
  cancel(): void
}

export function createVoiceRecorder(env: RecorderEnv): VoiceRecorder {
  const format = pickRecorderFormat(env.isTypeSupported)
  if (!format) throw new RecorderError('unsupported')
  const listeners = new Set<(peak: number, elapsedMs: number) => void>()
  const samples: LevelSample[] = []
  const chunks: Blob[] = []
  let stream: MediaStreamLike | null = null
  let recorder: MediaRecorderLike | null = null
  let tap: LevelTap | null = null
  let timer: unknown = null
  let startedAt = 0
  let stoppedAt: number | null = null
  let cancelled = false

  const release = () => {
    if (timer !== null) env.clearInterval(timer)
    timer = null
    tap?.close()
    tap = null
    for (const track of stream?.getTracks() ?? []) track.stop()
    stream = null
  }

  const elapsedMs = () => (startedAt === 0 ? 0 : (stoppedAt ?? env.now()) - startedAt)

  return {
    format,
    async start() {
      try {
        stream = await env.getUserMedia()
      } catch (error) {
        throw new RecorderError(mediaFailure(error))
      }
      if (cancelled) {
        release()
        return
      }
      recorder = env.createRecorder(stream, format.mimeType)
      recorder.ondataavailable = (event) => {
        if (event.data.size > 0) chunks.push(event.data)
      }
      tap = env.createLevelTap(stream)
      startedAt = env.now()
      recorder.start(250)
      timer = env.setInterval(() => {
        const peak = tap?.read() ?? 0
        const at = elapsedMs()
        samples.push({ atMs: at, peak })
        for (const listener of listeners) listener(peak, at)
      }, LEVEL_INTERVAL_MS)
    },
    onLevel(listener) {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    elapsedMs,
    stop() {
      const active = recorder
      if (!active) return Promise.reject(new RecorderError('device'))
      stoppedAt = env.now()
      const durationMs = elapsedMs()
      return new Promise<VoiceRecording>((resolve, reject) => {
        active.onstop = () => {
          release()
          const type = active.mimeType || chunks[0]?.type || format.mimeType
          const container = containerOf(type, format.container)
          resolve({
            blob: new Blob(chunks, { type: type.split(';')[0] ?? type }),
            container,
            durationMs,
            waveform: buildWaveform(samples, durationMs),
          })
        }
        active.onerror = () => {
          release()
          reject(new RecorderError('device'))
        }
        active.stop()
      })
    },
    cancel() {
      cancelled = true
      listeners.clear()
      if (recorder) {
        recorder.ondataavailable = null
        recorder.onstop = null
        try {
          recorder.stop()
        } catch {
          // Already inactive.
        }
      }
      recorder = null
      release()
    },
  }
}
