/**
 * lottie-web's *light canvas* build driven container-less into a caller-owned 2D context —
 * shared by the main-thread engine and the render worker.
 *
 * Why this build: canvas measured cheapest per frame of svg / canvas / html
 * (docs/devlog/TG-301.md "Benchmark"), and "light" omits the expression engine, the only
 * part of lottie-web that calls `eval` — which the server's `script-src 'self'` CSP forbids
 * and Telegram's sticker spec does not allow anyway.
 */
import type { AnimationConfigWithData, AnimationItem, LottiePlayer } from 'lottie-web'

export type Canvas2D = CanvasRenderingContext2D | OffscreenCanvasRenderingContext2D

export function loadLottie(lottie: LottiePlayer, context: Canvas2D, animation: object): Promise<AnimationItem> {
  const config: Omit<AnimationConfigWithData<'canvas'>, 'container'> = {
    renderer: 'canvas',
    loop: false,
    autoplay: false,
    // Shared parse: lottie completes the document in place once (`__complete`), so every unit
    // and snapshot of one sticker reuses the same parsed object.
    animationData: animation,
    rendererSettings: {
      context: context as CanvasRenderingContext2D,
      clearCanvas: true,
      preserveAspectRatio: 'xMidYMid meet',
      progressiveLoad: false,
      // The canvas is already sized in device pixels; lottie's default would multiply by
      // devicePixelRatio a second time.
      dpr: 1,
    },
  }
  // The typings demand `container`, but the canvas renderer is built to run container-less
  // when handed a `context` (CanvasRendererBase.configAnimation): it then draws into that
  // context and never touches the DOM tree.
  const item = lottie.loadAnimation(config as AnimationConfigWithData<'canvas'>)
  if (item.isLoaded) return Promise.resolve(item)
  return new Promise((resolve, reject) => {
    item.addEventListener('DOMLoaded', () => resolve(item))
    item.addEventListener('data_failed', () => {
      item.destroy()
      reject(new Error('lottie failed to load the animation'))
    })
  })
}

export function frameCount(item: AnimationItem): number {
  return Math.max(1, Math.floor(item.totalFrames))
}
