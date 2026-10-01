// TG-1301: recording without a live microphone/camera — the system recorder's file becomes a
// voice message (as is, or as WAV) or a round/regular video, through the same RecordController.
import { describe, expect, test } from 'bun:test'
import type { ChatActionSender, TypingAction } from '@tg/core'
import { pickedPoint } from '../../location/LocationPicker'
import { isRegularVideo, sniffVideoContainer, videoUploadFromFile, type VideoProbe } from '../../videoNote/videoFile'
import { createRecordController, IDLE_RECORD_STATE, recordErrorText } from '../recordController'
import {
  encodeWav,
  MediaFileError,
  sniffVoiceContainer,
  voiceRecordingFromFile,
  waveformOfSamples,
  type AudioDecoder,
} from '../voiceFile'
import type { VoiceRecording } from '../voiceRecorder'

const bytes = (...parts: Array<string | number[]>) =>
  new Uint8Array(parts.flatMap((part) => (typeof part === 'string' ? [...part].map((c) => c.charCodeAt(0)) : part)))
const M4A_HEAD = bytes([0, 0, 0, 0x20], 'ftypM4A ', [0, 0, 0, 0])
const MP3_HEAD = bytes('ID3', [4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
const tone = (seconds: number, rate = 16_000) =>
  Float32Array.from({ length: seconds * rate }, (_, i) => Math.sin(i / 10) * (i < (seconds * rate) / 2 ? 0.2 : 0.8))
const decoderOf =
  (samples: Float32Array, rate = 16_000): AudioDecoder =>
  async () => ({ sampleRate: rate, samples })
const failing: AudioDecoder = async () => {
  throw new DOMException('Unable to decode audio data', 'EncodingError')
}

describe('TG-1301 voice from a system recorder file', () => {
  test('containers are recognised as the server sniffs them', () => {
    expect(sniffVoiceContainer(bytes('OggS', [0, 2]))).toBe('ogg')
    expect(sniffVoiceContainer(bytes([0x1a, 0x45, 0xdf, 0xa3]))).toBe('webm')
    expect(sniffVoiceContainer(M4A_HEAD)).toBe('mp4')
    expect(sniffVoiceContainer(bytes('RIFF', [0, 0, 0, 0], 'WAVE'))).toBe('wav')
    expect(sniffVoiceContainer(MP3_HEAD)).toBeNull()
  })

  test('the waveform has 100 bars and follows the loudness', () => {
    const waveform = waveformOfSamples(tone(2), 16_000)
    expect(waveform).toHaveLength(100)
    expect(Math.max(...waveform.slice(0, 45))).toBeLessThan(Math.min(...waveform.slice(55)))
    expect(Math.max(...waveform)).toBe(31)
  })

  test('WAV encoding writes a 16-bit mono header the server can read', () => {
    const wav = encodeWav(Float32Array.from([0, 1, -1, 0.5]), 16_000)
    const view = new DataView(wav.buffer)
    expect(String.fromCharCode(...wav.subarray(0, 4), ...wav.subarray(8, 16))).toBe('RIFFWAVEfmt ')
    expect(view.getUint32(28, true)).toBe(32_000) // byte rate → the server's duration
    expect(view.getUint32(40, true)).toBe(8)
    expect([view.getInt16(46, true), view.getInt16(48, true)]).toEqual([0x7fff, -0x8000])
  })

  test('an M4A from the recorder is sent as it is, with duration and waveform from its audio', async () => {
    const file = new Blob([M4A_HEAD, new Uint8Array(100)], { type: 'audio/mp4' })
    const recording = await voiceRecordingFromFile(file, decoderOf(tone(3)))
    expect(recording.blob).toBe(file)
    expect(recording.container).toBe('mp4')
    expect(recording.durationMs).toBe(3_000)
    expect(recording.waveform).toHaveLength(100)
  })

  test('an MP3 (no recorder container) becomes 16 kHz mono WAV', async () => {
    const file = new Blob([MP3_HEAD, new Uint8Array(100)], { type: 'audio/mpeg' })
    const recording = await voiceRecordingFromFile(file, decoderOf(tone(1)))
    expect(recording.container).toBe('wav')
    expect(recording.blob.type).toBe('audio/wav')
    expect(recording.blob.size).toBe(44 + 16_000 * 2)
    expect(recording.durationMs).toBe(1_000)
  })

  test('an undecodable or empty file is refused with its own copy, never sent', async () => {
    const file = new Blob([MP3_HEAD])
    const refused = await voiceRecordingFromFile(file, failing).catch((error: unknown) => error)
    expect(refused).toEqual(new MediaFileError('unreadable'))
    expect(await voiceRecordingFromFile(file, decoderOf(new Float32Array(0))).catch((e: unknown) => e)).toEqual(
      new MediaFileError('unreadable'),
    )
    expect(recordErrorText(refused)).toBe('无法识别该录音格式')
  })

  test('a recording too long for the server as WAV is refused as too large', async () => {
    const long = new Float32Array(16_000 * 60 * 14) // 14 min ≈ 26.9 MB of WAV
    const error = await voiceRecordingFromFile(new Blob([MP3_HEAD]), decoderOf(long)).catch((e: unknown) => e)
    expect(error).toEqual(new MediaFileError('toolarge'))
    expect(recordErrorText(error)).toBe('语音消息过大')
  })
})

describe('TG-1301 round video from a system camera file', () => {
  const probeOf =
    (durationMs: number, thumbnail: Blob | null = new Blob(['jpg'])): VideoProbe =>
    async () => ({ durationMs, thumbnail: async () => thumbnail })
  const mp4 = (size: number) => new Blob([M4A_HEAD, new Uint8Array(Math.max(0, size - M4A_HEAD.length))])

  test('containers are recognised as the server sniffs them', () => {
    expect(sniffVideoContainer(M4A_HEAD)).toBe('mp4')
    expect(sniffVideoContainer(bytes([0x1a, 0x45, 0xdf, 0xa3]))).toBe('webm')
    expect(sniffVideoContainer(bytes('RIFF'))).toBeNull()
  })

  test('a short, small MP4 is a round video with its thumbnail', async () => {
    const upload = await videoUploadFromFile(mp4(2_000_000), probeOf(9_000))
    expect(isRegularVideo(upload)).toBe(false)
    if (isRegularVideo(upload)) return
    expect(upload.container).toBe('mp4')
    expect(upload.durationMs).toBe(9_000)
    expect(upload.thumbnail).not.toBeNull()
  })

  test('over 16 MB, over 61 s, or not MP4/WebM: sent as a regular video instead', async () => {
    expect(isRegularVideo(await videoUploadFromFile(mp4(16 * 1024 * 1024 + 1), probeOf(5_000)))).toBe(true)
    expect(isRegularVideo(await videoUploadFromFile(mp4(1_000), probeOf(61_001)))).toBe(true)
    const avi = new Blob([bytes('RIFF', [0, 0, 0, 0], 'AVI ')], { type: 'video/x-msvideo' })
    const regular = await videoUploadFromFile(avi, probeOf(3_000))
    expect(isRegularVideo(regular) && regular.fileName).toBe('video.mp4')
  })

  test('a file the browser cannot load is refused', async () => {
    const unreadable: VideoProbe = async () => {
      throw new MediaFileError('unreadable')
    }
    expect(await videoUploadFromFile(mp4(100), unreadable).catch((e: unknown) => e)).toEqual(
      new MediaFileError('unreadable'),
    )
  })
})

describe('TG-1301 sendFile through the record controller', () => {
  const setup = (fromFile: (file: Blob) => Promise<VoiceRecording>, upload = async () => {}) => {
    const actions: TypingAction[] = []
    const sender: ChatActionSender = { sendChatAction: (_c, action) => actions.push(action), dispose: () => {} }
    const uploads: VoiceRecording[] = []
    let sent = 0
    const controller = createRecordController({
      chatId: 'c1',
      actions: sender,
      now: () => 0,
      createRecorder: () => {
        throw new Error('no live recording in this test')
      },
      fromFile,
      upload: async (recording) => {
        uploads.push(recording)
        await upload()
      },
      onSent: () => (sent += 1),
    })
    return { controller, actions, uploads, sent: () => sent }
  }
  const recording: VoiceRecording = { blob: new Blob(['x']), container: 'wav', durationMs: 1_000, waveform: [] }

  test('converts, uploads with the uploading action, then returns to idle', async () => {
    const { controller, actions, uploads, sent } = setup(async () => recording)
    controller.sendFile(new Blob(['raw']))
    expect(controller.getState().phase).toBe('sending')
    await new Promise((resolve) => setTimeout(resolve, 0))
    expect(uploads).toEqual([recording])
    expect(actions).toEqual(['uploading_voice', 'cancel'])
    expect(sent()).toBe(1)
    expect(controller.getState()).toEqual(IDLE_RECORD_STATE)
  })

  test('a conversion failure shows its copy and nothing is uploaded', async () => {
    const { controller, uploads } = setup(async () => {
      throw new MediaFileError('unreadable')
    })
    controller.sendFile(new Blob(['raw']))
    await new Promise((resolve) => setTimeout(resolve, 0))
    expect(uploads).toEqual([])
    expect(controller.getState().error).toBe('无法识别该录音格式')
    expect(controller.getState().phase).toBe('idle')
  })

  test('ignored while busy', async () => {
    const { controller, uploads } = setup(
      async () => recording,
      () => new Promise(() => {}),
    )
    controller.sendFile(new Blob(['a']))
    controller.sendFile(new Blob(['b']))
    await new Promise((resolve) => setTimeout(resolve, 0))
    expect(uploads).toHaveLength(1)
  })
})

describe('TG-1301 map picker point', () => {
  test('rounds to 6 decimals and wraps the longitude into -180..180', () => {
    expect(pickedPoint(31.2304161, 121.4737011)).toEqual({ latitude: 31.230416, longitude: 121.473701 })
    expect(pickedPoint(10, 190)).toEqual({ latitude: 10, longitude: -170 })
    expect(pickedPoint(-95, -540)).toEqual({ latitude: -90, longitude: -180 })
  })
})
