#!/usr/bin/env node
/**
 * TG-306 browser verification of `<Sticker>` on the mixed-format fixture page (mixed.html).
 *
 * Serves packages/web with the Vite dev server, opens the page in headless Chromium and
 * checks, per scenario, what the user would see; screenshots go to --shots (default
 * /tmp/tg-shots/TG-306). Exit code 1 on the first failed check.
 *
 *   live            TGS live, WebP decoded, WebM videos playing — all in the one cap
 *   offscreen       scrolled away: every video paused (currentTime frozen), none admitted
 *   reduced-motion  videos held on the first frame, TGS on its still
 *   no-canplaytype  HTMLMediaElement.canPlayType denies WebM → thumbnail / emoji fallback
 *   safari-ua       a Safari user agent → the same fallback (VP9 alpha is not trusted there)
 *
 * Usage (from packages/web): node src/features/sticker/renderer/bench/mixedShots.mjs [--shots dir]
 */
import { mkdirSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { createServer } from 'vite'

const HERE = dirname(fileURLToPath(import.meta.url))
const WEB_ROOT = resolve(HERE, '../../../../..')
const args = process.argv.slice(2)
const shots = args.includes('--shots') ? args[args.indexOf('--shots') + 1] : '/tmp/tg-shots/TG-306'
mkdirSync(shots, { recursive: true })
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE ?? '/tmp/pw/node_modules/playwright/index.mjs')

const SAFARI_UA =
  'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.6 Safari/605.1.15'
const DENY_WEBM = `(() => {
  const original = HTMLMediaElement.prototype.canPlayType
  HTMLMediaElement.prototype.canPlayType = function (type) {
    return /webm/i.test(type) ? '' : original.call(this, type)
  }
})()`

const server = await createServer({ root: WEB_ROOT, logLevel: 'error', server: { port: 0, strictPort: false } })
await server.listen()
const url = `${server.resolvedUrls.local[0]}src/features/sticker/renderer/bench/mixed.html`
const browser = await chromium.launch({ args: ['--autoplay-policy=no-user-gesture-required'] })
const results = []
let failed = false
const check = (scenario, name, ok, detail) => {
  results.push({ scenario, name, ok, detail })
  if (!ok) failed = true
  console.log(
    `${ok ? 'PASS' : 'FAIL'} ${scenario}: ${name}${detail === undefined ? '' : ` — ${JSON.stringify(detail)}`}`,
  )
}

const snapshot = (page) =>
  page.evaluate(() => {
    const cells = [...document.querySelectorAll('.tg-sticker')].map((el) => ({
      format: el.getAttribute('data-format') ?? (el.querySelector('canvas') ? 'tgs' : null),
      phase: el.getAttribute('data-phase'),
      fallback: el.getAttribute('data-fallback'),
    }))
    const videos = [...document.querySelectorAll('video')].map((v) => ({
      paused: v.paused,
      t: Number(v.currentTime.toFixed(3)),
      ready: v.readyState,
      muted: v.muted,
    }))
    const webp = [...document.querySelectorAll('[data-format="webp"] img')].map((img) => img.naturalWidth)
    return { cells, videos, webp, stats: window.__mixed.stats() }
  })

async function open(scenario, contextOptions = {}, init = null, query = '') {
  const context = await browser.newContext({ viewport: { width: 520, height: 600 }, ...contextOptions })
  if (init) await context.addInitScript(init)
  const page = await context.newPage()
  page.on('pageerror', (error) => check(scenario, 'no page error', false, String(error)))
  await page.goto(url + query)
  await page.waitForFunction(() => window.__mixed?.ready, null, { timeout: 30_000 })
  const error = await page.evaluate(() => window.__mixed.error)
  if (error) check(scenario, 'fixture media generated', false, error)
  await page.waitForTimeout(2500)
  return { context, page }
}

const shot = (page, name) => page.locator('.mixed-panel').screenshot({ path: `${shots}/${name}.png` })

{
  const { context, page } = await open('live')
  const s = await snapshot(page)
  await shot(page, 'mixed-live')
  await page.waitForTimeout(250)
  const s2 = await snapshot(page)
  check(
    'live',
    'three WebM videos, all playing (time advancing), muted, data-phase live',
    s.videos.length === 3 &&
      s.videos.every((v, i) => !v.paused && v.muted && v.t !== s2.videos[i].t) &&
      s.cells.filter((c) => c.format === 'webm').every((c) => c.phase === 'live'),
    [s.videos, s2.videos.map((v) => v.t)],
  )
  check('live', 'WebP decoded', s.webp.length === 3 && s.webp.every((w) => w > 0), s.webp)
  check('live', 'TGS live', s.cells.filter((c) => c.format === 'tgs' && c.phase === 'live').length === 3, s.cells)
  check('live', 'WebM counted in the manager cap', s.stats.playingMotionViews === 3, s.stats)

  await page.evaluate(() => window.scrollTo(0, document.body.scrollHeight))
  await page.waitForTimeout(600)
  const away = await snapshot(page)
  await page.waitForTimeout(600)
  const later = await snapshot(page)
  check('offscreen', 'every video paused', away.videos.length === 3 && away.videos.every((v) => v.paused), away.videos)
  check(
    'offscreen',
    'no frame advances while paused',
    away.videos.every((v, i) => v.t === later.videos[i].t),
    later.videos.map((v) => v.t),
  )
  check(
    'offscreen',
    'nothing admitted',
    later.stats.playingMotionViews === 0 && later.stats.runningGroups === 0,
    later.stats,
  )
  await page.evaluate(() => window.scrollTo(0, 0))
  await page.waitForTimeout(800)
  const back = await snapshot(page)
  check(
    'offscreen',
    'resumes on scroll back',
    back.videos.length === 3 && back.videos.every((v) => !v.paused),
    back.videos,
  )
  await context.close()
}

{
  const { context, page } = await open('reduced-motion', { reducedMotion: 'reduce' })
  const s = await snapshot(page)
  await shot(page, 'mixed-reduced-motion')
  check(
    'reduced-motion',
    'videos held on the first frame',
    s.videos.length === 3 && s.videos.every((v) => v.paused && v.t === 0 && v.ready >= 2),
    s.videos,
  )
  check(
    'reduced-motion',
    'nothing live',
    s.cells.every((c) => c.phase === 'static'),
    s.cells,
  )
  await context.close()
}

for (const [scenario, options, init] of [
  ['no-canplaytype', {}, DENY_WEBM],
  ['safari-ua', { userAgent: SAFARI_UA }, null],
]) {
  const { context, page } = await open(scenario, options, init)
  const s = await snapshot(page)
  await shot(page, `mixed-webm-fallback-${scenario}`)
  const webm = s.cells.filter((c) => c.format === 'webm')
  check(scenario, 'no <video> created', s.videos.length === 0, s.videos)
  check(
    scenario,
    'all three WebM show the fallback',
    webm.length === 3 && webm.every((c) => c.fallback === 'unsupported' && c.phase === 'static'),
    webm,
  )
  const kinds = await page.evaluate(() =>
    [...document.querySelectorAll('[data-fallback]')].map((el) =>
      el.querySelector('img') ? 'thumbnail' : el.querySelector('.tg-sticker__emoji')?.textContent,
    ),
  )
  check(scenario, 'thumbnail when present, else the emoji', kinds.join(',') === '🥝,thumbnail,🥝', kinds)
  check(
    scenario,
    'TGS and WebP unaffected',
    s.cells.filter((c) => c.format !== 'webm').every((c) => c.phase === 'live' || c.phase === 'static'),
    s.cells,
  )
  await context.close()
}

await browser.close()
await server.close()
console.log(`${results.filter((r) => r.ok).length}/${results.length} checks passed; screenshots in ${shots}`)
process.exit(failed ? 1 : 0)
