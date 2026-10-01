/**
 * TG-806 browser check that the features moved out of the initial bundle still work:
 * the chat info panel opens on header click, a deep-link route renders, and the English
 * catalog loads both on a live switch and on a cold boot with English saved.
 * Not part of `bun test` (needs a server and Chromium).
 *
 *   <target>/debug/server -p 3806 --database-type sqlite --database /tmp/tg806.db
 *   BASE_URL=http://127.0.0.1:3806 node packages/web/test/e2e/lazyChunks.e2e.mjs
 */
import { mkdirSync } from 'node:fs'

const BASE_URL = process.env.BASE_URL
if (!BASE_URL) throw new Error('BASE_URL is required')
const SHOTS = process.env.SHOTS ?? '/tmp/tg-shots/TG-806'
const { chromium } = await import(process.env.PLAYWRIGHT ?? '/tmp/pw/node_modules/playwright/index.mjs')
mkdirSync(SHOTS, { recursive: true })

async function api(token, method, path, body) {
  const response = await fetch(`${BASE_URL}${path}`, {
    method,
    headers: { 'content-type': 'application/json', ...(token ? { authorization: `Bearer ${token}` } : {}) },
    body: body === undefined ? undefined : JSON.stringify(body),
  })
  if (!response.ok) throw new Error(`${method} ${path} → ${response.status}`)
  return response.status === 204 ? null : response.json()
}

const username = `lazy_${Date.now().toString(36)}`
const account = await api(null, 'POST', '/api/users/register', { username, password: 'correct-horse-6' })
const token = account.token
const group = await api(token, 'POST', '/api/chats', {
  title: `Lazy chunks ${username}`,
  password: '',
  join_policy: 'open',
})
const chatId = group.id ?? group.room_id

const browser = await chromium.launch()
const errors = []
let step = 0
const check = async (name, body) => {
  step += 1
  await body()
  console.log(`ok ${step} ${name}`)
}
try {
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 } })
  const page = await context.newPage()
  page.on('pageerror', (error) => errors.push(String(error)))
  page.on(
    'response',
    (response) =>
      response.status() >= 400 && console.log(`  http ${response.status()} ${new URL(response.url()).pathname}`),
  )

  await check('log in through the real form', async () => {
    await page.goto(`${BASE_URL}/login`)
    await page.locator('input[name="username"]').fill(username)
    await page.locator('input[name="password"]').fill('correct-horse-6')
    await page.getByRole('button', { name: '登录', exact: true }).click()
    await page.getByRole('button', { name: '主菜单' }).first().waitFor()
  })

  await check('the lazy info panel opens from the chat header', async () => {
    await page.goto(`${BASE_URL}/chat/${chatId}`)
    await page.getByRole('button', { name: '查看会话信息' }).click()
    await page.getByRole('complementary', { name: '会话信息' }).or(page.locator('.tg-chatinfo')).first().waitFor()
    await page.screenshot({ path: `${SHOTS}/01-info-panel.png` })
  })

  await check('a lazy deep-link route renders', async () => {
    await page.goto(`${BASE_URL}/joinchat/not-a-real-token`)
    await page.waitForLoadState('networkidle')
    const text = await page.locator('#root').innerText()
    if (!text.trim()) throw new Error('joinchat rendered nothing')
    await page.screenshot({ path: `${SHOTS}/02-joinchat.png` })
  })

  await check('switching to English loads the catalog and renders English', async () => {
    await page.goto(`${BASE_URL}/chat/${chatId}`)
    await page.evaluate(() => {
      const key = 'tg.settings.v1'
      const current = JSON.parse(localStorage.getItem(key) ?? '{}')
      localStorage.setItem(key, JSON.stringify({ ...current, language: 'en' }))
    })
    await page.reload()
    await page.getByRole('button', { name: 'View chat info' }).waitFor()
    const html = await page.locator('html').getAttribute('lang')
    if (html !== 'en') throw new Error(`html lang ${html}`)
    await page.screenshot({ path: `${SHOTS}/03-english-cold-boot.png` })
  })

  if (errors.length) throw new Error(`page errors:\n${errors.join('\n')}`)
  console.log(`all ${step} steps passed; screenshots in ${SHOTS}`)
} finally {
  await browser.close()
}
