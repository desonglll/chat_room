/**
 * TG-1301: the record button when the page cannot record live — plain http on a LAN address,
 * where the browser grants no microphone or camera (or a browser without MediaRecorder). It opens
 * the system recorder / camera through `<input type="file" capture>` instead and sends the file
 * through the same `RecordController` (`sendFile`), so uploading, chat actions and errors match a
 * live recording.
 *
 * Gesture: a tap still toggles mic ↔ camera (`onTap`); a press held past `TAP_MS` opens the
 * system app when it is released — a file picker may only open on a user activation, and on touch
 * screens that is the pointer's release, not its press. Enter/Space opens it, Shift+Enter/Space
 * toggles. A short secondary line says why the system app opened.
 */
import { useEffect, useRef, useState, type PointerEvent, type ReactNode } from 'react'
import { IconButton } from '@tg/ui'
import type { RecordController, RecordState } from './recordController'
import { TAP_MS } from './recordGesture'
import { t } from '../../i18n/index'

export interface CaptureRecordButtonProps {
  state: RecordState
  controller: RecordController
  canSend: boolean
  micGlyph: ReactNode
  /** `audio/*` or `video/*`. */
  accept: string
  /** `capture` on the input: `true` for any recorder, `'user'` for the front camera. */
  capture: true | 'user'
  /** The secondary line shown when the system app opens. */
  hint: string
  idleLabel?: string
  kind?: string
  onTap?(): void
  /** An informational line after a send (e.g. «sent as a regular video»). */
  notice?: string | null
  onNoticeShown?(): void
}

const LINE_MS = 4_000

export function CaptureRecordButton(props: CaptureRecordButtonProps) {
  const { state, controller, canSend, onTap } = props
  const input = useRef<HTMLInputElement>(null)
  const pressedAt = useRef<number | null>(null)
  const [hinting, setHinting] = useState(false)

  useEffect(() => {
    if (!state.error) return
    const timer = window.setTimeout(() => controller.dismissError(), LINE_MS)
    return () => window.clearTimeout(timer)
  }, [state.error, controller])
  useEffect(() => {
    if (!hinting) return
    const timer = window.setTimeout(() => setHinting(false), LINE_MS)
    return () => window.clearTimeout(timer)
  }, [hinting])
  const { notice, onNoticeShown } = props
  useEffect(() => {
    if (!notice) return
    const timer = window.setTimeout(() => onNoticeShown?.(), LINE_MS)
    return () => window.clearTimeout(timer)
  }, [notice, onNoticeShown])

  const open = () => {
    if (!canSend || state.phase !== 'idle') return
    controller.dismissError()
    setHinting(true)
    input.current?.click()
  }
  const onPointerDown = (event: PointerEvent<HTMLButtonElement>) => {
    if (event.button !== 0) return
    pressedAt.current = event.timeStamp
  }
  const onPointerUp = (event: PointerEvent<HTMLButtonElement>) => {
    const since = pressedAt.current
    pressedAt.current = null
    if (since === null) return
    if (onTap && event.timeStamp - since < TAP_MS) onTap()
    else open()
  }

  const line = state.error ?? notice ?? (hinting ? props.hint : null)
  const sending = state.phase === 'sending'
  return (
    <>
      <input
        ref={input}
        type="file"
        hidden
        accept={props.accept}
        // React types `capture` as boolean | 'user' | 'environment'.
        capture={props.capture}
        data-capture-kind={props.kind ?? 'voice'}
        onChange={(event) => {
          const file = event.currentTarget.files?.[0]
          event.currentTarget.value = ''
          setHinting(false)
          if (file) controller.sendFile(file)
        }}
      />
      <IconButton
        label={props.idleLabel ?? t('w.voice.9ccb9e')}
        variant="filled"
        size="lg"
        className="tg-compose__send tg-voice-mic"
        data-kind={props.kind ?? 'voice'}
        data-capture=""
        disabled={!canSend || sending}
        aria-busy={sending || undefined}
        onPointerDown={onPointerDown}
        onPointerUp={onPointerUp}
        onPointerCancel={() => (pressedAt.current = null)}
        onKeyDown={(event) => {
          if (!onTap || !event.shiftKey || (event.key !== 'Enter' && event.key !== ' ')) return
          event.preventDefault()
          onTap()
        }}
        onClick={(event) => {
          // Keyboard activation (no pointer).
          if (event.detail === 0) open()
        }}
        onContextMenu={(event) => event.preventDefault()}
      >
        {props.micGlyph}
      </IconButton>
      {line ? (
        <p
          className="tg-voice-error"
          role={state.error ? 'alert' : 'status'}
          data-tone={state.error ? 'error' : 'info'}
        >
          {line}
        </p>
      ) : null}
    </>
  )
}
