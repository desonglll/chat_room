/**
 * React glue for the composer's recorder: one `RecordController` per chat, wired to the
 * browser recorder, the voice upload and TG-107's chat action sender.
 */
import { useEffect, useMemo, useRef, useSyncExternalStore } from 'react'
import type { ClientFrame } from '@tg/core'
import { authStore, createChatActionSender, selectToken, sendVoiceMessage } from '@tg/core'
import { browserClock, browserFetch } from '../../app/platform'
import { browserRecorderEnv } from './browserRecorderEnv'
import { createRecordController, type RecordController, type RecordState } from './recordController'
import { voiceFileName } from './recorderFormat'
import { createVoiceRecorder, RecorderError } from './voiceRecorder'

export interface VoiceRecordingOptions {
  chatId: string
  replyTo: string | null
  sendFrame(frame: ClientFrame): boolean
  onSent?(): void
}

export function useVoiceRecording(options: VoiceRecordingOptions): {
  state: RecordState
  controller: RecordController
} {
  const latest = useRef(options)
  latest.current = options
  const { chatId } = options

  const controller = useMemo(() => {
    const actions = createChatActionSender({
      clock: browserClock,
      send: (_chatId, frame) => latest.current.sendFrame(frame),
    })
    const created = createRecordController({
      chatId,
      actions,
      now: () => performance.now(),
      createRecorder: () => {
        const env = browserRecorderEnv()
        if (!env) throw new RecorderError('unsupported')
        return createVoiceRecorder(env)
      },
      upload: async (recording) => {
        await sendVoiceMessage(browserFetch, {
          chatId,
          token: selectToken(authStore.getState()),
          audio: recording.blob,
          fileName: voiceFileName(recording.container),
          durationMs: recording.durationMs,
          waveform: recording.waveform,
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
    return created
  }, [chatId])

  useEffect(() => () => controller.dispose(), [controller])
  const state = useSyncExternalStore(controller.subscribe, controller.getState, controller.getState)
  return { state, controller }
}
