/**
 * TG-1301: a round video from a file the phone's own camera made. Over plain http the browser
 * grants no camera, but `<input type="file" accept="video/*" capture="user">` still opens the
 * system camera. The server keeps a video note only as MP4/WebM of at most 16 MiB and 61 s, and
 * a phone camera's file passes that only when it is short; re-encoding a longer one in the page
 * would need its sound played back, which mobile browsers refuse after a file picker closes.
 * So a file that fits is sent as a round video, and any other playable video is sent as a
 * regular video message, with a notice saying so.
 */
import { centerSquare, type VideoContainer, type VideoNoteRecording } from './videoRecorder'
import { VIDEO_NOTE_MAX_MS } from './videoNoteApi'
import { MediaFileError } from '../voice/voiceFile'

/** The server's limits (`MAX_VIDEO_NOTE_BYTES`, `MAX_VIDEO_NOTE_DURATION_MS`). */
export const MAX_VIDEO_NOTE_FILE_BYTES = 16 * 1024 * 1024
const MAX_VIDEO_NOTE_FILE_MS = VIDEO_NOTE_MAX_MS + 1_000
/** The server's limit for the inline thumbnail (`MAX_THUMBNAIL_BYTES`). */
const MAX_THUMBNAIL_BYTES = 16 * 1024
const THUMBNAIL_SIDE = 96

/** A video too large or long for a round video: sent as a regular video message instead. */
export interface RegularVideo {
  regular: Blob
  fileName: string
}

export type VideoUpload = VideoNoteRecording | RegularVideo

export const isRegularVideo = (upload: VideoUpload): upload is RegularVideo => 'regular' in upload

/** What the browser learned by loading the file's metadata. */
export interface ProbedVideo {
  durationMs: number
  /** A small JPEG of the opening square, or `null`. */
  thumbnail(): Promise<Blob | null>
}

export type VideoProbe = (file: Blob) => Promise<ProbedVideo>

/** The round-video container a file already is, by its first bytes — the server's own sniff. */
export function sniffVideoContainer(head: Uint8Array): VideoContainer | null {
  if (String.fromCharCode(...head.subarray(4, 8)) === 'ftyp') return 'mp4'
  if (head[0] === 0x1a && head[1] === 0x45 && head[2] === 0xdf && head[3] === 0xa3) return 'webm'
  return null
}

const whenEvent = (target: HTMLMediaElement, ok: string, ms: number) =>
  new Promise<void>((resolve, reject) => {
    const timer = window.setTimeout(() => reject(new Error('timeout')), ms)
    const done = (error?: unknown) => {
      window.clearTimeout(timer)
      target.removeEventListener(ok, onOk)
      target.removeEventListener('error', onError)
      if (error) reject(error)
      else resolve()
    }
    const onOk = () => done()
    const onError = () => done(new Error('media error'))
    target.addEventListener(ok, onOk)
    target.addEventListener('error', onError)
  })

/** The browser probe: a muted, detached `<video>` loading the file's metadata and first frame. */
export const browserVideoProbe: VideoProbe = async (file) => {
  const url = URL.createObjectURL(file)
  const video = document.createElement('video')
  video.muted = true
  video.playsInline = true
  video.preload = 'metadata'
  try {
    const loaded = whenEvent(video, 'loadedmetadata', 10_000)
    video.src = url
    await loaded
  } catch {
    URL.revokeObjectURL(url)
    throw new MediaFileError('unreadable')
  }
  const seconds = Number.isFinite(video.duration) ? video.duration : 0
  return {
    durationMs: Math.round(seconds * 1000),
    async thumbnail() {
      try {
        const seeked = whenEvent(video, 'seeked', 3_000)
        video.currentTime = Math.min(0.1, seconds / 2)
        await seeked
        const canvas = document.createElement('canvas')
        canvas.width = THUMBNAIL_SIDE
        canvas.height = THUMBNAIL_SIDE
        const { sx, sy, side } = centerSquare(video.videoWidth, video.videoHeight)
        canvas.getContext('2d')?.drawImage(video, sx, sy, side, side, 0, 0, THUMBNAIL_SIDE, THUMBNAIL_SIDE)
        const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, 'image/jpeg', 0.7))
        return blob && blob.size <= MAX_THUMBNAIL_BYTES ? blob : null
      } catch {
        return null
      } finally {
        URL.revokeObjectURL(url)
      }
    },
  }
}

const fileNameOf = (file: Blob, container: VideoContainer | null) =>
  (file as Partial<File>).name || `video.${container ?? 'mp4'}`

export async function videoUploadFromFile(file: Blob, probe: VideoProbe = browserVideoProbe): Promise<VideoUpload> {
  const head = new Uint8Array(await file.slice(0, 16).arrayBuffer())
  const container = sniffVideoContainer(head)
  const probed = await probe(file)
  const fits =
    container !== null &&
    file.size <= MAX_VIDEO_NOTE_FILE_BYTES &&
    probed.durationMs > 0 &&
    probed.durationMs <= MAX_VIDEO_NOTE_FILE_MS
  if (!fits || container === null) {
    void probed.thumbnail() // releases the probe's object URL
    return { regular: file, fileName: fileNameOf(file, container) }
  }
  return { blob: file, container, durationMs: probed.durationMs, thumbnail: await probed.thumbnail() }
}
