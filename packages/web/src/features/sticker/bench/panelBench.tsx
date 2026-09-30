/**
 * Bench page: installs `sets × per` synthetic TGS stickers (TG-301's generator, gzip → blob
 * URLs, so fetch → inflate → parse run as in the app), then exposes to `measurePanel.mjs`:
 *
 *  - `open()`: mounts the media panel on the 贴纸 tab and reports the time to the first
 *    painted frame, then samples frames and long tasks while the panel settles;
 *  - `scroll(ms)`: scrolls the grid top → bottom over `ms` while sampling frames;
 *  - `suggest()`: types one emoji into a textarea wired to `<StickerSuggestions>` and
 *    reports keystroke → strip-in-DOM latency.
 *
 * `mode=naive` renders every sticker at once (no virtual list) as the comparison baseline.
 */
import '@tg/ui/styles.css'
import '../../../styles/index.css'
import { gzip } from 'pako'
import { useState } from 'react'
import { createRoot } from 'react-dom/client'
import { flushSync } from 'react-dom'
import { stickerStore, type Sticker, type StickerSet } from '@tg/core'
import { syntheticLottie } from '../renderer/fixtures/syntheticLottie'
import { sampleFrames } from '../renderer/bench/metrics'
import { MediaPanel } from '../panel/MediaPanel'
import { StickerSuggestions } from '../suggest/StickerSuggestions'
import { StickerView } from '../StickerView'

const params = new URLSearchParams(location.search)
const SETS = Number(params.get('sets') ?? 12)
const PER = Number(params.get('per') ?? 25)
const LAYERS = Number(params.get('layers') ?? 24)
const NAIVE = params.get('mode') === 'naive'
const TAB = params.get('tab') ?? 'stickers'
const EMOJI = ['😀', '😂', '😍', '👍', '🐱', '🎉', '❤️', '🔥']

function install(setCount: number, perSet: number, seedBase = 0) {
  const sets: StickerSet[] = []
  for (let s = 0; s < setCount; s += 1) {
    const stickers: Sticker[] = []
    for (let i = 0; i < perSet; i += 1) {
      const seed = seedBase + s * 1000 + i + 1
      const bytes = gzip(JSON.stringify(syntheticLottie({ seed, layers: LAYERS })))
      const url = URL.createObjectURL(new Blob([bytes], { type: 'application/x-tgsticker' }))
      const emoji = EMOJI[(s + i) % EMOJI.length]!
      stickers.push({
        id: `s${seed}`,
        set_id: `set-${seedBase}-${s}`,
        emoji,
        emojis: [emoji],
        format: 'tgs',
        mime_type: 'application/x-tgsticker',
        width: 512,
        height: 512,
        duration_ms: 3000,
        size_bytes: bytes.length,
        file_url: url,
      })
    }
    sets.push({
      id: `set-${seedBase}-${s}`,
      short_name: `bench_${seedBase}_${s}`,
      title: `Bench set ${s + 1}`,
      set_type: 'regular',
      owner_id: null,
      stickers,
      installed: true,
      archived: false,
      created_at: '',
      updated_at: '',
    })
  }
  stickerStore.setState({ status: 'ready', revision: 1, sets, recent: [], favorites: [] })
  return sets
}

function NaivePanel({ sets }: { sets: StickerSet[] }) {
  return (
    <div className="tg-media-panel" style={{ overflowY: 'auto' }}>
      {sets.flatMap((set) =>
        set.stickers.map((sticker) => (
          <StickerView key={sticker.id} src={sticker.file_url} format="tgs" size={64} label={sticker.emoji} />
        )),
      )}
    </div>
  )
}

function SuggestHarness() {
  const [draft, setDraft] = useState('')
  return (
    <div style={{ position: 'relative', marginTop: 200, width: 360 }}>
      <StickerSuggestions draft={draft} onPick={() => undefined} />
      <textarea id="draft" value={draft} onChange={(event) => setDraft(event.target.value)} />
    </div>
  )
}

const nextPaint = () =>
  new Promise<number>((resolve) => requestAnimationFrame(() => setTimeout(() => resolve(performance.now()))))

const setupStart = performance.now()
const sets = install(SETS, PER)
const setupMs = Math.round(performance.now() - setupStart)
const host = document.getElementById('bench')!
const panelHost = document.createElement('div')
const suggestHost = document.createElement('div')
host.append(panelHost, suggestHost)
const panelRoot = createRoot(panelHost)
createRoot(suggestHost).render(<SuggestHarness />)

declare global {
  interface Window {
    __panelBench: Record<string, unknown>
  }
}

window.__panelBench = {
  ready: true,
  setupMs,
  stickers: SETS * PER,
  async open(sampleMs: number) {
    const start = performance.now()
    const sampling = sampleFrames(sampleMs)
    flushSync(() =>
      panelRoot.render(
        NAIVE ? (
          <NaivePanel sets={sets} />
        ) : (
          <MediaPanel
            chatId="bench"
            emoji={null}
            initialTab={TAB}
            canSend
            onSendSticker={() => undefined}
            onClose={() => undefined}
          />
        ),
      ),
    )
    const mountedMs = performance.now() - start
    const paintedMs = (await nextPaint()) - start
    const frames = await sampling
    return {
      mountMs: Math.round(mountedMs * 10) / 10,
      firstPaintMs: Math.round(paintedMs * 10) / 10,
      mountedStickers: panelHost.querySelectorAll('.tg-sticker').length,
      ...frames,
    }
  },
  close() {
    flushSync(() => panelRoot.render(null))
  },
  /**
   * Opens and closes the panel once over a tiny, different library: warms code, fonts and
   * styles as a running app would have, but none of the measured library's sticker caches.
   */
  async warmUp() {
    const measured = stickerStore.getState().sets
    install(1, 4, 900_000)
    const bench = window.__panelBench as { open(ms: number): Promise<unknown>; close(): void }
    await bench.open(300)
    bench.close()
    stickerStore.setState({ sets: measured, revision: 2 })
  },
  async scroll(ms: number) {
    const grid = panelHost.querySelector<HTMLElement>('.tg-sticker-grid, .tg-media-panel')
    if (!grid) throw new Error('panel not open')
    grid.style.scrollSnapType = 'none'
    const sampling = sampleFrames(ms)
    const start = performance.now()
    const distance = grid.scrollHeight - grid.clientHeight
    await new Promise<void>((resolve) => {
      const step = (now: number) => {
        const t = Math.min(1, (now - start) / ms)
        grid.scrollTop = distance * t
        if (t < 1) requestAnimationFrame(step)
        else resolve()
      }
      requestAnimationFrame(step)
    })
    return { distance, maxMounted: panelHost.querySelectorAll('.tg-sticker').length, ...(await sampling) }
  },
  suggestWatch() {
    return new Promise<number>((resolve) => {
      const textarea = document.getElementById('draft')!
      let typedAt = 0
      textarea.addEventListener('input', () => (typedAt = performance.now()), { once: true, capture: true })
      const observer = new MutationObserver(() => {
        if (typedAt && suggestHost.querySelector('.tg-sticker-suggest')) {
          observer.disconnect()
          resolve(Math.round((performance.now() - typedAt) * 10) / 10)
        }
      })
      observer.observe(suggestHost, { childList: true, subtree: true })
    })
  },
}
