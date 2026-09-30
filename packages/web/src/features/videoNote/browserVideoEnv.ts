/**
 * The browser implementation of `VideoRecorderEnv`: getUserMedia (front camera + microphone),
 * a hidden `<video>` feeding a square canvas, `canvas.captureStream` plus the microphone's
 * audio tracks, and MediaRecorder. The only file of this feature that names recording globals.
 */
import type { MediaRecorderLike } from '../voice/voiceRecorder'
import {
  centerSquare,
  FRAME_INTERVAL_MS,
  VIDEO_NOTE_SIDE,
  type CameraSession,
  type VideoRecorderEnv,
} from './videoRecorder'

/** Side of the stored thumbnail: small enough to inline (a few KiB of JPEG). */
const THUMBNAIL_SIDE = 96

async function openCamera(): Promise<CameraSession> {
  const media = await navigator.mediaDevices.getUserMedia({
    video: { facingMode: 'user', width: { ideal: 640 }, height: { ideal: 480 } },
    audio: { echoCancellation: true, noiseSuppression: true },
  })
  const video = document.createElement('video')
  video.muted = true
  video.playsInline = true
  video.srcObject = media
  const stopAll = () => {
    for (const track of media.getTracks()) track.stop()
  }
  try {
    await video.play()
  } catch (error) {
    stopAll()
    throw error
  }
  const canvas = document.createElement('canvas')
  canvas.width = VIDEO_NOTE_SIDE
  canvas.height = VIDEO_NOTE_SIDE
  canvas.className = 'tg-video-note-rec__canvas'
  const context = canvas.getContext('2d')
  const stream = canvas.captureStream(Math.round(1000 / FRAME_INTERVAL_MS))
  for (const track of media.getAudioTracks()) stream.addTrack(track)
  return {
    stream,
    preview: canvas,
    draw() {
      if (!context || video.videoWidth === 0 || video.videoHeight === 0) return false
      const { sx, sy, side } = centerSquare(video.videoWidth, video.videoHeight)
      context.drawImage(video, sx, sy, side, side, 0, 0, VIDEO_NOTE_SIDE, VIDEO_NOTE_SIDE)
      return true
    },
    thumbnail() {
      const small = document.createElement('canvas')
      small.width = THUMBNAIL_SIDE
      small.height = THUMBNAIL_SIDE
      small.getContext('2d')?.drawImage(canvas, 0, 0, THUMBNAIL_SIDE, THUMBNAIL_SIDE)
      return new Promise((resolve) => small.toBlob((blob) => resolve(blob), 'image/jpeg', 0.7))
    },
    close() {
      stopAll()
      for (const track of stream.getTracks()) track.stop()
      video.srcObject = null
    },
  }
}

/** `undefined` when this browser has no MediaRecorder, camera API or canvas capture at all. */
export function browserVideoEnv(): VideoRecorderEnv | undefined {
  if (typeof MediaRecorder === 'undefined' || !navigator.mediaDevices?.getUserMedia) return undefined
  if (typeof HTMLCanvasElement === 'undefined' || !('captureStream' in HTMLCanvasElement.prototype)) return undefined
  return {
    isTypeSupported:
      typeof MediaRecorder.isTypeSupported === 'function' ? (type) => MediaRecorder.isTypeSupported(type) : undefined,
    openCamera,
    createRecorder: (stream, mimeType) =>
      new MediaRecorder(stream as MediaStream, {
        mimeType,
        videoBitsPerSecond: 1_000_000,
        audioBitsPerSecond: 48_000,
      }) as unknown as MediaRecorderLike,
    now: () => performance.now(),
    setInterval: (callback, ms) => window.setInterval(callback, ms),
    clearInterval: (handle) => window.clearInterval(handle as number),
  }
}
