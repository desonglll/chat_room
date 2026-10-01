/**
 * The bubble's waveform: bars from the stored 100 samples, the played part in the accent,
 * and a draggable/keyboard seek (it is an ARIA slider over the voice message's seconds).
 */
import { useRef, type KeyboardEvent, type PointerEvent } from 'react'
import { formatVoiceDuration, resampleWaveform, WAVEFORM_MAX } from './waveform'
import { t } from '../../i18n/index'

export interface VoiceWaveformProps {
  waveform: readonly number[]
  bars: number
  /** 0..1 played. */
  progress: number
  durationMs: number
  onSeek(fraction: number): void
}

const KEY_STEP_MS = 5_000

export function VoiceWaveform({ waveform, bars, progress, durationMs, onSeek }: VoiceWaveformProps) {
  const ref = useRef<HTMLDivElement>(null)
  const dragging = useRef(false)
  const values = resampleWaveform(waveform, bars)
  const playedBars = progress * values.length

  const fractionAt = (clientX: number) => {
    const rect = ref.current?.getBoundingClientRect()
    if (!rect || rect.width <= 0) return 0
    return Math.min(1, Math.max(0, (clientX - rect.left) / rect.width))
  }

  const onPointerDown = (event: PointerEvent<HTMLDivElement>) => {
    event.stopPropagation()
    dragging.current = true
    event.currentTarget.setPointerCapture?.(event.pointerId)
    onSeek(fractionAt(event.clientX))
  }
  const onPointerMove = (event: PointerEvent<HTMLDivElement>) => {
    if (dragging.current) onSeek(fractionAt(event.clientX))
  }
  const onPointerUp = (event: PointerEvent<HTMLDivElement>) => {
    dragging.current = false
    event.currentTarget.releasePointerCapture?.(event.pointerId)
  }
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (durationMs <= 0) return
    const step = KEY_STEP_MS / durationMs
    const next =
      event.key === 'ArrowRight' || event.key === 'ArrowUp'
        ? progress + step
        : event.key === 'ArrowLeft' || event.key === 'ArrowDown'
          ? progress - step
          : event.key === 'Home'
            ? 0
            : event.key === 'End'
              ? 1
              : null
    if (next === null) return
    event.preventDefault()
    onSeek(Math.min(1, Math.max(0, next)))
  }

  const seconds = Math.round(durationMs / 1000)
  const now = Math.round((progress * durationMs) / 1000)
  return (
    <div
      ref={ref}
      className="tg-voice__wave"
      role="slider"
      tabIndex={0}
      aria-label={t('w.voice.7a5c0f')}
      aria-valuemin={0}
      aria-valuemax={seconds}
      aria-valuenow={now}
      aria-valuetext={`${formatVoiceDuration(progress * durationMs)} / ${formatVoiceDuration(durationMs)}`}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={onPointerUp}
      onClick={(event) => event.stopPropagation()}
      onKeyDown={onKeyDown}
    >
      {values.map((value, index) => (
        <span
          key={index}
          className="tg-voice__bar"
          data-played={index < playedBars || undefined}
          style={{ blockSize: `${Math.round(12 + (value / WAVEFORM_MAX) * 88)}%` }}
        />
      ))}
    </div>
  )
}
