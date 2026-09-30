/**
 * One round video recording (TG-402): camera + microphone → a square canvas, centre-cropped
 * from whatever aspect ratio the camera delivers → `MediaRecorder`. The canvas is both what
 * gets recorded and what the viewfinder shows, so the preview is exactly the message.
 *
 * Framework-free; every browser capability is injected (`VideoRecorderEnv`), and it satisfies
 * TG-401's `RecorderSession`, so the voice record controller (gesture, chat actions, upload)
 * drives it unchanged.
 */
import type { RecorderSession } from '../voice/recordController'
import { containerOf, type VoiceContainer } from '../voice/recorderFormat'
import { RecorderError, type MediaRecorderLike, type MediaStreamLike } from '../voice/voiceRecorder'

/** Side of the recorded square, in pixels (Telegram records 384×384 on mobile). */
export const VIDEO_NOTE_SIDE = 384
/** Canvas repaint period: ~30 fps. */
export const FRAME_INTERVAL_MS = 33

export type VideoContainer = Extract<VoiceContainer, 'webm' | 'mp4'>

export interface VideoRecorderFormat {
  mimeType: string
  container: VideoContainer
}

/** VP9/VP8 + Opus in WebM (Chromium, Firefox) first; H.264/AAC in MP4 (Safari) last. */
export const VIDEO_RECORDER_CANDIDATES: readonly VideoRecorderFormat[] = [
  { mimeType: 'video/webm;codecs=vp9,opus', container: 'webm' },
  { mimeType: 'video/webm;codecs=vp8,opus', container: 'webm' },
  { mimeType: 'video/webm', container: 'webm' },
  { mimeType: 'video/mp4;codecs=avc1.42E01E,mp4a.40.2', container: 'mp4' },
  { mimeType: 'video/mp4', container: 'mp4' },
]

export function pickVideoFormat(isTypeSupported: ((type: string) => boolean) | undefined): VideoRecorderFormat | null {
  if (!isTypeSupported) return null
  return VIDEO_RECORDER_CANDIDATES.find((candidate) => isTypeSupported(candidate.mimeType)) ?? null
}

export function videoNoteFileName(container: VideoContainer): string {
  return `video_note.${container}`
}

/**
 * The largest centred square of a `width × height` frame: the source rectangle to draw into
 * the round viewfinder. A 16:9 camera loses equal strips left and right, a portrait one top
 * and bottom — never a squash.
 */
export function centerSquare(width: number, height: number): { sx: number; sy: number; side: number } {
  const side = Math.min(width, height)
  return { sx: (width - side) / 2, sy: (height - side) / 2, side }
}

/** An open camera, painting into the square canvas that is recorded and previewed. */
export interface CameraSession {
  /** The canvas's video track plus the microphone's audio tracks. */
  stream: MediaStreamLike
  /** What the viewfinder shows (the browser's canvas element). */
  preview: unknown
  /** Paint the camera's current frame; `false` until the camera delivers frames. */
  draw(): boolean
  /** A small JPEG of the current square, or `null`. */
  thumbnail(): Promise<Blob | null>
  close(): void
}

export interface VideoRecorderEnv {
  isTypeSupported: ((type: string) => boolean) | undefined
  /** Rejects like `getUserMedia` (`NotAllowedError`, `NotFoundError` …). */
  openCamera(): Promise<CameraSession>
  createRecorder(stream: MediaStreamLike, mimeType: string): MediaRecorderLike
  now(): number
  setInterval(callback: () => void, ms: number): unknown
  clearInterval(handle: unknown): void
}

export interface VideoNoteRecording {
  blob: Blob
  container: VideoContainer
  durationMs: number
  thumbnail: Blob | null
}

export interface VideoNoteRecorder extends RecorderSession<VideoNoteRecording> {
  readonly format: VideoRecorderFormat
  /** The viewfinder's element once the camera is open, else `null`. */
  preview(): unknown
}

export function createVideoNoteRecorder(env: VideoRecorderEnv): VideoNoteRecorder {
  const format = pickVideoFormat(env.isTypeSupported)
  if (!format) throw new RecorderError('unsupported')
  const listeners = new Set<(peak: number, elapsedMs: number) => void>()
  const chunks: Blob[] = []
  let camera: CameraSession | null = null
  let recorder: MediaRecorderLike | null = null
  let timer: unknown = null
  let startedAt = 0
  let stoppedAt: number | null = null
  let cancelled = false
  let thumbnail: Promise<Blob | null> | null = null

  const release = () => {
    if (timer !== null) env.clearInterval(timer)
    timer = null
    camera?.close()
    camera = null
  }

  const elapsedMs = () => (startedAt === 0 ? 0 : (stoppedAt ?? env.now()) - startedAt)

  const paint = () => {
    const open = camera
    if (!open) return
    // The first painted frame is the thumbnail (Telegram shows the opening frame too).
    if (open.draw() && thumbnail === null) thumbnail = open.thumbnail().catch(() => null)
    const at = elapsedMs()
    for (const listener of listeners) listener(0, at)
  }

  return {
    format,
    preview: () => camera?.preview ?? null,
    async start() {
      let opened: CameraSession
      try {
        opened = await env.openCamera()
      } catch (error) {
        const name = (error as { name?: string } | null)?.name
        throw new RecorderError(name === 'NotAllowedError' || name === 'SecurityError' ? 'permission' : 'device')
      }
      camera = opened
      if (cancelled) {
        release()
        return
      }
      recorder = env.createRecorder(opened.stream, format.mimeType)
      recorder.ondataavailable = (event) => {
        if (event.data.size > 0) chunks.push(event.data)
      }
      opened.draw()
      startedAt = env.now()
      recorder.start(250)
      timer = env.setInterval(paint, FRAME_INTERVAL_MS)
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
      const still = thumbnail ?? camera?.thumbnail().catch(() => null) ?? Promise.resolve(null)
      return new Promise<VideoNoteRecording>((resolve, reject) => {
        active.onstop = () => {
          release()
          const type = active.mimeType || chunks[0]?.type || format.mimeType
          const container = containerOf(type, format.container) === 'mp4' ? 'mp4' : 'webm'
          const blob = new Blob(chunks, { type: type.split(';')[0] ?? type })
          void still.then((jpeg) => resolve({ blob, container, durationMs, thumbnail: jpeg }))
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
