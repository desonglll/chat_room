/**
 * The stateless face of a round video message (TG-402): the circle with its video, the
 * thumbnail as poster, the progress ring while it plays with sound, the play badge when it
 * is not moving, and the duration pill with the muted speaker and the unwatched dot.
 * `VideoNoteContent` owns the element and the playback rules.
 */
import type { Ref } from 'react'
import { PlayGlyph } from '../voice/glyphs'
import { formatVoiceDuration } from '../voice/waveform'
import { SpeakerOffGlyph } from './glyphs'
import { ProgressRing } from './ProgressRing'
import { t } from '../../i18n/index'

export interface VideoNoteBodyProps {
  url: string
  durationMs: number
  /** Base64 JPEG or null. */
  thumbnail: string | null
  outgoing: boolean
  /** Playing with sound, enlarged. */
  active: boolean
  /** Whether the picture is moving right now (muted preview or active playback). */
  moving: boolean
  positionMs: number
  listened: boolean
  videoRef?: Ref<HTMLVideoElement>
  onActivate(): void
}

export function VideoNoteBody(props: VideoNoteBodyProps) {
  const { url, durationMs, thumbnail, outgoing, active, moving, positionMs, listened } = props
  const progress = durationMs > 0 ? positionMs / durationMs : 0
  const shown = active ? Math.max(0, durationMs - positionMs) : durationMs
  const label = active ? (moving ? t('w.videoNote.5d4e88') : t('w.videoNote.930e14')) : t('w.videoNote.0d3a49')
  return (
    <div
      className="tg-video-note"
      data-active={active || undefined}
      data-outgoing={outgoing || undefined}
      data-moving={moving || undefined}
    >
      <button
        type="button"
        className="tg-video-note__disc"
        aria-label={`${label}，${formatVoiceDuration(durationMs)}`}
        onClick={(event) => {
          event.stopPropagation()
          props.onActivate()
        }}
      >
        <video
          ref={props.videoRef}
          className="tg-video-note__video"
          src={url}
          poster={thumbnail ? `data:image/jpeg;base64,${thumbnail}` : undefined}
          muted
          loop
          playsInline
          preload="metadata"
          disablePictureInPicture
          aria-hidden="true"
          tabIndex={-1}
        />
        {active ? <ProgressRing className="tg-video-note__ring" progress={progress} /> : null}
        {moving ? null : (
          <span className="tg-video-note__play" aria-hidden="true">
            <PlayGlyph />
          </span>
        )}
      </button>
      <span className="tg-video-note__badge">
        <span className="tg-video-note__time">{formatVoiceDuration(shown)}</span>
        {active ? null : <SpeakerOffGlyph />}
        {listened ? null : (
          <span className="tg-video-note__unwatched" role="img" aria-label={t('w.videoNote.ecea6e')} />
        )}
      </span>
    </div>
  )
}
