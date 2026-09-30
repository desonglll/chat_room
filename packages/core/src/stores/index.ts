/**
 * Zustand **vanilla** stores (`zustand/vanilla`), one domain per file, so that parallel
 * worktrees never contend for the same file: `authStore.ts`, `chatListStore.ts`,
 * `messageStore.ts`, `composerStore.ts`, `presenceStore.ts`, `stickerStore.ts`,
 * `mediaStore.ts`, `settingsStore.ts`, `uiStore.ts` (architecture.md section 2).
 *
 * `packages/web` subscribes with `useStore` from `zustand/react`; the stores themselves never
 * import React. `zustand` is not a dependency yet — TG-011 adds it when the first store lands.
 *
 * Owner of the contents: TG-011. This barrel is the frozen entry point `@tg/core/stores`.
 */
export {}
