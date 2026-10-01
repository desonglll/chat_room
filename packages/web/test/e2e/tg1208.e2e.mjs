/**
 * TG-1208 end-to-end check of the walkthrough leftovers against a REAL server, two accounts in
 * two browser contexts. Not part of `bun test`. Seeds itself (two accounts, a friendship, a
 * private chat, a group, a custom-emoji set).
 *
 *   (cd packages/web && bun run build)
 *   CARGO_TARGET_DIR=<private dir> cargo run --bin server -- -p 18208 --config <redis-off copy> \
 *     --database-type sqlite --database /tmp/tg-1208.db
 *   BASE_URL=http://127.0.0.1:18208 node packages/web/test/e2e/tg1208.e2e.mjs
 *
 * The «send before the chat is ready» race (items 1 and 7) is made deterministic by holding the
 * chat socket's server frames in a Playwright WebSocket route: everything for 2.5 s (still
 * connecting), or everything after `auth_ok` (online, replay pending).
 *
 * Env: BASE_URL (required); SHOTS (default /tmp/tg-shots/TG-1208); PLAYWRIGHT (module path);
 * BASE_DIST — serve the page and assets from another `dist` (e.g. one built from the base
 * commit) while the API and sockets stay on BASE_URL: the A/B run that shows the defects.
 * Every step runs even if an earlier one failed; exit code 0 = all passed.
 */
import { execFileSync } from 'node:child_process'
import { existsSync, mkdirSync, readFileSync } from 'node:fs'
import { extname, join } from 'node:path'

const BASE = process.env.BASE_URL
if (!BASE) throw new Error('BASE_URL is required, e.g. http://127.0.0.1:18208')
const SHOTS = process.env.SHOTS ?? '/tmp/tg-shots/TG-1208'
const BASE_DIST = process.env.BASE_DIST ?? ''
const { chromium } = await import(process.env.PLAYWRIGHT ?? '/tmp/pw/node_modules/playwright/index.mjs')
mkdirSync(SHOTS, { recursive: true })

const run = Date.now().toString(36)
const failed = []
const pageErrors = []
let n = 0

async function step(page, name, body) {
  n += 1
  const shot = `${SHOTS}/${String(n).padStart(2, '0')}-${name}`
  try {
    await body()
    await page.waitForTimeout(1300)
    await page.screenshot({ path: `${shot}.png` })
    console.log('ok', n, name)
  } catch (error) {
    failed.push(name)
    console.log('FAIL', n, name, String(error).split('\n')[0])
    await page.screenshot({ path: `${shot}-FAIL.png` }).catch(() => {})
  }
}

async function api(token, method, path, body) {
  const form = body instanceof FormData
  const response = await fetch(`${BASE}${path}`, {
    method,
    headers: {
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
      ...(body === undefined || form ? {} : { 'Content-Type': 'application/json' }),
    },
    body: body === undefined ? undefined : form ? body : JSON.stringify(body),
  })
  if (!response.ok) throw new Error(`${method} ${path} → ${response.status} ${await response.text()}`)
  const text = await response.text()
  return text ? JSON.parse(text) : null
}

async function account(prefix) {
  const username = `${prefix}_${run}`
  const password = 'correct-horse-8'
  const session = await api(null, 'POST', '/api/users/register', { username, password })
  return { username, password, token: session.token, id: session.user.id }
}

async function login(page, user) {
  await page.goto(`${BASE}/login`)
  await page.locator('input[name="username"]').fill(user.username)
  await page.locator('input[name="password"]').fill(user.password)
  await page.getByRole('button', { name: '登录', exact: true }).click()
  await page.getByRole('button', { name: '主菜单' }).first().waitFor()
}

const input = (page) => page.getByLabel('消息内容')
const contents = (page) =>
  page.locator('.tg-message .tg-message__column').evaluateAll((nodes) => nodes.map((node) => node.textContent ?? ''))
const indexOf = (list, needle) => list.findIndex((text) => text.includes(needle))
const chatRow = (page, chatId) => page.locator(`a.tg-chatrow[href="/chat/${encodeURIComponent(chatId)}"]`)

// ---- seed
const alice = await account('t8_alice')
const bob = await account('t8_bob')
await api(alice.token, 'POST', '/api/friend-requests', { user_id: bob.id })
await api(bob.token, 'PATCH', `/api/friend-requests/${alice.id}`, { action: 'accept' })
const dmId = (await api(alice.token, 'POST', '/api/direct-chats', { user_id: bob.id })).room_id
const group = await api(alice.token, 'POST', '/api/chats', {
  title: `遗留群 ${run}`,
  password: null,
  join_policy: 'open',
})
await api(bob.token, 'POST', `/api/chats/${group.id}/join-requests`, { password: null })

// A custom-emoji set for alice's emoji status (item 5).
const emojiFile = `/tmp/tg1208-emoji-${run}.webp`
execFileSync('ffmpeg', [
  '-loglevel',
  'error',
  '-f',
  'lavfi',
  '-i',
  'color=c=orange:s=100x100',
  '-frames:v',
  '1',
  emojiFile,
])
const shortName = `tg1208_${run}`
const emojiSet = await api(alice.token, 'POST', '/api/sticker-sets', {
  short_name: shortName,
  title: '遗留表情',
  set_type: 'custom_emoji',
})
const form = new FormData()
form.append('file', new Blob([readFileSync(emojiFile)], { type: 'image/webp' }), 'emoji.webp')
form.append('emoji', '⭐')
await api(alice.token, 'POST', `/api/sticker-sets/${shortName}/stickers`, form)
await api(alice.token, 'PUT', `/api/stickers/installed/${emojiSet.id}`, {}).catch(() => {})

// ---- browser
// A page fulfilled from BASE_DIST is not a local origin to Chromium, which then blocks its
// sockets to 127.0.0.1 (ERR_BLOCKED_BY_LOCAL_NETWORK_ACCESS_CHECKS); the A/B run lifts that.
const browser = await chromium.launch(BASE_DIST ? { args: ['--disable-features=LocalNetworkAccessChecks'] } : {})
const MIME = {
  '.html': 'text/html',
  '.js': 'text/javascript',
  '.css': 'text/css',
  '.json': 'application/json',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.webmanifest': 'application/manifest+json',
  '.wasm': 'application/wasm',
}

/** What the chat sockets of a context hold back right now (items 1 and 7). */
async function newContext(hold) {
  const context = await browser.newContext({
    viewport: { width: 1280, height: 820 },
    ...(BASE_DIST ? { serviceWorkers: 'block' } : {}),
  })
  if (BASE_DIST) {
    await context.route(`${BASE}/**`, (route) => {
      const path = new URL(route.request().url()).pathname
      if (path.startsWith('/api/') || path.startsWith('/ws/')) return route.continue()
      const file = join(BASE_DIST, path)
      const target = existsSync(file) && extname(file) ? file : join(BASE_DIST, 'index.html')
      return route.fulfill({
        body: readFileSync(target),
        contentType: MIME[extname(target)] ?? 'application/octet-stream',
      })
    })
  }
  await context.routeWebSocket(/\/ws\/(?!account)/, (ws) => {
    const server = ws.connectToServer()
    const mode = hold.mode
    let queue = mode ? [] : null
    if (mode) {
      setTimeout(() => {
        for (const message of queue ?? []) ws.send(message)
        queue = null
      }, hold.ms)
    }
    server.onMessage((message) => {
      if (queue === null) return ws.send(message)
      if (mode === 'replay' && String(message).includes('"auth_ok"')) return ws.send(message)
      queue.push(message)
    })
  })
  const page = await context.newPage()
  page.on('pageerror', (error) => pageErrors.push(error.message))
  return page
}

const holdA = { mode: null, ms: 0 }
const holdB = { mode: null, ms: 0 }
const a = await newContext(holdA)
const b = await newContext(holdB)
try {
  await login(a, alice)
  await login(b, bob)

  // History in the private chat, sent by alice before bob opens it.
  await a.goto(`${BASE}/chat/${dmId}`)
  await input(a).waitFor()
  // Seeding retries Enter: on the base bundle (BASE_DIST) an Enter while the socket is still
  // connecting is swallowed — item 1 itself — and the text stays in the composer.
  for (const text of ['hist-1', 'hist-2', 'hist-3']) {
    await input(a).fill(text)
    for (let attempt = 0; attempt < 10; attempt += 1) {
      if ((await input(a).inputValue()) === text) await input(a).press('Enter')
      const sent = await a
        .locator('.tg-message', { hasText: text })
        .waitFor({ timeout: 1500 })
        .then(
          () => true,
          () => false,
        )
      if (sent) break
    }
  }

  // ---- items 1 and 7: sending before the chat is ready
  await step(b, 'enter-while-connecting-is-not-dropped', async () => {
    Object.assign(holdB, { mode: 'connect', ms: 2500 })
    await b.goto(`${BASE}/chat/${dmId}`)
    await input(b).waitFor()
    await input(b).fill('early-connect')
    await input(b).press('Enter')
    await b.waitForTimeout(300)
    const left = await input(b).inputValue()
    if (left !== '') throw new Error(`Enter was swallowed; the composer still holds «${left}»`)
    await a.locator('.tg-message', { hasText: 'early-connect' }).waitFor({ timeout: 8000 })
    const order = await contents(b)
    if (!(indexOf(order, 'hist-3') >= 0 && indexOf(order, 'hist-3') < indexOf(order, 'early-connect')))
      throw new Error(`own message not below the history: ${JSON.stringify(order)}`)
  })

  await step(b, 'send-mid-replay-lands-below-history', async () => {
    Object.assign(holdB, { mode: 'replay', ms: 2500 })
    await b.goto(`${BASE}/chat/${dmId}`)
    await input(b).waitFor()
    await b.waitForTimeout(400) // auth_ok is through; the replay is held
    await input(b).fill('early-replay')
    await input(b).press('Enter')
    await a.locator('.tg-message', { hasText: 'early-replay' }).waitFor({ timeout: 8000 })
    await b.locator('.tg-message', { hasText: 'hist-1' }).waitFor({ timeout: 8000 }) // replay released
    await b.waitForTimeout(500)
    const order = await contents(b)
    if (indexOf(order, 'early-replay') !== order.length - 1 || indexOf(order, 'hist-1') < 0)
      throw new Error(`own message not last, below the replay: ${JSON.stringify(order)}`)
    Object.assign(holdB, { mode: null, ms: 0 })
  })

  // ---- item 2: the row's dot and the header agree
  await step(b, 'row-and-header-agree-on-presence', async () => {
    Object.assign(holdB, { mode: null, ms: 0 })
    await a.goto(`${BASE}/chat/${group.id}`) // alice is not in the private chat
    await input(a).waitFor()
    await b.goto(`${BASE}/chat/${dmId}`)
    await input(b).waitFor()
    await b.waitForTimeout(1200)
    const agree = async (label) => {
      const header = (await b.locator('.tg-chat__header').first().innerText()).includes('在线')
      const dot = (await chatRow(b, dmId).locator('.tg-avatar__online').count()) > 0
      if (header !== dot) throw new Error(`${label}: header online=${header}, row dot=${dot}`)
      return header
    }
    const away = await agree('alice elsewhere')
    if (away) throw new Error('alice is not in the private chat, yet the header says online')
    await a.goto(`${BASE}/chat/${dmId}`)
    await input(a).waitFor()
    await b.waitForTimeout(1500)
    const here = await agree('alice in the chat')
    if (!here) throw new Error('alice opened the private chat, yet the header does not say online')
  })

  // ---- item 3: mute choices follow the state
  await step(b, 'mute-choices-follow-state', async () => {
    await b.goto(`${BASE}/chat/${group.id}`)
    await b.getByRole('button', { name: '查看会话信息' }).click()
    const choices = b.locator('.tg-chatinfo__notify-choices button')
    await choices.first().waitFor()
    const unmuted = await choices.allInnerTexts()
    if (unmuted.includes('取消静音')) throw new Error(`unmuted chat offers «取消静音»: ${unmuted}`)
    await choices.filter({ hasText: '1 小时' }).click()
    await b.locator('.tg-chatinfo__notify-choices button', { hasText: '取消静音' }).waitFor({ timeout: 4000 })
    const muted = await choices.allInnerTexts()
    if (muted.length !== 1) throw new Error(`muted chat offers more than «取消静音»: ${muted}`)
    await choices.first().click()
    await b.locator('.tg-chatinfo__notify-choices button', { hasText: '1 小时' }).waitFor({ timeout: 4000 })
    await b.keyboard.press('Escape')
  })

  // ---- item 5: emoji status — set in settings, shown in the list row and the member list
  await step(a, 'emoji-status-set-from-settings', async () => {
    await a.getByRole('button', { name: '主菜单' }).first().click()
    await a.getByRole('menuitem', { name: '设置' }).click()
    await a.locator('.tg-settings').getByText('我的账号', { exact: true }).first().click()
    await a.locator('.tg-settings').getByText('表情状态', { exact: true }).first().click()
    await a.locator('.tg-emoji-status-picker .tg-custom-emoji-grid__cell').first().click({ timeout: 8000 })
    await a.locator('.tg-emoji-status-picker').waitFor({ state: 'detached', timeout: 5000 })
  })
  await step(b, 'emoji-status-in-row-and-members', async () => {
    await b.goto(`${BASE}/chat/${group.id}`)
    await chatRow(b, dmId).locator('.tg-emoji-status').waitFor({ timeout: 8000 })
    await b.getByRole('button', { name: '查看会话信息' }).click()
    const membersTab = b.getByRole('tab', { name: /成员/ })
    if (await membersTab.count()) await membersTab.first().click()
    await b
      .locator('.tg-chatinfo__item[data-kind="member"]', { hasText: alice.username })
      .locator('.tg-emoji-status')
      .waitFor({ timeout: 8000 })
  })
} finally {
  await browser.close()
}

if (pageErrors.length) {
  console.log('page errors:', pageErrors)
  failed.push('page-errors')
}
console.log(failed.length ? `FAILED: ${failed.join(', ')}` : `ALL ${n} STEPS PASSED`)
process.exit(failed.length ? 1 : 0)
