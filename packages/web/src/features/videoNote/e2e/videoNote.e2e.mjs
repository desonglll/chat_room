/**
 * TG-402 end-to-end check against a REAL server and headless Chromium with a fake camera and
 * microphone. The camera is deliberately NOT square (640×360, red | green | blue strips, see
 * `fakeCamera.mjs`), so the round crop is verified on pixels — in the live viewfinder and in
 * the stored file. No `--autoplay-policy` override: Chromium's default policy must allow the
 * muted preview on its own. Not part of `bun test`.
 *
 * Run (see docs/devlog/TG-402.md "E2E"):
 *   (cd packages/web && bun run build)
 *   CARGO_TARGET_DIR=<private dir> cargo run --bin server -- -p 3942 \
 *     --database-type sqlite --database /tmp/tg402-e2e.db
 *   BASE_URL=http://127.0.0.1:3942 node packages/web/src/features/videoNote/e2e/videoNote.e2e.mjs
 *
 * Env: BASE_URL (required), SHOTS (default /tmp/tg-shots/TG-402), PLAYWRIGHT (module path,
 * default /tmp/pw/node_modules/playwright/index.mjs). Exit 0 = every step passed.
 */
import { mkdirSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { assertAllGreen, EDGE_PROBE, writeFakeCamera } from './fakeCamera.mjs'
import { createHarness } from './harness.mjs'

const BASE_URL = process.env.BASE_URL
if (!BASE_URL) throw new Error('BASE_URL is required, e.g. http://127.0.0.1:3942')
const SHOTS = process.env.SHOTS ?? '/tmp/tg-shots/TG-402'
const { chromium } = await import(process.env.PLAYWRIGHT ?? '/tmp/pw/node_modules/playwright/index.mjs')
mkdirSync(SHOTS, { recursive: true })
const CAMERA_FILE = join(tmpdir(), 'tg402-fake-camera-640x360.y4m')
writeFakeCamera(CAMERA_FILE)

const run = Date.now().toString(36)
const ALICE = { username: `na_${run}`, password: 'correct-horse-1' }
const BOB = { username: `nb_${run}`, password: 'correct-horse-2' }
const pageErrors = []
const { check, shot, api, register, waitFor, buttonCentre, hold, openAs, steps } = createHarness(BASE_URL, SHOTS)

async function videoNotes(token, chatId) {
  const page = await api(token, 'GET', `/api/chats/${chatId}/messages`)
  return page.filter((message) => message.video_note).sort((x, y) => x.created_at.localeCompare(y.created_at))
}

const noteBubbles = (page) => page.locator('.tg-message').filter({ has: page.locator('.tg-video-note') })

const browser = await chromium.launch({
  args: [
    '--use-fake-ui-for-media-stream',
    '--use-fake-device-for-media-stream',
    `--use-file-for-fake-video-capture=${CAMERA_FILE}`,
  ],
})
try {
  const permissions = ['microphone', 'camera']
  const contextA = await browser.newContext({ viewport: { width: 1280, height: 800 }, permissions })
  const contextB = await browser.newContext({ viewport: { width: 1280, height: 800 }, permissions })
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
      title: `圆视频 ${run}`,
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

  await check('the page may open the camera (Permissions-Policy camera=(self))', async () => {
    const response = await fetch(`${BASE_URL}/`)
    const policy = response.headers.get('permissions-policy') ?? ''
    if (!policy.includes('camera=(self)') || !policy.includes('microphone=(self)')) throw new Error(policy)
  })

  await check('a tap on the mic toggles to camera mode (no recording starts)', async () => {
    await a.locator('.tg-voice-mic[data-kind="voice"]').click()
    await a.locator('.tg-voice-mic[data-kind="video_note"]').waitFor()
    await a.getByRole('button', { name: '按住录制视频消息' }).waitFor()
    if (await a.locator('.tg-voice-rec').count()) throw new Error('a tap started recording')
    await shot(a, 'a-camera-mode')
  })

  await check('hold: round viewfinder, centre-cropped from the 16:9 camera; B sees «正在录制视频消息»', async () => {
    const native = await a.evaluate(async () => {
      const stream = await navigator.mediaDevices.getUserMedia({ video: true })
      const { width, height } = stream.getVideoTracks()[0].getSettings()
      for (const track of stream.getTracks()) track.stop()
      return [width, height]
    })
    if (native[0] !== 640 || native[1] !== 360) throw new Error(`the fake camera is ${native}, not 640×360`)
    const { x, y } = await buttonCentre(a)
    await a.mouse.move(x, y)
    await a.mouse.down()
    await a.locator('.tg-video-note-rec__canvas').waitFor({ timeout: 6_000 })
    await b.locator('.tg-chat__header', { hasText: '正在录制视频消息' }).waitFor({ timeout: 6_000 })
    await a.waitForTimeout(1_200)
    const canvas = a.locator('.tg-video-note-rec__canvas')
    const size = await canvas.evaluate((node) => [node.width, node.height])
    if (size[0] !== 384 || size[1] !== 384) throw new Error(`viewfinder canvas ${size}`)
    await a.evaluate(EDGE_PROBE)
    const edges = assertAllGreen(await canvas.evaluate((node) => window.__edgeProbe(node, 384)), 'viewfinder')
    const box = await a.locator('.tg-video-note-rec__lens').boundingBox()
    const radius = await a.locator('.tg-video-note-rec__lens').evaluate((node) => getComputedStyle(node).borderRadius)
    if (Math.abs(box.width - box.height) > 1 || (!radius.includes('50%') && parseFloat(radius) < box.width / 2)) {
      throw new Error(`viewfinder is not round: ${box.width}×${box.height} r=${radius}`)
    }
    console.log(`   viewfinder edges ${JSON.stringify(edges)}; lens ${Math.round(box.width)}px`)
    await a.waitForTimeout(1_000)
    await shot(a, 'a-viewfinder')
    await shot(b, 'b-sees-recording-video-note')
    await a.mouse.up()
    await noteBubbles(b).first().waitFor({ timeout: 10_000 })
    await noteBubbles(a).first().waitFor()
  })

  await check('the stored note: media_kind video_note, WebM, ~2.5 s, thumbnail, square centred file', async () => {
    const [stored] = await videoNotes(alice.token, chatId)
    if (stored.media_kind !== 'video_note' || stored.attachment.mime_type !== 'video/webm') {
      throw new Error(`unexpected ${JSON.stringify(stored).slice(0, 300)}`)
    }
    if (stored.video_note.duration_ms < 1_800 || stored.video_note.duration_ms > 3_600) {
      throw new Error(`duration ${stored.video_note.duration_ms}`)
    }
    if (!stored.video_note.thumbnail) throw new Error('no thumbnail')
    await b.evaluate(EDGE_PROBE)
    const file = await b.evaluate(
      async ([url, thumbnail]) => {
        const video = document.createElement('video')
        video.muted = true
        video.src = url
        await video.play()
        await new Promise((resolve) => setTimeout(resolve, 400))
        video.pause()
        const image = new Image()
        image.src = 'data:image/jpeg;base64,' + thumbnail
        await image.decode()
        const probe = window.__edgeProbe
        return {
          width: video.videoWidth,
          height: video.videoHeight,
          frame: probe(video, 384),
          thumb: probe(image, image.naturalWidth),
          thumbSide: [image.naturalWidth, image.naturalHeight],
        }
      },
      [stored.attachment.download_url, stored.video_note.thumbnail],
    )
    if (file.width !== 384 || file.height !== 384) throw new Error(`stored video is ${file.width}×${file.height}`)
    assertAllGreen(file.frame, 'stored video frame')
    assertAllGreen(file.thumb, 'thumbnail')
    console.log(
      `   file ${file.width}×${file.height}, thumbnail ${file.thumbSide.join('×')}, ${stored.video_note.duration_ms} ms`,
    )
    await b.locator('.tg-chat__header', { hasText: '正在录制视频消息' }).waitFor({ state: 'detached', timeout: 7_000 })
  })

  await check(
    'B: the round bubble autoplays muted and looping in the viewport; unwatched dot on both sides',
    async () => {
      const video = noteBubbles(b).first().locator('video')
      await waitFor(
        () => video.evaluate((node) => !node.paused && node.muted && node.loop && node.currentTime > 0.2),
        'muted autoplay',
      )
      await noteBubbles(b).first().locator('.tg-video-note__unwatched').waitFor()
      await noteBubbles(a).first().locator('.tg-video-note__unwatched').waitFor()
      await shot(b, 'b-muted-preview')
    },
  )

  await check('slide left cancels: nothing is sent', async () => {
    await hold(a, 1_400, { dx: -160, dy: 0 })
    await a.waitForTimeout(800)
    if ((await videoNotes(alice.token, chatId)).length !== 1) throw new Error('a cancelled note was sent')
    if (await a.locator('.tg-video-note-rec').count()) throw new Error('viewfinder still open after cancel')
  })

  await check('slide up locks (hands-free); «发送视频消息» sends it', async () => {
    const { x, y } = await buttonCentre(a)
    await a.mouse.move(x, y)
    await a.mouse.down()
    await a.waitForTimeout(600)
    await a.mouse.move(x, y - 110, { steps: 8 })
    await a.mouse.up()
    await a.locator('.tg-voice-rec__cancel', { hasText: '取消' }).waitFor()
    await a.waitForTimeout(1_500)
    await shot(a, 'a-locked')
    await a.getByRole('button', { name: '发送视频消息' }).click()
    await waitFor(async () => (await videoNotes(alice.token, chatId)).length === 2, 'second video note')
    await noteBubbles(b).nth(1).waitFor()
  })

  await check('B taps: plays from the start with sound, enlarged, progress ring; both dots clear', async () => {
    const first = noteBubbles(b).first()
    const before = await first.locator('.tg-video-note').boundingBox()
    await first.getByRole('button', { name: /^播放视频消息/ }).click()
    await first.locator('.tg-video-note[data-active]').waitFor()
    await first.locator('.tg-video-note__ring').waitFor()
    await b.waitForTimeout(700)
    const state = await first.locator('video').evaluate((node) => ({
      muted: node.muted,
      paused: node.paused,
      loop: node.loop,
      at: node.currentTime,
    }))
    if (state.muted || state.paused || state.loop || state.at > 2)
      throw new Error(`active playback ${JSON.stringify(state)}`)
    const after = await first.locator('.tg-video-note').boundingBox()
    if (!(after.width > before.width * 1.2)) throw new Error(`not enlarged: ${before.width} → ${after.width}`)
    await first.locator('.tg-video-note__unwatched').waitFor({ state: 'detached' })
    await noteBubbles(a).first().locator('.tg-video-note__unwatched').waitFor({ state: 'detached', timeout: 5_000 })
    await shot(b, 'b-playing-enlarged')
    await shot(a, 'a-watched-dot-cleared')
    const [stored] = await videoNotes(alice.token, chatId)
    if (!stored.video_note.listened) throw new Error('server does not report watched to the sender')
  })

  await check('tap pauses; at the end it shrinks back into the muted preview', async () => {
    const first = noteBubbles(b).first()
    await first.getByRole('button', { name: /^暂停视频消息/ }).click()
    await first.getByRole('button', { name: /^继续播放视频消息/ }).waitFor()
    await first.getByRole('button', { name: /^继续播放视频消息/ }).click()
    await first.locator('.tg-video-note[data-active]').waitFor({ state: 'detached', timeout: 8_000 })
    await waitFor(
      () => first.locator('video').evaluate((node) => !node.paused && node.muted && node.loop),
      'preview resumes',
    )
  })

  await check('off-screen notes pause; scrolled back into view they play again', async () => {
    const composer = b.locator('textarea').first()
    for (let index = 0; index < 18; index += 1) {
      await composer.fill(`填充消息 ${index}`)
      await composer.press('Enter')
    }
    await b.locator('.tg-message', { hasText: '填充消息 17' }).waitFor()
    await b.waitForTimeout(600)
    const first = noteBubbles(b).first()
    const offscreen = (await first.count()) === 0 || (await first.locator('video').evaluate((node) => node.paused))
    if (!offscreen) throw new Error('an off-screen video note keeps playing')
    await first.scrollIntoViewIfNeeded()
    await waitFor(
      () =>
        noteBubbles(b)
          .first()
          .locator('video')
          .evaluate((node) => !node.paused),
      'plays again in view',
    )
  })

  await check('reduced motion: no autoplay, the thumbnail with a play badge', async () => {
    const { page: r, context: contextR } = await openAs(browser, b, `/chat/${chatId}`, {
      permissions,
      reducedMotion: 'reduce',
      errors: pageErrors,
    })
    const note = noteBubbles(r).last()
    await note.waitFor()
    await note.scrollIntoViewIfNeeded()
    await r.waitForTimeout(1_200)
    if (!(await note.locator('video').evaluate((node) => node.paused)))
      throw new Error('autoplays under reduced motion')
    await note.locator('.tg-video-note__play').waitFor()
    await shot(r, 'r-reduced-motion')
    await contextR.close()
  })

  await check('a second tap toggles back to the microphone', async () => {
    await a.locator('.tg-voice-mic[data-kind="video_note"]').click()
    await a.locator('.tg-voice-mic[data-kind="voice"]').waitFor()
  })

  await check('Safari fallback: with only MP4 the recorder writes MP4 and the server accepts it', async () => {
    const { page: s, context: contextS } = await openAs(browser, a, `/chat/${chatId}`, {
      permissions,
      errors: pageErrors,
      init: () => {
        const original = MediaRecorder.isTypeSupported.bind(MediaRecorder)
        MediaRecorder.isTypeSupported = (type) => type.startsWith('video/mp4') && original(type)
        localStorage.setItem('tg.recordMode.v1', 'video')
      },
    })
    await s.locator('.tg-voice-mic[data-kind="video_note"]').waitFor()
    await hold(s, 2_300)
    const stored = await waitFor(async () => (await videoNotes(alice.token, chatId))[2], 'mp4 video note')
    if (stored.attachment.mime_type !== 'video/mp4') throw new Error(`stored ${stored.attachment.mime_type}`)
    if (stored.video_note.duration_ms < 1_300 || stored.video_note.duration_ms > 3_000) {
      throw new Error(`mp4 duration ${stored.video_note.duration_ms}`)
    }
    console.log(`   mp4 note: ${stored.attachment.size_bytes} bytes, ${stored.video_note.duration_ms} ms`)
    await b.reload()
    await b.locator(`.tg-video-note video[src="${stored.attachment.download_url}"]`).waitFor({ state: 'attached' })
    await contextS.close()
  })

  await check('dark theme, phone width: the circle and the viewfinder', async () => {
    const { page: d, context: contextD } = await openAs(browser, b, `/chat/${chatId}`, {
      viewport: { width: 390, height: 780 },
      permissions,
      colorScheme: 'dark',
      deviceScaleFactor: 2,
      init: () => localStorage.setItem('tg.recordMode.v1', 'video'),
    })
    await noteBubbles(d).last().scrollIntoViewIfNeeded()
    await d.waitForTimeout(1_000)
    await shot(d, 'd-dark-phone-bubbles')
    const { x, y } = await buttonCentre(d)
    await d.mouse.move(x, y)
    await d.mouse.down()
    await d.locator('.tg-video-note-rec__canvas').waitFor({ timeout: 6_000 })
    await d.waitForTimeout(1_500)
    await shot(d, 'd-dark-phone-viewfinder')
    await d.mouse.move(x - 200, y, { steps: 6 })
    await d.mouse.up()
    await contextD.close()
  })

  if (pageErrors.length) throw new Error(`page errors:\n${pageErrors.join('\n')}`)
  console.log(`all ${steps()} steps passed; screenshots in ${SHOTS}`)
} finally {
  await browser.close()
}
