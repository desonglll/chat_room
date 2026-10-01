/**
 * TG-1209 walkthrough against a REAL server: map tiles and link-card images render under the
 * production CSP, with no CSP violation. Not part of `bun test`. Self-seeding, re-runnable.
 *
 *   (cd packages/web && bun run build)
 *   cargo run --bin server -- -p 18209 --database-type sqlite --database /tmp/tg-1209.db \
 *     --config <toml: [redis] off; [link_preview] unsafe_allow_sockets = ["127.0.0.1:18293"];
 *               [map] tile_url = "http://127.0.0.1:18294/{z}/{x}/{y}.png">
 *   FIXTURE=1 node packages/web/test/e2e/tg1209.e2e.mjs   # FIXTURE=1 also serves 18293 + 18294
 *   BASE_URL=http://127.0.0.1:18209 node packages/web/test/e2e/tg1209.e2e.mjs
 *
 * 18293 serves an Open Graph page whose og:image is on that same third-party host; 18294 is a
 * tile server answering every /{z}/{x}/{y}.png with a PNG. Exit 0 = every step passed.
 */
import { mkdirSync } from 'node:fs'
import { createServer } from 'node:http'

const BASE = process.env.BASE_URL ?? 'http://127.0.0.1:18209'
const OG = process.env.OG_URL ?? 'http://127.0.0.1:18293/'
const OUT = process.env.SHOTS ?? '/tmp/tg-shots/TG-1209'
const { chromium } = await import(process.env.PLAYWRIGHT ?? '/tmp/pw/node_modules/playwright/index.mjs')
mkdirSync(OUT, { recursive: true })

// A real 1×1 PNG (the same bytes the Rust test uses).
const PNG = Buffer.from([
  137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 2, 0, 0, 0, 144, 119, 83,
  222, 0, 0, 0, 12, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 0, 0, 3, 1, 1, 0, 201, 254, 146, 239, 0, 0, 0, 0, 73,
  69, 78, 68, 174, 66, 96, 130,
])
const tiles = []
if (process.env.FIXTURE) {
  createServer((request, response) => {
    if (request.url === '/cover.png') {
      response.writeHead(200, { 'content-type': 'image/png' }).end(PNG)
    } else {
      response.writeHead(200, { 'content-type': 'text/html; charset=utf-8' }).end(
        `<html><head><meta property="og:title" content="封面走查夹具">
         <meta property="og:description" content="卡片图片应由本站提供。">
         <meta property="og:image" content="${OG}cover.png"></head></html>`,
      )
    }
  }).listen(18293, '127.0.0.1')
  createServer((request, response) => {
    tiles.push(request.url)
    response.writeHead(200, { 'content-type': 'image/png', 'access-control-allow-origin': '*' }).end(PNG)
  }).listen(18294, '127.0.0.1')
}

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
const run = Date.now().toString(36)
async function account(name) {
  const username = `${name}_${run}`
  const session = await api(null, 'POST', '/api/users/register', { username, password: 'correct-horse-9' })
  return { token: session.token, id: session.user.id, username, password: 'correct-horse-9' }
}

const errors = []
const cspViolations = []
const failed = []
let n = 0
async function step(page, name, body) {
  n += 1
  const shot = `${OUT}/${String(n).padStart(2, '0')}-${name}`
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

const alice = await account('csp_a')
const bob = await account('csp_b')
const group = await api(alice.token, 'POST', '/api/chats', { title: `CSP ${run}`, password: null, join_policy: 'open' })
await api(bob.token, 'POST', `/api/chats/${group.id}/join-requests`, { password: null })

const browser = await chromium.launch()
async function open(who) {
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 } })
  const page = await context.newPage()
  page.on('pageerror', (error) => errors.push(`${who.username}: ${error.message}`))
  page.on('console', (message) => {
    if (/Content Security Policy/i.test(message.text())) cspViolations.push(`${who.username}: ${message.text()}`)
  })
  page.on('response', (response) => {
    if (response.status() >= 400)
      errors.push(`${response.status()} ${response.request().method()} ${new URL(response.url()).pathname}`)
  })
  await page.goto(`${BASE}/login`)
  await page.locator('input[name="username"]').fill(who.username)
  await page.locator('input[name="password"]').fill(who.password)
  await page.getByRole('button', { name: '登录', exact: true }).click()
  await page.getByRole('button', { name: '主菜单' }).first().waitFor()
  await page.goto(`${BASE}/chat/${group.id}`)
  await page.getByLabel('消息内容').waitFor()
  return page
}
const a = await open(alice)
const b = await open(bob)

await step(b, 'csp-header-admits-only-the-tile-host', async () => {
  const response = await fetch(`${BASE}/api/config`)
  const img = (response.headers.get('content-security-policy') ?? '')
    .split(';')
    .map((part) => part.trim())
    .find((part) => part.startsWith('img-src'))
  if (img !== "img-src 'self' data: blob: http://127.0.0.1:18294") throw new Error(`img-src = ${img}`)
})

await step(b, 'map-tiles-render', async () => {
  await api(alice.token, 'POST', `/api/chats/${group.id}/location-messages`, { latitude: 31.2304, longitude: 121.4737 })
  const location = b.locator('.tg-message:has(.tg-location)').last()
  await location.waitFor({ timeout: 8000 })
  await location.locator('.tg-location__open').first().click()
  await b.locator('.leaflet-tile-loaded').first().waitFor({ timeout: 8000 })
  const decoded = await b
    .locator('img.leaflet-tile-loaded')
    .evaluateAll((images) => images.filter((image) => image.naturalWidth > 0).length)
  if (decoded === 0 || tiles.length === 0) throw new Error(`tiles decoded ${decoded}, requested ${tiles.length}`)
  await b.keyboard.press('Escape')
})

await step(a, 'composer-card-image-same-origin', async () => {
  await a.getByLabel('消息内容').fill(`看封面 ${OG}`)
  const image = a.locator('.tg-compose-link .tg-link-card__image')
  await image.waitFor({ timeout: 8000 })
  await checkImage(image)
  await a.getByLabel('消息内容').press('Enter')
})

await step(b, 'received-card-image-same-origin', async () => {
  const card = b.locator('.tg-message', { hasText: OG }).last()
  await card.getByText('封面走查夹具').waitFor({ timeout: 10_000 })
  await checkImage(card.locator('.tg-link-card__image'))
})

async function checkImage(image) {
  const { src, width } = await image.evaluate(async (element) => {
    if (!element.complete) await new Promise((resolve) => element.addEventListener('load', resolve, { once: true }))
    return { src: element.src, width: element.naturalWidth }
  })
  if (!src.startsWith(`${BASE}/api/link-previews/images/`)) throw new Error(`card image not same-origin: ${src}`)
  if (width !== 1) throw new Error(`card image did not decode (naturalWidth ${width})`)
}

await browser.close()
const unexpected = errors.filter((line) => !/\/api\/link-preview\b/.test(line))
if (unexpected.length) console.log('page errors / failed requests:\n  ' + unexpected.join('\n  '))
if (cspViolations.length) console.log('CSP violations:\n  ' + cspViolations.join('\n  '))
console.log(failed.length ? `FAILED ${failed.length}/${n}: ${failed.join(', ')}` : `ALL ${n} STEPS PASSED`)
process.exit(failed.length || unexpected.length || cspViolations.length ? 1 : 0)
