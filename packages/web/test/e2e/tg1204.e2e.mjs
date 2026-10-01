/**
 * TG-1204 walkthrough of settings and organisation against a REAL server, two accounts in two
 * browser contexts. Self-seeding (fresh accounts per run), so it re-runs on any database.
 * Not part of `bun test` (needs a server and Chromium).
 *
 * Run (see docs/devlog/TG-1204.md):
 *   server --no-web -p 18204 --database-type sqlite --database /tmp/tg-1204.db   # API
 *   (cd packages/web && bun run build && vite preview …)                          # this worktree's UI, proxying /api + /ws
 *   BASE_URL=http://127.0.0.1:15204 node packages/web/test/e2e/tg1204.e2e.mjs
 *
 * Env: BASE_URL (required), SHOTS (default /tmp/tg-shots/TG-1204), PLAYWRIGHT (module path).
 * Exit code 0 = every step passed; each step prints `ok <n> <name>` or `FAIL <n> <name>: …`.
 */
import { mkdirSync, writeFileSync } from 'node:fs'

const BASE = process.env.BASE_URL
if (!BASE) throw new Error('BASE_URL is required, e.g. http://127.0.0.1:15204')
const SHOTS = process.env.SHOTS ?? '/tmp/tg-shots/TG-1204'
const { chromium } = await import(process.env.PLAYWRIGHT ?? '/tmp/pw/node_modules/playwright/index.mjs')
mkdirSync(SHOTS, { recursive: true })

const run = Date.now().toString(36)
const PASSWORD = 'correct-horse-9'
const CJK = /[㐀-鿿＀-￯　-〿]/

async function api(token, method, path, body) {
  const response = await fetch(`${BASE}${path}`, {
    method,
    headers: { 'content-type': 'application/json', ...(token ? { authorization: `Bearer ${token}` } : {}) },
    body: body === undefined ? undefined : JSON.stringify(body),
  })
  const text = await response.text()
  if (!response.ok) throw new Error(`${method} ${path} → ${response.status} ${text}`)
  return text ? JSON.parse(text) : null
}

async function register(name) {
  const username = `${name}_${run}`
  const session = await api(null, 'POST', '/api/users/register', { username, password: PASSWORD })
  return { ...session, username, password: PASSWORD }
}

// ---- seed: two friends, a private chat with a message each way, a group
const alice = await register('alice')
const bob = await register('bob')
await api(alice.token, 'POST', '/api/friend-requests', { user_id: bob.user.id })
await api(bob.token, 'PATCH', `/api/friend-requests/${alice.user.id}`, { action: 'accept' })
const dm = await api(alice.token, 'POST', '/api/direct-chats', { user_id: bob.user.id })
const dmId = dm.room_id ?? dm.id
const group = await api(alice.token, 'POST', '/api/chats', { title: `Ops ${run}` })
const groupId = group.id ?? group.room_id
const groupTitle = `Ops ${run}`

const browser = await chromium.launch()
const failures = []
const pageErrors = []
let n = 0

async function newPage(width = 1440, height = 900) {
  const page = await (await browser.newContext({ viewport: { width, height } })).newPage()
  page.on('pageerror', (error) => pageErrors.push(String(error)))
  page.on('response', (response) => {
    const path = new URL(response.url()).pathname
    // 403 on the admin probe and 404 for a missing avatar are answers, not faults.
    if (response.status() >= 400 && !path.startsWith('/api/admin') && !path.endsWith('/avatar'))
      pageErrors.push(`${response.status()} ${response.request().method()} ${path}`)
  })
  return page
}

async function login(page, account) {
  await page.goto(`${BASE}/login`)
  await page.locator('input[name="username"]').fill(account.username)
  await page.locator('input[name="password"]').fill(account.password)
  await page.getByRole('button', { name: /^(登录|Log in|Sign in)$/ }).click()
  await page
    .getByRole('button', { name: /^(主菜单|Main menu)$/ })
    .first()
    .waitFor()
}

async function step(page, name, body) {
  n += 1
  const tag = `${String(n).padStart(2, '0')}-${name}`
  try {
    await body()
    await page.waitForTimeout(1300)
    await page.screenshot({ path: `${SHOTS}/${tag}.png` })
    console.log(`ok ${n} ${name}`)
  } catch (error) {
    failures.push(`${name}: ${String(error).split('\n')[0]}`)
    console.log(`FAIL ${n} ${name}: ${String(error).split('\n')[0]}`)
    await page.screenshot({ path: `${SHOTS}/${tag}-FAIL.png` }).catch(() => {})
  }
}

const menu = (page) => page.getByRole('button', { name: /^(主菜单|Main menu)$/ }).first()
async function openSettings(page, item) {
  // Close whatever a previous step left open (a dialog, the settings panel itself).
  for (let i = 0; i < 3 && (await page.locator('[role="dialog"], .tg-settings').count()); i++)
    await page.keyboard.press('Escape')
  await menu(page).click()
  await page.getByRole('menuitem', { name: /^(设置|Settings)$/ }).click()
  await page.locator('.tg-settings').first().waitFor()
  await page.waitForTimeout(700)
  if (item) {
    await page.locator('.tg-settings').getByText(item, { exact: true }).first().click()
    await page.waitForTimeout(900)
  }
}
const chatRow = (page, title) => page.locator('.tg-chatrow', { hasText: title }).first()
const composer = (page) => page.locator('.tg-compose__input')

async function send(page, chatId, text) {
  await page.goto(`${BASE}/chat/${chatId}`)
  await composer(page).fill(text)
  await page.keyboard.press('Enter')
  await page.locator('.tg-message', { hasText: text }).first().waitFor({ timeout: 6000 })
}

const a = await newPage()
const b = await newPage()
await login(a, alice)
await login(b, bob)

// ---- messages to organise and search
await step(a, 'seed-messages', async () => {
  await send(a, dmId, `hello bob ${run}`)
  await send(b, dmId, `hello alice ${run}`)
  await send(a, groupId, `ops note ${run}`)
})

// ---- archive
await step(a, 'archive-chat', async () => {
  await a.goto(`${BASE}/`)
  await chatRow(a, groupTitle).click({ button: 'right' })
  await a.getByRole('menuitem', { name: '归档' }).click()
  await a.locator('.tg-archive').first().waitFor({ timeout: 5000 })
  if (await chatRow(a, groupTitle).isVisible()) throw new Error('archived chat still in the main list')
})
await step(a, 'archive-open-and-restore', async () => {
  await a.locator('.tg-archive').first().click()
  await chatRow(a, groupTitle).waitFor({ timeout: 5000 })
  await chatRow(a, groupTitle).click({ button: 'right' })
  await a.getByRole('menuitem', { name: '取消归档' }).click()
  await a.goto(`${BASE}/`)
  await chatRow(a, groupTitle).waitFor({ timeout: 5000 })
})

// ---- folders
await step(a, 'folder-create', async () => {
  await openSettings(a, '聊天文件夹')
  await a.getByRole('button', { name: '新建文件夹' }).click()
  await a
    .getByRole('textbox', { name: /文件夹名称|名称/ })
    .first()
    .fill('Work')
  await a.getByRole('checkbox', { name: '群组' }).first().check()
  await a.getByRole('button', { name: '保存' }).click()
  await a.locator('.tg-settings').getByText('Work').first().waitFor({ timeout: 5000 })
})
await step(a, 'folder-filters-list', async () => {
  await a.keyboard.press('Escape')
  await a.goto(`${BASE}/`)
  await a.getByRole('tab', { name: /^Work/ }).click()
  await chatRow(a, groupTitle).waitFor({ timeout: 5000 })
  if (await chatRow(a, bob.username).isVisible()) throw new Error('the private chat shows in a groups-only folder')
  await a.getByRole('tab', { name: /^全部/ }).click()
})

// ---- saved messages
await step(a, 'saved-messages-note', async () => {
  await a.goto(`${BASE}/`)
  await a.getByRole('button', { name: '收藏夹' }).or(a.locator('.tg-saved-row')).first().click()
  await a.getByRole('textbox', { name: '写一条笔记' }).or(a.getByPlaceholder('写一条笔记')).first().fill(`note ${run}`)
  await a.keyboard.press('Enter')
  await a.getByText(`note ${run}`).first().waitFor({ timeout: 5000 })
})

// ---- global search
await step(a, 'search-messages-tab', async () => {
  await a.goto(`${BASE}/`)
  const box = a.getByRole('searchbox').or(a.getByPlaceholder(/搜索/)).first()
  await box.fill(`hello`)
  await a.getByRole('tab', { name: /^消息/ }).click()
  await a
    .locator('.tg-search-results__body', { hasText: `hello alice ${run}` })
    .first()
    .waitFor({ timeout: 6000 })
  const sender = await a.locator('.tg-search-results__sender').first().innerText()
  if (!sender.endsWith('：')) throw new Error(`zh sender prefix is "${sender}"`)
  await box.press('Enter')
})

// ---- appearance: accent and a per-chat wallpaper
await step(a, 'accent-color', async () => {
  await openSettings(a, '外观')
  const before = await a.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue('--tg-accent'))
  await a.getByRole('button', { name: /绿/ }).first().click()
  await a.waitForTimeout(400)
  const accent = await a.evaluate(() =>
    getComputedStyle(document.querySelector('.tg-settings')).getPropertyValue('--tg-accent'),
  )
  if (accent.trim() === before.trim()) throw new Error(`accent unchanged (${accent})`)
  const colour = await a.locator('input[type=color]').first().inputValue()
  if (colour === '#000000') throw new Error('colour input starts black')
  await a.getByRole('button', { name: /蓝/ }).first().click()
})
await step(a, 'wallpaper-per-chat', async () => {
  await a.goto(`${BASE}/chat/${dmId}`)
  await composer(a).waitFor()
  await openSettings(a, '外观')
  await a.getByRole('radio', { name: '当前聊天' }).check()
  await a.locator('[data-tg-wallpaper="sunset"]').click()
  await a.getByText('已应用').or(a.getByRole('status')).first().waitFor({ timeout: 5000 })
  await a.keyboard.press('Escape')
  await a.goto(`${BASE}/chat/${dmId}`)
  await a.locator('.tg-wallpaper').first().waitFor({ timeout: 5000 })
  const inDm = await a
    .locator('.tg-wallpaper')
    .first()
    .evaluate((el) => el.outerHTML.slice(0, 200))
  await a.goto(`${BASE}/chat/${groupId}`)
  await composer(a).waitFor()
  await a.waitForTimeout(600)
  const inGroup = (await a.locator('.tg-wallpaper').count())
    ? await a
        .locator('.tg-wallpaper')
        .first()
        .evaluate((el) => el.outerHTML.slice(0, 200))
    : ''
  if (inDm === inGroup) throw new Error('the per-chat wallpaper also shows in another chat')
})

// ---- notification exception set from chat info, listed in settings
await step(a, 'notification-exception', async () => {
  await a.goto(`${BASE}/chat/${groupId}`)
  await a.getByRole('button', { name: '查看会话信息' }).click()
  await a.waitForTimeout(800)
  await a
    .locator('select', { has: a.locator('option', { hasText: '风铃' }) })
    .first()
    .selectOption({ label: '风铃' })
  await a.waitForTimeout(600)
  await a.keyboard.press('Escape')
  await openSettings(a, '通知与声音')
  await a.locator('.tg-notify-settings__exceptions li', { hasText: groupTitle }).waitFor({ timeout: 5000 })
})

// ---- privacy: alice hides last seen; bob's contacts page no longer shows her exact status
await step(a, 'privacy-last-seen-nobody', async () => {
  await openSettings(a, '隐私与安全')
  await a.getByText('最后上线时间').first().click()
  await a.getByRole('radio', { name: '没有人' }).check()
  await a.waitForTimeout(800)
})
await step(b, 'privacy-observed-by-bob', async () => {
  await b.goto(`${BASE}/contacts`)
  const row = b.locator('.tg-contacts__row', { hasText: `@${alice.username}` })
  await row.waitFor()
  const text = await row.innerText()
  if (/在线|online/.test(text) && !/最近|recently/.test(text)) throw new Error(`bob still sees "${text}"`)
})

// ---- avatars and QR card
await step(a, 'avatars-and-qr', async () => {
  const png = (r, g, bl) =>
    Buffer.from(
      'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==',
      'base64',
    )
  writeFileSync(`/tmp/tg1204-a.png`, png())
  await a.goto(`${BASE}/`)
  await openSettings(a, '我的账号')
  await a.locator('.tg-settings').getByText('头像', { exact: true }).click()
  const input = a.locator('.tg-avatar-carousel input[type=file]')
  await input.setInputFiles('/tmp/tg1204-a.png')
  await a.locator('.tg-avatar-carousel__count', { hasText: '1 / 1' }).waitFor({ timeout: 5000 })
  await input.setInputFiles('/tmp/tg1204-a.png')
  await a.locator('.tg-avatar-carousel__count', { hasText: '1 / 2' }).waitFor({ timeout: 5000 })
  await a.screenshot({ path: `${SHOTS}/${String(n).padStart(2, '0')}-avatars-uploaded.png` })
  await openSettings(a, '我的账号')
  await a.locator('.tg-settings').getByText('我的二维码', { exact: true }).click()
  await a
    .locator('.tg-settings svg, .tg-settings canvas, .tg-settings img[alt*="二维码"]')
    .first()
    .waitFor({ timeout: 5000 })
})
await step(b, 'bob-sees-avatar-carousel', async () => {
  const avatars = await api(bob.token, 'GET', `/api/users/${alice.user.id}/avatars`)
  const list = Array.isArray(avatars) ? avatars : avatars.avatars
  if (!list || list.length < 2) throw new Error(`bob sees ${list?.length ?? 0} avatars`)
})

// ---- data and storage persists
await step(a, 'storage-persists', async () => {
  await openSettings(a, '数据与存储')
  const toggle = a.locator('.tg-settings').getByRole('switch').nth(4)
  const before = await toggle.getAttribute('aria-checked')
  await toggle.click()
  await a.reload()
  await openSettings(a, '数据与存储')
  const after = await a.locator('.tg-settings').getByRole('switch').nth(4).getAttribute('aria-checked')
  if (after === before) throw new Error('auto-download change did not persist')
})

// ---- account switch in one tab: nothing of alice's stays
await step(a, 'account-switch-isolation', async () => {
  await a.goto(`${BASE}/`)
  await menu(a).click()
  await a.getByRole('menuitem', { name: '退出登录' }).click()
  await a.locator('input[name="username"]').waitFor()
  await login(a, bob)
  await a.waitForTimeout(1000)
  if (await a.getByRole('tab', { name: /^Work/ }).count()) throw new Error("alice's folder tab shows for bob")
  const box = a.getByRole('searchbox').or(a.getByPlaceholder(/搜索/)).first()
  await box.click()
  await a.waitForTimeout(500)
  if (await a.getByText('hello', { exact: true }).count()) throw new Error("alice's recent search shows for bob")
  await a.goto(`${BASE}/chat/${dmId}`)
  await composer(a).waitFor()
  await a.waitForTimeout(800)
  if (await a.locator('.tg-wallpaper[data-tg-wallpaper="sunset"]').count())
    throw new Error("alice's wallpaper shows for bob")
  await menu(a).click()
  await a.getByRole('menuitem', { name: '退出登录' }).click()
  await login(a, alice)
})

// ---- English UI: no Chinese left in chrome (user content is ASCII in this run)
const leftovers = new Set()
async function collectCjk(page, where) {
  const found = await page.evaluate((source) => {
    const re = new RegExp(source)
    const out = []
    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT)
    while (walker.nextNode()) {
      const text = walker.currentNode.textContent.trim()
      if (text && re.test(text) && !walker.currentNode.parentElement.closest('[lang="zh-CN"]')) out.push(text)
    }
    for (const el of document.querySelectorAll('[aria-label],[placeholder],[title]'))
      for (const attr of ['aria-label', 'placeholder', 'title']) {
        const value = el.getAttribute(attr)
        if (value && re.test(value) && !el.closest('[lang="zh-CN"]')) out.push(`@${attr}=${value}`)
      }
    return out
  }, CJK.source)
  for (const text of found) leftovers.add(`${where}: ${text}`)
}
await step(a, 'english-switch', async () => {
  await openSettings(a, '语言')
  await a.getByRole('radio', { name: /English/ }).check()
  await a.getByText('Language').first().waitFor({ timeout: 5000 })
})
for (const item of [
  'My account',
  'Notifications and sounds',
  'Privacy and security',
  'Data and storage',
  'Appearance',
  'Chat folders',
  'Language',
  'Devices',
]) {
  await step(a, `english-${item.replace(/\W+/g, '-').toLowerCase()}`, async () => {
    await openSettings(a, item)
    await collectCjk(a, item)
  })
}
await step(a, 'english-chat-and-lists', async () => {
  await a.keyboard.press('Escape')
  await a.goto(`${BASE}/`)
  await a.waitForTimeout(800)
  await collectCjk(a, 'chat list')
  await menu(a).click()
  await a.waitForTimeout(500)
  await collectCjk(a, 'main menu')
  await a.keyboard.press('Escape')
  await a.goto(`${BASE}/chat/${dmId}`)
  await composer(a).waitFor()
  await collectCjk(a, 'private chat')
  await a.goto(`${BASE}/contacts`)
  await a.waitForTimeout(1000)
  await collectCjk(a, 'contacts')
  await a.goto(`${BASE}/`)
  await a
    .getByRole('searchbox')
    .or(a.getByPlaceholder(/Search/))
    .first()
    .fill('hello')
  await a.waitForTimeout(1200)
  await collectCjk(a, 'search')
  if (leftovers.size) throw new Error(`${leftovers.size} Chinese strings in the English UI`)
})
await step(a, 'back-to-chinese', async () => {
  await openSettings(a, 'Language')
  await a.getByRole('radio', { name: /简体中文/ }).check()
  await a.getByText('语言').first().waitFor()
})

await browser.close()
for (const text of leftovers) console.log(`CJK ${text}`)
const unexpected = pageErrors.filter((error) => !/401 /.test(error))
for (const error of unexpected) console.log(`ERROR ${error}`)
console.log(`${n - failures.length}/${n} steps passed, ${unexpected.length} page errors`)
process.exit(failures.length || unexpected.length ? 1 : 0)
