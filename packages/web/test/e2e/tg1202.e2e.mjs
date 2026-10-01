/**
 * TG-1202 rich-media walkthrough against a REAL server, two accounts in two browser contexts.
 * Not part of `bun test`. Self-seeding and re-runnable (fresh accounts and group every run).
 *
 *   (cd packages/web && bun run build)          # or serve this worktree through `vite`
 *   cargo run --bin server -- -p 18202 --database-type sqlite --database /tmp/tg-1202.db \
 *     --config <toml with [link_preview] unsafe_allow_sockets = ["127.0.0.1:18292"]>
 *   python3 -m http.server 18292 --bind 127.0.0.1   # in MEDIA/og: index.html with og:* tags
 *   BASE_URL=http://127.0.0.1:18202 MEDIA=/path/to/media node packages/web/test/e2e/tg1202.e2e.mjs
 *
 * MEDIA holds red/green/blue.png, voice.ogg, note.mp4 (square), gif.mp4, sticker.webp (512²),
 * emoji.webp (100²) — see docs/devlog/TG-1202.md for the ffmpeg lines. OG_URL (default
 * http://127.0.0.1:18292/) must serve og:title «富媒体走查夹具». Exit 0 = every step passed.
 */
import { mkdirSync, readFileSync } from 'node:fs'
import { join } from 'node:path'

const BASE = process.env.BASE_URL
const MEDIA = process.env.MEDIA
if (!BASE || !MEDIA) throw new Error('BASE_URL and MEDIA are required')
const API = process.env.API_URL ?? BASE
const OG_URL = process.env.OG_URL ?? 'http://127.0.0.1:18292/'
const OUT = process.env.SHOTS ?? '/tmp/tg-shots/TG-1202'
const { chromium } = await import(process.env.PLAYWRIGHT ?? '/tmp/pw/node_modules/playwright/index.mjs')
mkdirSync(OUT, { recursive: true })

const media = (name) => join(MEDIA, name)
const blob = (name, type) => new Blob([readFileSync(media(name))], { type })

async function api(token, method, path, body) {
  const form = body instanceof FormData
  const raw = body instanceof Blob
  const response = await fetch(`${API}${path}`, {
    method,
    headers: {
      ...(token ? { authorization: `Bearer ${token}` } : {}),
      ...(form ? {} : raw ? { 'content-type': 'application/octet-stream' } : { 'content-type': 'application/json' }),
    },
    body: body === undefined ? undefined : form || raw ? body : JSON.stringify(body),
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

// ---- seed: two accounts, one open group, a sticker set and a custom-emoji set for alice
const alice = await account('rm_a')
const bob = await account('rm_b')
const group = await api(alice.token, 'POST', '/api/chats', {
  title: `富媒体 ${run}`,
  password: null,
  join_policy: 'open',
})
await api(bob.token, 'POST', `/api/chats/${group.id}/join-requests`, { password: null })

async function stickerSet(setType, file, emoji) {
  const shortName = `tg1202_${setType}_${run}`
  const set = await api(alice.token, 'POST', '/api/sticker-sets', {
    short_name: shortName,
    title: setType === 'custom_emoji' ? '走查表情' : '走查贴纸',
    set_type: setType,
  })
  const form = new FormData()
  form.append('file', blob(file, 'image/webp'), file)
  form.append('emoji', emoji)
  const sticker = await api(alice.token, 'POST', `/api/sticker-sets/${shortName}/stickers`, form)
  await api(alice.token, 'PUT', `/api/stickers/installed/${set.id}`, {}).catch(() => {})
  return sticker
}
await stickerSet('regular', 'sticker.webp', '😀')
const customEmoji = await stickerSet('custom_emoji', 'emoji.webp', '⭐')

const browser = await chromium.launch({
  args: ['--autoplay-policy=no-user-gesture-required'],
})
async function open(who) {
  const context = await browser.newContext({
    viewport: { width: 1440, height: 900 },
    permissions: ['geolocation'],
    geolocation: { latitude: 31.2304, longitude: 121.4737, accuracy: 20 },
  })
  const page = await context.newPage()
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
  return page
}
const a = await open(alice)
const b = await open(bob)
const last = (page, selector) => page.locator(`.tg-message:has(${selector})`).last()
const attach = async (page, item) => {
  await page.getByRole('button', { name: '添加附件' }).click()
  await page.getByRole('menuitem', { name: item }).click()
}

// ---- album
await step(a, 'album-send', async () => {
  await a
    .locator('input[type=file][accept="image/*,video/*"]')
    .setInputFiles(['red.png', 'green.png', 'blue.png'].map(media))
  const dialog = a.locator('.tg-compose__pending')
  await dialog.getByLabel('说明').fill('三张图一条消息')
  await dialog.getByRole('button', { name: '发送', exact: true }).click()
  await last(a, '.tg-album').waitFor({ timeout: 10_000 })
})
await step(b, 'album-received', async () => {
  const album = last(b, '.tg-album')
  await album.waitFor({ timeout: 10_000 })
  const tiles = await album.locator('.tg-album__tile').count()
  if (tiles !== 3) throw new Error(`expected 3 tiles, got ${tiles}`)
  if (!(await album.getByText('三张图一条消息').isVisible())) throw new Error('caption missing')
  const messages = await b.locator('.tg-message:has(.tg-album__tile)').count()
  if (messages !== 1) throw new Error(`album split into ${messages} bubbles`)
})
await step(b, 'album-viewer-pages', async () => {
  await last(b, '.tg-album').locator('.tg-album__tile').nth(1).click()
  const position = b.locator('.tg-mv__position')
  await position.waitFor()
  if (!/2\D+3/.test(await position.innerText())) throw new Error(`position ${await position.innerText()}`)
  await b.keyboard.press('ArrowRight')
  await b.waitForTimeout(500)
  if (!/3\D+3/.test(await position.innerText())) throw new Error(`after → ${await position.innerText()}`)
  await b.keyboard.press('ArrowLeft')
  await b.keyboard.press('ArrowLeft')
  await b.waitForTimeout(500)
  if (!/1\D+3/.test(await position.innerText())) throw new Error(`after ←← ${await position.innerText()}`)
})
await step(b, 'album-viewer-close', async () => {
  await b.keyboard.press('Escape')
  await b.locator('.tg-mv__position').waitFor({ state: 'detached', timeout: 3000 })
})

// ---- voice
let voiceId = ''
await step(b, 'voice-received-unlistened', async () => {
  const form = new FormData()
  form.append('file', blob('voice.ogg', 'audio/ogg'), 'voice.ogg')
  form.append('waveform', Array.from({ length: 100 }, (_, i) => (i * 7) % 32).join(','))
  form.append('duration_ms', '4000')
  voiceId = (await api(alice.token, 'POST', `/api/chats/${group.id}/voice`, form)).id
  const voice = last(b, '.tg-voice')
  await voice.waitFor({ timeout: 10_000 })
  await voice.locator('.tg-voice__unlistened').waitFor()
})
await step(b, 'voice-plays-and-rate', async () => {
  const voice = last(b, '.tg-voice')
  await voice.locator('.tg-voice__play').click()
  // The label flips only once the <audio> element actually plays (it did not with the SW on).
  await voice.getByRole('button', { name: '暂停语音' }).waitFor({ timeout: 4000 })
  const rate = voice.locator('.tg-voice__rate')
  const before = await rate.innerText()
  await rate.click()
  const after = await rate.innerText()
  if (before === after) throw new Error(`rate stuck at ${before}`)
  await voice.locator('.tg-voice__unlistened').waitFor({ state: 'detached', timeout: 3000 })
})
await step(a, 'voice-sender-sees-listened', async () => {
  const voice = a.locator(`[data-message-id="${voiceId}"] .tg-voice, .tg-message:has(.tg-voice) .tg-voice`).last()
  await voice.waitFor()
  await voice.locator('.tg-voice__unlistened').waitFor({ state: 'detached', timeout: 5000 })
})

// ---- round video
await step(b, 'video-note-plays', async () => {
  const form = new FormData()
  form.append('file', blob('note.mp4', 'video/mp4'), 'note.mp4')
  form.append('duration_ms', '3000')
  await api(alice.token, 'POST', `/api/chats/${group.id}/video_note`, form)
  const note = last(b, '.tg-video-note')
  await note.waitFor({ timeout: 10_000 })
  await note.locator('.tg-video-note__unwatched').waitFor()
  await note.locator('.tg-video-note__disc, .tg-video-note__video').first().click()
  await b.waitForTimeout(1500)
  const playing = await note.locator('video').evaluate((video) => !video.paused && video.currentTime > 0)
  if (!playing) throw new Error('video note is not playing')
  await note.locator('.tg-video-note__unwatched').waitFor({ state: 'detached', timeout: 3000 })
})

// ---- polls
await step(a, 'poll-create-ui', async () => {
  await attach(a, '投票')
  const dialog = a.locator('.tg-poll-create')
  await dialog.getByLabel('问题').fill('周五去哪吃？')
  await dialog.getByLabel('选项 1', { exact: true }).fill('火锅')
  await dialog.getByLabel('选项 2', { exact: true }).fill('烧烤')
  await a
    .getByRole('button', { name: /^(创建|发送)$/ })
    .last()
    .click()
  await last(a, '.tg-poll').getByText('周五去哪吃？').waitFor({ timeout: 8000 })
})
await step(b, 'poll-vote-live', async () => {
  const poll = b.locator('.tg-message:has(.tg-poll)', { hasText: '周五去哪吃？' }).last()
  await poll.waitFor({ timeout: 8000 })
  await poll.locator('.tg-poll__choice', { hasText: '火锅' }).click()
  await poll.locator('.tg-poll__percent').first().waitFor({ timeout: 5000 })
  const aliceSide = a.locator('.tg-message:has(.tg-poll)', { hasText: '周五去哪吃？' }).last()
  await aliceSide.locator('.tg-poll__count', { hasText: '1' }).waitFor({ timeout: 5000 })
})
await step(b, 'poll-retract', async () => {
  const poll = b.locator('.tg-message:has(.tg-poll)', { hasText: '周五去哪吃？' }).last()
  await poll.getByRole('button', { name: '撤回投票' }).click()
  await poll.locator('.tg-poll__choice', { hasText: '火锅' }).waitFor({ timeout: 5000 })
  if (await poll.locator('.tg-poll__percent').count()) throw new Error('results still shown after retract')
})
await step(b, 'quiz-wrong-explains', async () => {
  await api(alice.token, 'POST', `/api/chats/${group.id}/polls`, {
    question: '1 + 1 = ?',
    options: ['2', '3'],
    quiz: true,
    correct_option: 0,
    explanation: '小学算术',
  })
  const quiz = b.locator('.tg-message:has(.tg-poll)', { hasText: '1 + 1 = ?' }).last()
  await quiz.waitFor({ timeout: 8000 })
  await quiz.locator('.tg-poll__choice', { hasText: '3' }).click()
  await quiz.locator('.tg-poll__explanation, .tg-poll__result').first().waitFor({ timeout: 5000 })
  await b.getByText('小学算术').first().waitFor({ timeout: 5000 })
})
await step(b, 'poll-multi-vote', async () => {
  await api(alice.token, 'POST', `/api/chats/${group.id}/polls`, {
    question: '会哪些语言？',
    options: ['Rust', 'TypeScript', 'Go'],
    multiple_choice: true,
    public_voters: true,
  })
  const poll = b.locator('.tg-message:has(.tg-poll)', { hasText: '会哪些语言？' }).last()
  await poll.waitFor({ timeout: 8000 })
  await poll.locator('.tg-poll__choice', { hasText: 'Rust' }).click()
  await poll.locator('.tg-poll__choice', { hasText: 'TypeScript' }).click()
  await poll.getByRole('button', { name: '投票', exact: true }).click()
  await poll.getByText('1 人已投票').waitFor({ timeout: 5000 })
  if ((await poll.locator('.tg-poll__percent').count()) !== 3) throw new Error('results not shown after voting')
})
await step(a, 'poll-public-voters', async () => {
  const mine = a.locator('.tg-message:has(.tg-poll)', { hasText: '会哪些语言？' }).last()
  await mine.locator('.tg-poll__choice', { hasText: 'Go' }).click()
  await mine.getByRole('button', { name: '投票', exact: true }).click()
  await mine.getByText('2 人已投票').waitFor({ timeout: 5000 })
  await mine.getByRole('button', { name: '查看投票人' }).click()
  await a.getByRole('dialog').getByText(bob.username).first().waitFor({ timeout: 5000 })
})
await step(a, 'poll-close-ui', async () => {
  await a.keyboard.press('Escape')
  const mine = a.locator('.tg-message:has(.tg-poll)', { hasText: '会哪些语言？' }).last()
  await mine.getByRole('button', { name: '结束投票' }).click()
  const confirm = a.getByRole('dialog').getByRole('button', { name: '结束投票' })
  if (await confirm.isVisible().catch(() => false)) await confirm.click()
  await mine.getByText('投票已结束').waitFor({ timeout: 5000 })
})
await step(b, 'poll-closed-live', async () => {
  const poll = b.locator('.tg-message:has(.tg-poll)', { hasText: '会哪些语言？' }).last()
  await poll.getByText('投票已结束').waitFor({ timeout: 5000 })
})

// ---- locations
await step(a, 'location-static-ui', async () => {
  await attach(a, '位置')
  const dialog = a.locator('.tg-location-share')
  await dialog.waitFor()
  await a.waitForTimeout(800)
  await dialog.getByRole('button', { name: '发送当前位置' }).click()
  await last(a, '.tg-location').waitFor({ timeout: 8000 })
})
await step(b, 'location-static-received-and-map', async () => {
  const location = last(b, '.tg-location')
  await location.waitFor({ timeout: 8000 })
  await location.locator('.tg-location__open').first().click()
  await b.locator('.tg-location-map, .tg-location-sheet').first().waitFor({ timeout: 5000 })
  await b.keyboard.press('Escape')
})
await step(b, 'location-live-updates-and-stops', async () => {
  const sent = await api(alice.token, 'POST', `/api/chats/${group.id}/location-messages`, {
    latitude: 31.2,
    longitude: 121.4,
    live_seconds: 900,
  })
  const live = b.locator(`.tg-message:has(.tg-location__live)`).last()
  await live.waitFor({ timeout: 8000 })
  await api(alice.token, 'PUT', `/api/chats/${group.id}/live-locations/${sent.id}`, {
    latitude: 31.3,
    longitude: 121.5,
  })
  await b.waitForTimeout(1500)
  await api(alice.token, 'DELETE', `/api/chats/${group.id}/live-locations/${sent.id}`)
  await b.waitForTimeout(1500)
  const text = await live.innerText()
  if (!text.includes('实时位置已结束')) throw new Error(`live location still live: ${text.replace(/\s+/g, ' ')}`)
})

// ---- link preview
await step(a, 'link-preview-composer', async () => {
  await a.getByLabel('消息内容').fill(`看这个 ${OG_URL}`)
  await a.locator('.tg-compose-link').getByText('富媒体走查夹具').waitFor({ timeout: 8000 })
  await a.getByLabel('消息内容').press('Enter')
})
await step(b, 'link-preview-received', async () => {
  const card = b.locator('.tg-message', { hasText: OG_URL }).last()
  await card.getByText('富媒体走查夹具').waitFor({ timeout: 10_000 })
  await card.getByText('链接预览应显示这段描述文字。').waitFor()
})

// ---- sticker, custom emoji, GIF
await step(a, 'sticker-send-ui', async () => {
  await a.getByRole('button', { name: '表情' }).click()
  const panel = a.locator('.tg-media-panel')
  await panel.waitFor()
  await panel.getByRole('tab', { name: /贴纸/ }).click()
  await panel.locator('.tg-sticker-grid__cell').first().click()
  await last(a, '.tg-sticker-message').waitFor({ timeout: 8000 })
})
await step(b, 'sticker-received', async () => {
  const sticker = last(b, '.tg-sticker-message')
  await sticker.waitFor({ timeout: 8000 })
  const ok = await sticker
    .locator('img, canvas, video')
    .first()
    .evaluate((el) => (el instanceof HTMLImageElement ? el.naturalWidth > 0 : true))
  if (!ok) throw new Error('sticker image did not decode')
})
// TG-1206: picked from the composer's «表情 › 自定义» tab, text typed around it, sent with Enter.
await step(a, 'custom-emoji-sent-shows', async () => {
  const input = a.getByLabel('消息内容')
  await input.fill('自定义表情 ')
  await a.getByRole('button', { name: '表情' }).click()
  const panel = a.locator('.tg-media-panel')
  await panel.waitFor()
  await panel.getByRole('tab', { name: '表情', exact: true }).click()
  await panel.getByRole('tab', { name: '自定义' }).click()
  await panel.locator('.tg-custom-emoji-grid__cell').first().click()
  await a.keyboard.press('Escape')
  await input.press('End')
  await input.pressSequentially(' 好')
  await input.press('Enter')
  const sent = a.locator('.tg-message', { hasText: '自定义表情' }).last()
  await sent.locator('.tg-custom-emoji__image').waitFor({ timeout: 8000 })
})
await step(b, 'custom-emoji-received', async () => {
  const image = b.locator('.tg-message', { hasText: '自定义表情' }).last().locator('.tg-custom-emoji__image')
  await image.waitFor({ timeout: 8000 })
  const decoded = await image.evaluate((el) => (el instanceof HTMLImageElement ? el.naturalWidth > 0 : true))
  if (!decoded) throw new Error('custom emoji image did not decode')
})
await step(b, 'gif-autoplays', async () => {
  await api(alice.token, 'POST', `/api/chats/${group.id}/gif-messages/upload`, blob('gif.mp4', 'video/mp4'))
  const gif = last(b, '.tg-gif__media')
  await gif.waitFor({ timeout: 8000 })
  await b.waitForTimeout(1500)
  const playing = await gif
    .locator('.tg-gif__media')
    .last()
    .evaluate((el) => (el instanceof HTMLVideoElement ? !el.paused : true))
  if (!playing) throw new Error('GIF is not autoplaying')
})
await step(b, 'gif-save-and-send-from-panel', async () => {
  const history = await api(bob.token, 'GET', `/api/chats/${group.id}/messages?limit=5`)
  const gifMessage = history.findLast((message) => message.media_kind === 'gif')
  if (!gifMessage) throw new Error('GIF message not in history')
  await api(bob.token, 'POST', '/api/gifs/saved', { message_id: gifMessage.id })
  const before = await a.locator('.tg-message:has(.tg-gif__media)').count()
  await b.getByRole('button', { name: '表情' }).click()
  const panel = b.locator('.tg-media-panel')
  await panel.getByRole('tab', { name: 'GIF' }).click()
  await panel.locator('.tg-gif-tab__cell').first().click()
  await a.locator('.tg-message:has(.tg-gif__media)').nth(before).waitFor({ timeout: 8000 })
})

await browser.close()
const unexpected = errors.filter((line) => !/\/api\/link-preview|two-factor/.test(line))
if (unexpected.length) console.log('page errors / failed requests:\n  ' + unexpected.join('\n  '))
console.log(failed.length ? `FAILED ${failed.length}/${n}: ${failed.join(', ')}` : `ALL ${n} STEPS PASSED`)
process.exit(failed.length || unexpected.length ? 1 : 0)
