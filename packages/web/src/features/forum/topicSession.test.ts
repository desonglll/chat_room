// TG-204: the chat session in forum modes, under the chatSession fakes — topic frame
// filtering, the topic history merge before historyReady, topic reads, topic sends.
import { describe, expect, test } from 'bun:test'
import type { ApiClient, DraftsApi, StoredMessage, TopicReadResult, TopicsApi } from '@tg/core'
import {
  createChatListStore,
  createComposerStore,
  createMessageStore,
  createPresenceStore,
  selectTimeline,
} from '@tg/core'
import {
  AUTH_OK,
  broadcastFrame,
  CHAT_ID,
  FakeClock,
  FakeSocket,
  ME,
  settle,
  storedMessage,
} from '../../../test/chatSessionHarness'
import { createChatSession } from '../chat/chatSession'
import type { TopicRef } from './topicSessionMode'
import { createTopicListApi, createTopicSessionMode } from './topicSessionMode'

function fakeTopicsApi(page: StoredMessage[]) {
  const reads: string[] = []
  const pages: Array<{ topicId: string; before?: string | undefined }> = []
  let release: () => void = () => {}
  const gate = new Promise<void>((resolve) => (release = resolve))
  const api = {
    messages: async (_chatId: string, topicId: string, options: { before?: string } = {}) => {
      pages.push({ topicId, before: options.before })
      await gate
      return page
    },
    context: async () => page,
    read: async (_chatId: string, topicId: string, messageId: string): Promise<TopicReadResult> => {
      reads.push(`${topicId}:${messageId}`)
      return { topic_id: topicId, unread_count: 0, chat_read_advanced: true }
    },
  } as unknown as TopicsApi
  return { api, reads, pages, release }
}

function topicHarness(options: { topic?: TopicRef; page?: StoredMessage[]; readCursor?: boolean } = {}) {
  const socket = new FakeSocket()
  const topicsApi = fakeTopicsApi(options.page ?? [])
  const readResults: TopicReadResult[] = []
  const stores = {
    message: createMessageStore(),
    presence: createPresenceStore(),
    composer: createComposerStore(),
    chatList: createChatListStore(),
  }
  const draftsApi = { get: async () => null, put: async () => ({}) } as unknown as DraftsApi
  const session = createChatSession({
    chatId: CHAT_ID,
    token: 't',
    currentUserId: ME,
    socketUrl: 'ws://test',
    createSocket: () => socket,
    clock: new FakeClock(),
    client: { json: async () => [], request: async () => new Response() } as unknown as ApiClient,
    draftsApi,
    stores,
    topic: options.topic
      ? createTopicSessionMode(topicsApi.api, CHAT_ID, options.topic, (result) => readResults.push(result))
      : null,
    ...(options.readCursor === undefined ? {} : { readCursor: options.readCursor }),
  })
  const open = async () => {
    session.start()
    await settle()
    socket.open()
    socket.receive(AUTH_OK)
  }
  const timeline = () => selectTimeline(CHAT_ID)(stores.message.getState())
  return { session, socket, stores, topicsApi, readResults, open, timeline }
}

const ids = (messages: ReturnType<ReturnType<typeof topicHarness>['timeline']>['messages']) =>
  messages.flatMap((row) => (row.type === 'broadcast' ? [row.message_id] : []))

describe('topic mode', () => {
  test('only this topic’s broadcasts reach the timeline', async () => {
    const h = topicHarness({ topic: { id: 't1', is_general: false } })
    await h.open()
    h.socket.receive(broadcastFrame('m1', '2026-10-01T09:00:00Z', 'in', { topic_id: 't1' }))
    h.socket.receive(broadcastFrame('m2', '2026-10-01T09:01:00Z', 'other', { topic_id: 't2' }))
    h.socket.receive(broadcastFrame('m3', '2026-10-01T09:02:00Z', 'general'))
    expect(ids(h.timeline().messages)).toEqual(['m1'])
    h.session.stop()
  })

  test('General takes messages without a topic_id', async () => {
    const h = topicHarness({ topic: { id: 'g', is_general: true } })
    await h.open()
    h.socket.receive(broadcastFrame('m1', '2026-10-01T09:00:00Z', 'in', { topic_id: 't1' }))
    h.socket.receive(broadcastFrame('m2', '2026-10-01T09:01:00Z', 'general', { topic_id: null }))
    h.socket.receive(broadcastFrame('m3', '2026-10-01T09:02:00Z', 'general'))
    expect(ids(h.timeline().messages)).toEqual(['m2', 'm3'])
    h.session.stop()
  })

  test('the topic page merges under the replay before the timeline opens, then reads the topic', async () => {
    const page = [
      { ...storedMessage('old1', '2026-09-01T00:00:00Z', 'a'), topic_id: 't1' },
      { ...storedMessage('old2', '2026-09-02T00:00:00Z', 'b'), topic_id: 't1' },
      { ...storedMessage('m1', '2026-10-01T09:00:00Z', 'dup'), topic_id: 't1' },
    ]
    const h = topicHarness({ topic: { id: 't1', is_general: false }, page })
    await h.open()
    h.socket.receive(broadcastFrame('m1', '2026-10-01T09:00:00Z', 'dup', { topic_id: 't1' }))
    h.socket.receive({ type: 'history_complete' })
    await settle()
    expect(h.timeline().historyReady).toBeFalse()
    expect(h.topicsApi.pages).toEqual([{ topicId: 't1', before: undefined }])
    h.topicsApi.release()
    await settle()
    expect(ids(h.timeline().messages)).toEqual(['old1', 'old2', 'm1'])
    expect(h.timeline().historyReady).toBeTrue()
    await settle()
    expect(h.topicsApi.reads).toEqual(['t1:m1'])
    expect(h.readResults[0]?.chat_read_advanced).toBeTrue()
    expect(h.socket.sentFrames().some((frame) => frame.type === 'read')).toBeFalse()
    h.session.stop()
  })

  test('sends carry the topic id; General omits it', async () => {
    const topic = topicHarness({ topic: { id: 't1', is_general: false } })
    await topic.open()
    topic.session.sendMessage('hi')
    expect(topic.socket.sentFrames().find((frame) => frame.type === 'message')?.topic_id).toBe('t1')
    topic.session.stop()

    const general = topicHarness({ topic: { id: 'g', is_general: true } })
    await general.open()
    general.session.sendMessage('hi')
    expect(general.socket.sentFrames().find((frame) => frame.type === 'message')).not.toHaveProperty('topic_id')
    general.session.stop()
  })

  test('a stopped session ignores a late topic page', async () => {
    const h = topicHarness({
      topic: { id: 't1', is_general: false },
      page: [storedMessage('x', '2026-09-01T00:00:00Z', 'x')],
    })
    await h.open()
    h.socket.receive({ type: 'history_complete' })
    h.session.stop()
    h.topicsApi.release()
    await settle()
    expect(h.timeline().messages).toHaveLength(0)
    expect(h.timeline().historyReady).toBeFalse()
  })
})

test('the forum topic list (readCursor false) never sends the chat-level read frame', async () => {
  const h = topicHarness({ readCursor: false })
  await h.open()
  h.socket.receive(broadcastFrame('m1', '2026-10-01T09:00:00Z', 'x'))
  h.socket.receive({ type: 'history_complete' })
  await settle()
  h.socket.receive(broadcastFrame('m2', '2026-10-01T09:01:00Z', 'y'))
  expect(h.timeline().historyReady).toBeTrue()
  expect(h.socket.sentFrames().some((frame) => frame.type === 'read')).toBeFalse()
  h.session.stop()
})

test('onFrame sees every server frame', async () => {
  const h = topicHarness()
  const seen: string[] = []
  const off = h.session.onFrame((frame) => seen.push(frame.type))
  await h.open()
  h.socket.receive({
    type: 'topic_updated',
    topic: { id: 't1', title: 'x', icon_emoji: '', closed: false, pinned: false },
  })
  off()
  h.socket.receive({ type: 'history_complete' })
  expect(seen).toEqual(['auth_ok', 'topic_updated'])
  h.session.stop()
})

test('the topic list api pages the topic endpoints into broadcast rows', async () => {
  const { api, pages, release } = fakeTopicsApi([
    { ...storedMessage('m1', '2026-10-01T00:00:00Z', 'x'), topic_id: 't1' },
  ])
  release()
  const listApi = createTopicListApi(api, CHAT_ID, 't1')
  const older = await listApi.loadOlder('m9', 50)
  expect(pages).toEqual([{ topicId: 't1', before: 'm9' }])
  expect(older[0]).toMatchObject({ type: 'broadcast', message_id: 'm1', topic_id: 't1' })
  expect(await listApi.loadAround('m1', 60)).toHaveLength(1)
})
