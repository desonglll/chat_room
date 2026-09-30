#!/usr/bin/env node
/**
 * TG-303 sticker panel benchmark driver (headless Chromium via Playwright).
 *
 * Builds `bench/index.html` in production mode, serves it with `vite preview`, installs
 * 12 × 25 = 300 synthetic TGS stickers, and measures:
 *
 *   open     — click-to-first-paint and long tasks while the panel settles (virtual vs naive)
 *   scroll   — page fps / long tasks while the grid scrolls top → bottom
 *   suggest  — keystroke → suggestion strip in the DOM, for one typed emoji (5 runs)
 *
 * Usage (from packages/web):
 *   node src/features/sticker/bench/measurePanel.mjs [--json out.json] [--shots dir]
 *
 * Exit code 1 when a gate in THRESHOLDS fails. Rationale: docs/devlog/TG-303.md "Benchmark".
 */
import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs'
import { loadavg, tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const HERE = dirname(fileURLToPath(import.meta.url))
const WEB_ROOT = resolve(HERE, '../../../..')
const args = process.argv.slice(2)
const jsonOut = args.includes('--json') ? args[args.indexOf('--json') + 1] : null
const shots = args.includes('--shots') ? args[args.indexOf('--shots') + 1] : null
if (shots) mkdirSync(shots, { recursive: true })
const OPEN_SAMPLE_MS = 3000
const WARM_RUNS = 3
const median = (runs, key) => [...runs].sort((a, b) => key(a) - key(b))[Math.floor(runs.length / 2)]
const SCROLL_MS = 3000

// Gates apply to the warm virtual run: the page has already rendered this UI once (code,
// fonts and styles warm, as in a running app) but has never seen the measured stickers.
// The cold first open of a fresh page is reported, not gated: it is dominated by one-off
// first layout (font fallback), identical with 5 or 300 stickers (see the devlog).
const THRESHOLDS = {
  // Opening never blocks input. Median run: not a single long task (> 50 ms) while the panel
  // mounts and settles, on screen within 50 ms. Every run: no task over the 100 ms RAIL
  // response budget.
  open: { maxLongTaskMs: 0, totalBlockingMs: 0, firstPaintMs: 50, everyRunMaxLongTaskMs: 100 },
  // Scrolling 300 stickers keeps the page at display rate.
  scroll: { fps: 50, maxLongTaskMs: 100 },
  // Card acceptance: suggestions within 200 ms of the keystroke.
  suggest: { maxMs: 200 },
}

async function buildAndServe() {
  const vite = await import(join(WEB_ROOT, 'node_modules/vite/dist/node/index.js'))
  const outDir = mkdtempSync(join(tmpdir(), 'tg-303-bench-'))
  await vite.build({
    root: WEB_ROOT,
    logLevel: 'warn',
    build: { outDir, emptyOutDir: true, rollupOptions: { input: join(HERE, 'index.html') } },
  })
  const server = await vite.preview({ root: WEB_ROOT, logLevel: 'warn', build: { outDir }, preview: { port: 0 } })
  const base = server.resolvedUrls?.local?.[0] ?? `http://127.0.0.1:${server.httpServer.address().port}/`
  return { server, url: new URL('src/features/sticker/bench/index.html', base).href }
}

async function page(browser, url, query) {
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 } })
  const tab = await context.newPage()
  // Long tasks with their durations, observed for the page's whole life.
  await tab.addInitScript(() => {
    window.__longTasks = []
    new PerformanceObserver((list) => {
      for (const entry of list.getEntries()) window.__longTasks.push({ at: entry.startTime, ms: entry.duration })
    }).observe({ type: 'longtask', buffered: true })
  })
  await tab.goto(`${url}?${query}`)
  await tab.waitForFunction(() => window.__panelBench?.ready === true, null, { timeout: 120_000 })
  await tab.waitForTimeout(500)
  return { context, tab }
}

const longTasksSince = (tab, since) =>
  tab.evaluate((t) => window.__longTasks.filter((task) => task.at >= t).map((task) => task.ms), since)

function blocking(tasks) {
  return {
    longTasks: tasks.length,
    maxLongTaskMs: Math.round(Math.max(0, ...tasks)),
    totalBlockingMs: Math.round(tasks.reduce((sum, ms) => sum + Math.max(0, ms - 50), 0)),
  }
}

async function openAndScroll(browser, url, mode, warm) {
  const { context, tab } = await page(browser, url, `mode=${mode}`)
  if (warm) await tab.evaluate(() => window.__panelBench.warmUp())
  const setup = await tab.evaluate(() => ({
    setupMs: window.__panelBench.setupMs,
    stickers: window.__panelBench.stickers,
  }))
  const t0 = await tab.evaluate(() => performance.now())
  const open = await tab.evaluate((ms) => window.__panelBench.open(ms), OPEN_SAMPLE_MS)
  const openBlocking = blocking(await longTasksSince(tab, t0))
  if (shots) await tab.screenshot({ path: join(shots, `panel-${mode}-${warm ? 'warm' : 'cold'}.png`) })
  const t1 = await tab.evaluate(() => performance.now())
  const scroll = await tab.evaluate((ms) => window.__panelBench.scroll(ms), SCROLL_MS)
  const scrollBlocking = blocking(await longTasksSince(tab, t1))
  await context.close()
  const name = `${mode}-${warm ? 'warm' : 'cold'}`
  return { mode: name, ...setup, open: { ...open, ...openBlocking }, scroll: { ...scroll, ...scrollBlocking } }
}

async function suggest(browser, url) {
  const { context, tab } = await page(browser, url, 'mode=virtual&sets=12&per=25')
  const runs = []
  for (let run = 0; run < 5; run += 1) {
    await tab.fill('#draft', '')
    await tab.waitForTimeout(200)
    const watch = tab.evaluate(() => window.__panelBench.suggestWatch())
    await tab.focus('#draft')
    await tab.keyboard.insertText(['😀', '🐱', '👍', '🔥', '😍'][run])
    runs.push(await watch)
  }
  if (shots) await tab.screenshot({ path: join(shots, 'suggest.png') })
  await context.close()
  return { runs, maxMs: Math.max(...runs) }
}

const playwright = await import(process.env.PLAYWRIGHT_MODULE ?? '/tmp/pw/node_modules/playwright/index.mjs')
const { server, url } = await buildAndServe()
const browser = await playwright.chromium.launch()
const failures = []
const report = {
  load: loadavg()
    .map((v) => v.toFixed(1))
    .join(' '),
}
try {
  report.cold = await openAndScroll(browser, url, 'virtual', false)
  // The warm open is gated on the median of WARM_RUNS fresh pages: the host is shared, and a
  // single run swings by 2–4× under load (see the devlog).
  report.warmRuns = []
  for (let run = 0; run < WARM_RUNS; run += 1) report.warmRuns.push(await openAndScroll(browser, url, 'virtual', true))
  report.virtual = median(report.warmRuns, (run) => run.open.firstPaintMs)
  report.naive = await openAndScroll(browser, url, 'naive', true)
  report.suggest = await suggest(browser, url)
} finally {
  await browser.close()
  await server.close()
}

const { open, scroll } = report.virtual
if (open.maxLongTaskMs > THRESHOLDS.open.maxLongTaskMs) failures.push(`open max long task ${open.maxLongTaskMs} ms`)
if (open.totalBlockingMs > THRESHOLDS.open.totalBlockingMs) failures.push(`open blocking ${open.totalBlockingMs} ms`)
if (open.firstPaintMs > THRESHOLDS.open.firstPaintMs) failures.push(`open first paint ${open.firstPaintMs} ms`)
for (const run of report.warmRuns) {
  if (run.open.maxLongTaskMs > THRESHOLDS.open.everyRunMaxLongTaskMs)
    failures.push(`a warm run had a ${run.open.maxLongTaskMs} ms task`)
}
if (scroll.fps < THRESHOLDS.scroll.fps) failures.push(`scroll fps ${scroll.fps}`)
if (scroll.maxLongTaskMs > THRESHOLDS.scroll.maxLongTaskMs) failures.push(`scroll long task ${scroll.maxLongTaskMs} ms`)
if (report.suggest.maxMs > THRESHOLDS.suggest.maxMs) failures.push(`suggest ${report.suggest.maxMs} ms`)

console.log(`load average ${report.load}`)
for (const run of [report.cold, ...report.warmRuns, report.naive]) {
  const o = run.open
  const s = run.scroll
  console.log(
    `${run.mode.padEnd(8)} ${run.stickers} stickers | open: mount ${o.mountMs} ms, paint ${o.firstPaintMs} ms, ` +
      `mounted ${o.mountedStickers}, decoded ${o.decoded}, long ${o.longTasks} (max ${o.maxLongTaskMs} ms, TBT ${o.totalBlockingMs} ms), fps ${o.fps} | ` +
      `scroll: fps ${s.fps}, p95 ${s.p95FrameMs} ms, long ${s.longTasks} (max ${s.maxLongTaskMs} ms), mounted ${s.maxMounted}`,
  )
}
console.log(`gated warm run (median first paint): ${report.warmRuns.indexOf(report.virtual) + 1} of ${WARM_RUNS}`)
console.log(`suggest  keystroke → strip: ${report.suggest.runs.join(', ')} ms (max ${report.suggest.maxMs})`)
console.log(failures.length === 0 ? 'PASS' : `FAIL ${failures.join('; ')}`)
if (jsonOut) writeFileSync(jsonOut, JSON.stringify(report, null, 2))
process.exit(failures.length === 0 ? 0 : 1)
