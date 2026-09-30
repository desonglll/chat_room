#!/usr/bin/env node
/**
 * TG-301 sticker benchmark driver.
 *
 * Builds `bench/index.html` in production mode (minified React and lottie, like the shipped
 * client), serves it with `vite preview`, and measures each scenario in headless Chromium:
 *
 *   - fps / p95 frame interval / dropped intervals / long tasks: in-page rAF sampling
 *   - cpu: main-thread busy fraction from CDP `Performance.getMetrics` (TaskDuration delta
 *     over wall time), which is what "off-screen CPU ≈ 0" is judged on
 *
 * Usage (from packages/web):
 *   node src/features/sticker/renderer/bench/measure.mjs [--quick] [--json out.json] [--only name]
 *     [--shots dir]   (screenshot of each scenario after warm-up)
 *
 * Playwright is not a workspace dependency; the driver loads it from $PLAYWRIGHT_MODULE
 * (default /tmp/pw/node_modules/playwright/index.mjs). Exit code 1 when a gated scenario
 * misses its threshold (see THRESHOLDS and docs/devlog/TG-301.md "Benchmark").
 */
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { gzipSync } from 'node:zlib'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const HERE = dirname(fileURLToPath(import.meta.url))
const WEB_ROOT = resolve(HERE, '../../../../..')
const args = process.argv.slice(2)
const quick = args.includes('--quick')
const jsonOut = args.includes('--json') ? args[args.indexOf('--json') + 1] : null
const only = args.includes('--only') ? args[args.indexOf('--only') + 1] : null
const shots = args.includes('--shots') ? args[args.indexOf('--shots') + 1] : null
if (shots) mkdirSync(shots, { recursive: true })
const SAMPLE_MS = quick ? 3000 : 6000
const WARMUP_MS = quick ? 1500 : 3000

/**
 * Gates. `fps` is a floor, `cpu` a ceiling (fraction of one core). Scenarios without a gate
 * are informational (the renderer comparison and the stress runs).
 */
const THRESHOLDS = {
  // The page stays at display rate (no jank, the hard requirement); stickers keep >= 2/3 of
  // their 30 fps cap even on an oversubscribed host; the page thread stays mostly free, i.e.
  // rendering really happens off-thread. Rationale: docs/devlog/TG-301.md "Benchmark".
  'managed-20-onscreen': { fps: 50, p95FrameMs: 25, stickerFps: 20, cpu: 0.5 },
  'managed-20-offscreen': { cpu: 0.02 },
  'managed-20-hidden-tab': { cpu: 0.02 },
  'managed-20-reduced-motion': { cpu: 0.02 },
}

const raw = (renderer, extra = '') => `mode=raw&renderer=${renderer}&count=20${extra}`
// `cpuOnly` scenarios do not run the rAF sampler (it would itself keep the page rendering at
// 60 Hz); they only read main-thread busy time, which is what "off-screen ≈ 0" is about.
const SCENARIOS = [
  { name: 'cost-24-layers', query: 'mode=cost&layers=24', cost: true },
  { name: 'cost-60-layers', query: 'mode=cost&layers=60', cost: true },
  { name: 'idle-page', query: 'mode=managed&count=0', cpuOnly: true },
  { name: 'raw-svg-20', query: raw('svg') },
  { name: 'raw-canvas-20', query: raw('canvas') },
  { name: 'raw-html-20', query: raw('html') },
  { name: 'raw-svg-20-heavy', query: raw('svg', '&layers=60') },
  { name: 'raw-canvas-20-heavy', query: raw('canvas', '&layers=60') },
  { name: 'raw-svg-40', query: 'mode=raw&renderer=svg&count=40&size=100' },
  { name: 'raw-canvas-40', query: 'mode=raw&renderer=canvas&count=40&size=100' },
  { name: 'managed-20-onscreen', query: 'mode=managed&count=20&engine=worker' },
  { name: 'managed-20-onscreen-main', query: 'mode=managed&count=20&engine=main' },
  { name: 'managed-20-hidpi', query: 'mode=managed&count=20&engine=worker', dpr: 2 },
  { name: 'managed-20-onscreen-heavy', query: 'mode=managed&count=20&layers=60&engine=worker' },
  { name: 'managed-20-heavy-main', query: 'mode=managed&count=20&layers=60&engine=main' },
  { name: 'managed-20-same-sticker', query: 'mode=managed&count=20&distinct=1&engine=worker' },
  { name: 'managed-60-capped', query: 'mode=managed&count=60&distinct=60&size=100&engine=worker' },
  { name: 'managed-20-offscreen', query: 'mode=managed&count=20&offscreen=1&engine=worker', cpuOnly: true },
  { name: 'managed-20-hidden-tab', query: 'mode=managed&count=20&engine=worker', hidden: true, cpuOnly: true },
  {
    name: 'managed-20-reduced-motion',
    query: 'mode=managed&count=20&engine=worker',
    reducedMotion: true,
    cpuOnly: true,
  },
].filter((scenario) => !only || scenario.name.includes(only))

async function buildAndServe() {
  const vite = await import(join(WEB_ROOT, 'node_modules/vite/dist/node/index.js'))
  const outDir = mkdtempSync(join(tmpdir(), 'tg-301-bench-'))
  await vite.build({
    root: WEB_ROOT,
    logLevel: 'warn',
    build: { outDir, emptyOutDir: true, rollupOptions: { input: join(HERE, 'index.html') } },
  })
  // Chunk report: shows lottie-web and pako land only in lazy chunks and the worker.
  const assets = join(outDir, 'assets')
  for (const file of readdirSync(assets).filter((name) => name.endsWith('.js'))) {
    const bytes = readFileSync(join(assets, file))
    console.log(
      `  chunk ${file.padEnd(40)} ${String(bytes.length).padStart(7)} raw ${String(gzipSync(bytes).length).padStart(6)} gzip`,
    )
  }
  const server = await vite.preview({ root: WEB_ROOT, logLevel: 'warn', build: { outDir }, preview: { port: 0 } })
  const base = server.resolvedUrls?.local?.[0] ?? `http://127.0.0.1:${server.httpServer.address().port}/`
  return { server, url: new URL('src/features/sticker/renderer/bench/index.html', base).href }
}

async function measure(browser, url, scenario) {
  const context = await browser.newContext({
    viewport: { width: 1280, height: 900 },
    deviceScaleFactor: scenario.dpr ?? 1,
    reducedMotion: scenario.reducedMotion ? 'reduce' : 'no-preference',
  })
  const page = await context.newPage()
  const cdp = await context.newCDPSession(page)
  await cdp.send('Performance.enable', { timeDomain: 'timeTicks' })
  await page.goto(`${url}?${scenario.query}`)
  await page.waitForFunction(() => window.__stickerBench?.ready === true, null, { timeout: 60_000 })
  if (scenario.cost) {
    const costs = await page.evaluate(() => window.__stickerBench.mounted())
    await context.close()
    return { name: scenario.name, cost: costs }
  }
  if (scenario.hidden) {
    // Playwright pages are always "visible"; emulate the visibilitychange the renderer hears.
    await page.evaluate(() => {
      Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => 'hidden' })
      Object.defineProperty(document, 'hidden', { configurable: true, get: () => true })
      document.dispatchEvent(new Event('visibilitychange'))
    })
  }
  await page.waitForTimeout(WARMUP_MS)
  if (shots && !scenario.hidden) await page.screenshot({ path: join(shots, `${scenario.name}.png`) })
  const metric = async () => {
    const { metrics } = await cdp.send('Performance.getMetrics')
    const pick = (name) => metrics.find((entry) => entry.name === name)?.value ?? 0
    return { task: pick('TaskDuration'), script: pick('ScriptDuration'), time: pick('Timestamp') }
  }
  const statsBefore = await page.evaluate(() => window.__stickerBench.mounted())
  const before = await metric()
  const frames = scenario.cpuOnly
    ? await page.waitForTimeout(SAMPLE_MS).then(() => ({
        durationMs: SAMPLE_MS,
        fps: 0,
        p95FrameMs: 0,
        p99FrameMs: 0,
        droppedIntervals: 0,
        longTasks: 0,
        longTaskMs: 0,
      }))
    : await page.evaluate((ms) => window.__stickerBench.sample(ms), SAMPLE_MS)
  const after = await metric()
  const statsAfter = await page.evaluate(() => window.__stickerBench.mounted())
  const wall = after.time - before.time
  await context.close()
  // Raw mode reports a count (lottie draws every sticker every frame, so sticker fps = page
  // fps); managed mode reports manager stats, from which the per-sticker draw rate follows.
  const managed = typeof statsAfter === 'object'
  const groups = managed ? statsAfter.runningGroups : statsAfter
  const stickerFps = managed
    ? groups === 0
      ? 0
      : Math.round(((statsAfter.draws - statsBefore.draws) / groups / (frames.durationMs / 1000)) * 10) / 10
    : frames.fps
  return {
    name: scenario.name,
    ...frames,
    animating: managed ? statsAfter.playingViews : statsAfter,
    groups,
    stickerFps,
    deferred: managed ? statsAfter.deferred - statsBefore.deferred : 0,
    cpu: Math.round(((after.task - before.task) / wall) * 1000) / 1000,
    scriptCpu: Math.round(((after.script - before.script) / wall) * 1000) / 1000,
  }
}

function verdict(result) {
  const gate = THRESHOLDS[result.name]
  if (!gate) return 'info'
  const failures = []
  if (gate.fps !== undefined && result.fps < gate.fps) failures.push(`fps<${gate.fps}`)
  if (gate.p95FrameMs !== undefined && result.p95FrameMs > gate.p95FrameMs) failures.push(`p95>${gate.p95FrameMs}`)
  if (gate.cpu !== undefined && result.cpu > gate.cpu) failures.push(`cpu>${gate.cpu}`)
  if (gate.stickerFps !== undefined && result.stickerFps < gate.stickerFps)
    failures.push(`sticker-fps<${gate.stickerFps}`)
  return failures.length === 0 ? 'PASS' : `FAIL ${failures.join(' ')}`
}

const playwright = await import(process.env.PLAYWRIGHT_MODULE ?? '/tmp/pw/node_modules/playwright/index.mjs')
const { server, url } = await buildAndServe()
const GPU_ARGS = ['--enable-gpu', '--ignore-gpu-blocklist', '--use-angle=swiftshader', '--enable-unsafe-swiftshader']
// Default: software raster, the conservative case (no GPU in headless CI). BENCH_GPU=1 routes
// canvas through SwiftShader in the GPU process instead — measured slower here, because every
// worker's raster then funnels into one emulated-GPU thread that the compositor also needs.
const browser = await playwright.chromium.launch({ args: process.env.BENCH_GPU === '1' ? GPU_ARGS : [] })
const results = []
const load = (await import('node:os')).loadavg().map((value) => value.toFixed(1))
console.log(`load average ${load.join(' ')}; sample ${SAMPLE_MS} ms after ${WARMUP_MS} ms warm-up`)
try {
  for (const scenario of SCENARIOS) {
    const result = await measure(browser, url, scenario)
    results.push(result)
    if (result.cost) {
      const parts = Object.entries(result.cost).map(([renderer, ms]) => `${renderer} ${ms} ms/frame`)
      console.log(`${result.name.padEnd(28)}  ${parts.join('  ')}`)
      result.verdict = 'info'
      continue
    }
    result.verdict = verdict(result)
    console.log(
      [
        result.name.padEnd(28),
        `fps ${String(result.fps).padStart(5)}`,
        `p95 ${String(result.p95FrameMs).padStart(5)}ms`,
        `drop ${String(result.droppedIntervals).padStart(3)}`,
        `long ${String(result.longTasks).padStart(3)}/${String(result.longTaskMs).padStart(5)}ms`,
        `cpu ${result.cpu.toFixed(3)} js ${result.scriptCpu.toFixed(3)}`,
        `anim ${String(result.animating).padStart(2)}`,
        `sticker-fps ${String(result.stickerFps).padStart(5)}`,
        result.verdict,
      ].join('  '),
    )
  }
} finally {
  await browser.close()
  await server.close()
}
if (jsonOut) writeFileSync(jsonOut, JSON.stringify({ sampleMs: SAMPLE_MS, results }, null, 2))
process.exit(results.some((result) => result.verdict.startsWith('FAIL')) ? 1 : 0)
