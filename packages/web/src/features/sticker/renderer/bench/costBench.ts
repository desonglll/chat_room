/**
 * Per-frame render cost of one sticker, per renderer, without rAF or compositing in the way:
 * render every frame of the animation `rounds` times and divide. Machine-load sensitive like
 * everything else here, but it isolates what the ticker budget has to pay per draw.
 */
import lottie from 'lottie-web'
import { syntheticLottie } from '../fixtures/syntheticLottie'
import type { BenchParams } from './params'

export async function mountCost(root: HTMLElement, params: BenchParams): Promise<() => unknown> {
  const results: Record<string, number> = {}
  for (const renderer of ['svg', 'canvas', 'html'] as const) {
    const cell = document.createElement('div')
    cell.className = 'bench-cell'
    cell.style.width = `${params.size}px`
    cell.style.height = `${params.size}px`
    root.append(cell)
    const item = lottie.loadAnimation({
      container: cell,
      renderer,
      loop: false,
      autoplay: false,
      animationData: syntheticLottie({ seed: 7, layers: params.layers }),
    })
    const frames = Math.floor(item.totalFrames)
    for (let frame = 0; frame < frames; frame += 1) item.goToAndStop(frame, true)
    const rounds = 3
    const start = performance.now()
    for (let round = 0; round < rounds; round += 1) {
      for (let frame = 0; frame < frames; frame += 1) item.goToAndStop(frame, true)
      if (renderer !== 'canvas') cell.getBoundingClientRect()
    }
    results[renderer] = Math.round(((performance.now() - start) / (rounds * frames)) * 1000) / 1000
    item.destroy()
    cell.remove()
  }
  return () => results
}
