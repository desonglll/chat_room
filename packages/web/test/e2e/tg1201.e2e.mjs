/**
 * TG-1201 end-to-end walkthrough of the message operations against a REAL server, two accounts
 * in two browser contexts (plus a second tab for Alice's cloud draft). Not part of `bun test`.
 * Seeds itself: two fresh accounts, a friendship, a private chat and a group.
 *
 * Run (see docs/devlog/TG-1201.md):
 *   (cd packages/web && bun run build)
 *   CARGO_TARGET_DIR=<private dir> cargo run --bin server -- -p 18201 \
 *     --database-type sqlite --database /tmp/tg-1201.db
 *   BASE_URL=http://127.0.0.1:18201 node packages/web/test/e2e/tg1201.e2e.mjs
 *
 * Env: BASE_URL (required), SHOTS (default /tmp/tg-shots/TG-1201), PLAYWRIGHT (module path).
 * Every step runs even if an earlier one failed; exit code 0 = all passed.
 */
import { mkdirSync } from 'node:fs'

const BASE_URL = process.env.BASE_URL
if (!BASE_URL) throw new Error('BASE_URL is required, e.g. http://127.0.0.1:18201')
const SHOTS = process.env.SHOTS ?? '/tmp/tg-shots/TG-1201'
const { chromium } = await import(process.env.PLAYWRIGHT ?? '/tmp/pw/node_modules/playwright/index.mjs')
mkdirSync(SHOTS, { recursive: true })

const run = Date.now().toString(36)
const ALICE = { username: `alice_${run}`, password: 'correct-horse-1' }
const BOB = { username: `bob_${run}`, password: 'correct-horse-2' }
const GROUP = `操作群 ${run}`
const pageErrors = []
const failures = []
const badResponses = []
let step = 0
const allPages = []

async function check(name, page, body) {
  step += 1
  const tag = String(step).padStart(2, '0')
  try {
    await body()
    await page.waitForTimeout(1300)
    await page.screenshot({ path: `${SHOTS}/${tag}-${name}.png` })
    console.log(`ok ${step} ${name}`)
  } catch (error) {
    failures.push(`${name}: ${String(error).split('\n')[0]}`)
    console.log(`FAIL ${step} ${name} — ${String(error).split('\n')[0]}`)
    for (const [index, other] of allPages.entries())
      await other.screenshot({ path: `${SHOTS}/${tag}-${name}-FAIL-${'AB'[index]}.png` }).catch(() => {})
  }
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

async function login(page, user) {
  await page.goto(`${BASE_URL}/login`)
  await page.locator('input[name="username"]').fill(user.username)
  await page.locator('input[name="password"]').fill(user.password)
  await page.getByRole('button', { name: '登录', exact: true }).click()
  await page.getByRole('button', { name: '主菜单' }).first().waitFor()
}

const bubble = (page, text) => page.locator('.tg-message', { hasText: text }).last()
/** The message itself, not a later reply whose quote repeats its text. */
const original = (page, text) =>
  page
    .locator('.tg-message', { hasText: text })
    .filter({ hasNot: page.locator('.tg-bubble__reply') })
    .last()
const menuOn = async (page, locator) => {
  await locator.locator('.tg-message__column').first().click({ button: 'right' })
}
const openChat = async (page, chatId) => {
  await page.goto(`${BASE_URL}/chat/${encodeURIComponent(chatId)}`)
  await page.getByLabel('消息内容').waitFor()
}
const send = async (page, text) => {
  const input = page.getByLabel('消息内容')
  await input.fill(text)
  await input.press('Enter')
}
const sendMenu = async (page, text, item) => {
  await page.getByLabel('消息内容').fill(text)
  await page.locator('.tg-compose__send-menu button').last().click({ button: 'right' })
  await page.getByRole('menuitem', { name: item }).click()
}

const browser = await chromium.launch()
try {
  const contexts = [
    await browser.newContext({ viewport: { width: 1280, height: 820 } }),
    await browser.newContext({ viewport: { width: 1280, height: 820 } }),
  ]
  const [a, b] = await Promise.all(contexts.map((context) => context.newPage()))
  allPages.push(a, b)
  for (const [who, page] of [
    ['A', a],
    ['B', b],
  ]) {
    page.on('pageerror', (error) => pageErrors.push(`${who}: ${error.message}`))
    page.on('response', (response) => {
      if (response.status() >= 400)
        badResponses.push(
          `${who} ${response.status()} ${response.request().method()} ${new URL(response.url()).pathname}`,
        )
    })
    page.on('console', (message) => {
      if (message.type() === 'error' && !message.text().includes('status of 503'))
        pageErrors.push(`${who} console: ${message.text()}`)
    })
  }

  // ---- seed: accounts, friendship, private chat, group
  const alice = await register(a, ALICE)
  const bob = await register(b, BOB)
  await api(alice.token, 'POST', '/api/friend-requests', { user_id: bob.id })
  await api(bob.token, 'PATCH', `/api/friend-requests/${alice.id}`, { action: 'accept' })
  const direct = await api(alice.token, 'POST', '/api/direct-chats', { user_id: bob.id })
  const dmId = direct.room_id
  await a.getByRole('button', { name: '主菜单' }).first().click()
  await a.getByRole('menuitem', { name: '新建群组' }).click()
  await a.getByLabel('群组名称').fill(GROUP)
  await a.getByRole('button', { name: '创建', exact: true }).click()
  await a.waitForURL(/\/chat\//)
  const groupId = decodeURIComponent(a.url().split('/chat/')[1].split(/[?#]/)[0])
  await api(bob.token, 'POST', `/api/chats/${groupId}/join-requests`, { password: null })
  await openChat(a, groupId)
  await openChat(b, groupId)

  await check('send-and-receive', b, async () => {
    await send(a, '水果清单：苹果 香蕉 橙子')
    await original(b, '水果清单').waitFor({ timeout: 10_000 })
  })

  await check('send-flight-effect', a, async () => {
    const fresh = await original(a, '水果清单').getAttribute('data-fresh')
    if (fresh === null) throw new Error('own new message has no data-fresh (send flight)')
  })

  await check('quote-fragment-reply', a, async () => {
    const text = original(b, '水果清单').locator('.tg-bubble__text, .tg-message__text').first()
    await text.evaluate((node) => {
      const walker = document.createTreeWalker(node, NodeFilter.SHOW_TEXT)
      let textNode
      while ((textNode = walker.nextNode())) {
        const at = textNode.data.indexOf('香蕉')
        if (at >= 0) {
          const range = document.createRange()
          range.setStart(textNode, at)
          range.setEnd(textNode, at + 2)
          const selection = getSelection()
          selection.removeAllRanges()
          selection.addRange(range)
          return
        }
      }
      throw new Error('香蕉 not found in bubble text')
    })
    // Right-click ON the selection, as a person does (a right click elsewhere collapses it).
    const rect = await b.evaluate(() => {
      const box = getSelection().getRangeAt(0).getBoundingClientRect()
      return { x: box.x + box.width / 2, y: box.y + box.height / 2 }
    })
    await b.mouse.click(rect.x, rect.y, { button: 'right' })
    await b.getByRole('menuitem', { name: '引用', exact: true }).click()
    await b.locator('.tg-compose').getByText('「香蕉」').waitFor({ timeout: 3000 })
    await send(b, '只要香蕉')
    const reply = bubble(a, '只要香蕉')
    await reply.waitFor({ timeout: 10_000 })
    const quoted = await reply.locator('.tg-bubble__reply-text').innerText()
    if (!quoted.includes('香蕉') || quoted.includes('苹果'))
      throw new Error(`quote shows «${quoted}», expected only the fragment «香蕉»`)
  })

  await check('reply-in-another-chat', a, async () => {
    await menuOn(b, original(b, '水果清单'))
    await b.getByRole('menuitem', { name: '在其他聊天中回复' }).click()
    await b.getByRole('dialog').locator('.tg-forward__row').filter({ hasText: ALICE.username }).first().click()
    await b.waitForURL(new RegExp(encodeURIComponent(dmId)))
    await send(b, '跨聊天回复')
    await openChat(a, dmId)
    const reply = bubble(a, '跨聊天回复')
    await reply.waitFor({ timeout: 10_000 })
    await reply.locator('.tg-bubble__reply-chat', { hasText: GROUP }).waitFor()
  })

  await check('cross-chat-reply-jumps-to-source', a, async () => {
    await bubble(a, '跨聊天回复').locator('.tg-bubble__reply').click()
    await a.waitForURL(new RegExp(encodeURIComponent(groupId)), { timeout: 6000 })
    await original(a, '水果清单').waitFor()
  })

  await check('forward-to-private-chat', b, async () => {
    await openChat(a, groupId)
    await menuOn(a, original(a, '水果清单'))
    await a.getByRole('menuitem', { name: '转发' }).click()
    await a.getByRole('dialog').locator('.tg-forward__row').filter({ hasText: BOB.username }).first().click()
    await a.waitForURL(new RegExp(encodeURIComponent(dmId)))
    // The group's composer may still be unmounting: wait for the DM's forward bar first.
    await a.locator('.tg-compose').getByText('转发消息').waitFor()
    await a.getByLabel('消息内容').press('Enter')
    await openChat(b, dmId)
    await b.locator('.tg-message', { hasText: '水果清单' }).locator('.tg-bubble__forward').waitFor({ timeout: 10_000 })
  })

  await check('edit', b, async () => {
    await openChat(a, groupId)
    await openChat(b, groupId)
    await send(a, '待修改的消息')
    await original(a, '待修改的消息').waitFor()
    await menuOn(a, original(a, '待修改的消息'))
    await a.getByRole('menuitem', { name: '编辑' }).click()
    const input = a.getByLabel('消息内容')
    await input.fill('已修改的消息')
    await input.press('Enter')
    await original(b, '已修改的消息').locator('.tg-bubble__meta .tg-bubble__edited').waitFor({ timeout: 10_000 })
  })

  await check('react-with-burst', a, async () => {
    const target = original(b, '已修改的消息')
    await target.hover()
    await target.getByRole('button', { name: '添加回应' }).click()
    await b.getByRole('button', { name: '回应 👍' }).click()
    await original(a, '已修改的消息').locator('.tg-bubble__reaction', { hasText: '👍' }).waitFor({ timeout: 10_000 })
    const burst = await original(b, '已修改的消息').locator('.tg-bubble__reaction[data-burst]').count()
    if (burst === 0) throw new Error('own reaction did not burst')
  })

  await check('recall', b, async () => {
    await menuOn(a, original(a, '已修改的消息'))
    await a.getByRole('menuitem', { name: '删除' }).click()
    await a.getByRole('dialog').getByRole('button', { name: '删除', exact: true }).click()
    await b.locator('.tg-bubble__deleted').first().waitFor({ timeout: 10_000 })
  })

  await check('big-emoji', b, async () => {
    await send(a, '🎉')
    await b.locator('.tg-message .tg-big-emoji').last().waitFor({ timeout: 10_000 })
  })

  await check('schedule-then-send-now', b, async () => {
    await sendMenu(a, '定时消息一号', '定时发送')
    await a.getByRole('dialog').getByRole('button', { name: '1 小时后' }).click()
    const dialog = a.getByRole('dialog')
    const confirm = dialog.getByRole('button', { name: /定时发送|发送|保存/ }).last()
    if (await confirm.isVisible().catch(() => false)) await confirm.click().catch(() => {})
    const entry = a.getByRole('button', { name: /定时消息（1）/ })
    await entry.waitFor({ timeout: 6000 })
    if ((await a.getByLabel('消息内容').inputValue()) !== '') throw new Error('composer not cleared after scheduling')
    if ((await b.locator('.tg-message', { hasText: '定时消息一号' }).count()) > 0)
      throw new Error('scheduled message delivered immediately')
    await entry.click()
    await a.locator('.tg-scheduled__item', { hasText: '定时消息一号' }).waitFor()
    await a
      .locator('.tg-scheduled__item', { hasText: '定时消息一号' })
      .getByRole('button', { name: '立即发送' })
      .click()
    await bubble(b, '定时消息一号').waitFor({ timeout: 10_000 })
    await a.keyboard.press('Escape')
  })

  await check('schedule-then-delete', a, async () => {
    await sendMenu(a, '定时消息二号', '定时发送')
    await a.getByRole('dialog').getByRole('button', { name: '1 小时后' }).click()
    const dialog = a.getByRole('dialog')
    const confirm = dialog.getByRole('button', { name: /定时发送|发送|保存/ }).last()
    if (await confirm.isVisible().catch(() => false)) await confirm.click().catch(() => {})
    await a.getByRole('button', { name: /定时消息（1）/ }).click()
    const item = a.locator('.tg-scheduled__item', { hasText: '定时消息二号' })
    await item.getByRole('button', { name: '删除' }).click()
    await a.getByRole('button', { name: '确认删除' }).click()
    await a.locator('.tg-scheduled__empty').waitFor({ timeout: 6000 })
    await a.keyboard.press('Escape')
    await a.waitForTimeout(500)
    if (await a.getByRole('button', { name: /定时消息（/ }).count()) throw new Error('calendar entry still shown')
  })

  await check('silent-send', b, async () => {
    await sendMenu(a, '悄悄说一句', '静默发送')
    await bubble(b, '悄悄说一句').waitFor({ timeout: 10_000 })
    if ((await a.getByLabel('消息内容').inputValue()) !== '') throw new Error('composer not cleared after silent send')
  })

  await check('translate-hidden-without-provider', a, async () => {
    const availability = await api(alice.token, 'GET', '/api/translation')
    await menuOn(a, original(a, '悄悄说一句'))
    const shown = await a.getByRole('menuitem', { name: '翻译' }).count()
    await a.keyboard.press('Escape')
    if (availability.available !== shown > 0)
      throw new Error(`translation available=${availability.available} but menu shown=${shown > 0}`)
  })

  await check('share-contact-card', b, async () => {
    await a.getByRole('button', { name: '添加附件' }).click()
    await a.getByRole('menuitem', { name: '联系人' }).click()
    await a.locator('.tg-contact-picker__row', { hasText: BOB.username }).first().click()
    const confirm = a.getByRole('dialog').getByRole('button', { name: /发送|分享/ })
    if (await confirm.count()) await confirm.last().click()
    await b.locator('.tg-contact-card').last().waitFor({ timeout: 10_000 })
  })

  await check('auto-delete-timer', b, async () => {
    await a.getByRole('button', { name: '查看会话信息' }).click()
    await a.waitForTimeout(800)
    await a.locator('.tg-chatinfo__autodelete-button').click()
    await a
      .getByRole('menuitemradio', { name: '1 天' })
      .or(a.getByRole('menuitem', { name: '1 天' }))
      .or(a.getByRole('option', { name: '1 天' }))
      .first()
      .click()
    await a.locator('.tg-chatinfo__autodelete-button', { hasText: '1 天' }).waitFor({ timeout: 6000 })
    await b.getByRole('button', { name: '查看会话信息' }).click()
    await b.locator('.tg-chatinfo__autodelete-button', { hasText: '1 天' }).waitFor({ timeout: 6000 })
    await a.keyboard.press('Escape')
    await b.keyboard.press('Escape')
  })

  await check('cloud-draft-second-tab', a, async () => {
    await openChat(a, dmId)
    await a.getByLabel('消息内容').pressSequentially('云端草稿内容', { delay: 30 })
    await a.waitForTimeout(2500)
    const second = await (await browser.newContext({ viewport: { width: 1280, height: 820 } })).newPage()
    second.on('pageerror', (error) => pageErrors.push(`A2: ${error.message}`))
    await login(second, ALICE)
    await openChat(second, dmId)
    await second.waitForFunction(
      () => document.querySelector('[aria-label="消息内容"]')?.value === '云端草稿内容',
      null,
      { timeout: 8000 },
    )
    await second.screenshot({ path: `${SHOTS}/draft-second-tab.png` })
    await a.getByLabel('消息内容').fill('')
  })
} finally {
  await browser.close()
}

// Known and owned elsewhere (reported in docs/devlog/TG-1201.md): «操作日志» probes
// /audit-events for every group member and a non-manager gets 403 (chatInfo/ChatInfoPanel.tsx).
// Drop this filter once the probe is gated on the member's role.
const KNOWN = /\/audit-events$/
const unexpected = [...new Set(badResponses)].filter((line) => !KNOWN.test(line))
const errors = pageErrors.filter((line) => !line.includes('status of 403') || unexpected.length > 0)
console.log('page errors', errors)
console.log('responses >= 400', [...new Set(badResponses)])
console.log(failures.length ? `FAILED ${failures.length}:\n  ${failures.join('\n  ')}` : 'ALL PASSED')
if (failures.length || errors.length || unexpected.length) process.exitCode = 1
