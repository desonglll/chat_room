/**
 * React glue for the composer's round video recorder (TG-402): one TG-401 `RecordController`
 * per chat, driving a `VideoNoteRecorder` instead of the voice one — `recording_video_note` /
 * `uploading_video` chat actions, an automatic send at 60 s, and the video upload.
 */
import { useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore } from 'react'
import type { ClientFrame } from '@tg/core'
import { authStore, createChatActionSender, selectToken } from '@tg/core'
import { browserClock, browserFetch } from '../../app/platform'
import { createRecordController, type RecordController, type RecordState } from '../voice/recordController'
import { RecorderError, unavailableFailure } from '../voice/voiceRecorder'
import { browserVideoEnv } from './browserVideoEnv'
import { sendVideoNote, VIDEO_NOTE_MAX_MS } from './videoNoteApi'
import { videoNoteErrorText } from './videoNoteErrors'
import { createVideoNoteRecorder, videoNoteFileName, type VideoNoteRecorder } from './videoRecorder'
import { isRegularVideo, videoUploadFromFile, type VideoUpload } from './videoFile'
import { activeTopicId } from '../forum/activeTopic'
import { uploadAttachment } from '../composer/uploadClient'
import { t } from '../../i18n/index'

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
  /** TG-1301: whether the camera can record live; otherwise the system camera's file. */
  live: boolean
  /** TG-1301: «sent as a regular video» after a file too large for a round one, else `null`. */
  notice: string | null
  dismissNotice(): void
}

export function useVideoNoteRecording(options: VideoNoteRecordingOptions): VideoNoteRecordingHandle {
  const latest = useRef(options)
  latest.current = options
  const { chatId } = options
  const [notice, setNotice] = useState<string | null>(null)

  const { controller, preview } = useMemo(() => {
    let current: VideoNoteRecorder | null = null
    const actions = createChatActionSender({
      clock: browserClock,
      send: (_chatId, frame) => latest.current.sendFrame(frame),
    })
    const created = createRecordController<VideoUpload>({
      chatId,
      actions,
      now: () => performance.now(),
      chatActions: { recording: 'recording_video_note', uploading: 'uploading_video' },
      maxMs: VIDEO_NOTE_MAX_MS,
      errorText: videoNoteErrorText,
      createRecorder: () => {
        const env = browserVideoEnv()
        if (!env) throw new RecorderError(unavailableFailure())
        current = createVideoNoteRecorder(env)
        return current
      },
      fromFile: (file) => videoUploadFromFile(file),
      upload: async (recording) => {
        const token = selectToken(authStore.getState())
        if (isRegularVideo(recording)) {
          const file = recording.regular
          await uploadAttachment(browserFetch, {
            chatId,
            token,
            file: { name: recording.fileName, size: file.size, type: file.type, slice: (a, b) => file.slice(a, b) },
            caption: '',
            replyTo: latest.current.replyTo,
            topicId: activeTopicId(chatId),
            onProgress: () => {},
          })
          setNotice(t('w.videoNote.756e8a'))
          return
        }
        await sendVideoNote(browserFetch, {
          chatId,
          token,
          video: recording.blob,
          fileName: videoNoteFileName(recording.container),
          durationMs: recording.durationMs,
          thumbnail: recording.thumbnail,
          replyTo: latest.current.replyTo,
          topicId: activeTopicId(chatId),
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
  const live = useMemo(() => browserVideoEnv() !== undefined, [])
  const dismissNotice = useCallback(() => setNotice(null), [])
  return { state, controller, preview, live, notice, dismissNotice }
}
