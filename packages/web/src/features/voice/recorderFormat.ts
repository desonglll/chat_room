/**
 * Which container the recorder writes. Opus in WebM (Chromium, Firefox) or Ogg (Firefox) is
 * Telegram's own codec; Safari records neither and falls back to AAC in MP4. Detection is
 * `MediaRecorder.isTypeSupported`, injected so the fallback branch is testable anywhere.
 */

/** `wav` is never recorded: TG-1301 converts a system recorder's file to it over plain http. */
export type VoiceContainer = 'webm' | 'ogg' | 'mp4' | 'wav'

export interface RecorderFormat {
  /** What `new MediaRecorder(stream, { mimeType })` is asked for. */
  mimeType: string
  container: VoiceContainer
}

/** Preference order: Opus first (smaller, Telegram-native), AAC/MP4 last (Safari). */
export const RECORDER_CANDIDATES: readonly RecorderFormat[] = [
  { mimeType: 'audio/webm;codecs=opus', container: 'webm' },
  { mimeType: 'audio/ogg;codecs=opus', container: 'ogg' },
  { mimeType: 'audio/mp4;codecs=mp4a.40.2', container: 'mp4' },
  { mimeType: 'audio/mp4', container: 'mp4' },
]

/** The first supported candidate, or `null` when this browser cannot record audio at all. */
export function pickRecorderFormat(isTypeSupported: ((type: string) => boolean) | undefined): RecorderFormat | null {
  if (!isTypeSupported) return null
  return RECORDER_CANDIDATES.find((candidate) => isTypeSupported(candidate.mimeType)) ?? null
}

/** The container a finished blob actually is (browsers may ignore the requested type). */
export function containerOf(mimeType: string, requested: VoiceContainer): VoiceContainer {
  const base = mimeType.split(';')[0]!.trim().toLowerCase()
  if (base.endsWith('/webm')) return 'webm'
  if (base.endsWith('/ogg')) return 'ogg'
  if (base.endsWith('/mp4') || base === 'audio/aac' || base === 'audio/x-m4a') return 'mp4'
  return requested
}

export function voiceFileName(container: VoiceContainer): string {
  return container === 'mp4' ? 'voice.m4a' : `voice.${container}`
}

/** The plain MIME type to label the upload with (no codecs parameter). */
export function uploadMimeType(container: VoiceContainer): string {
  return `audio/${container}`
}
