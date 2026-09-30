/**
 * Browser benchmark harness for the virtual list (TG-101). NOT part of the app bundle:
 * nothing imports it; `runBrowserBench.mjs` bundles it with `bun build` into a static page
 * and drives it with Playwright. It mounts the REAL `MessageList` against the real stores,
 * with an in-memory fake of the two history endpoints (same semantics as the server:
 * inclusive `before` cursor, chronological pages, `limit/2 + 1` older context) over
 * `?total=` synthetic messages of varied length, so row heights are genuinely dynamic.
 *
 * `window.tgBench` lets the runner push live messages and read the list's timing.
 */
import '@tg/ui/styles.css'
import '../../../styles/index.css'
import { createRoot } from 'react-dom/client'
import type { BroadcastMessage } from '@tg/core'
import { authStore, chatListStore, messageStore } from '@tg/core'
import { MessageList } from '../MessageList'
import type { MessageListApi } from '../messageListController'

const params = new URLSearchParams(location.search)
const TOTAL = Number(params.get('total') ?? 100_000)
const LIVE = Number(params.get('live') ?? 100)
const LATENCY_MS = Number(params.get('latency') ?? 40)
const MOUNT = params.get('mount') !== '0'
/** The message 5 rows from the end quotes this one: the "jump 50k back" target. */
const FAR_TARGET = Math.max(0, TOTAL - 50_001)
const CHAT_ID = 'bench-chat'
const ME = 'u0'
const BASE = Date.parse('2026-01-01T00:00:00Z')
const WORDS = '今天 我们 讨论 一下 这个 虚拟 列表 的 滚动 性能 以及 锚点 保持 问题 好的 没问题'.split(' ')

const idOf = (n: number) => `m${String(n).padStart(7, '0')}`

function synthetic(n: number): BroadcastMessage {
  const sender = `u${Math.floor(n / (1 + (n % 3))) % 5}`
  const words = 2 + ((n * 7919) % 60) // 2..61 words: one to several lines
  const content = Array.from({ length: words }, (_, i) => WORDS[(n + i) % WORDS.length]).join(' ')
  return {
    type: 'broadcast',
    message_id: idOf(n),
    sender_id: sender,
    sender: `用户${sender}`,
    sender_avatar: '',
    content: `#${n} ${content}`,
    attachment: null,
    reply_to:
      n === TOTAL - 5
        ? {
            message_id: idOf(FAR_TARGET),
            sender: 'far',
            content: '很久以前的消息',
            attachment_file_name: null,
            recalled: false,
          }
        : n % 17 === 0 && n > 50
          ? { message_id: idOf(n - 40), sender: 'x', content: '引用', attachment_file_name: null, recalled: false }
          : null,
    recalled_at: null,
    edited_at: null,
    timestamp: new Date(BASE + n * 45_000).toISOString(),
    favorite_id: null,
    forwarded_from: null,
    reactions: [],
  }
}

const server: BroadcastMessage[] = Array.from({ length: TOTAL }, (_, n) => synthetic(n))
const indexById = new Map(server.map((message, index) => [message.message_id, index]))
const delay = () => new Promise((resolve) => setTimeout(resolve, LATENCY_MS))
const requests: string[] = []

const api: MessageListApi = {
  async loadOlder(beforeId, limit) {
    requests.push(`older:${beforeId}`)
    await delay()
    const at = indexById.get(beforeId) ?? -1
    return server.slice(Math.max(0, at + 1 - limit), at + 1)
  },
  async loadAround(messageId, limit) {
    requests.push(`around:${messageId}`)
    await delay()
    const at = indexById.get(messageId)
    if (at === undefined) return []
    const older = server.slice(Math.max(0, at + 1 - (Math.floor(limit / 2) + 1)), at + 1)
    return [...older, ...server.slice(at + 1, at + 1 + (limit - older.length))]
  },
}

authStore.getState().setSession({
  token: 'bench',
  user: { id: ME, username: 'bench' },
} as Parameters<ReturnType<typeof authStore.getState>['setSession']>[0])
chatListStore.getState().setChats([
  {
    id: CHAT_ID,
    chat_type: 'group',
    title: 'Bench',
    has_password: false,
    creator_user_id: null,
    join_policy: 'open',
    avatar_emoji: '',
    description: '',
    username: null,
    is_forum: false,
    linked_chat_id: null,
    slow_mode_seconds: 0,
    auto_delete_seconds: 0,
    signatures_enabled: false,
    history_visible_to_new_members: true,
    member_count: 5,
    unread_count: 0,
    created_at: new Date(BASE).toISOString(),
  },
])
// Bulk seed (applyBroadcast per message is O(n) each): `?live=100000` puts the whole
// history in the store timeline, which is how the 100k-rows-mounted memory is measured.
messageStore.setState((state) => ({
  timelines: { ...state.timelines, [CHAT_ID]: { messages: server.slice(TOTAL - LIVE), historyReady: true } },
}))

let next = TOTAL
declare global {
  interface Window {
    tgBench: {
      total: number
      requests: string[]
      pushIncoming(): void
      pushOwnPending(): void
      idOf(n: number): string
      farTarget: string
    }
  }
}

window.tgBench = {
  total: TOTAL,
  requests,
  pushIncoming() {
    const message = { ...synthetic(next++), sender_id: 'u3', sender: '用户u3', timestamp: new Date().toISOString() }
    messageStore.getState().applyBroadcast(CHAT_ID, message, 'incoming')
  },
  pushOwnPending() {
    messageStore.getState().appendOptimistic(CHAT_ID, {
      clientMessageId: `c${next++}`,
      content: '我发的消息',
      replyTo: '',
      currentUserId: ME,
      participants: [],
    })
  },
  idOf,
  farTarget: idOf(FAR_TARGET),
}

const container = document.getElementById('root')
if (!container) throw new Error('missing #root')
createRoot(container).render(
  <div className="tg-chat" style={{ height: '100vh' }}>
    {MOUNT ? <MessageList chatId={CHAT_ID} currentUserId={ME} api={api} /> : null}
  </div>,
)
