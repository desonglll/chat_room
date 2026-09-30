/**
 * The one voice player of the app (Telegram plays one voice message at a time). A single
 * audio element is reused; the store says which message is loaded, whether it plays, where it
 * is, and at what speed. Framework-free: the audio element, the frame clock and "what plays
 * next" are injected, so `voicePlayer.test.ts` runs the whole lifecycle with fakes.
 *
 * - `toggle` plays/pauses a track (switching tracks stops the previous one);
 * - `seek` jumps to a fraction, starting the track if it is not the loaded one;
 * - when a track ends, `findNext` names the next voice message and it plays automatically;
 * - `cycleRate` steps 1× → 1.5× → 2× → 1×, kept for every later track.
 *
 * Durations come from the message (`voice.duration_ms`), not from the element: a
 * MediaRecorder WebM reports `Infinity` until it has been scanned once.
 */
import { createStore } from 'zustand/vanilla'

export interface AudioLike {
  src: string
  currentTime: number
  playbackRate: number
  readonly paused: boolean
  play(): Promise<void>
  pause(): void
  onended: ((event: Event) => void) | null
  onerror: ((event: Event | string) => void) | null
}

export interface VoiceTrack {
  messageId: string
  url: string
  durationMs: number
}

export const VOICE_RATES = [1, 1.5, 2] as const
export type VoiceRate = (typeof VOICE_RATES)[number]

export interface VoicePlayerState {
  current: VoiceTrack | null
  playing: boolean
  positionMs: number
  rate: VoiceRate
  /** The loaded track failed to load or play. */
  failed: boolean
}

export interface VoicePlayerDeps {
  createAudio(): AudioLike
  requestFrame(callback: () => void): unknown
  cancelFrame(handle: unknown): void
  /** The voice message after `messageId` in its chat, or null. */
  findNext(messageId: string): VoiceTrack | null
  /** A track began playing (listened reporting hooks in here). */
  onStart?(track: VoiceTrack): void
  initialRate?: VoiceRate
  onRateChange?(rate: VoiceRate): void
}

export type VoicePlayerStore = ReturnType<typeof createPlayerStore>

const createPlayerStore = (rate: VoiceRate) =>
  createStore<VoicePlayerState>()(() => ({ current: null, playing: false, positionMs: 0, rate, failed: false }))

export interface VoicePlayer {
  store: VoicePlayerStore
  toggle(track: VoiceTrack): void
  seek(track: VoiceTrack, fraction: number): void
  cycleRate(): void
  stop(): void
}

export function createVoicePlayer(deps: VoicePlayerDeps): VoicePlayer {
  const store = createPlayerStore(deps.initialRate ?? 1)
  let audio: AudioLike | null = null
  let frame: unknown = null

  const element = (): AudioLike => {
    if (audio) return audio
    audio = deps.createAudio()
    audio.onended = () => {
      const ended = store.getState().current
      stopTicking()
      store.setState({ playing: false, positionMs: ended?.durationMs ?? 0 })
      const next = ended ? deps.findNext(ended.messageId) : null
      if (next) start(next, 0)
    }
    audio.onerror = () => {
      stopTicking()
      store.setState({ playing: false, failed: true })
    }
    return audio
  }

  const tick = () => {
    const current = store.getState().current
    if (audio && current) {
      store.setState({ positionMs: Math.min(current.durationMs, Math.max(0, audio.currentTime * 1000)) })
    }
    frame = deps.requestFrame(tick)
  }

  const stopTicking = () => {
    if (frame !== null) deps.cancelFrame(frame)
    frame = null
  }

  const play = () => {
    const media = element()
    const track = store.getState().current
    media.playbackRate = store.getState().rate
    store.setState({ playing: true, failed: false })
    stopTicking()
    frame = deps.requestFrame(tick)
    media.play().then(
      () => {
        if (track) deps.onStart?.(track)
      },
      () => {
        stopTicking()
        if (store.getState().current === track) store.setState({ playing: false, failed: true })
      },
    )
  }

  const start = (track: VoiceTrack, fraction: number) => {
    const media = element()
    media.pause()
    if (media.src !== track.url) media.src = track.url
    const positionMs = clampFraction(fraction) * track.durationMs
    media.currentTime = positionMs / 1000
    store.setState({ current: track, positionMs })
    play()
  }

  return {
    store,
    toggle(track) {
      const state = store.getState()
      if (state.current?.messageId !== track.messageId) {
        start(track, 0)
        return
      }
      if (state.playing) {
        audio?.pause()
        stopTicking()
        store.setState({ playing: false })
        return
      }
      const fromEnd = state.positionMs >= track.durationMs - 50
      if (fromEnd && audio) audio.currentTime = 0
      play()
    },
    seek(track, fraction) {
      const state = store.getState()
      if (state.current?.messageId !== track.messageId) {
        start(track, fraction)
        return
      }
      const positionMs = clampFraction(fraction) * track.durationMs
      element().currentTime = positionMs / 1000
      store.setState({ positionMs })
    },
    cycleRate() {
      const rates: readonly VoiceRate[] = VOICE_RATES
      const rate = rates[(rates.indexOf(store.getState().rate) + 1) % rates.length]!
      if (audio) audio.playbackRate = rate
      store.setState({ rate })
      deps.onRateChange?.(rate)
    },
    stop() {
      audio?.pause()
      stopTicking()
      store.setState({ current: null, playing: false, positionMs: 0 })
    },
  }
}

function clampFraction(value: number): number {
  return Number.isFinite(value) ? Math.min(1, Math.max(0, value)) : 0
}
