/**
 * TG-104 acceptance through the real chat session: draft survives a chat switch and
 * syncs to a second device (TG-008 via a fake server); reply → edit → cancel over the
 * store + socket; typing / cancel chat actions (TG-107 sender); forward on send.
 */
import { describe, expect, test } from 'bun:test'
import { selectDraft } from '@tg/core'
import { broadcastFrame } from '../../../../test/chatSessionHarness'
import { FakeClock, FakeDraftServer, ME, createDevice, settle } from './composerHarness'

/** Chat actions the composer itself sent (the session's legacy preview is excluded). */
const actionsOf = (frames: ReadonlyArray<{ type: string; action?: string }>) =>
  frames.filter((frame) => frame.type === 'typing').map((frame) => frame.action)

describe('draft round-trip (TG-008)', () => {
  test('a draft survives switching to another chat and back', async () => {
    const server = new FakeDraftServer()
    const device = createDevice(server)
    const first = await device.open('chat-a')
    first.controller.input('写到一半的话')
    first.session.stop() // switching away flushes the pending save
    await settle()
    expect(server.drafts.get('chat-a')?.text).toBe('写到一半的话')

    const second = await device.open('chat-b')
    expect(second.controller.text()).toBe('')
    second.session.stop()

    // Even a fresh device state (reload) gets it back from the server on open.
    const reloaded = createDevice(server)
    const back = await reloaded.open('chat-a')
    expect(back.controller.text()).toBe('写到一半的话')
    back.session.stop()
  })

  test('typing on one device shows up in the other device’s composer, reply target included', async () => {
    const server = new FakeDraftServer()
    const clock = new FakeClock()
    const phone = createDevice(server, clock)
    const laptop = createDevice(server, clock)
    const onPhone = await phone.open('chat-a')
    const onLaptop = await laptop.open('chat-a')

    onPhone.controller.reply('m-42')
    onPhone.controller.input('手机上写的')
    clock.advance(1500)
    await settle()

    expect(selectDraft('chat-a')(laptop.stores.composer.getState())).toMatchObject({
      text: '手机上写的',
      replyToMessageId: 'm-42',
    })
    expect(onLaptop.controller.text()).toBe('手机上写的')

    // Sending clears the cloud draft everywhere.
    expect(onPhone.controller.submit()).toBe('sent')
    await settle()
    expect(server.drafts.has('chat-a')).toBe(false)
    expect(onLaptop.controller.text()).toBe('')
    onPhone.session.stop()
    onLaptop.session.stop()
  })
})

describe('bars and submit', () => {
  test('reply → edit → cancel: the edit never touches the draft; saving sends one edit frame', async () => {
    const device = createDevice(new FakeDraftServer())
    const chat = await device.open('chat-a')
    chat.socket.receive(broadcastFrame('m1', '2026-10-01T09:00:00Z', '原来的话', { sender_id: ME, sender: 'me' }))
    chat.controller.input('草稿')
    chat.controller.reply('m1')

    expect(chat.controller.edit('m1')).toBe(true)
    expect(chat.controller.text()).toBe('原来的话')
    chat.controller.input('改过的话')
    expect(selectDraft('chat-a')(device.stores.composer.getState()).text).toBe('草稿')

    expect(chat.controller.cancel()).toBe(true)
    expect(chat.controller.text()).toBe('草稿')
    expect(device.stores.composer.getState().drafts['chat-a']?.replyToMessageId).toBe('m1')

    chat.controller.edit('m1')
    chat.controller.input('改过的话')
    expect(chat.controller.submit()).toBe('edited')
    const edits = chat.socket.sentFrames().filter((frame) => frame.type === 'edit')
    expect(edits).toEqual([{ type: 'edit', message_id: 'm1', content: '改过的话' }])
    expect(chat.controller.text()).toBe('草稿')

    // An unchanged edit closes without sending anything.
    chat.controller.edit('m1')
    expect(chat.controller.submit()).toBe('unchanged')
    expect(chat.socket.sentFrames().filter((frame) => frame.type === 'edit')).toHaveLength(1)

    expect(chat.controller.edit('missing')).toBe(false)
    chat.session.stop()
  })

  test('sending a reply carries reply_to and consumes the reply bar', async () => {
    const device = createDevice(new FakeDraftServer())
    const chat = await device.open('chat-a')
    chat.controller.reply('m7')
    chat.controller.input('回你')
    expect(chat.controller.submit()).toBe('sent')
    const message = chat.socket.sentFrames().find((frame) => frame.type === 'message')
    expect(message).toMatchObject({ content: '回你', reply_to: 'm7' })
    expect(device.stores.composer.getState().drafts['chat-a']?.replyToMessageId ?? null).toBeNull()
    expect(chat.controller.submit()).toBe('empty')
    chat.session.stop()
  })

  test('forward: comment first, then the forwarded ids; an empty comment still forwards', async () => {
    const device = createDevice(new FakeDraftServer())
    const chat = await device.open('chat-a')
    chat.controller.forward(['x1', 'x2'], 'chat-z')
    chat.controller.input('看看这个')
    expect(chat.controller.submit()).toBe('sent')
    expect(device.forwards).toEqual([{ ids: ['x1', 'x2'], target: 'chat-a' }])
    chat.controller.forward(['x3'], 'chat-z')
    expect(chat.controller.submit()).toBe('forwarded')
    expect(device.forwards).toHaveLength(2)
    expect(device.stores.composer.getState().forwarding['chat-a']).toBeUndefined()
    chat.session.stop()
  })
})

describe('chat actions (TG-107 sender)', () => {
  test('typing while typing, cancel when cleared, upload actions around uploads', async () => {
    const device = createDevice(new FakeDraftServer())
    const chat = await device.open('chat-a')
    chat.controller.input('h')
    chat.controller.input('he')
    chat.controller.input('')
    chat.controller.uploadStarted('photo')
    chat.controller.uploadStarted('file')
    chat.controller.uploadFinished()
    // 'he' within the resend window costs no second frame.
    expect(actionsOf(chat.framesViaComposer)).toEqual([
      'typing',
      'cancel',
      'uploading_photo',
      'uploading_document',
      'cancel',
    ])
    chat.session.stop()
  })

  test('sending cancels an announced typing action', async () => {
    const device = createDevice(new FakeDraftServer())
    const chat = await device.open('chat-a')
    chat.controller.input('发出去')
    chat.controller.submit()
    expect(actionsOf(chat.framesViaComposer)).toEqual(['typing', 'cancel'])
    chat.session.stop()
  })
})
