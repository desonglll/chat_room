/**
 * The draft policy of the chat wiring seam («打字的键盘赢», devlog frozen interface §4):
 * every remote draft state goes through draftSync.accept first and reaches the composer
 * only when no local save is pending. Harness in ./chatSessionHarness.
 */
import { describe, expect, test } from 'bun:test'
import type { ChatDraft } from '@tg/core'
import { selectDraft } from '@tg/core'
import { CHAT_ID, harness, ME, settle } from './chatSessionHarness'

describe('draft policy — the typing keyboard wins', () => {
  test('local edits debounce 1.5 s into one PUT; the PUT response stamps updatedAt', async () => {
    const { session, clock, stores, puts, online } = harness()
    await online()
    session.setDraftText('第一版')
    session.setDraftText('第二版')
    expect(puts).toHaveLength(0)
    clock.advance(1500)
    await settle()
    expect(puts).toEqual([{ chatId: CHAT_ID, text: '第二版' }])
    const draft = selectDraft(CHAT_ID)(stores.composer.getState())
    expect(draft.text).toBe('第二版')
    expect(draft.updatedAt).not.toBe('')
    session.stop()
  })

  test('a draft_updated frame mid-debounce does NOT clobber the composer; the local edit still commits', async () => {
    const { session, sockets, clock, stores, puts, online } = harness()
    await online()
    session.setDraftText('本地正在打的字')
    sockets[0]!.receive({
      type: 'draft_updated',
      user_id: ME,
      text: '另一台设备的草稿',
      reply_to_message_id: null,
      topic_id: null,
      updated_at: '2026-09-30T10:00:00Z',
    })
    expect(selectDraft(CHAT_ID)(stores.composer.getState()).text).toBe('本地正在打的字')
    clock.advance(1500)
    await settle()
    expect(puts).toEqual([{ chatId: CHAT_ID, text: '本地正在打的字' }])
    session.stop()
  })

  test('a draft_updated frame with no pending local edit applies (cross-device adopt)', async () => {
    const { session, sockets, stores, online } = harness()
    await online()
    sockets[0]!.receive({
      type: 'draft_updated',
      user_id: ME,
      text: '别处写的',
      reply_to_message_id: null,
      topic_id: null,
      updated_at: '2026-09-30T10:00:00Z',
    })
    expect(selectDraft(CHAT_ID)(stores.composer.getState()).text).toBe('别处写的')
    session.stop()
  })

  test('the stored draft loads at open unless the keyboard got there first', async () => {
    const stored: ChatDraft = {
      user_id: ME,
      text: '云端草稿',
      reply_to_message_id: null,
      topic_id: null,
      updated_at: '2026-09-30T10:00:00Z',
    }
    const idle = harness({ storedDraft: stored })
    await idle.online()
    expect(selectDraft(CHAT_ID)(idle.stores.composer.getState()).text).toBe('云端草稿')
    idle.session.stop()

    const racing = harness({ storedDraft: stored })
    racing.session.start()
    racing.session.setDraftText('抢先本地输入') // GET resolves after this
    await settle()
    expect(selectDraft(CHAT_ID)(racing.stores.composer.getState()).text).toBe('抢先本地输入')
    racing.session.stop()
  })

  test('send clears the composer and flushes an empty save only when a draft was stored', async () => {
    const { session, clock, stores, puts, online } = harness()
    await online()
    // Never-saved draft: sending must not PUT an empty clear.
    session.setDraftText('未保存就发送')
    session.sendMessage('未保存就发送')
    await settle()
    expect(puts).toHaveLength(0)
    // Saved draft: sending flushes the clear immediately.
    session.setDraftText('已保存的草稿')
    clock.advance(1500)
    await settle()
    expect(puts).toEqual([{ chatId: CHAT_ID, text: '已保存的草稿' }])
    session.sendMessage('已保存的草稿')
    await settle()
    expect(puts).toEqual([
      { chatId: CHAT_ID, text: '已保存的草稿' },
      { chatId: CHAT_ID, text: '' },
    ])
    expect(selectDraft(CHAT_ID)(stores.composer.getState()).text).toBe('')
    session.stop()
  })
})
