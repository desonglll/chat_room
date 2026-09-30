/** TG-306 fixture: every sticker format in one panel, through the single `<Sticker>` entry. */
import '@tg/ui/styles.css'
import './bench.css'
import { createRoot } from 'react-dom/client'
import { gzip } from 'pako'
import { Sticker, setWebmStickerSupport, stickerRenderManager, type StickerDescriptor } from '../index'
import { syntheticLottie } from '../fixtures/syntheticLottie'
import { makeWebm, makeWebp } from './mixedMedia'

declare global {
  interface Window {
    __mixed?: { ready: boolean; error?: string; stats: () => unknown }
  }
}

interface Item {
  sticker: StickerDescriptor
  data: Uint8Array
}

const params = new URLSearchParams(location.search)
if (params.get('webm') === 'off') setWebmStickerSupport(false)

async function items(): Promise<Item[]> {
  const tgs = (seed: number) => gzip(JSON.stringify(syntheticLottie({ seed, layers: 12 })))
  const [webpA, webpB, webmA, webmB] = await Promise.all([makeWebp(40), makeWebp(200), makeWebm(120), makeWebm(300)])
  const thumb = URL.createObjectURL(new Blob([webpB as BlobPart], { type: 'image/webp' }))
  return [
    { sticker: { format: 'tgs', emoji: '🐱' }, data: tgs(1) },
    { sticker: { format: 'webp', emoji: '🍊' }, data: webpA },
    { sticker: { format: 'webm', emoji: '🥝' }, data: webmA },
    { sticker: { format: 'webm', emoji: '🍇', thumbnail_url: thumb }, data: webmB },
    { sticker: { format: 'tgs', emoji: '🐶' }, data: tgs(2) },
    { sticker: { mime_type: 'image/webp', emoji: '🫐' }, data: webpB },
    // No format, no mime: the magic bytes decide.
    { sticker: { emoji: '🥝' }, data: webmA },
    { sticker: { emoji: '🐭' }, data: tgs(3) },
    { sticker: { emoji: '🍊' }, data: webpA },
  ]
}

function Panel({ list }: { list: Item[] }) {
  return (
    <div className="mixed-panel">
      {list.map((item, index) => (
        <figure key={index} className="mixed-cell">
          <Sticker sticker={item.sticker} data={item.data} cacheKey={`fixture-${index}`} size={120} />
          <figcaption>{item.sticker.format ?? item.sticker.mime_type ?? 'sniffed'}</figcaption>
        </figure>
      ))}
    </div>
  )
}

const root = document.getElementById('mixed')
if (!root) throw new Error('missing #mixed')
window.__mixed = { ready: false, stats: () => stickerRenderManager().stats() }
items().then(
  (list) => {
    createRoot(root).render(
      <>
        <style>{`.mixed-panel{display:grid;grid-template-columns:repeat(3,136px);gap:8px;padding:12px;width:max-content;background:var(--tg-bg-secondary);border-radius:var(--tg-radius-lg)}
.mixed-cell{margin:0;display:grid;justify-items:center;gap:4px;font:12px var(--tg-font-sans);color:var(--tg-text-secondary)}
.mixed-spacer{height:300vh}`}</style>
        <Panel list={list} />
        <div className="mixed-spacer" />
      </>,
    )
    window.__mixed!.ready = true
  },
  (error: unknown) => {
    window.__mixed = { ready: true, error: String(error), stats: () => null }
  },
)
