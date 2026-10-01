/**
 * TG-1302 end-to-end: image thumbnails against a REAL server (two accounts), with the network
 * as the witness. Not part of `bun test`. Needs the server's embedded bundle (or `vite preview`
 * proxied to it) and a MEDIA directory with big.jpg (4000×3000) and red/green/blue.jpg:
 *   ffmpeg -f lavfi -i testsrc2=s=4000x3000 -frames:v 1 -q:v 3 big.jpg
 *   ffmpeg -f lavfi -i color=c=red:s=1600x1200 -frames:v 1 -q:v 3 red.jpg   (green, blue alike)
 *   BASE_URL=http://127.0.0.1:18302 MEDIA=/tmp/tg-1302/media node packages/web/test/e2e/tg1302.e2e.mjs
 *
 * What it proves: a chat (bubble, album, info panel) downloads only `/thumbnail` URLs; the
 * original is requested only once the viewer opens; the viewer paints the thumbnail first.
 * Desktop and phone viewports.
 */
import { mkdirSync, readFileSync } from 'node:fs'
import { join } from 'node:path'
const { chromium } = await import(process.env.PLAYWRIGHT ?? '/tmp/pw/node_modules/playwright/index.mjs')

const BASE = process.env.BASE_URL ?? 'http://127.0.0.1:18302'
const MEDIA = process.env.MEDIA ?? '/tmp/tg-1302/media'
const OUT = process.env.SHOTS ?? '/tmp/tg-shots/TG-1302'
mkdirSync(OUT, { recursive: true })
const media = (name) => join(MEDIA, name)

async function api(token, method, path, body) {
  const form = body instanceof FormData
  const response = await fetch(`${BASE}${path}`, {
    method,
    headers: {
      ...(form || body === undefined ? {} : { 'content-type': 'application/json' }),
      ...(token ? { authorization: `Bearer ${token}` } : {}),
    },
    body: form ? body : body === undefined ? undefined : JSON.stringify(body),
  })
  const text = await response.text()
  if (!response.ok) throw new Error(`${method} ${path} → ${response.status} ${text}`)
  return text ? JSON.parse(text) : null
}

const run = Date.now().toString(36)
async function account(name) {
  const username = `${name}_${run}`
  const session = await api(null, 'POST', '/api/users/register', { username, password: 'correct-horse-9' })
  return { token: session.token, username, password: 'correct-horse-9' }
}

const errors = []
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

// ---- seed: alice and bob in one open group; alice posts a 4000×3000 photo
const alice = await account('th_a')
const bob = await account('th_b')
const group = await api(alice.token, 'POST', '/api/chats', {
  title: `缩略图 ${run}`,
  password: null,
  join_policy: 'open',
})
await api(bob.token, 'POST', `/api/chats/${group.id}/join-requests`, { password: null })
const form = new FormData()
form.append('file', new Blob([readFileSync(media('big.jpg'))], { type: 'image/jpeg' }), 'big.jpg')
const photo = await api(alice.token, 'POST', `/api/chats/${group.id}/attachments`, form)
const original = photo.attachment.download_url
const thumbnail = photo.attachment.thumbnail_url
if (!thumbnail) throw new Error('server sent no thumbnail_url')

const browser = await chromium.launch()
/** Every attachment request a page makes, as paths with query. */
async function open(who, viewport, extra = {}) {
  const context = await browser.newContext({ viewport, ...extra })
  const page = await context.newPage()
  const requests = []
  page.on('request', (request) => {
    const url = new URL(request.url())
    if (url.pathname.startsWith('/api/attachments/')) requests.push(url.pathname + url.search)
  })
  page.on('pageerror', (error) => errors.push(`${who.username}: ${error.message}`))
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
  return { page, requests }
}
const originals = (requests) => requests.filter((url) => url.includes('?key=') && !url.includes('/thumbnail'))
const thumbs = (requests) => requests.filter((url) => url.includes('/thumbnail?key='))

const desktop = await open(bob, { width: 1440, height: 900 })
const b = desktop.page
const bubbleImage = () => b.locator('.tg-bubble__media img').last()

await step(b, 'chat-shows-thumbnail-only', async () => {
  await bubbleImage().waitFor()
  await b.waitForFunction((el) => el.complete && el.naturalWidth > 0, await bubbleImage().elementHandle())
  const src = await bubbleImage().getAttribute('src')
  if (src !== thumbnail) throw new Error(`bubble src ${src}`)
  const natural = await bubbleImage().evaluate((el) => [el.naturalWidth, el.naturalHeight])
  if (natural[0] !== 640 || natural[1] !== 480) throw new Error(`thumbnail is ${natural}`)
  if (thumbs(desktop.requests).length === 0) throw new Error('no thumbnail request seen')
  if (originals(desktop.requests).length) throw new Error(`original fetched early: ${originals(desktop.requests)}`)
})

await step(b, 'viewer-fetches-original-then-swaps', async () => {
  const before = originals(desktop.requests).length
  const originalResponse = b.waitForResponse((r) => r.url().endsWith(original))
  await bubbleImage().click()
  await b.locator('.tg-mv').waitFor()
  await originalResponse
  if (originals(desktop.requests).length <= before) throw new Error('viewer did not request the original')
  const stageImage = b.locator('.tg-mv__slide[data-offset="0"] img.tg-mv__media:not(.tg-mv__media--preview)')
  await stageImage.waitFor()
  await b.waitForFunction(
    () => !document.querySelector('.tg-mv__slide[data-offset="0"] .tg-mv__media--preview'),
    null,
    { timeout: 10_000 },
  )
  const natural = await stageImage.evaluate((el) => [el.naturalWidth, el.naturalHeight])
  if (natural[0] !== 4000) throw new Error(`stage shows ${natural}, not the original`)
  await b.keyboard.press('Escape')
  await b.locator('.tg-mv').waitFor({ state: 'detached' })
})

// A fresh context: in the page above the original now sits in the in-memory image cache and a
// reopen paints it at once (no request, nothing to wait for) — which is right, but proves
// nothing about the slow path.
const cold = (await open(bob, { width: 1440, height: 900 })).page
await step(cold, 'viewer-paints-thumbnail-first-on-slow-network', async () => {
  // Hold the original back so the stage's first paint can be inspected.
  let release
  const held = new Promise((resolve) => (release = resolve))
  const isOriginal = (url) => url.pathname === original.split('?')[0]
  // On the context, not the page: the app's service worker makes the fetch, and page routes
  // do not see service-worker requests.
  await cold.context().route(isOriginal, async (route) => {
    await held
    await route.continue()
  })
  await cold.locator('.tg-bubble__media img').last().click()
  const preview = cold.locator('.tg-mv__slide[data-offset="0"] .tg-mv__media--preview')
  await preview.waitFor({ timeout: 5_000 })
  if ((await preview.getAttribute('src')) !== thumbnail) throw new Error('preview layer is not the thumbnail')
  const box = await preview.boundingBox()
  if (!box || box.height < 700) throw new Error(`preview does not fill the stage: ${JSON.stringify(box)}`)
  await cold.screenshot({ path: `${OUT}/${String(n).padStart(2, '0')}-viewer-preview-layer-while-original-loads.png` })
  release()
  await preview.waitFor({ state: 'detached', timeout: 10_000 })
  await cold.keyboard.press('Escape')
})

// ---- album sent from alice's composer: bob's mosaic uses thumbnails
const a = (await open(alice, { width: 1440, height: 900 })).page
await step(a, 'album-send', async () => {
  await a
    .locator('input[type=file][accept="image/*,video/*"]')
    .setInputFiles(['red.jpg', 'green.jpg', 'blue.jpg'].map(media))
  const dialog = a.locator('.tg-compose__pending')
  await dialog.getByRole('button', { name: '发送', exact: true }).click()
  await a.locator('.tg-album').last().waitFor({ timeout: 10_000 })
})
await step(b, 'album-mosaic-uses-thumbnails', async () => {
  const tiles = b.locator('.tg-album').last().locator('img.tg-album__media')
  await tiles.first().waitFor({ timeout: 10_000 })
  const sources = await tiles.evaluateAll((els) => els.map((el) => el.getAttribute('src')))
  if (sources.length !== 3 || !sources.every((src) => src.includes('/thumbnail?key=')))
    throw new Error(`album tiles: ${sources}`)
})

await step(b, 'info-panel-media-uses-thumbnails', async () => {
  const before = originals(desktop.requests).length
  await b.getByRole('button', { name: '查看会话信息' }).click()
  await b.getByRole('tab', { name: '媒体', exact: true }).click()
  const tile = b.locator('.tg-chatinfo__tile img').first()
  await tile.waitFor({ timeout: 10_000 })
  const sources = await b
    .locator('.tg-chatinfo__tile img')
    .evaluateAll((els) => els.map((el) => el.getAttribute('src')))
  if (!sources.length || !sources.every((src) => src.includes('/thumbnail?key=')))
    throw new Error(`info tiles: ${sources}`)
  if (originals(desktop.requests).length !== before) throw new Error('info panel fetched an original')
})

// ---- phone: a fresh context (cold cache) sees thumbnails only until it taps
const phone = await open(bob, { width: 390, height: 844 }, { isMobile: true, hasTouch: true, deviceScaleFactor: 3 })
const p = phone.page
await step(p, 'phone-chat-thumbnails-only', async () => {
  const images = p.locator('.tg-bubble__media img, img.tg-album__media')
  await images.first().waitFor()
  await p.waitForTimeout(1500)
  const sources = await images.evaluateAll((els) => els.map((el) => el.getAttribute('src')))
  if (!sources.every((src) => src.includes('/thumbnail?key='))) throw new Error(`phone sources: ${sources}`)
  if (originals(phone.requests).length) throw new Error(`phone fetched originals: ${originals(phone.requests)}`)
})
await step(p, 'phone-tap-opens-original', async () => {
  const originalResponse = p.waitForResponse((r) => r.url().endsWith(original))
  await p.locator('.tg-bubble__media img').first().tap()
  await p.locator('.tg-mv').waitFor()
  await originalResponse
})

await browser.close()
const unexpected = errors.filter((line) => !/\/api\/link-preview/.test(line))
if (unexpected.length) console.log('page errors / failed requests:\n  ' + unexpected.join('\n  '))
console.log(failed.length ? `FAILED ${failed.length}/${n}: ${failed.join(', ')}` : `ALL ${n} STEPS PASSED`)
process.exit(failed.length || unexpected.length ? 1 : 0)
