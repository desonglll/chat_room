/**
 * TG-100 end-to-end check of the wired M1 chat against a REAL server, two accounts in two
 * browser contexts. Not part of `bun test` (needs a server, Chromium and ~1 minute).
 *
 * Run (see docs/devlog/TG-100.md "E2E"):
 *   (cd packages/web && bun run build)            # the bundle the server embeds
 *   CARGO_TARGET_DIR=<private dir> cargo run --bin server -- -p 3917 \
 *     --database-type sqlite --database /tmp/tg100-e2e.db
 *   BASE_URL=http://127.0.0.1:3917 node packages/web/test/e2e/m1-chat.e2e.mjs
 *
 * Env: BASE_URL (required), SHOTS (default /tmp/tg-shots/TG-100), PLAYWRIGHT (module path,
 * default /tmp/pw/node_modules/playwright/index.mjs), SEED (older messages, default 300).
 * Exit code 0 = every step passed; each step prints `ok <n> <name>` or throws.
 */
import { mkdirSync } from 'node:fs'

const BASE_URL = process.env.BASE_URL
if (!BASE_URL) throw new Error('BASE_URL is required, e.g. http://127.0.0.1:3917')
const SHOTS = process.env.SHOTS ?? '/tmp/tg-shots/TG-100'
const SEED = Number(process.env.SEED ?? 300)
const { chromium } = await import(process.env.PLAYWRIGHT ?? '/tmp/pw/node_modules/playwright/index.mjs')
mkdirSync(SHOTS, { recursive: true })

const run = Date.now().toString(36)
const ALICE = { username: `alice_${run}`, password: 'correct-horse-1' }
const BOB = { username: `bob_${run}`, password: 'correct-horse-2' }
const GROUP = `M1 群 ${run}`
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
  return response.status === 204 ? null : response.json()
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

/** Seed older history over the chat socket (there is no REST send), paced under the rate limit. */
async function seedHistory(token, chatId, count) {
  const ws = new WebSocket(`${BASE_URL.replace(/^http/, 'ws')}/ws/${chatId}`)
  let acked = 0
  await new Promise((resolve, reject) => {
    ws.onerror = () => reject(new Error('seed socket failed'))
    ws.onopen = () => ws.send(JSON.stringify({ type: 'join', token }))
    ws.onmessage = async (event) => {
      const frame = JSON.parse(event.data)
      if (frame.type === 'auth_fail') reject(new Error(`seed auth_fail: ${frame.reason}`))
      if (frame.type === 'history_complete') {
        for (let n = 1; n <= count; n += 1) {
          ws.send(JSON.stringify({ type: 'message', content: `seed ${n}`, client_message_id: crypto.randomUUID() }))
          if (n % 20 === 0) await new Promise((r) => setTimeout(r, 150))
        }
      }
      if (frame.type === 'broadcast' && frame.content.startsWith('seed ')) {
        acked += 1
        if (acked === count) resolve()
      }
    }
  })
  ws.close()
}

const bubble = (page, text) => page.locator('.tg-message', { hasText: text }).last()
/** The message itself, not a later reply whose quote repeats its text. */
const original = (page, text) =>
  page
    .locator('.tg-message', { hasText: text })
    .filter({ hasNot: page.locator('.tg-bubble__reply') })
    .last()

const browser = await chromium.launch()
try {
  const contextA = await browser.newContext({ viewport: { width: 1280, height: 800 } })
  const contextB = await browser.newContext({ viewport: { width: 1280, height: 800 } })
  const a = await contextA.newPage()
  const b = await contextB.newPage()
  for (const [who, page] of [
    ['A', a],
    ['B', b],
  ]) {
    page.on('pageerror', (error) => pageErrors.push(`${who}: ${error.message}`))
    page.on('console', (message) => {
      if (message.type() === 'error') pageErrors.push(`${who} console: ${message.text()}`)
    })
  }

  let alice
  let bob
  let chatId = ''
  await check('register both accounts in the UI', async () => {
    alice = await register(a, ALICE)
    bob = await register(b, BOB)
  })

  await check('A creates a group from the main menu', async () => {
    await a.getByRole('button', { name: '主菜单' }).first().click()
    await a.getByRole('menuitem', { name: '新建群组' }).click()
    await a.getByLabel('群组名称').fill(GROUP)
    await a.getByRole('button', { name: '创建', exact: true }).click()
    await a.waitForURL(/\/chat\//)
    chatId = decodeURIComponent(a.url().split('/chat/')[1].split(/[?#]/)[0])
  })

  await check(`B joins; ${SEED} older messages are seeded`, async () => {
    await api(bob.token, 'POST', `/api/chats/${chatId}/join-requests`, { password: null })
    await seedHistory(alice.token, chatId, SEED)
    await a.reload()
    await b.goto(`${BASE_URL}/chat/${chatId}`)
    await bubble(b, `seed ${SEED}`).waitFor()
    await bubble(a, `seed ${SEED}`).waitFor()
    await shot(b, 'b-opened')
  })

  await check('A sends text, B receives it live', async () => {
    const input = a.getByLabel('消息内容')
    await input.fill('你好，Bob！')
    await input.press('Enter')
    await bubble(b, '你好，Bob！').waitFor({ timeout: 10_000 })
    await shot(b, 'b-received')
  })

  await check('B replies from the hover bar; the reply quote shows on both sides', async () => {
    const target = bubble(b, '你好，Bob！')
    await target.hover()
    await target.getByRole('button', { name: '回复' }).click()
    const input = b.getByLabel('消息内容')
    await input.fill('收到，Alice')
    await input.press('Enter')
    const reply = bubble(a, '收到，Alice')
    await reply.waitFor()
    await reply.locator('.tg-bubble__reply', { hasText: '你好，Bob！' }).waitFor()
    await bubble(b, '收到，Alice').locator('.tg-bubble__reply').waitFor()
    await shot(a, 'a-reply-quote')
  })

  await check('A edits the message from the context menu; B sees the edited marker', async () => {
    await original(a, '你好，Bob！').locator('.tg-message__column').first().click({ button: 'right' })
    await a.getByRole('menuitem', { name: '编辑' }).click()
    const input = a.getByLabel('消息内容')
    await input.fill('你好，Bob！（已修改）')
    await input.press('Enter')
    const edited = original(b, '（已修改）')
    await edited.waitFor()
    await edited.locator('.tg-bubble__meta .tg-bubble__edited').waitFor()
    await shot(b, 'b-edited')
  })

  await check('B reacts; A sees the reaction chip', async () => {
    const target = original(b, '（已修改）')
    await target.hover()
    await target.getByRole('button', { name: '添加回应' }).click()
    await b.getByRole('button', { name: '回应 👍' }).click()
    await original(a, '（已修改）').locator('.tg-bubble__reaction', { hasText: '👍' }).waitFor()
    await shot(a, 'a-reaction')
  })

  await check('B marks it read: A sees the double tick', async () => {
    await original(a, '（已修改）').locator('.tg-bubble__meta .tg-bubble__delivery[data-delivery="read"]').waitFor()
  })

  await check('A deletes the message (confirm dialog); B sees it recalled', async () => {
    await original(a, '（已修改）').locator('.tg-message__column').first().click({ button: 'right' })
    await a.getByRole('menuitem', { name: '删除' }).click()
    await a.getByRole('dialog').getByRole('button', { name: '删除', exact: true }).click()
    await b.locator('.tg-bubble__deleted').first().waitFor()
    await shot(b, 'b-recalled')
  })

  await check('typing in A shows the typing line in B’s header', async () => {
    await a.getByLabel('消息内容').pressSequentially('正在打字', { delay: 40 })
    await b.locator('.tg-chat__header .tg-presence-status[data-state="typing"]').waitFor({ timeout: 6_000 })
    await shot(b, 'b-typing')
    await a.getByLabel('消息内容').fill('')
  })

  await check('A sends an image; B receives it and opens it in the media viewer', async () => {
    // A 64×48 PNG drawn in the page, so the test ships no binary fixture.
    const png = await a.evaluate(async () => {
      const canvas = document.createElement('canvas')
      canvas.width = 64
      canvas.height = 48
      const context = canvas.getContext('2d')
      context.fillStyle = '#3390ec'
      context.fillRect(0, 0, 64, 48)
      context.fillStyle = '#ffffff'
      context.fillRect(16, 12, 32, 24)
      return canvas.toDataURL('image/png').split(',')[1]
    })
    const chooser = a.waitForEvent('filechooser')
    await a.getByRole('button', { name: '添加附件' }).click()
    await a.getByRole('menuitem', { name: '图片或视频' }).click()
    await (await chooser).setFiles({ name: 'square.png', mimeType: 'image/png', buffer: Buffer.from(png, 'base64') })
    await a.getByRole('dialog').getByRole('button', { name: '发送', exact: true }).click()
    const image = b.locator('.tg-bubble__image').last()
    await image.waitFor({ timeout: 15_000 })
    // The image grows its row after loading (no dimensions on the wire): the list must
    // still end at the bottom on both sides.
    for (const page of [a, b]) {
      await page.waitForFunction(() => {
        const scroller = document.querySelector('.tg-mlist__scroller')
        return scroller !== null && scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < 2
      })
    }
    await shot(b, 'b-image')
    await b
      .getByRole('button', { name: /查看图片 square\.png/ })
      .last()
      .click()
    await b.locator('.tg-mv').waitFor()
    await b.waitForTimeout(600)
    await shot(b, 'b-viewer')
    await b.keyboard.press('Escape')
    await b.locator('.tg-mv').waitFor({ state: 'detached', timeout: 5_000 })
  })

  await check('A selects two messages, forwards them through the picker; B sees the forward header', async () => {
    await original(a, 'seed 300').locator('.tg-message__column').first().click({ button: 'right' })
    await a.getByRole('menuitem', { name: '选择' }).click()
    await original(a, 'seed 299').click()
    await a.getByText('已选 2 条').waitFor()
    await shot(a, 'a-selection')
    await a.getByRole('toolbar', { name: '已选消息' }).getByRole('button', { name: '转发' }).click()
    await a.getByRole('dialog').getByRole('button', { name: GROUP }).click()
    await a.locator('.tg-compose').getByText(/转发/).first().waitFor()
    await shot(a, 'a-forward-bar')
    await a.getByLabel('消息内容').press('Enter')
    await b.locator('.tg-bubble__forward').first().waitFor({ timeout: 10_000 })
    await shot(b, 'b-forwarded')
  })

  await check('narrow screen: the back button sits in the chat header and returns to the list', async () => {
    await b.setViewportSize({ width: 390, height: 780 })
    const back = b.locator('.tg-chat__header').getByRole('button', { name: '返回会话列表' })
    await back.waitFor()
    await shot(b, 'b-mobile-chat')
    await back.click()
    await b.waitForURL(`${BASE_URL}/`)
    await b.setViewportSize({ width: 1280, height: 800 })
    await b.goto(`${BASE_URL}/chat/${chatId}`)
  })

  await check('reload keeps the history', async () => {
    await b.reload()
    await bubble(b, '收到，Alice').waitFor()
    await b.locator('.tg-bubble__image').last().waitFor()
  })

  await check('scrolling up loads older pages down to the first seeded message', async () => {
    const scroller = b.locator('.tg-mlist__scroller')
    const oldestShown = () =>
      b.$$eval('.tg-message', (rows) =>
        Math.min(
          ...rows.map((row) => {
            // Text only: the sender name and the time (meta + its spacer) would glue digits on.
            const text = row.cloneNode(true)
            for (const noise of text.querySelectorAll(
              '.tg-bubble__meta, .tg-bubble__meta-spacer, .tg-bubble__sender',
            )) {
              noise.remove()
            }
            return Number(/^\s*seed (\d+)\s*$/.exec(text.textContent ?? '')?.[1] ?? Infinity)
          }),
        ),
      )
    const box = await scroller.boundingBox()
    await b.mouse.move(box.x + box.width / 2, box.y + box.height / 2)
    let oldest = await oldestShown()
    for (let attempt = 0; attempt < 80 && oldest > 1; attempt += 1) {
      // Real wheel input: the list's prepend-anchor guard (TG-101) stands down on wheel,
      // whereas a programmatic scrollBy right after a prepend is "corrected" back.
      await b.mouse.wheel(0, -2500)
      await b.waitForTimeout(250)
      oldest = await oldestShown()
    }
    if (oldest !== 1) throw new Error(`oldest seed on screen after scrolling: ${oldest}`)
    await shot(b, 'b-oldest')
  })

  await check('no page errors in either browser', async () => {
    const real = pageErrors.filter((line) => !/favicon|ResizeObserver loop/.test(line))
    if (real.length) throw new Error(`page errors:\n${real.join('\n')}`)
  })
  console.log(`all ${step} steps passed; screenshots in ${SHOTS}`)
} finally {
  await browser.close()
}
