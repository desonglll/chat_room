/**
 * Raw lottie-web, no manager: every instance plays, nothing pauses. This is the renderer
 * comparison (svg / canvas / html) the task card asks for, measured without any of the
 * manager's mitigations so the renderers are compared on equal terms.
 */
import lottie from 'lottie-web'
import { syntheticLottie } from '../fixtures/syntheticLottie'
import type { BenchParams } from './params'

export async function mountRaw(root: HTMLElement, params: BenchParams): Promise<() => number> {
  const documents = Array.from({ length: params.distinct }, (_, seed) =>
    JSON.stringify(syntheticLottie({ seed: seed + 1, layers: params.layers })),
  )
  const shared = documents.map((text) => JSON.parse(text) as object)
  const animations = []
  for (let index = 0; index < params.count; index += 1) {
    const cell = document.createElement('div')
    cell.className = 'bench-cell'
    cell.style.width = `${params.size}px`
    cell.style.height = `${params.size}px`
    root.append(cell)
    const slot = index % params.distinct
    animations.push(
      lottie.loadAnimation({
        container: cell,
        renderer: params.renderer,
        loop: true,
        autoplay: true,
        animationData: params.share ? shared[slot] : JSON.parse(documents[slot] ?? '{}'),
        rendererSettings: { progressiveLoad: false },
      }),
    )
  }
  return () => animations.length
}
