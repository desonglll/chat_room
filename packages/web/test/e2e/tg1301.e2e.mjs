/**
 * TG-1301 walkthrough: the app opened over plain http on a LAN address — an INSECURE context, no
 * `crypto.subtle`, no microphone/camera, no geolocation — on a phone viewport, two accounts.
 * Emoji, voice, round video and location must still work. Not part of `bun test`. Re-runnable.
 *
 *   (cd packages/web && bun run build) && cargo build --bin server   # embeds this bundle
 *   server -p 18301 --database-type sqlite --database /tmp/tg-1301.db --config <toml, redis off>
 *   BASE_URL=http://<LAN IP>:18301 MEDIA=/path/to/media node packages/web/test/e2e/tg1301.e2e.mjs
 *
 * BASE_URL must NOT be localhost/127.0.0.1 (those count as secure). MEDIA holds (ffmpeg):
 *   voice.m4a  -f lavfi -i sine=frequency=330:duration=3 -c:a aac
 *   voice.mp3  -f lavfi -i sine=frequency=550:duration=2 -c:a libmp3lame
 *   note.mp4   -f lavfi -i testsrc=s=640x480:d=4:r=25 -f lavfi -i sine=f=600:d=4 -c:v libx264
 *              -pix_fmt yuv420p -c:a aac -shortest -movflags +faststart
 *   big.mp4    as note.mp4 but testsrc2=s=1920x1080:d=18:r=30 at -b:v 9M (> 16 MB)
 * Exit 0 = every step passed.
 */
import { mkdirSync } from 'node:fs'
import { join } from 'node:path'

const BASE = process.env.BASE_URL
const MEDIA = process.env.MEDIA
if (!BASE || !MEDIA) throw new Error('BASE_URL and MEDIA are required')
if (/localhost|127\.0\.0\.1/.test(BASE)) throw new Error('BASE_URL must be a non-localhost (insecure) origin')
const OUT = process.env.SHOTS ?? '/tmp/tg-shots/TG-1301'
const { chromium } = await import(process.env.PLAYWRIGHT ?? '/tmp/pw/node_modules/playwright/index.mjs')
mkdirSync(OUT, { recursive: true })
const media = (name) => join(MEDIA, name)

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

const alice = await account('ic_a')
const bob = await account('ic_b')
const group = await api(alice.token, 'POST', '/api/chats', {
  title: `HTTP 手机 ${run}`,
  password: null,
  join_policy: 'open',
})
await api(bob.token, 'POST', `/api/chats/${group.id}/join-requests`, { password: null })
const history = async () => api(bob.token, 'GET', `/api/chats/${group.id}/messages?limit=20`)

const browser = await chromium.launch({ args: ['--autoplay-policy=no-user-gesture-required'] })
async function open(who) {
  const context = await browser.newContext({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true })
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
const recordButton = (page) => page.locator('.tg-voice-mic[data-capture]')

/** The real gesture: press, hold past the tap threshold, release → the system picker opens. */
async function holdToPick(page, file, hint) {
  const box = await recordButton(page).boundingBox()
  const chooser = page.waitForEvent('filechooser', { timeout: 5000 })
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2)
  await page.mouse.down()
  await page.waitForTimeout(450)
  await page.mouse.up()
  const picker = await chooser
  // The secondary line says why the system app opened; it clears once a file is chosen.
  if (hint) {
    await page.getByText(hint).waitFor({ timeout: 2000 })
    await page.screenshot({ path: `${OUT}/${String(n).padStart(2, '0')}a-system-app-hint.png` })
  }
  await picker.setFiles(media(file))
}

await step(a, 'insecure-context-premise', async () => {
  const facts = await a.evaluate(() => ({
    secure: window.isSecureContext,
    subtle: typeof crypto.subtle,
    media: typeof navigator.mediaDevices,
  }))
  if (facts.secure !== false || facts.media !== 'undefined')
    throw new Error(`not an insecure context: ${JSON.stringify(facts)}`)
})

await step(a, 'emoji-panel-loads-and-inserts', async () => {
  await a.getByRole('button', { name: '表情', exact: true }).click()
  const emoji = a.locator('emoji-picker').locator('.emoji-menu button.emoji:visible').first()
  await emoji.waitFor({ timeout: 10_000 })
  if ((await a.locator('.tg-compose__emoji-error').count()) > 0) throw new Error('emoji panel shows its error')
  await emoji.click()
  // The picker inserts asynchronously: poll the composer instead of reading it once.
  await a.waitForFunction(
    () => /\p{Extended_Pictographic}/u.test(document.querySelector('textarea')?.value ?? ''),
    null,
    { timeout: 3000 },
  )
  await a.getByRole('button', { name: '表情', exact: true }).click()
  await a.getByLabel('消息内容').fill('')
})

await step(a, 'voice-button-opens-system-recorder-with-hint', async () => {
  await recordButton(a).waitFor()
  const kind = await a.locator('input[type=file][data-capture-kind=voice]').getAttribute('accept')
  if (kind !== 'audio/*') throw new Error(`voice input accepts ${kind}`)
  await holdToPick(a, 'voice.m4a', '通过 HTTP 访问时将使用系统录音机')
  await last(a, '.tg-voice').waitFor({ timeout: 10_000 })
})
await step(b, 'voice-m4a-received-and-plays', async () => {
  const voice = last(b, '.tg-voice')
  await voice.waitFor({ timeout: 10_000 })
  await voice.locator('.tg-voice__play').click()
  await voice.getByRole('button', { name: '暂停语音' }).waitFor({ timeout: 4000 })
  const sent = (await history()).findLast((m) => m.media_kind === 'voice')
  if (sent?.attachment?.mime_type !== 'audio/mp4') throw new Error(`m4a stored as ${sent?.attachment?.mime_type}`)
})

await step(a, 'voice-mp3-converted-to-wav', async () => {
  await holdToPick(a, 'voice.mp3')
  await a.waitForFunction(() => document.querySelectorAll('.tg-voice').length >= 2, null, { timeout: 15_000 })
  const voices = (await history()).filter((m) => m.media_kind === 'voice')
  const wav = voices.at(-1)
  if (voices.length < 2 || wav.attachment?.mime_type !== 'audio/wav')
    throw new Error(`mp3 stored as ${wav?.attachment?.mime_type}`)
  if (Math.abs(wav.voice.duration_ms - 2000) > 150) throw new Error(`duration ${wav.voice.duration_ms}`)
})
await step(b, 'voice-wav-received-and-plays', async () => {
  await b.waitForFunction(() => document.querySelectorAll('.tg-voice').length >= 2, null, { timeout: 10_000 })
  // Ending the m4a plays the next voice by itself (Telegram's "play next"): let that finish first.
  await b.getByRole('button', { name: '暂停语音' }).first().waitFor({ state: 'detached', timeout: 8000 })
  const voice = last(b, '.tg-voice')
  await voice.locator('.tg-voice__play').click()
  await voice.getByRole('button', { name: '暂停语音' }).waitFor({ timeout: 4000 })
})

await step(a, 'tap-toggles-to-video-mode', async () => {
  await recordButton(a).tap()
  await a.getByRole('button', { name: '按住录制视频消息' }).waitFor()
  const accept = await a.locator('input[type=file][data-capture-kind=video_note]').getAttribute('accept')
  if (accept !== 'video/*') throw new Error(`video input accepts ${accept}`)
})
await step(a, 'round-video-from-system-camera', async () => {
  await holdToPick(a, 'note.mp4', '通过 HTTP 访问时将使用系统相机')
  await last(a, '.tg-video-note').waitFor({ timeout: 15_000 })
})
await step(b, 'round-video-received-and-plays', async () => {
  const note = last(b, '.tg-video-note')
  await note.waitFor({ timeout: 10_000 })
  await note.locator('.tg-video-note__disc, .tg-video-note__video').first().click()
  await b.waitForFunction(
    () => [...document.querySelectorAll('.tg-video-note video')].some((v) => !v.paused && v.currentTime > 0),
    null,
    { timeout: 6000 },
  )
})

await step(a, 'large-video-sent-as-regular-with-notice', async () => {
  const before = (await history()).length
  await holdToPick(a, 'big.mp4')
  await a.getByText('视频超过 16 MB 或 60 秒，已作为普通视频发送').waitFor({ timeout: 60_000 })
  const latest = (await history()).at(-1)
  if ((await history()).length <= before || !latest.attachment?.mime_type?.startsWith('video/'))
    throw new Error(`no regular video: ${JSON.stringify(latest?.attachment)}`)
})
await step(b, 'regular-video-received', async () => {
  await b.locator('.tg-message video').last().waitFor({ timeout: 10_000 })
})

await step(a, 'location-picks-on-map-without-gps', async () => {
  await a.getByRole('button', { name: '添加附件' }).click()
  await a.getByRole('menuitem', { name: '位置' }).click()
  const dialog = a.locator('.tg-location-share')
  await dialog.getByText('需要通过 HTTPS 打开本站才能使用定位', { exact: false }).waitFor()
  await dialog.locator('.tg-location-picker__map.leaflet-container').waitFor({ timeout: 10_000 })
  const map = await dialog.locator('.tg-location-picker__map').boundingBox()
  await a.mouse.click(map.x + map.width * 0.7, map.y + map.height * 0.4) // tap brings that spot to the pin
  await a.waitForTimeout(1300)
  await a.screenshot({ path: `${OUT}/${String(n).padStart(2, '0')}a-map-picker.png` })
  await dialog.getByRole('button', { name: '发送所选位置' }).click()
  await last(a, '.tg-location').waitFor({ timeout: 8000 })
})
await step(b, 'location-received', async () => {
  await last(b, '.tg-location').waitFor({ timeout: 8000 })
  const sent = (await history()).findLast((m) => m.location)
  const { latitude, longitude } = sent.location
  if (!(latitude > 0 && latitude < 70 && longitude > 70 && longitude < 160))
    throw new Error(`picked point off the start view: ${latitude},${longitude}`)
})

await browser.close()
const unexpected = errors.filter((line) => !/ 404 GET \/api\/link-preview/.test(line))
if (unexpected.length) console.log('ERRORS', unexpected)
console.log(failed.length || unexpected.length ? `FAILED: ${failed.join(', ')}` : `ALL ${n} STEPS PASSED`)
process.exit(failed.length || unexpected.length ? 1 : 0)
