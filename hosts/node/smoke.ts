/**
 * TG-605: a minimal non-DOM host for `packages/core` — the proof that the mobile door is open.
 *
 * Run under plain Node (no DOM, no polyfill): it injects Node's own storage (a Map), WebSocket
 * (Node's built-in WHATWG client) and clock into core, then signs up, lists chats, creates one,
 * joins its socket, sends a message, sees it come back as a broadcast, and reads it from history.
 *
 *   bun build hosts/node/smoke.ts --target=node --outfile=/tmp/core-smoke.mjs
 *   node /tmp/core-smoke.mjs http://127.0.0.1:3000
 */
import {
  createApiClient,
  createChat,
  createChatSocket,
  listChatMessages,
  listChats,
  registerUser,
  type CoreClock,
  type CoreSocket,
  type CoreSocketFactory,
  type CoreStorage,
} from '../../packages/core/src/index'

const base = process.argv[2] ?? 'http://127.0.0.1:3000'
const host = globalThis as Record<string, unknown>
if ('window' in host || 'document' in host || 'HTMLElement' in host) {
  throw new Error('this host must run without a DOM')
}

/** Storage: an in-memory map (a real app would persist it). */
function createStorage(): CoreStorage {
  const values = new Map<string, string>()
  return {
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => void values.set(key, value),
    removeItem: (key) => void values.delete(key),
  }
}

/** WebSocket: Node's built-in WHATWG client, adapted to core's handler-property shape. */
const createSocket: CoreSocketFactory = (url) => {
  const socket = new WebSocket(url)
  const adapter: CoreSocket = {
    send: (data) => socket.send(data),
    close: (code, reason) => socket.close(code, reason),
    onopen: null,
    onmessage: null,
    onclose: null,
    onerror: null,
  }
  socket.onopen = () => adapter.onopen?.()
  socket.onmessage = (event) => adapter.onmessage?.(String(event.data))
  socket.onclose = (event) => adapter.onclose?.({ code: event.code, reason: event.reason })
  socket.onerror = () => adapter.onerror?.()
  return adapter
}

const clock: CoreClock = {
  now: () => Date.now(),
  setTimeout: (callback, delayMs) => setTimeout(callback, delayMs),
  clearTimeout: (handle) => clearTimeout(handle as ReturnType<typeof setTimeout>),
}

function step(name: string): void {
  console.log(`✓ ${name}`)
}

async function main(): Promise<void> {
  const storage = createStorage()
  const client = createApiClient({ baseUrl: base, fetchImpl: (url, init) => fetch(url, init) })

  const username = `node-host-${Date.now().toString(36)}`
  const session = await registerUser(client, username, 'node-host-password')
  storage.setItem('session', JSON.stringify(session))
  step(`signed up as ${session.user.username}`)

  const before = await listChats(client, session.token)
  step(`listed ${before.length} chats`)
  const chat = await createChat(client, session.token, {
    title: `${username}-chat`,
    password: null,
    join_policy: 'open',
  })
  step(`created chat ${chat.id}`)

  const socket = createChatSocket({
    url: `${base.replace(/^http/, 'ws')}/ws/${chat.id}`,
    token: session.token,
    createSocket,
    clock,
  })
  const content = `hello from a non-DOM host ${Date.now()}`
  const echoed = new Promise<string>((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('no broadcast within 10 s')), 10_000)
    socket.on('broadcast', (frame) => {
      if (frame.content === content) {
        clearTimeout(timer)
        resolve(frame.message_id)
      }
    })
  })
  socket.onStatus((status) => {
    if (status === 'online') socket.send({ type: 'message', content, client_message_id: crypto.randomUUID() })
  })
  socket.connect()
  const messageId = await echoed
  step(`sent a message and received its broadcast ${messageId}`)

  const page = await listChatMessages(client, chat.id, { token: session.token })
  if (!page.some((message) => message.id === messageId)) throw new Error('the message is not in history')
  step('read it back from history')
  socket.close()
}

main().then(
  () => {
    console.log('core-node-host: OK')
    process.exit(0)
  },
  (error: unknown) => {
    console.error('core-node-host: FAILED', error)
    process.exit(1)
  },
)
