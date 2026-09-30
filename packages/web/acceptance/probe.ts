/**
 * TG-012 probe-level acceptance, run with bun against a REAL server serving THIS
 * worktree's embedded build:
 *
 *     cargo run --bin server -- --listen 127.0.0.1:3123 \
 *         --database-type sqlite --database /tmp/tg012-probe.db
 *     bun packages/web/acceptance/probe.ts http://127.0.0.1:3123
 *
 * It walks the M0 completion bar over the same wire the shell uses — register →
 * create chat → send over WS → GET shows it → reconnect replays it (refresh
 * persistence) → a second account's live connection receives the broadcast — plus the
 * served-shell checks (index + hashed asset) and the TG-008 draft frame the shell's
 * draft seam consumes. A REAL BROWSER walk-through is deliberately NOT claimed here;
 * this is the transport-level proof, visual QA stays with a human.
 */

const base = process.argv[2] ?? 'http://127.0.0.1:3123'
const wsBase = base.replace(/^http/, 'ws')
const stamp = Date.now().toString(36)
let step = 0

function log(line: string): void {
  console.log(line)
}

function pass(what: string, detail = ''): void {
  step += 1
  log(`ok ${String(step).padStart(2, '0')}  ${what}${detail ? `  — ${detail}` : ''}`)
}

function fail(what: string, detail: unknown): never {
  console.error(`FAIL at step ${step + 1}: ${what}`, detail)
  process.exit(1)
}

async function json<T>(method: string, path: string, token: string, body?: unknown): Promise<T> {
  const response = await fetch(`${base}${path}`, {
    method,
    headers: {
      accept: 'application/json',
      ...(body === undefined ? {} : { 'content-type': 'application/json' }),
      ...(token ? { authorization: `Bearer ${token}` } : {}),
    },
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
  })
  if (!response.ok) fail(`${method} ${path}`, `${response.status} ${await response.text()}`)
  return (await response.json()) as T
}

interface Frame {
  type: string
  [key: string]: unknown
}

/** A WS connection that performs the join handshake and hands out awaited frames. */
class Probe {
  private frames: Frame[] = []
  private waiters: Array<{ match: (frame: Frame) => boolean; resolve: (frame: Frame) => void }> = []
  private socket: WebSocket

  constructor(
    readonly name: string,
    chatId: string,
    token: string,
  ) {
    this.socket = new WebSocket(`${wsBase}/ws/${chatId}`)
    this.socket.onopen = () => this.socket.send(JSON.stringify({ type: 'join', token }))
    this.socket.onmessage = (event) => {
      const frame = JSON.parse(String(event.data)) as Frame
      this.frames.push(frame)
      const at = this.waiters.findIndex((waiter) => waiter.match(frame))
      if (at >= 0) {
        const [waiter] = this.waiters.splice(at, 1)
        waiter!.resolve(frame)
      }
    }
  }

  /** The next frame matching — including ones already received (history replay races). */
  await(label: string, match: (frame: Frame) => boolean, timeoutMs = 5000): Promise<Frame> {
    const seen = this.frames.find(match)
    if (seen) return Promise.resolve(seen)
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error(`${this.name}: timed out waiting for ${label}`)), timeoutMs)
      this.waiters.push({
        match,
        resolve: (frame) => {
          clearTimeout(timer)
          resolve(frame)
        },
      })
    })
  }

  send(frame: object): void {
    this.socket.send(JSON.stringify(frame))
  }

  close(): void {
    this.socket.close()
  }
}

// ── 1. The server serves this worktree's shell ────────────────────────────────
const index = await fetch(`${base}/`)
const html = await index.text()
if (index.status !== 200 || !html.includes('<div id="root"></div>')) fail('GET /', index.status)
const entry = html.split('src="')[1]?.split('"')[0] ?? ''
if (!entry.startsWith('/assets/') || !entry.endsWith('.js')) fail('module entry in index.html', entry)
const asset = await fetch(`${base}${entry}`)
const bundle = await asset.text()
if (asset.status !== 200 || !bundle.includes('createRoot')) fail(`GET ${entry}`, asset.status)
pass('served shell', `index.html + ${entry} (${bundle.length} bytes, createRoot present)`)

// ── 2. 注册 (two accounts) ────────────────────────────────────────────────────
interface Session {
  token: string
  user: { id: string; username: string }
}
const alice = await json<Session>('POST', '/api/users/register', '', {
  username: `alice-${stamp}`,
  password: 'probe-password-1',
})
const bob = await json<Session>('POST', '/api/users/register', '', {
  username: `bob-${stamp}`,
  password: 'probe-password-2',
})
pass('register', `${alice.user.username} + ${bob.user.username}`)

// ── 3. 建群 (canonical dialect: title, open, passwordless) ────────────────────
const chat = await json<{ id: string; title: string; chat_type: string }>('POST', '/api/chats', alice.token, {
  title: `TG-012 验收群 ${stamp}`,
  password: null,
  join_policy: 'open',
})
pass('create chat', `${chat.id} (${chat.chat_type}) "${chat.title}"`)

// ── 4. 发消息 over the live socket ────────────────────────────────────────────
const a1 = new Probe('alice#1', chat.id, alice.token)
await a1.await('auth_ok', (frame) => frame.type === 'auth_ok')
await a1.await('history_complete', (frame) => frame.type === 'history_complete')
const clientMessageId = crypto.randomUUID()
a1.send({ type: 'message', content: '第一条消息，经由 WS 发送', client_message_id: clientMessageId })
const echoed = await a1.await(
  'own broadcast',
  (frame) => frame.type === 'broadcast' && frame.client_message_id === clientMessageId,
)
pass('WS send → broadcast echo', `message_id ${String(echoed.message_id)}`)

// ── 5. 消息已持久化 (REST shows it) ───────────────────────────────────────────
const page = await json<Array<{ id: string; content: string }>>(
  'GET',
  `/api/chats/${chat.id}/messages?limit=50`,
  alice.token,
)
if (!page.some((row) => row.id === echoed.message_id)) fail('message in GET /messages', page)
pass('REST persistence', `GET /api/chats/:id/messages contains ${String(echoed.message_id)}`)

// ── 6. 刷新后消息仍在 (reconnect replays history) ─────────────────────────────
a1.close()
const a2 = new Probe('alice#2 (refresh)', chat.id, alice.token)
await a2.await('auth_ok', (frame) => frame.type === 'auth_ok')
const replayed = await a2.await(
  'replayed broadcast',
  (frame) => frame.type === 'broadcast' && frame.message_id === echoed.message_id,
)
await a2.await('history_complete', (frame) => frame.type === 'history_complete')
pass('refresh persistence', `reconnect replayed "${String(replayed.content)}"`)

// ── 7. 第二个浏览器实时收到 ───────────────────────────────────────────────────
await json('POST', `/api/chats/${chat.id}/join-requests`, bob.token, { password: null })
const b1 = new Probe('bob#1', chat.id, bob.token)
await b1.await('auth_ok', (frame) => frame.type === 'auth_ok')
await b1.await('history_complete', (frame) => frame.type === 'history_complete')
a2.send({ type: 'message', content: '给第二个浏览器的实时消息', client_message_id: crypto.randomUUID() })
const live = await b1.await(
  'live broadcast at bob',
  (frame) => frame.type === 'broadcast' && frame.content === '给第二个浏览器的实时消息',
)
pass('second browser receives live', `bob got message_id ${String(live.message_id)}`)

// ── 8. TG-008 seam: a REST draft save reaches the account's own live socket ───
await json('PUT', `/api/chats/${chat.id}/draft`, alice.token, { text: '云端草稿（探针）' })
const draftFrame = await a2.await(
  'draft_updated at alice',
  (frame) => frame.type === 'draft_updated' && frame.text === '云端草稿（探针）',
)
if (draftFrame.user_id !== alice.user.id) fail('draft frame user_id', draftFrame)
pass('draft_updated frame', 'delivered to the drafting account’s own connection')

a2.close()
b1.close()
log('')
log(`TG-012 acceptance probe: all ${step} steps passed against ${base}`)
process.exit(0)
