/**
 * `@tg/core/stores` — Zustand VANILLA stores, one domain one file (architecture.md §2:
 * the split is both a design and a parallel-development decision; cross-store reads use
 * selector composition, never a merged store). `packages/web` subscribes via
 * `zustand/react`'s `useStore`; nothing in here knows React.
 */
export * from './authStore'
export * from './chatListStore'
export * from './messageStore'
export * from './composerStore'
export * from './presenceStore'
export * from './stickerStore'
export * from './mediaStore'
export * from './settingsStore'
export * from './uiStore'
