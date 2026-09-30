/**
 * TG-401 end-to-end check against a REAL server and headless Chromium with a fake microphone
 * (a generated tone). Two accounts in two browser contexts. Not part of `bun test`.
 *
 * Run (see docs/devlog/TG-401.md "E2E"):
 *   (cd packages/web && bun run build)
 *   CARGO_TARGET_DIR=<private dir> cargo run --bin server -- -p 3931 \
 *     --database-type sqlite --database /tmp/tg401-e2e.db
 *   BASE_URL=http://127.0.0.1:3931 node packages/web/src/features/voice/e2e/voice.e2e.mjs
 *
 * Env: BASE_URL (required), SHOTS (default /tmp/tg-shots/TG-401), PLAYWRIGHT (module path,
 * default /tmp/pw/node_modules/playwright/index.mjs). Exit 0 = every step passed.
 */
import { mkdirSync } from 'node:fs'

const BASE_URL = process.env.BASE_URL
if (!BASE_URL) throw new Error('BASE_URL is required, e.g. http://127.0.0.1:3931')
const SHOTS = process.env.SHOTS ?? '/tmp/tg-shots/TG-401'
const { chromium } = await import(process.env.PLAYWRIGHT ?? '/tmp/pw/node_modules/playwright/index.mjs')
mkdirSync(SHOTS, { recursive: true })

const run = Date.now().toString(36)
const ALICE = { username: `va_${run}`, password: 'correct-horse-1' }
const BOB = { username: `vb_${run}`, password: 'correct-horse-2' }
const pageErrors = []
let step = 0

async function check(name, body) {
  step += 1
  await body()
  console.log(`ok ${step} ${name}`)
}

async function shot(page, name) {
  await page.screenshot({ path: `${SHOTS}/${String(step).padStart(2, '0')}-${name}.png` })
}

async function api(token, method, path, body) {
  const response = await fetch(`${BASE_URL}${path}`, {
    method,
    headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
  })
  if (!response.ok) throw new Error(`${method} ${path} → ${response.status} ${await response.text()}`)
  const text = await response.text()
  return text ? JSON.parse(text) : null
}

async function register(page, user) {
  await page.goto(`${BASE_URL}/login`)
  await page.getByRole('button', { name: '注册新账号' }).click()
  await page.locator('input[name="username"]').fill(user.username)
  await page.locator('input[name="password"]').fill(user.password)
  await page.getByRole('button', { name: '注册', exact: true }).click()
  await page.getByRole('button', { name: '主菜单' }).first().waitFor()
  const session = await page.evaluate(() => JSON.parse(localStorage.getItem('tg.session.v1') ?? 'null'))
  return { token: session.token, id: session.user.id }
}

async function voiceMessages(token, chatId) {
  const page = await api(token, 'GET', `/api/chats/${chatId}/messages`)
  return page.filter((message) => message.voice).sort((x, y) => x.created_at.localeCompare(y.created_at))
}

async function waitFor(predicate, what, timeout = 10_000) {
  const until = Date.now() + timeout
  for (;;) {
    const value = await predicate()
    if (value) return value
    if (Date.now() > until) throw new Error(`timed out: ${what}`)
    await new Promise((resolve) => setTimeout(resolve, 150))
  }
}

async function micBox(page) {
  const box = await page.locator('.tg-voice-mic').boundingBox()
  return { x: box.x + box.width / 2, y: box.y + box.height / 2 }
}

/** Hold the mic for `ms`, optionally sliding by (dx, dy) before letting go. */
async function hold(page, ms, slide = null) {
  const { x, y } = await micBox(page)
  await page.mouse.move(x, y)
  await page.mouse.down()
  await page.waitForTimeout(ms)
  if (slide) await page.mouse.move(x + slide.dx, y + slide.dy, { steps: 8 })
  await page.mouse.up()
}

const voiceBubbles = (page) => page.locator('.tg-message').filter({ has: page.locator('.tg-voice') })

// Chromium's fake microphone; permission pre-granted.
const browser = await chromium.launch({
  args: [
    '--use-fake-ui-for-media-stream',
    '--use-fake-device-for-media-stream',
    '--autoplay-policy=no-user-gesture-required',
  ],
})
try {
  const contextA = await browser.newContext({ viewport: { width: 1280, height: 800 }, permissions: ['microphone'] })
  const contextB = await browser.newContext({ viewport: { width: 1280, height: 800 }, permissions: ['microphone'] })
  const a = await contextA.newPage()
  const b = await contextB.newPage()
  for (const [who, page] of [
    ['A', a],
    ['B', b],
  ]) {
    page.on('pageerror', (error) => pageErrors.push(`${who}: ${error.message}`))
  }

  let alice
  let bob
  let chatId = ''
  await check('register both accounts; A creates a group, B joins, both open it', async () => {
    alice = await register(a, ALICE)
    bob = await register(b, BOB)
    const chat = await api(alice.token, 'POST', '/api/chats', {
      title: `语音 ${run}`,
      password: '',
      join_policy: 'open',
    })
    chatId = chat.id
    await api(bob.token, 'POST', `/api/chats/${chatId}/join-requests`, { password: null })
    await a.goto(`${BASE_URL}/chat/${chatId}`)
    await b.goto(`${BASE_URL}/chat/${chatId}`)
    await a.locator('.tg-voice-mic').waitFor()
    await b.locator('.tg-voice-mic').waitFor()
  })

  await check('hold to record: panel with timer + live waveform; B sees «正在录音»; release sends', async () => {
    const { x, y } = await micBox(a)
    await a.mouse.move(x, y)
    await a.mouse.down()
    await a.locator('.tg-voice-rec').waitFor()
    await a.locator('.tg-voice-rec__hint', { hasText: '滑动取消' }).waitFor()
    await b.locator('.tg-chat__header', { hasText: '正在录音' }).waitFor({ timeout: 6_000 })
    await a.waitForTimeout(2_200)
    await shot(a, 'a-recording-panel')
    await shot(b, 'b-sees-recording-voice')
    await a.mouse.up()
    await voiceBubbles(b).first().waitFor({ timeout: 10_000 })
    await voiceBubbles(a).first().waitFor()
    const [stored] = await voiceMessages(alice.token, chatId)
    if (stored.media_kind !== 'voice' || stored.attachment.mime_type !== 'audio/webm') {
      throw new Error(`unexpected voice message ${JSON.stringify(stored).slice(0, 300)}`)
    }
    if (stored.voice.duration_ms < 2_000 || stored.voice.duration_ms > 4_000) {
      throw new Error(`duration ${stored.voice.duration_ms} is not the ~2.5 s held`)
    }
    if (stored.voice.waveform.length !== 100 || Math.max(...stored.voice.waveform) === 0) {
      throw new Error('waveform missing or flat for a tone')
    }
    await b.locator('.tg-chat__header', { hasText: '正在录音' }).waitFor({ state: 'detached', timeout: 7_000 })
    await shot(b, 'b-received-voice')
  })

  await check('slide left cancels: nothing is sent', async () => {
    await hold(a, 1_200, { dx: -160, dy: 0 })
    await a.waitForTimeout(800)
    if ((await voiceMessages(alice.token, chatId)).length !== 1) throw new Error('a cancelled recording was sent')
    if (await a.locator('.tg-voice-rec').count()) throw new Error('panel still open after cancel')
  })

  await check('slide up locks (hands-free); «发送语音» sends it', async () => {
    const { x, y } = await micBox(a)
    await a.mouse.move(x, y)
    await a.mouse.down()
    await a.waitForTimeout(400)
    await a.mouse.move(x, y - 110, { steps: 8 })
    await a.mouse.up()
    await a.locator('.tg-voice-rec__cancel', { hasText: '取消' }).waitFor()
    await a.waitForTimeout(1_500)
    await shot(a, 'a-locked')
    await a.getByRole('button', { name: '发送语音' }).click()
    await waitFor(async () => (await voiceMessages(alice.token, chatId)).length === 2, 'second voice message')
    await voiceBubbles(b).nth(1).waitFor()
  })

  // TG-402: a pointer tap now toggles mic ↔ camera, so hands-free is Enter/Space (and slide up).
  await check('Enter records hands-free; «取消» discards it', async () => {
    await a.locator('.tg-voice-mic').focus()
    await a.keyboard.press('Enter')
    await a.locator('.tg-voice-rec__cancel').waitFor()
    await a.waitForTimeout(900)
    await a.locator('.tg-voice-rec__cancel').click()
    await a.waitForTimeout(600)
    if ((await voiceMessages(alice.token, chatId)).length !== 2) throw new Error('a discarded recording was sent')
  })

  await check('both sides show the unlistened dot before anyone plays', async () => {
    await voiceBubbles(a).first().locator('.tg-voice__unlistened').waitFor()
    await voiceBubbles(b).first().locator('.tg-voice__unlistened').waitFor()
    await shot(a, 'a-unlistened')
  })

  await check('B plays: progress runs, speed chip cycles, both dots clear (listened sync)', async () => {
    const first = voiceBubbles(b).first()
    await first.getByRole('button', { name: '播放语音' }).click()
    await first.getByRole('button', { name: '暂停语音' }).waitFor()
    await first.locator('.tg-voice__bar[data-played]').first().waitFor({ timeout: 5_000 })
    await first.locator('.tg-voice__unlistened').waitFor({ state: 'detached' })
    await voiceBubbles(a).first().locator('.tg-voice__unlistened').waitFor({ state: 'detached', timeout: 5_000 })
    await first.locator('.tg-voice__rate', { hasText: '1x' }).click()
    await first.locator('.tg-voice__rate', { hasText: '1.5x' }).waitFor()
    await shot(b, 'b-playing')
    await shot(a, 'a-listened-dot-cleared')
    const [stored] = await voiceMessages(alice.token, chatId)
    if (!stored.voice.listened) throw new Error('server does not report listened to the sender')
  })

  await check('drag seek on the waveform moves playback', async () => {
    const first = voiceBubbles(b).first()
    const wave = first.locator('.tg-voice__wave')
    const box = await wave.boundingBox()
    await b.mouse.move(box.x + box.width * 0.1, box.y + box.height / 2)
    await b.mouse.down()
    await b.mouse.move(box.x + box.width * 0.8, box.y + box.height / 2, { steps: 6 })
    await b.mouse.up()
    const now = Number(await wave.getAttribute('aria-valuenow'))
    const max = Number(await wave.getAttribute('aria-valuemax'))
    if (!(now >= Math.floor(max * 0.6))) throw new Error(`seek landed at ${now}/${max}`)
  })

  await check('when one voice message ends, the next one plays automatically', async () => {
    const second = voiceBubbles(b).nth(1)
    await second.getByRole('button', { name: '暂停语音' }).waitFor({ timeout: 10_000 })
    await second.locator('.tg-voice__unlistened').waitFor({ state: 'detached' })
    await shot(b, 'b-autoplay-next')
    await second.getByRole('button', { name: '暂停语音' }).click()
  })

  await check('Safari fallback: without Opus support the recorder uses MP4 and the server accepts it', async () => {
    const contextS = await browser.newContext({ viewport: { width: 1280, height: 800 }, permissions: ['microphone'] })
    await contextS.addInitScript(() => {
      const original = MediaRecorder.isTypeSupported.bind(MediaRecorder)
      MediaRecorder.isTypeSupported = (type) => type.startsWith('audio/mp4') && original(type)
    })
    const s = await contextS.newPage()
    s.on('pageerror', (error) => pageErrors.push(`S: ${error.message}`))
    await s.goto(`${BASE_URL}/login`)
    await s.evaluate(
      ([session]) => localStorage.setItem('tg.session.v1', session),
      [await a.evaluate(() => localStorage.getItem('tg.session.v1'))],
    )
    await s.goto(`${BASE_URL}/chat/${chatId}`)
    await s.locator('.tg-voice-mic').waitFor()
    await hold(s, 2_000)
    const stored = await waitFor(async () => (await voiceMessages(alice.token, chatId))[2], 'mp4 voice message')
    if (stored.attachment.mime_type !== 'audio/mp4') throw new Error(`stored ${stored.attachment.mime_type}`)
    if (stored.voice.duration_ms < 1_300 || stored.voice.duration_ms > 3_000) {
      throw new Error(`mp4 duration ${stored.voice.duration_ms}`)
    }
    console.log(`   mp4 voice: ${stored.attachment.size_bytes} bytes, ${stored.voice.duration_ms} ms`)
    await voiceBubbles(b).nth(2).waitFor()
    await contextS.close()
  })

  await check('privacy: a peer who refuses voice messages gets none, with a clear message', async () => {
    await api(alice.token, 'POST', '/api/friend-requests', { user_id: bob.id })
    await api(bob.token, 'PATCH', `/api/friend-requests/${alice.id}`, { action: 'accept' })
    const direct = await api(alice.token, 'POST', '/api/direct-chats', { user_id: bob.id })
    await api(bob.token, 'PUT', '/api/users/me/privacy/voice_messages', {
      tier: 'nobody',
      allow_user_ids: [],
      deny_user_ids: [],
    })
    await a.goto(`${BASE_URL}/chat/${direct.room_id}`)
    await a.locator('.tg-voice-mic').waitFor()
    await hold(a, 1_500)
    await a.getByRole('alert').filter({ hasText: '对方设置了不接收你的语音消息' }).waitFor({ timeout: 10_000 })
    await shot(a, 'a-privacy-refused')
    if ((await voiceMessages(alice.token, direct.room_id)).length !== 0) throw new Error('refused voice stored')
  })

  if (pageErrors.length) throw new Error(`page errors:\n${pageErrors.join('\n')}`)
  console.log(`all ${step} steps passed; screenshots in ${SHOTS}`)
} finally {
  await browser.close()
}
