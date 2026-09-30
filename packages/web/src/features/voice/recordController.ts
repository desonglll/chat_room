/**
 * The composer's voice recording flow without React: gesture → recorder → chat actions →
 * upload. `useVoiceRecording` is a thin `useSyncExternalStore` over this, and
 * `test/recordController.test.ts` drives it with a fake recorder, clock and uploader.
 *
 * Chat actions (TG-107): `recording_voice` while the microphone is open, `uploading_voice`
 * while the file goes out, `cancel` when it is sent, discarded or fails.
 */
import type { ChatActionSender } from '@tg/core'
import { ApiError, VOICE_RESTRICTED } from '@tg/core'
import {
  IDLE_GESTURE,
  moveGesture,
  pressGesture,
  releaseGesture,
  type GestureOutcome,
  type GestureState,
} from './recordGesture'
import { RecorderError, type VoiceRecorder, type VoiceRecording } from './voiceRecorder'

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

export interface RecordControllerDeps {
  chatId: string
  createRecorder(): VoiceRecorder
  upload(recording: VoiceRecording): Promise<void>
  actions: ChatActionSender
  now(): number
  /** The upload went out (the composer consumes its reply bar). */
  onSent?(): void
}

export interface RecordController {
  getState(): RecordState
  subscribe(listener: () => void): () => void
  press(x: number, y: number): void
  move(x: number, y: number): void
  release(): void
  /** Hands-free mode's «发送». */
  send(): void
  cancel(): void
  dismissError(): void
  dispose(): void
}

export function recordErrorText(error: unknown): string {
  if (error instanceof RecorderError) {
    if (error.reason === 'unsupported') return '此浏览器不支持录制语音'
    if (error.reason === 'permission') return '无法使用麦克风：请在浏览器设置中允许访问'
    return '麦克风不可用'
  }
  if (error instanceof ApiError) {
    if (error.serverMessage === VOICE_RESTRICTED) return '对方设置了不接收你的语音消息'
    if (error.status === 403) return '没有在此会话发送消息的权限'
    if (error.status === 413) return '语音消息过大'
  }
  return '语音发送失败，请重试'
}

export function createRecordController(deps: RecordControllerDeps): RecordController {
  const listeners = new Set<() => void>()
  let state: RecordState = IDLE_RECORD_STATE
  let recorder: VoiceRecorder | null = null
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
    set({ ...IDLE_RECORD_STATE, error: recordErrorText(error) })
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
    deps.actions.sendChatAction(deps.chatId, 'uploading_voice')
    active
      .stop()
      .then((recording) => deps.upload(recording))
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
    press(x, y) {
      if (state.phase !== 'idle') return
      let created: VoiceRecorder
      try {
        created = deps.createRecorder()
      } catch (error) {
        fail(error)
        return
      }
      recorder = created
      set({ ...IDLE_RECORD_STATE, phase: 'starting', gesture: pressGesture(x, y, deps.now()) })
      stopLevels = created.onLevel((peak, elapsedMs) => {
        const levels = state.levels.length >= LIVE_LEVELS ? state.levels.slice(1) : state.levels.slice()
        levels.push(peak)
        set({ levels, elapsedMs })
      })
      created.start().then(
        () => {
          if (recorder !== created) return
          set({ phase: 'recording' })
          deps.actions.sendChatAction(deps.chatId, 'recording_voice')
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
    cancel,
    dismissError: () => set({ error: null }),
    dispose() {
      cancel()
      listeners.clear()
    },
  }
}
