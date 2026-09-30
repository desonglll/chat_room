/**
 * TG-101 browser benchmark: bundles `harness.tsx`, serves it, drives headless Chromium.
 *
 *   node packages/web/src/features/messageList/bench/runBrowserBench.mjs
 *
 * Env: PLAYWRIGHT_DIR (default /tmp/pw/node_modules), SHOTS (default /tmp/tg-shots/TG-101).
 * Prints one line per measurement and exits 1 when a budget in docs/devlog/TG-101.md is
 * exceeded. Dev-only; never bundled into the app.
 */
import { spawnSync } from 'node:child_process'
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { createServer } from 'node:http'
import { createRequire } from 'node:module'
import { dirname, extname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const here = dirname(fileURLToPath(import.meta.url))
const webRoot = join(here, '../../../..')
const out = '/tmp/tg-bench'
const shots = process.env.SHOTS ?? '/tmp/tg-shots/TG-101'
const require = createRequire(join(process.env.PLAYWRIGHT_DIR ?? '/tmp/pw/node_modules', 'noop.js'))
const { chromium } = require('playwright')

export const BUDGET = {
  prependFinalShiftPx: 0,
  prependMaxTransientPx: 1,
  prependBlankFrames: 0,
  returnShiftPx: 1,
  longFramePercent: 5,
  listHeapMB100k: 24,
  domRows: 120,
}

function build() {
  mkdirSync(out, { recursive: true })
  mkdirSync(shots, { recursive: true })
  const result = spawnSync(
    'bun',
    ['build', join(here, 'harness.tsx'), '--outdir', out, '--target', 'browser', '--production'],
    { cwd: webRoot, stdio: 'inherit' },
  )
  if (result.status !== 0) throw new Error('harness build failed')
  writeFileSync(
    join(out, 'index.html'),
    '<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><link rel="stylesheet" href="harness.css"></head>' +
      '<body style="margin:0"><div id="root"></div><script type="module" src="harness.js"></script></body></html>',
  )
}

function serve() {
  const types = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css' }
  const server = createServer((request, response) => {
    const path = new URL(request.url, 'http://x').pathname
    try {
      const body = readFileSync(join(out, path === '/' ? 'index.html' : path))
      response.writeHead(200, { 'content-type': types[extname(path) || '.html'] ?? 'application/octet-stream' })
      response.end(body)
    } catch {
      response.writeHead(404)
      response.end()
    }
  })
  return new Promise((resolve) => server.listen(0, '127.0.0.1', () => resolve(server)))
}

const results = []
function record(name, value, budget, unit) {
  results.push({ name, value, budget, unit })
  const ok = value <= budget
  console.log(
    `${ok ? 'PASS' : 'FAIL'} ${name.padEnd(26)} ${String(value).padStart(9)} ${unit}  (budget ${budget} ${unit})`,
  )
}

async function openPage(browser, base, query) {
  const page = await browser.newPage({ viewport: { width: 900, height: 800 } })
  page.on('pageerror', (error) => console.error('pageerror:', error.message))
  await page.goto(`${base}/?${query}`)
  await page.waitForSelector('[data-row-key]')
  await page.waitForTimeout(300)
  return page
}

/**
 * Track what the reader SEES, one sample per rendered frame, taken at paint time: a
 * ResizeObserver on a probe resized in every rAF is delivered after layout and after
 * Virtuoso's own (earlier-created) observers, immediately before paint. (A setTimeout
 * sample can land between a React commit and the next frame and report a state that is
 * never painted.) The first painted frame after `scrollTop` is applied fixes the anchor:
 * its topmost fully visible row. Every later frame reports that row's offset from where it
 * was, and whether rendered rows covered the whole viewport (else: a blank frame).
 */
function trackPaintedAnchor({ scrollTop, durationMs }) {
  return new Promise((resolve) => {
    const scroller = document.querySelector('.tg-mlist__scroller')
    const viewport = () => scroller.getBoundingClientRect()
    const rows = () => [...scroller.querySelectorAll('[data-row-key]')]
    let anchor = null
    const deltas = []
    let blankFrames = 0
    const sample = () => {
      const box = viewport()
      const rendered = rows()
      if (!anchor) {
        const first = rendered.find((row) => row.getBoundingClientRect().top >= box.top)
        if (first) anchor = { key: first.dataset.rowKey, top: first.getBoundingClientRect().top - box.top }
        return
      }
      const row = rendered.find((element) => element.dataset.rowKey === anchor.key)
      deltas.push(row ? row.getBoundingClientRect().top - box.top - anchor.top : null)
      const firstTop = rendered.length ? rendered[0].getBoundingClientRect().top : Infinity
      const lastBottom = rendered.length ? rendered[rendered.length - 1].getBoundingClientRect().bottom : -Infinity
      const coversTop = firstTop <= box.top + 1 || scroller.scrollTop <= 1
      if (!coversTop || lastBottom < box.bottom - 1) blankFrames += 1
    }
    if (scrollTop !== null) scroller.scrollTop = scrollTop
    const probe = document.createElement('div')
    probe.style.cssText = 'position:fixed;left:0;top:0;height:1px;width:1px;pointer-events:none'
    document.body.append(probe)
    const observer = new ResizeObserver(sample)
    observer.observe(probe)
    const start = performance.now()
    let wide = false
    const tick = () => {
      wide = !wide
      probe.style.width = wide ? '2px' : '1px'
      if (performance.now() - start < durationMs) requestAnimationFrame(tick)
      else {
        observer.disconnect()
        probe.remove()
        resolve({ anchor, deltas, blankFrames })
      }
    }
    requestAnimationFrame(tick)
  })
}

async function prependScenario(browser, base) {
  const latency = Number(process.env.LATENCY ?? 60)
  const page = await openPage(browser, base, `total=100000&live=100&latency=${latency}`)
  await page.screenshot({ path: join(shots, '01-live-bottom.png') })
  let finalMax = 0
  let transientMax = 0
  let blank = 0
  const iterations = Number(process.env.ITERATIONS ?? 25)
  for (let i = 0; i < iterations; i += 1) {
    const before = await page.evaluate(() => window.tgBench.requests.length)
    // Park the viewport near the top of what is loaded (this triggers the older page),
    // then keep hands off while the page lands.
    const { anchor, deltas, blankFrames } = await page.evaluate(trackPaintedAnchor, {
      scrollTop: 40,
      durationMs: latency + 600,
    })
    const after = await page.evaluate(() => window.tgBench.requests.length)
    if (after === before) throw new Error(`iteration ${i}: no older page was requested`)
    const valid = deltas.filter((value) => typeof value === 'number')
    if (valid.length === 0) throw new Error(`iteration ${i}: the anchor row was never painted`)
    finalMax = Math.max(finalMax, Math.abs(valid[valid.length - 1] ?? Infinity))
    for (const value of valid) transientMax = Math.max(transientMax, Math.abs(value))
    transientMax = Math.max(transientMax, valid.length < deltas.length ? Infinity : 0)
    blank += blankFrames
    if (process.env.BENCH_DEBUG) {
      console.log(i, anchor, blankFrames, deltas.map((v) => (v === null ? 'x' : Math.round(v))).join(' '))
    }
  }
  await page.screenshot({ path: join(shots, '02-after-prepends.png') })
  record('prependFinalShiftPx', Math.round(finalMax * 100) / 100, BUDGET.prependFinalShiftPx, 'px')
  record('prependMaxTransientPx', Math.round(transientMax * 100) / 100, BUDGET.prependMaxTransientPx, 'px')
  record('prependBlankFrames', blank, BUDGET.prependBlankFrames, 'frames')
  await page.close()
}

const firstVisibleRow = () => {
  const scroller = document.querySelector('.tg-mlist__scroller')
  const top = scroller.getBoundingClientRect().top
  for (const row of scroller.querySelectorAll('[data-row-key]')) {
    const rect = row.getBoundingClientRect()
    if (rect.top >= top) return { key: row.dataset.rowKey, top: rect.top - top }
  }
  return null
}

const rowOffset = (key) => {
  const scroller = document.querySelector('.tg-mlist__scroller')
  const row = scroller.querySelector(`[data-row-key="${key}"]`)
  return row ? row.getBoundingClientRect().top - scroller.getBoundingClientRect().top : null
}

const distanceToBottom = () => {
  const scroller = document.querySelector('.tg-mlist__scroller')
  return scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight
}

async function settle(page, ms = 400) {
  await page.waitForTimeout(ms)
  await page.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))))
}

async function jumpScenario(browser, base) {
  const page = await openPage(browser, base, 'total=100000&live=100&latency=60')
  // Read a little above the bottom so the return point is a real mid-list position.
  await page.evaluate(() => {
    document.querySelector('.tg-mlist__scroller').scrollTop -= 240
  })
  await settle(page)
  const anchor = await page.evaluate(firstVisibleRow)
  const far = await page.evaluate(() => window.tgBench.farTarget)
  const quoting = await page.evaluate(() => window.tgBench.idOf(window.tgBench.total - 5))
  await page.click(`[data-row-key="${quoting}"] .tg-mlist-default__reply`)
  await page.waitForSelector(`[data-row-key="${far}"][data-highlighted]`, { timeout: 5000 })
  await settle(page, 200)
  const targetTop = await page.evaluate(rowOffset, far)
  const viewportHeight = await page.evaluate(() => document.querySelector('.tg-mlist__scroller').clientHeight)
  record('jumpTargetInViewport', targetTop !== null && targetTop >= 0 && targetTop < viewportHeight ? 0 : 1, 0, 'miss')
  await page.screenshot({ path: join(shots, '03-jumped-50k-back.png') })
  await page.click('button[aria-label="返回原位置"]')
  await page.waitForSelector(`[data-row-key="${anchor.key}"]`, { timeout: 5000 })
  await settle(page)
  const back = await page.evaluate(rowOffset, anchor.key)
  record(
    'returnShiftPx',
    back === null ? Infinity : Math.round(Math.abs(back - anchor.top) * 100) / 100,
    BUDGET.returnShiftPx,
    'px',
  )
  await page.screenshot({ path: join(shots, '04-returned.png') })
  await page.close()
}

async function followScenario(browser, base) {
  const page = await openPage(browser, base, 'total=100000&live=100&latency=60')
  for (let i = 0; i < 5; i += 1) {
    await page.evaluate(() => window.tgBench.pushIncoming())
    await page.waitForTimeout(120)
  }
  await settle(page, 600)
  record('followAtBottomPx', Math.round(await page.evaluate(distanceToBottom)), 2, 'px')

  await page.evaluate(() => {
    document.querySelector('.tg-mlist__scroller').scrollTop -= 800
  })
  await settle(page)
  const anchor = await page.evaluate(firstVisibleRow)
  for (let i = 0; i < 3; i += 1) await page.evaluate(() => window.tgBench.pushIncoming())
  await settle(page)
  const after = await page.evaluate(rowOffset, anchor.key)
  record('noFollowShiftPx', after === null ? Infinity : Math.abs(after - anchor.top), 0, 'px')
  const badge = await page.evaluate(() => document.querySelector('.tg-mlist__fab-badge')?.textContent ?? '')
  record('unreadBadgeError', Math.abs(Number(badge || 0) - 3), 0, 'count')
  await page.screenshot({ path: join(shots, '05-unread-badge.png') })

  await page.evaluate(() => window.tgBench.pushOwnPending())
  await settle(page, 700)
  record('ownSendBottomPx', Math.round(await page.evaluate(distanceToBottom)), 2, 'px')
  await page.close()
}

async function scrollScenario(browser, base) {
  // All 100k messages in the live timeline: the heaviest steady state the list can be in.
  const page = await openPage(browser, base, 'total=100000&live=100000&latency=60')
  await page.mouse.move(450, 400)
  const frames = page.evaluate(
    (durationMs) =>
      new Promise((resolve) => {
        const deltas = []
        let last = performance.now()
        const start = last
        const tick = (now) => {
          deltas.push(now - last)
          last = now
          if (now - start < durationMs) requestAnimationFrame(tick)
          else resolve(deltas)
        }
        requestAnimationFrame(tick)
      }),
    3_000,
  )
  for (let i = 0; i < 120; i += 1) {
    await page.mouse.wheel(0, i < 80 ? -160 : 160)
    await page.waitForTimeout(16)
  }
  const deltas = (await frames).slice(1)
  const sorted = deltas.slice().sort((a, b) => a - b)
  const median = sorted[Math.floor(sorted.length / 2)]
  const long = deltas.filter((delta) => delta > 50).length
  console.log(
    `info frames=${deltas.length} median=${median.toFixed(1)}ms p95=${sorted[Math.floor(sorted.length * 0.95)].toFixed(1)}ms max=${sorted[sorted.length - 1].toFixed(1)}ms`,
  )
  record('longFramePercent', Math.round((long / deltas.length) * 1000) / 10, BUDGET.longFramePercent, '%')
  record(
    'domRows',
    await page.evaluate(() => document.querySelectorAll('[data-row-key]').length),
    BUDGET.domRows,
    'rows',
  )
  await page.screenshot({ path: join(shots, '06-100k-scrolled.png') })
  await page.close()
}

async function heapUsed(browser, base, query) {
  const page = await openPage(browser, base, query).catch(async () => {
    // mount=0 renders no rows: open without waiting for one.
    const bare = await browser.newPage({ viewport: { width: 900, height: 800 } })
    await bare.goto(`${base}/?${query}`)
    await bare.waitForTimeout(1500)
    return bare
  })
  const session = await page.context().newCDPSession(page)
  await session.send('HeapProfiler.collectGarbage')
  await session.send('HeapProfiler.collectGarbage')
  const { usedSize } = await session.send('Runtime.getHeapUsage')
  await page.close()
  return usedSize
}

async function memoryScenario(browser, base) {
  const withList = await heapUsed(browser, base, 'total=100000&live=100000')
  const storeOnly = await heapUsed(browser, base, 'total=100000&live=100000&mount=0')
  console.log(
    `info heap with list ${(withList / 2 ** 20).toFixed(1)} MB, store only ${(storeOnly / 2 ** 20).toFixed(1)} MB`,
  )
  record('listHeapMB100k', Math.round(((withList - storeOnly) / 2 ** 20) * 10) / 10, BUDGET.listHeapMB100k, 'MB')
}

async function main() {
  build()
  const server = await serve()
  const base = `http://127.0.0.1:${server.address().port}`
  const browser = await chromium.launch()
  try {
    const only = process.argv[2]
    const scenarios = {
      prepend: prependScenario,
      jump: jumpScenario,
      follow: followScenario,
      scroll: scrollScenario,
      memory: memoryScenario,
    }
    for (const [name, run] of Object.entries(scenarios)) if (!only || only === name) await run(browser, base)
  } finally {
    await browser.close()
    server.close()
  }
  if (results.some((result) => result.value > result.budget)) process.exit(1)
}

await main()
