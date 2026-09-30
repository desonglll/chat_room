/**
 * React glue for the composer's round video recorder (TG-402): one TG-401 `RecordController`
 * per chat, driving a `VideoNoteRecorder` instead of the voice one — `recording_video_note` /
 * `uploading_video` chat actions, an automatic send at 60 s, and the video upload.
 */
import { useEffect, useMemo, useRef, useSyncExternalStore } from 'react'
import type { ClientFrame } from '@tg/core'
import { authStore, createChatActionSender, selectToken } from '@tg/core'
import { browserClock, browserFetch } from '../../app/platform'
import { createRecordController, type RecordController, type RecordState } from '../voice/recordController'
import { RecorderError } from '../voice/voiceRecorder'
import { browserVideoEnv } from './browserVideoEnv'
import { sendVideoNote, VIDEO_NOTE_MAX_MS } from './videoNoteApi'
import { videoNoteErrorText } from './videoNoteErrors'
import { createVideoNoteRecorder, videoNoteFileName, type VideoNoteRecorder } from './videoRecorder'

export interface VideoNoteRecordingOptions {
  chatId: string
  replyTo: string | null
  sendFrame(frame: ClientFrame): boolean
  onSent?(): void
}

export interface VideoNoteRecordingHandle {
  state: RecordState
  controller: RecordController
  /** The open camera's viewfinder element (a canvas), or `null`. */
  preview(): unknown
}

export function useVideoNoteRecording(options: VideoNoteRecordingOptions): VideoNoteRecordingHandle {
  const latest = useRef(options)
  latest.current = options
  const { chatId } = options

  const { controller, preview } = useMemo(() => {
    let current: VideoNoteRecorder | null = null
    const actions = createChatActionSender({
      clock: browserClock,
      send: (_chatId, frame) => latest.current.sendFrame(frame),
    })
    const created = createRecordController({
      chatId,
      actions,
      now: () => performance.now(),
      chatActions: { recording: 'recording_video_note', uploading: 'uploading_video' },
      maxMs: VIDEO_NOTE_MAX_MS,
      errorText: videoNoteErrorText,
      createRecorder: () => {
        const env = browserVideoEnv()
        if (!env) throw new RecorderError('unsupported')
        current = createVideoNoteRecorder(env)
        return current
      },
      upload: async (recording) => {
        await sendVideoNote(browserFetch, {
          chatId,
          token: selectToken(authStore.getState()),
          video: recording.blob,
          fileName: videoNoteFileName(recording.container),
          durationMs: recording.durationMs,
          thumbnail: recording.thumbnail,
          replyTo: latest.current.replyTo,
        })
      },
      onSent: () => latest.current.onSent?.(),
    })
    const dispose = created.dispose
    created.dispose = () => {
      dispose()
      actions.dispose()
    }
    return { controller: created, preview: () => current?.preview() ?? null }
  }, [chatId])

  useEffect(() => () => controller.dispose(), [controller])
  const state = useSyncExternalStore(controller.subscribe, controller.getState, controller.getState)
  return { state, controller, preview }
}
