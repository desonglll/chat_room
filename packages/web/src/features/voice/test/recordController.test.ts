// TG-401: the composer's recording flow — gestures, TG-107 chat actions, upload, errors.
import { describe, expect, test } from 'bun:test'
import type { ChatActionSender, TypingAction } from '@tg/core'
import { ApiError, VOICE_RESTRICTED } from '@tg/core'
import { createRecordController, MIN_VOICE_MS, type RecordControllerDeps } from '../recordController'
import { CANCEL_DISTANCE, LOCK_DISTANCE, TAP_MS } from '../recordGesture'
import { createVoiceRecorder, type VoiceRecording } from '../voiceRecorder'
import { fakeEnv } from './fakes'

const settle = async () => {
  for (let i = 0; i < 12; i += 1) await Promise.resolve()
}

function setup(options: { upload?: (recording: VoiceRecording) => Promise<void>; supported?: string[] } = {}) {
  const fake = fakeEnv({ supported: options.supported })
  const actions: TypingAction[] = []
  const uploads: VoiceRecording[] = []
  let sent = 0
  const sender: ChatActionSender = {
    sendChatAction: (_chat, action) => actions.push(action),
    dispose: () => {},
  }
  const deps: RecordControllerDeps = {
    chatId: 'c1',
    actions: sender,
    now: () => fake.env.now(),
    createRecorder: () => createVoiceRecorder(fake.env),
    upload: async (recording) => {
      uploads.push(recording)
      await options.upload?.(recording)
    },
    onSent: () => {
      sent += 1
    },
  }
  const controller = createRecordController(deps)
  return { fake, controller, actions, uploads, sent: () => sent }
}

describe('record controller', () => {
  test('hold → recording_voice; release → uploading_voice, upload, cancel', async () => {
    const { fake, controller, actions, uploads, sent } = setup()
    controller.press(500, 700)
    expect(controller.getState().phase).toBe('starting')
    await settle()
    expect(controller.getState().phase).toBe('recording')
    expect(actions).toEqual(['recording_voice'])
    fake.advance(2_000, 0.6)
    expect(controller.getState().elapsedMs).toBe(2_000)
    expect(controller.getState().levels.length).toBeGreaterThan(0)
    controller.release()
    expect(controller.getState().phase).toBe('sending')
    await settle()
    expect(actions).toEqual(['recording_voice', 'uploading_voice', 'cancel'])
    expect(uploads).toHaveLength(1)
    expect(uploads[0]!.durationMs).toBe(2_000)
    expect(uploads[0]!.waveform).toHaveLength(100)
    expect(sent()).toBe(1)
    expect(controller.getState().phase).toBe('idle')
  })

  test('slide left past the threshold cancels: no upload, action cancelled', async () => {
    const { fake, controller, actions, uploads } = setup()
    controller.press(500, 700)
    await settle()
    fake.advance(1_000, 0.3)
    controller.move(500 - CANCEL_DISTANCE / 2, 700)
    expect(controller.getState().gesture.cancelProgress).toBeCloseTo(0.5)
    controller.move(500 - CANCEL_DISTANCE, 700)
    await settle()
    expect(controller.getState().phase).toBe('idle')
    expect(actions).toEqual(['recording_voice', 'cancel'])
    expect(uploads).toHaveLength(0)
    expect(fake.recorders[0]!.stopped).toBeTrue()
  })

  test('slide up locks: releasing keeps recording, «发送» sends', async () => {
    const { fake, controller, uploads } = setup()
    controller.press(500, 700)
    await settle()
    controller.move(500, 700 - LOCK_DISTANCE)
    expect(controller.getState().gesture.phase).toBe('locked')
    controller.release()
    fake.advance(3_000, 0.5)
    expect(controller.getState().phase).toBe('recording')
    controller.send()
    await settle()
    expect(uploads).toHaveLength(1)
  })

  test('a tap records hands-free; Escape-style cancel discards', async () => {
    const { fake, controller, actions, uploads } = setup()
    controller.press(500, 700)
    fake.advance(TAP_MS - 60, 0.1)
    controller.release()
    await settle()
    expect(controller.getState().gesture.phase).toBe('locked')
    expect(controller.getState().phase).toBe('recording')
    controller.cancel()
    expect(controller.getState().phase).toBe('idle')
    expect(actions.at(-1)).toBe('cancel')
    expect(uploads).toHaveLength(0)
  })

  test(`recordings shorter than ${MIN_VOICE_MS} ms are dropped`, async () => {
    const { fake, controller, uploads } = setup()
    controller.press(500, 700)
    await settle()
    controller.move(500, 700 - LOCK_DISTANCE)
    fake.advance(MIN_VOICE_MS - 100, 0.5)
    controller.send()
    await settle()
    expect(uploads).toHaveLength(0)
    expect(controller.getState().phase).toBe('idle')
  })

  test('a refused upload surfaces the privacy message and resets', async () => {
    const { fake, controller, actions } = setup({
      upload: async () => {
        throw new ApiError(403, '/api/chats/c1/voice', VOICE_RESTRICTED)
      },
    })
    controller.press(500, 700)
    await settle()
    fake.advance(1_500, 0.5)
    controller.release()
    await settle()
    expect(controller.getState()).toMatchObject({ phase: 'idle', error: '对方设置了不接收你的语音消息' })
    expect(actions.at(-1)).toBe('cancel')
    controller.dismissError()
    expect(controller.getState().error).toBeNull()
  })

  test('a browser that cannot record says so without touching the microphone', () => {
    const { controller, fake } = setup({ supported: [] })
    controller.press(500, 700)
    expect(controller.getState()).toMatchObject({ phase: 'idle', error: '此浏览器不支持录制语音' })
    expect(fake.recorders).toHaveLength(0)
  })
})
