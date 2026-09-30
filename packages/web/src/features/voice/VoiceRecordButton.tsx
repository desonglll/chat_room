/**
 * The composer's voice button and its recording panel (TG-401), Telegram-style:
 * press and hold to record, release to send, slide left to cancel, slide up to lock
 * (hands-free). A tap — or Enter/Space — records hands-free straight away. While recording,
 * the panel covers the input with the red dot, timer, live waveform and the cancel hint.
 */
import { useEffect, type CSSProperties, type PointerEvent, type ReactNode } from 'react'
import type { ClientFrame } from '@tg/core'
import { IconButton } from '@tg/ui'
import { ChevronLeftGlyph, LockGlyph } from './glyphs'
import { LIVE_LEVELS, type RecordController, type RecordState } from './recordController'
import { useVoiceRecording } from './useVoiceRecording'
import { formatRecordingTime } from './waveform'

export interface VoiceRecordButtonProps {
  chatId: string
  replyTo: string | null
  canSend: boolean
  sendFrame(frame: ClientFrame): boolean
  /** The voice message went out: the composer consumes its reply bar. */
  onSent?(): void
  /** The composer's own icons, so the button matches its send twin. */
  micGlyph: ReactNode
  sendGlyph: ReactNode
}

export function VoiceRecordButton(props: VoiceRecordButtonProps) {
  const { state, controller } = useVoiceRecording(props)
  return <VoiceRecordView {...props} state={state} controller={controller} />
}

export interface VoiceRecordViewProps extends VoiceRecordButtonProps {
  state: RecordState
  controller: RecordController
}

/** Stateless view over a `RecordController`, rendered by tests with a fake one. */
export function VoiceRecordView({ state, controller, canSend, micGlyph, sendGlyph }: VoiceRecordViewProps) {
  const active = state.phase === 'starting' || state.phase === 'recording'
  const locked = active && state.gesture.phase === 'locked'
  const holding = active && state.gesture.phase === 'holding'

  useEffect(() => {
    if (!active) return
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') controller.cancel()
    }
    document.addEventListener('keydown', onKey)
    return () => document.removeEventListener('keydown', onKey)
  }, [active, controller])

  useEffect(() => {
    if (!state.error) return
    const timer = window.setTimeout(() => controller.dismissError(), 4_000)
    return () => window.clearTimeout(timer)
  }, [state.error, controller])

  const onPointerDown = (event: PointerEvent<HTMLButtonElement>) => {
    if (event.button !== 0 || locked || !canSend) return
    event.currentTarget.setPointerCapture?.(event.pointerId)
    controller.press(event.clientX, event.clientY)
  }
  const level = state.levels.at(-1) ?? 0

  return (
    <>
      {active ? <RecordingPanel state={state} controller={controller} locked={locked} /> : null}
      {holding ? (
        <span
          className="tg-voice-lock"
          aria-hidden="true"
          style={{ '--tg-voice-lock-progress': state.gesture.lockProgress } as CSSProperties}
        >
          <LockGlyph open={state.gesture.lockProgress < 1} />
        </span>
      ) : null}
      <IconButton
        label={locked ? '发送语音' : active ? '松开发送，上滑锁定' : '按住录制语音消息'}
        variant="filled"
        size="lg"
        className="tg-compose__send tg-voice-mic"
        data-kind="voice"
        data-recording={holding || undefined}
        style={{ '--tg-voice-level': level } as CSSProperties}
        disabled={!canSend || state.phase === 'sending'}
        aria-busy={state.phase === 'sending' || undefined}
        onPointerDown={onPointerDown}
        onPointerMove={(event) => holding && controller.move(event.clientX, event.clientY)}
        onPointerUp={() => holding && controller.release()}
        onPointerCancel={() => holding && controller.cancel()}
        onClick={(event) => {
          if (locked) {
            controller.send()
            return
          }
          // Keyboard activation (no pointer): a tap, i.e. hands-free recording.
          if (event.detail === 0 && state.phase === 'idle') {
            const rect = event.currentTarget.getBoundingClientRect()
            controller.press(rect.left, rect.top)
            controller.release()
          }
        }}
        onContextMenu={(event) => event.preventDefault()}
      >
        {locked ? sendGlyph : micGlyph}
      </IconButton>
      {state.error ? (
        <p className="tg-voice-error" role="alert">
          {state.error}
        </p>
      ) : null}
    </>
  )
}

function RecordingPanel({
  state,
  controller,
  locked,
}: {
  state: RecordState
  controller: RecordController
  locked: boolean
}) {
  const bars = Array.from({ length: LIVE_LEVELS }, (_, index) => {
    const offset = LIVE_LEVELS - state.levels.length
    return index < offset ? 0 : (state.levels[index - offset] ?? 0)
  })
  return (
    <div
      className="tg-voice-rec"
      role="group"
      aria-label="正在录制语音"
      style={{ '--tg-voice-cancel-progress': state.gesture.cancelProgress } as CSSProperties}
    >
      <span className="tg-voice-rec__dot" aria-hidden="true" />
      <span className="tg-voice-rec__time" aria-live="off">
        {formatRecordingTime(state.elapsedMs)}
      </span>
      <span className="tg-voice-rec__wave" aria-hidden="true">
        {bars.map((peak, index) => (
          <span key={index} style={{ blockSize: `${Math.round(10 + Math.min(1, peak * 1.6) * 90)}%` }} />
        ))}
      </span>
      {locked ? (
        <button type="button" className="tg-voice-rec__cancel" onClick={() => controller.cancel()}>
          取消
        </button>
      ) : (
        <span className="tg-voice-rec__hint">
          <ChevronLeftGlyph />
          滑动取消
        </span>
      )}
    </div>
  )
}
