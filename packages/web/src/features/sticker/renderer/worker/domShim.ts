/**
 * The few DOM globals lottie-web's light canvas build touches, for running it in a worker.
 *
 * Measured, not guessed (docs/devlog/TG-301.md): at import it reads `document`/`navigator`
 * and scans `<script>` tags; the animation manager polls `document.readyState` and searches
 * `.lottie` elements; the image preloader creates a 1×1 canvas; `setupGlobalData` passes
 * `document.body` (used only for fonts, i.e. text layers, which stickers do not have);
 * `createWorker` reads `window.Worker`. `_isProxy` is lottie's own flag for exactly this
 * situation (its official worker build sets it): it skips the SVG-filter path for luma
 * mattes, which cannot run off the DOM.
 *
 * Must be evaluated before lottie is imported — `renderWorker.ts` imports lottie dynamically
 * after this module for that reason.
 */

interface InertElement {
  style: Record<string, string>
  setAttribute(): void
  appendChild<T>(child: T): T
  removeChild<T>(child: T): T
  getContext(): null
}

const inert = (): InertElement => ({
  style: {},
  setAttribute: () => undefined,
  appendChild: (child) => child,
  removeChild: (child) => child,
  getContext: () => null,
})

const scope = globalThis as Record<string, unknown>

if (typeof scope.document === 'undefined') {
  scope.window = globalThis
  scope.document = {
    _isProxy: true,
    readyState: 'complete',
    body: inert(),
    createElement: (tag: string) => (tag === 'canvas' ? new OffscreenCanvas(1, 1) : inert()),
    createElementNS: () => inert(),
    getElementsByTagName: () => [],
    getElementsByClassName: () => [],
    querySelectorAll: () => [],
  }
}

export {}
