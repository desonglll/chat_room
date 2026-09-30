/**
 * TG-404 through the real chat session: «静默发送» puts `silent: true` on the `message`
 * frame (and only then); «定时发送» takes the draft + reply target as its payload and
 * `scheduled()` clears the draft and the reply bar like a send.
 */
import { describe, expect, test } from 'bun:test'
import { selectDraft } from '@tg/core'
import { FakeDraftServer, createDevice, settle } from './composerHarness'

const messageFrames = (frames: ReadonlyArray<Record<string, unknown>>) =>
  frames.filter((frame) => frame.type === 'message') as Array<{ content: string; silent?: boolean }>

describe('send options (TG-404)', () => {
  test('silent send marks the frame; a normal send does not', async () => {
    const device = createDevice(new FakeDraftServer())
    const chat = await device.open('chat-a')
    chat.controller.input('悄悄说')
    expect(chat.controller.submitSilent()).toBe('sent')
    chat.controller.input('正常说')
    expect(chat.controller.submit()).toBe('sent')
    const frames = messageFrames(chat.socket.sentFrames())
    expect(frames.map((frame) => [frame.content, frame.silent])).toEqual([
      ['悄悄说', true],
      ['正常说', undefined],
    ])
    chat.session.stop()
  })

  test('silent send is refused while editing', async () => {
    const device = createDevice(new FakeDraftServer())
    const chat = await device.open('chat-a')
    chat.controller.input('草稿')
    device.stores.composer.getState().dispatchMode('chat-a', { type: 'edit', messageId: 'm1', text: '旧' })
    expect(chat.controller.submitSilent()).toBe('empty')
    expect(messageFrames(chat.socket.sentFrames())).toHaveLength(0)
    chat.session.stop()
  })

  test('the schedule payload is the trimmed draft with its reply target, and scheduling clears both', async () => {
    const device = createDevice(new FakeDraftServer())
    const chat = await device.open('chat-a')
    expect(chat.controller.schedulePayload()).toBeNull()
    chat.controller.reply('m-7')
    chat.controller.input('  明早提醒  ')
    expect(chat.controller.schedulePayload()).toEqual({ content: '明早提醒', replyTo: 'm-7' })

    chat.controller.scheduled()
    await settle()
    const draft = selectDraft('chat-a')(device.stores.composer.getState())
    expect(draft?.text ?? '').toBe('')
    expect(draft?.replyToMessageId ?? null).toBeNull()
    expect(chat.controller.schedulePayload()).toBeNull()
    expect(messageFrames(chat.socket.sentFrames())).toHaveLength(0)
    chat.session.stop()
  })
})
