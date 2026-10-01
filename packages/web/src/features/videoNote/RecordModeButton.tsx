/**
 * The composer's record button with Telegram's two modes (TG-402): a tap toggles microphone
 * ↔ camera, a press-and-hold records in the current mode — slide left to cancel, slide up to
 * lock, release to send, exactly TG-401's voice gesture. Enter/Space records hands-free in the
 * current mode; Shift+Enter/Space toggles the mode. The last mode is remembered.
 *
 * Both recorders are TG-401's `RecordController`; only the camera one gets the round
 * viewfinder overlay and loses the live waveform.
 *
 * TG-1301: when the page cannot record live (plain http on a LAN address), each mode opens the
 * system recorder / camera instead (`CaptureRecordButton`); the tap still toggles the mode.
 */
import { useState, type ReactNode } from 'react'
import type { ClientFrame } from '@tg/core'
import { browserStorage } from '../../app/platform'
import { useVoiceRecording } from '../voice/useVoiceRecording'
import { VoiceRecordView } from '../voice/VoiceRecordButton'
import { CaptureRecordButton } from '../voice/CaptureRecordButton'
import { CameraGlyph } from './glyphs'
import { useVideoNoteRecording } from './useVideoNoteRecording'
import { VideoNoteViewfinder } from './VideoNoteViewfinder'
import { t } from '../../i18n/index'

export type RecordMode = 'voice' | 'video'

const MODE_KEY = 'tg.recordMode.v1'

export interface RecordModeButtonProps {
  chatId: string
  replyTo: string | null
  canSend: boolean
  sendFrame(frame: ClientFrame): boolean
  onSent?(): void
  micGlyph: ReactNode
  sendGlyph: ReactNode
}

export function RecordModeButton(props: RecordModeButtonProps) {
  const [mode, setMode] = useState<RecordMode>(() => (browserStorage.getItem(MODE_KEY) === 'video' ? 'video' : 'voice'))
  const voice = useVoiceRecording(props)
  const video = useVideoNoteRecording(props)
  const toggle = () => {
    const next = mode === 'voice' ? 'video' : 'voice'
    browserStorage.setItem(MODE_KEY, next)
    setMode(next)
  }
  if (mode === 'voice' && !voice.live) {
    return (
      <CaptureRecordButton
        state={voice.state}
        controller={voice.controller}
        canSend={props.canSend}
        micGlyph={props.micGlyph}
        accept="audio/*"
        capture
        hint={t('w.voice.4a05b0')}
        onTap={toggle}
        idleLabel={t('w.videoNote.9ccb9e')}
      />
    )
  }
  if (mode === 'video' && !video.live) {
    return (
      <CaptureRecordButton
        state={video.state}
        controller={video.controller}
        canSend={props.canSend}
        micGlyph={<CameraGlyph />}
        accept="video/*"
        capture="user"
        hint={t('w.videoNote.92b4c5')}
        onTap={toggle}
        kind="video_note"
        idleLabel={t('w.videoNote.e87e92')}
        notice={video.notice}
        onNoticeShown={video.dismissNotice}
      />
    )
  }
  if (mode === 'voice') {
    return (
      <VoiceRecordView
        {...props}
        state={voice.state}
        controller={voice.controller}
        onTap={toggle}
        idleLabel={t('w.videoNote.9ccb9e')}
      />
    )
  }
  const recording = video.state.phase === 'starting' || video.state.phase === 'recording'
  return (
    <VoiceRecordView
      {...props}
      micGlyph={<CameraGlyph />}
      state={video.state}
      controller={video.controller}
      onTap={toggle}
      kind="video_note"
      idleLabel={t('w.videoNote.e87e92')}
      sendLabel={t('w.videoNote.f2368c')}
      waveform={false}
      overlay={
        recording ? (
          <VideoNoteViewfinder
            preview={video.state.phase === 'recording' ? video.preview() : null}
            elapsedMs={video.state.elapsedMs}
          />
        ) : null
      }
    />
  )
}
