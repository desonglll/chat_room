/**
 * TG-1206 through the real chat session: a custom emoji picked in the panel becomes an
 * entity on the `message` frame and on the optimistic row; text typed around it moves the
 * range; deleting it drops the range; a send clears the tracked entities.
 */
import { describe, expect, test } from 'bun:test'
import type { BroadcastMessage } from '@tg/core'
import { selectTimeline } from '@tg/core'
import { FakeDraftServer, createDevice } from './composerHarness'

type Frame = {
  content: string
  entities?: Array<{ type: string; offset: number; length: number; custom_emoji_id?: string }>
}
const messageFrames = (frames: ReadonlyArray<Record<string, unknown>>) =>
  frames.filter((frame) => frame.type === 'message') as Frame[]
const PARTY = { id: 'ce-1', emoji: '🎉' }

describe('custom emoji send path (TG-1206)', () => {
  test('a picked custom emoji is sent as an entity, shifted by typed text and the trim', async () => {
    const device = createDevice(new FakeDraftServer())
    const chat = await device.open('chat-a')
    chat.controller.input('  好 ')
    const inserted = chat.controller.insertCustomEmoji({ start: 4, end: 4 }, PARTY)
    expect(inserted).toEqual({ text: '  好 🎉', caret: 6 })
    // Typing in front of the emoji moves its range.
    chat.controller.input('  很好 🎉')
    expect(chat.controller.submit()).toBe('sent')

    const [frame] = messageFrames(chat.socket.sentFrames())
    expect(frame!.content).toBe('很好 🎉')
    expect(frame!.entities).toEqual([{ type: 'custom_emoji', offset: 3, length: 2, custom_emoji_id: 'ce-1' }])
    const row = selectTimeline('chat-a')(device.stores.message.getState()).messages.at(-1) as BroadcastMessage
    expect(row.entities).toEqual(frame!.entities!)
    chat.session.stop()
  })

  test('deleting the emoji drops its entity; the next message starts clean', async () => {
    const device = createDevice(new FakeDraftServer())
    const chat = await device.open('chat-a')
    chat.controller.insertCustomEmoji({ start: 0, end: 0 }, PARTY)
    chat.controller.input('')
    chat.controller.input('🎉 手打的')
    expect(chat.controller.submit()).toBe('sent')
    chat.controller.insertCustomEmoji({ start: 0, end: 0 }, PARTY)
    expect(chat.controller.submit()).toBe('sent')
    chat.controller.input('普通')
    expect(chat.controller.submit()).toBe('sent')

    const frames = messageFrames(chat.socket.sentFrames())
    expect(frames.map((frame) => [frame.content, frame.entities?.length ?? 0])).toEqual([
      ['🎉 手打的', 0],
      ['🎉', 1],
      ['普通', 0],
    ])
    chat.session.stop()
  })

  test('while editing the pick is refused, so the host inserts the plain character', async () => {
    const device = createDevice(new FakeDraftServer())
    const chat = await device.open('chat-a')
    device.stores.composer.getState().dispatchMode('chat-a', { type: 'edit', messageId: 'm1', text: '旧' })
    expect(chat.controller.insertCustomEmoji({ start: 0, end: 0 }, PARTY)).toBeNull()
    chat.session.stop()
  })
})
