// TG-402: TG-401's record controller driving the video recorder — chat actions, the 60 s cap,
// the delayed press used by the mic ↔ camera toggle, and the error copy.
import { describe, expect, test } from 'bun:test'
import type { ChatActionSender, TypingAction } from '@tg/core'
import { ApiError, VOICE_RESTRICTED } from '@tg/core'
import { createRecordController } from '../../voice/recordController'
import { TAP_MS } from '../../voice/recordGesture'
import { RecorderError } from '../../voice/voiceRecorder'
import { VIDEO_NOTE_MAX_MS } from '../videoNoteApi'
import { videoNoteErrorText } from '../videoNoteErrors'
import { createVideoNoteRecorder, type VideoNoteRecording } from '../videoRecorder'
import { fakeVideoEnv } from './fakes'

const settle = async () => {
  for (let i = 0; i < 12; i += 1) await Promise.resolve()
}

function setup(upload?: () => Promise<void>) {
  const fake = fakeVideoEnv()
  const actions: TypingAction[] = []
  const uploads: VideoNoteRecording[] = []
  const sender: ChatActionSender = { sendChatAction: (_chat, action) => actions.push(action), dispose: () => {} }
  const controller = createRecordController<VideoNoteRecording>({
    chatId: 'c1',
    actions: sender,
    now: () => fake.env.now(),
    chatActions: { recording: 'recording_video_note', uploading: 'uploading_video' },
    maxMs: VIDEO_NOTE_MAX_MS,
    errorText: videoNoteErrorText,
    createRecorder: () => createVideoNoteRecorder(fake.env),
    upload: async (recording) => {
      uploads.push(recording)
      await upload?.()
    },
  })
  return { fake, controller, actions, uploads }
}

describe('video note record controller', () => {
  test('hold → recording_video_note; release → uploading_video, upload, cancel', async () => {
    const { fake, controller, actions, uploads } = setup()
    controller.press(500, 700)
    await settle()
    expect(actions).toEqual(['recording_video_note'])
    fake.advance(3_000)
    controller.release()
    await settle()
    expect(actions).toEqual(['recording_video_note', 'uploading_video', 'cancel'])
    expect(uploads).toHaveLength(1)
    expect(uploads[0]!.thumbnail).not.toBeNull()
  })

  test('the minute is the cap: it sends by itself at 60 s', async () => {
    const { fake, controller, uploads } = setup()
    controller.press(500, 700)
    await settle()
    fake.advance(VIDEO_NOTE_MAX_MS - 100)
    expect(controller.getState().phase).toBe('recording')
    fake.advance(200)
    expect(controller.getState().phase).toBe('sending')
    await settle()
    expect(uploads).toHaveLength(1)
    expect(uploads[0]!.durationMs).toBeGreaterThanOrEqual(VIDEO_NOTE_MAX_MS)
    expect(uploads[0]!.durationMs).toBeLessThan(VIDEO_NOTE_MAX_MS + 100)
    expect(controller.getState().phase).toBe('idle')
  })

  test('a delayed press (the toggle button held past the tap window) releases into a send', async () => {
    const { fake, controller, uploads } = setup()
    controller.press(500, 700, TAP_MS)
    await settle()
    fake.advance(1_000)
    controller.release()
    await settle()
    expect(uploads).toHaveLength(1)
  })

  test('a quick release of a normal press still locks (hands-free), as for voice', async () => {
    const { controller } = setup()
    controller.press(500, 700)
    await settle()
    controller.release()
    expect(controller.getState().gesture.phase).toBe('locked')
    controller.cancel()
  })

  test('errors speak about the camera and video messages', async () => {
    expect(videoNoteErrorText(new RecorderError('permission'))).toContain('摄像头')
    expect(videoNoteErrorText(new RecorderError('unsupported'))).toContain('视频消息')
    expect(videoNoteErrorText(new ApiError(403, '/x', VOICE_RESTRICTED))).toBe('对方设置了不接收你的语音和视频消息')
    expect(videoNoteErrorText(new ApiError(413, '/x', 'too_large'))).toBe('视频消息过大')
    const { fake, controller } = setup(async () => {
      throw new ApiError(500, '/x', 'internal')
    })
    controller.press(1, 1)
    await settle()
    fake.advance(1_000)
    controller.release()
    await settle()
    expect(controller.getState().error).toBe('视频消息发送失败，请重试')
  })
})
