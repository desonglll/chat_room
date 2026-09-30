/**
 * The shipped path: `<AnimatedSticker>` + the page-wide manager, stickers served as `.tgs`
 * bytes through object URLs (so fetch → inflate → validate → parse all run as in the app).
 */
import { gzip } from 'pako'
import { createRoot } from 'react-dom/client'
import { AnimatedSticker, createBrowserStickerManager } from '../index'
import { syntheticLottie } from '../fixtures/syntheticLottie'
import type { BenchParams } from './params'

export async function mountManaged(root: HTMLElement, params: BenchParams): Promise<() => unknown> {
  const urls = Array.from({ length: params.distinct }, (_, seed) => {
    const bytes = gzip(JSON.stringify(syntheticLottie({ seed: seed + 1, layers: params.layers })))
    return URL.createObjectURL(new Blob([bytes], { type: 'application/x-tgsticker' }))
  })
  const manager = createBrowserStickerManager(params.engine)
  createRoot(root).render(
    <>
      {Array.from({ length: params.count }, (_, index) => (
        <AnimatedSticker
          key={index}
          src={urls[index % params.distinct] ?? ''}
          size={params.size}
          label="🙂"
          manager={manager}
        />
      ))}
    </>,
  )
  // Ready once every sticker is past loading (static or live) — or immediately for count=0.
  await new Promise<void>((resolve) => {
    const poll = () => {
      const stats = manager.stats()
      const settled = stats.views >= params.count && root.querySelectorAll('[data-phase="loading"]').length === 0
      if (settled) resolve()
      else setTimeout(poll, 50)
    }
    poll()
  })
  return () => manager.stats()
}
