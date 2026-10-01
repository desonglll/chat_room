/**
 * The composer's voice recording flow without React: gesture → recorder → chat actions →
 * upload. `useVoiceRecording` is a thin `useSyncExternalStore` over this, and
 * `test/recordController.test.ts` drives it with a fake recorder, clock and uploader.
 *
 * Chat actions (TG-107): `recording_voice` while the microphone is open, `uploading_voice`
 * while the file goes out, `cancel` when it is sent, discarded or fails.
 *
 * TG-402 reuses the same flow for round video messages: the recorder and upload are generic
 * (`RecorderSession<T>`), and the chat actions, the cap and the error copy are injectable.
 */
import type { ChatActionSender, TypingAction } from '@tg/core'
import { ApiError, VOICE_RESTRICTED } from '@tg/core'
import {
  IDLE_GESTURE,
  moveGesture,
  pressGesture,
  releaseGesture,
  type GestureOutcome,
  type GestureState,
} from './recordGesture'
import { MediaFileError } from './voiceFile'
import { RecorderError, type VoiceRecording } from './voiceRecorder'
import { t } from '../../i18n/index'

/** Shorter recordings are dropped (an accidental press), as Telegram does. */
export const MIN_VOICE_MS = 700
/** Bars of the live waveform shown while recording. */
export const LIVE_LEVELS = 48

export type RecordPhase = 'idle' | 'starting' | 'recording' | 'sending'

export interface RecordState {
  phase: RecordPhase
  gesture: GestureState
  elapsedMs: number
  /** Newest last, 0..1 each. */
  levels: number[]
  error: string | null
}

export const IDLE_RECORD_STATE: RecordState = {
  phase: 'idle',
  gesture: IDLE_GESTURE,
  elapsedMs: 0,
  levels: [],
  error: null,
}

/** What the controller needs from a recorder; `VoiceRecorder` is one (TG-402 adds video). */
export interface RecorderSession<T> {
  start(): Promise<void>
  onLevel(listener: (peak: number, elapsedMs: number) => void): () => void
  elapsedMs(): number
  stop(): Promise<T>
  cancel(): void
}

export interface RecordControllerDeps<T = VoiceRecording> {
  chatId: string
  createRecorder(): RecorderSession<T>
  upload(recording: T): Promise<void>
  actions: ChatActionSender
  now(): number
  /** The upload went out (the composer consumes its reply bar). */
  onSent?(): void
  /** Chat actions while recording / uploading; voice's by default. */
  chatActions?: { recording: TypingAction; uploading: TypingAction }
  /** Recording stops and sends by itself at this length (TG-402: 60 s). */
  maxMs?: number
  /** Error copy; voice's `recordErrorText` by default. */
  errorText?(error: unknown): string
  /** TG-1301: a recording from a file the system recorder/camera made (no live microphone). */
  fromFile?(file: Blob): Promise<T>
}

export interface RecordController {
  getState(): RecordState
  subscribe(listener: () => void): () => void
  /** `heldMs`: how long the pointer has already been down (a delayed press is no tap). */
  press(x: number, y: number, heldMs?: number): void
  move(x: number, y: number): void
  release(): void
  /** Hands-free mode's «发送». */
  send(): void
  /** TG-1301: convert and upload a system recorder's file (`deps.fromFile`), like a recording. */
  sendFile(file: Blob): void
  cancel(): void
  dismissError(): void
  dispose(): void
}

export function recordErrorText(error: unknown): string {
  if (error instanceof MediaFileError) return error.reason === 'toolarge' ? t('w.voice.1544d5') : t('w.voice.a056b7')
  if (error instanceof RecorderError) {
    if (error.reason === 'insecure') return t('w.voice.eec4bc')
    if (error.reason === 'unsupported') return t('w.voice.7fb030')
    if (error.reason === 'permission') return t('w.voice.10639e')
    if (error.reason === 'nodevice') return t('w.voice.c72649')
    return t('w.voice.003f6b')
  }
  if (error instanceof ApiError) {
    if (error.serverMessage === VOICE_RESTRICTED) return t('w.voice.162eb5')
    if (error.status === 403) return t('w.voice.4fcc34')
    if (error.status === 413) return t('w.voice.1544d5')
  }
  return t('w.voice.679eab')
}

export function createRecordController<T = VoiceRecording>(deps: RecordControllerDeps<T>): RecordController {
  const listeners = new Set<() => void>()
  const chatActions = deps.chatActions ?? { recording: 'recording_voice', uploading: 'uploading_voice' }
  const errorText = deps.errorText ?? recordErrorText
  let state: RecordState = IDLE_RECORD_STATE
  let recorder: RecorderSession<T> | null = null
  let stopLevels: (() => void) | null = null

  const set = (next: Partial<RecordState>) => {
    state = { ...state, ...next }
    for (const listener of listeners) listener()
  }

  const teardown = () => {
    stopLevels?.()
    stopLevels = null
    recorder = null
  }

  const fail = (error: unknown) => {
    recorder?.cancel()
    teardown()
    deps.actions.sendChatAction(deps.chatId, 'cancel')
    set({ ...IDLE_RECORD_STATE, error: errorText(error) })
  }

  const cancel = () => {
    if (state.phase !== 'starting' && state.phase !== 'recording') return
    recorder?.cancel()
    teardown()
    deps.actions.sendChatAction(deps.chatId, 'cancel')
    set(IDLE_RECORD_STATE)
  }

  const finish = () => {
    const active = recorder
    if (state.phase !== 'recording' || !active) {
      cancel()
      return
    }
    if (active.elapsedMs() < MIN_VOICE_MS) {
      cancel()
      return
    }
    teardown()
    set({ phase: 'sending', gesture: IDLE_GESTURE })
    upload(active.stop())
  }

  const upload = (recording: Promise<T>) => {
    deps.actions.sendChatAction(deps.chatId, chatActions.uploading)
    recording
      .then((ready) => deps.upload(ready))
      .then(() => {
        deps.actions.sendChatAction(deps.chatId, 'cancel')
        deps.onSent?.()
        set(IDLE_RECORD_STATE)
      })
      .catch(fail)
  }

  const apply = (outcome: GestureOutcome) => {
    if (outcome === 'cancel') cancel()
    if (outcome === 'send') finish()
  }

  return {
    getState: () => state,
    subscribe(listener) {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    press(x, y, heldMs = 0) {
      if (state.phase !== 'idle') return
      let created: RecorderSession<T>
      try {
        created = deps.createRecorder()
      } catch (error) {
        fail(error)
        return
      }
      recorder = created
      set({ ...IDLE_RECORD_STATE, phase: 'starting', gesture: pressGesture(x, y, deps.now() - heldMs) })
      stopLevels = created.onLevel((peak, elapsedMs) => {
        const levels = state.levels.length >= LIVE_LEVELS ? state.levels.slice(1) : state.levels.slice()
        levels.push(peak)
        set({ levels, elapsedMs })
        if (deps.maxMs !== undefined && elapsedMs >= deps.maxMs && recorder === created) finish()
      })
      created.start().then(
        () => {
          if (recorder !== created) return
          set({ phase: 'recording' })
          deps.actions.sendChatAction(deps.chatId, chatActions.recording)
          // Released (send) before the microphone opened: honour it now.
          if (state.gesture.phase === 'idle') finish()
        },
        (error: unknown) => {
          if (recorder === created) fail(error)
        },
      )
    },
    move(x, y) {
      const { state: gesture, outcome } = moveGesture(state.gesture, x, y)
      if (gesture !== state.gesture) set({ gesture })
      apply(outcome)
    },
    release() {
      const { state: gesture, outcome } = releaseGesture(state.gesture, deps.now())
      if (gesture !== state.gesture) set({ gesture })
      if (state.phase === 'starting' && outcome === 'send') return // finished once started
      apply(outcome)
    },
    send: finish,
    sendFile(file) {
      if (state.phase !== 'idle' || !deps.fromFile) return
      set({ ...IDLE_RECORD_STATE, phase: 'sending' })
      upload(deps.fromFile(file))
    },
    cancel,
    dismissError: () => set({ error: null }),
    dispose() {
      cancel()
      listeners.clear()
    },
  }
}
