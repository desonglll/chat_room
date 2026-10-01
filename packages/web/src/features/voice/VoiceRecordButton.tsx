/**
 * The composer's voice button and its recording panel (TG-401), Telegram-style:
 * press and hold to record, release to send, slide left to cancel, slide up to lock
 * (hands-free). A tap — or Enter/Space — records hands-free straight away. While recording,
 * the panel covers the input with the red dot, timer, live waveform and the cancel hint.
 *
 * TG-402: with `onTap`, a pointer tap calls it instead (the mic ↔ camera toggle, Telegram's
 * mobile behaviour) and recording starts once the press has been held for `TAP_MS`;
 * Shift+Enter/Space calls it from the keyboard. `overlay` (the round viewfinder) is drawn
 * while recording, and `waveform={false}` drops the live waveform from the panel.
 */
import { useEffect, useRef, type CSSProperties, type PointerEvent, type ReactNode } from 'react'
import type { ClientFrame } from '@tg/core'
import { IconButton } from '@tg/ui'
import { ChevronLeftGlyph, LockGlyph } from './glyphs'
import { LIVE_LEVELS, type RecordController, type RecordState } from './recordController'
import { TAP_MS } from './recordGesture'
import { useVoiceRecording } from './useVoiceRecording'
import { formatRecordingTime } from './waveform'
import { t } from '../../i18n/index'

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
  /** TG-402: a pointer tap calls this instead of recording hands-free. */
  onTap?(): void
  /** Idle / hands-free send labels and `data-kind`; voice's by default. */
  idleLabel?: string
  sendLabel?: string
  kind?: string
  /** Drawn while recording (TG-402's round viewfinder). */
  overlay?: ReactNode
  /** Live waveform in the recording panel (default on). */
  waveform?: boolean
}

/** Stateless view over a `RecordController`, rendered by tests with a fake one. */
export function VoiceRecordView(props: VoiceRecordViewProps) {
  const { state, controller, canSend, micGlyph, sendGlyph, onTap } = props
  const active = state.phase === 'starting' || state.phase === 'recording'
  const locked = active && state.gesture.phase === 'locked'
  const holding = active && state.gesture.phase === 'holding'
  // A press waiting to become a hold (only with `onTap`).
  const pending = useRef<number | null>(null)
  const clearPending = () => {
    if (pending.current !== null) window.clearTimeout(pending.current)
    pending.current = null
  }
  useEffect(() => clearPending, [])

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
    const { clientX, clientY } = event
    if (!onTap) {
      controller.press(clientX, clientY)
      return
    }
    clearPending()
    pending.current = window.setTimeout(() => {
      pending.current = null
      controller.press(clientX, clientY, TAP_MS)
    }, TAP_MS)
  }
  const onPointerUp = () => {
    if (pending.current !== null) {
      clearPending()
      onTap?.()
      return
    }
    if (holding) controller.release()
  }
  const level = state.levels.at(-1) ?? 0

  return (
    <>
      {active ? props.overlay : null}
      {active ? (
        <RecordingPanel state={state} controller={controller} locked={locked} waveform={props.waveform ?? true} />
      ) : null}
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
        label={
          locked
            ? (props.sendLabel ?? t('w.voice.cc01a4'))
            : active
              ? t('w.voice.bf0580')
              : (props.idleLabel ?? t('w.voice.9ccb9e'))
        }
        variant="filled"
        size="lg"
        className="tg-compose__send tg-voice-mic"
        data-kind={props.kind ?? 'voice'}
        data-recording={holding || undefined}
        style={{ '--tg-voice-level': level } as CSSProperties}
        disabled={!canSend || state.phase === 'sending'}
        aria-busy={state.phase === 'sending' || undefined}
        onPointerDown={onPointerDown}
        onPointerMove={(event) => holding && controller.move(event.clientX, event.clientY)}
        onPointerUp={onPointerUp}
        onPointerCancel={() => {
          clearPending()
          if (holding) controller.cancel()
        }}
        onKeyDown={(event) => {
          if (!onTap || !event.shiftKey || (event.key !== 'Enter' && event.key !== ' ')) return
          event.preventDefault()
          if (state.phase === 'idle') onTap()
        }}
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
  waveform,
}: {
  state: RecordState
  controller: RecordController
  locked: boolean
  waveform: boolean
}) {
  const bars = Array.from({ length: LIVE_LEVELS }, (_, index) => {
    const offset = LIVE_LEVELS - state.levels.length
    return index < offset ? 0 : (state.levels[index - offset] ?? 0)
  })
  return (
    <div
      className="tg-voice-rec"
      role="group"
      aria-label={waveform ? t('w.voice.3c4dff') : t('w.voice.8be901')}
      style={{ '--tg-voice-cancel-progress': state.gesture.cancelProgress } as CSSProperties}
    >
      <span className="tg-voice-rec__dot" aria-hidden="true" />
      <span className="tg-voice-rec__time" aria-live="off">
        {formatRecordingTime(state.elapsedMs)}
      </span>
      {waveform ? (
        <span className="tg-voice-rec__wave" aria-hidden="true">
          {bars.map((peak, index) => (
            <span key={index} style={{ blockSize: `${Math.round(10 + Math.min(1, peak * 1.6) * 90)}%` }} />
          ))}
        </span>
      ) : (
        <span className="tg-voice-rec__wave" aria-hidden="true" />
      )}
      {locked ? (
        <button type="button" className="tg-voice-rec__cancel" onClick={() => controller.cancel()}>
          {t('w.voice.4d0b46')}
        </button>
      ) : (
        <span className="tg-voice-rec__hint">
          <ChevronLeftGlyph />
          {t('w.voice.0c7fe0')}
        </span>
      )}
    </div>
  )
}
