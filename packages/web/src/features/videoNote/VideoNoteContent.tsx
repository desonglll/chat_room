/**
 * The round video message bubble (TG-402), registered with TG-103's content registry by
 * `register.ts`. Telegram's behaviour:
 *
 * - in the viewport (≥ half visible) it plays muted and looping; off-screen it pauses;
 *   with `prefers-reduced-motion` it never autoplays (thumbnail + play badge instead);
 * - a tap plays it from the start with sound, enlarged, with a progress ring; a second tap
 *   pauses/resumes; at the end it shrinks back into the muted preview;
 * - one sounds at a time (`videoNotePlayback`), and it stops a playing voice message;
 * - someone else's unwatched note loses its dot on the first sounding play (TG-401's
 *   listened mechanism: `voiceStore` + `voice_listened`).
 */
import { useEffect, useRef, useState } from 'react'
import { useStore } from 'zustand/react'
import { authStore, selectToken } from '@tg/core'
import { apiClient } from '../../app/client'
import type { MessageContentProps } from '../message'
import { appVoicePlayer } from '../voice/appVoicePlayer'
import { voiceStore } from '../voice/voiceStore'
import { useInViewport, usePrefersReducedMotion } from './useInViewport'
import { VideoNoteBody } from './VideoNoteBody'
import { reportVideoNoteWatched, videoNotePlayback } from './videoNotePlayback'

/** Idle (not sounding) playback: a muted loop only while visible and motion is welcome. */
export function idlePreviewPlays(inView: boolean, reducedMotion: boolean): boolean {
  return inView && !reducedMotion
}

export function VideoNoteContent({ message, ctx }: MessageContentProps) {
  const note = message.video_note
  const attachment = message.attachment
  const root = useRef<HTMLDivElement>(null)
  const video = useRef<HTMLVideoElement>(null)
  const id = message.message_id
  const active = useStore(videoNotePlayback, (state) => state.active === id)
  const listenedLive = useStore(voiceStore, (state) => state.listened[id] === true)
  const inView = useInViewport(root)
  const reducedMotion = usePrefersReducedMotion()
  const [paused, setPaused] = useState(false)
  const [previewing, setPreviewing] = useState(false)
  const [positionMs, setPositionMs] = useState(0)

  // Muted preview while idle.
  useEffect(() => {
    const element = video.current
    if (!element || active) return
    element.muted = true
    element.loop = true
    if (idlePreviewPlays(inView, reducedMotion)) {
      element.play().then(
        () => setPreviewing(true),
        () => setPreviewing(false),
      )
    } else {
      element.pause()
      setPreviewing(false)
    }
  }, [active, inView, reducedMotion])

  // Sounding playback from the start while active; progress on animation frames.
  useEffect(() => {
    const element = video.current
    if (!element || !active) return
    let frame = 0
    const tick = () => {
      setPositionMs(element.currentTime * 1000)
      frame = window.requestAnimationFrame(tick)
    }
    const onEnded = () => videoNotePlayback.getState().deactivate(id)
    element.addEventListener('ended', onEnded)
    element.loop = false
    element.muted = false
    element.currentTime = 0
    setPaused(false)
    element.play().catch(() => videoNotePlayback.getState().deactivate(id))
    frame = window.requestAnimationFrame(tick)
    return () => {
      window.cancelAnimationFrame(frame)
      element.removeEventListener('ended', onEnded)
      element.muted = true
      element.loop = true
      setPositionMs(0)
      setPaused(false)
    }
  }, [active, id])

  if (!note || !attachment) return null

  const onActivate = () => {
    const element = video.current
    if (active) {
      if (!element) return
      if (element.paused) {
        void element.play()
        setPaused(false)
      } else {
        element.pause()
        setPaused(true)
      }
      return
    }
    appVoicePlayer().stop()
    videoNotePlayback.getState().activate(id)
    const session = authStore.getState().session
    if (session && message.sender_id !== session.user.id && !note.listened) {
      reportVideoNoteWatched(id, { client: apiClient, token: selectToken(authStore.getState()) })
    }
  }

  return (
    <div ref={root} className="tg-video-note-host">
      <VideoNoteBody
        url={attachment.download_url}
        durationMs={note.duration_ms}
        thumbnail={note.thumbnail}
        outgoing={ctx.isOutgoing}
        active={active}
        moving={active ? !paused : previewing}
        positionMs={positionMs}
        listened={note.listened || listenedLive}
        videoRef={video}
        onActivate={onActivate}
      />
    </div>
  )
}
