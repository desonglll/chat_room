/**
 * TG-1301: a voice message from a file the phone's own recorder made. Over plain http (a LAN
 * address) the browser grants no microphone, but `<input type="file" accept="audio/*" capture>`
 * still opens the system recorder. Its file becomes a `VoiceRecording`:
 *
 * - decoded with `decodeAudioData` on a 16 kHz `OfflineAudioContext` (which resamples while it
 *   decodes, faster than real time, and works without a secure context) for the duration and
 *   the 100-bar waveform;
 * - sent as it is when it is a container the server keeps (Ogg, WebM, MP4/M4A); anything else
 *   the browser can decode (MP3, AAC, WAV, FLAC …) is re-encoded as 16 kHz mono PCM WAV, which
 *   the server accepts too. A file this browser cannot decode would not play for anyone either,
 *   so it is refused instead of sent.
 */
import type { VoiceContainer } from './recorderFormat'
import type { VoiceRecording } from './voiceRecorder'
import { buildWaveform, WAVEFORM_SAMPLES, type LevelSample } from './waveform'

/** Decoding rate: plenty for speech, and 32 KB/s as WAV. */
export const VOICE_FILE_RATE = 16_000
/** The server's limit for a voice upload (`MAX_VOICE_BYTES`). */
export const MAX_VOICE_FILE_BYTES = 24 * 1024 * 1024

export type MediaFileFailure = 'unreadable' | 'toolarge'

export class MediaFileError extends Error {
  constructor(readonly reason: MediaFileFailure) {
    super(`media file: ${reason}`)
    this.name = 'MediaFileError'
  }
}

/** Mono PCM at `sampleRate`. */
export interface DecodedAudio {
  sampleRate: number
  samples: Float32Array
}

export type AudioDecoder = (bytes: ArrayBuffer) => Promise<DecodedAudio>

/** The recorder container a file already is, by its first bytes — the server's own sniff. */
export function sniffVoiceContainer(head: Uint8Array): VoiceContainer | null {
  const ascii = (from: number, to: number) => String.fromCharCode(...head.subarray(from, to))
  if (ascii(0, 4) === 'OggS') return 'ogg'
  if (head[0] === 0x1a && head[1] === 0x45 && head[2] === 0xdf && head[3] === 0xa3) return 'webm'
  if (ascii(4, 8) === 'ftyp') return 'mp4'
  if (ascii(0, 4) === 'RIFF' && ascii(8, 12) === 'WAVE') return 'wav'
  return null
}

/** The 100-bar waveform of mono PCM: each bar is its slice's loudest sample, normalised. */
export function waveformOfSamples(samples: Float32Array, sampleRate: number): number[] {
  const durationMs = (samples.length / sampleRate) * 1000
  const levels: LevelSample[] = []
  const per = Math.max(1, Math.floor(samples.length / WAVEFORM_SAMPLES))
  for (let start = 0; start < samples.length; start += per) {
    let peak = 0
    const end = Math.min(samples.length, start + per)
    for (let i = start; i < end; i++) peak = Math.max(peak, Math.abs(samples[i]!))
    levels.push({ atMs: (start / sampleRate) * 1000, peak })
  }
  return buildWaveform(levels, durationMs)
}

/** 16-bit little-endian PCM WAV of mono samples. */
export function encodeWav(samples: Float32Array, sampleRate: number): Uint8Array<ArrayBuffer> {
  const out = new Uint8Array(44 + samples.length * 2)
  const view = new DataView(out.buffer)
  const ascii = (at: number, text: string) => [...text].forEach((c, i) => view.setUint8(at + i, c.charCodeAt(0)))
  ascii(0, 'RIFF')
  view.setUint32(4, 36 + samples.length * 2, true)
  ascii(8, 'WAVEfmt ')
  view.setUint32(16, 16, true)
  view.setUint16(20, 1, true) // PCM
  view.setUint16(22, 1, true) // mono
  view.setUint32(24, sampleRate, true)
  view.setUint32(28, sampleRate * 2, true)
  view.setUint16(32, 2, true)
  view.setUint16(34, 16, true)
  ascii(36, 'data')
  view.setUint32(40, samples.length * 2, true)
  samples.forEach((value, i) => {
    const clamped = Math.max(-1, Math.min(1, value))
    view.setInt16(44 + i * 2, clamped < 0 ? clamped * 0x8000 : clamped * 0x7fff, true)
  })
  return out
}

/** The browser decoder: a 16 kHz offline context, channels mixed down to mono. */
export const browserAudioDecoder: AudioDecoder = async (bytes) => {
  const scope = globalThis as unknown as {
    OfflineAudioContext?: typeof OfflineAudioContext
    webkitOfflineAudioContext?: typeof OfflineAudioContext
  }
  const Context = scope.OfflineAudioContext ?? scope.webkitOfflineAudioContext
  if (!Context) throw new MediaFileError('unreadable')
  const context = new Context(1, 1, VOICE_FILE_RATE)
  const buffer = await new Promise<AudioBuffer>((resolve, reject) => {
    // The callback form too: older Safari returns no promise.
    const pending = context.decodeAudioData(bytes, resolve, reject) as Promise<AudioBuffer> | undefined
    pending?.then(resolve, reject)
  })
  const samples = new Float32Array(buffer.length)
  for (let channel = 0; channel < buffer.numberOfChannels; channel++) {
    const data = buffer.getChannelData(channel)
    for (let i = 0; i < data.length; i++) samples[i]! += data[i]! / buffer.numberOfChannels
  }
  return { sampleRate: buffer.sampleRate, samples }
}

export async function voiceRecordingFromFile(
  file: Blob,
  decode: AudioDecoder = browserAudioDecoder,
): Promise<VoiceRecording> {
  const bytes = await file.arrayBuffer()
  const kept = sniffVoiceContainer(new Uint8Array(bytes, 0, Math.min(16, bytes.byteLength)))
  let audio: DecodedAudio
  try {
    // `decodeAudioData` detaches its buffer: hand it a copy so the original can still be sent.
    audio = await decode(bytes.slice(0))
  } catch {
    throw new MediaFileError('unreadable')
  }
  if (audio.samples.length === 0) throw new MediaFileError('unreadable')
  const durationMs = Math.round((audio.samples.length / audio.sampleRate) * 1000)
  const waveform = waveformOfSamples(audio.samples, audio.sampleRate)
  if (kept && kept !== 'wav' && file.size <= MAX_VOICE_FILE_BYTES) {
    return { blob: file, container: kept, durationMs, waveform }
  }
  const wav = encodeWav(audio.samples, audio.sampleRate)
  if (wav.byteLength > MAX_VOICE_FILE_BYTES) throw new MediaFileError('toolarge')
  return { blob: new Blob([wav], { type: 'audio/wav' }), container: 'wav', durationMs, waveform }
}
