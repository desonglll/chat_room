/**
 * `@tg/core` — the platform-agnostic business layer (architecture.md section 2).
 *
 * Everything that does not render lives here: contract types, the HTTP client, the WebSocket
 * client, Zustand vanilla stores, and pure domain functions. Two rules are non-negotiable and
 * are enforced by `test/platformBoundary.test.ts`:
 *
 *   1. no import of `react`, `react-dom`, `@tg/ui` or `@tg/web`
 *   2. no reference to `window`, `document`, `localStorage` or `navigator`
 *
 * Platform capabilities are injected by the host instead (`createStorage()`,
 * `createWebSocket()`, `createFileReader()`), which is what keeps React Native reachable.
 *
 * The five barrels below are also the package's public subpath exports. Add new modules to
 * the layer they belong to; do not add a sixth top-level directory.
 */
export * from './types'
export * from './api'
export * from './realtime'
export * from './stores'
export * from './domain'
