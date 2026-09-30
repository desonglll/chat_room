/**
 * How the archive entry shows at the top of the main list, Telegram's three states:
 * `collapsed` (a thin row with a stack of the archived chats' avatars — the default),
 * `expanded` (a full chat-height row with a preview of the archived chats' names) and
 * `hidden` (no row; the archive is reached from the hamburger menu). Per device, like the
 * sidebar width: stored under `tg.archive.v1`.
 */
import type { CoreStorage } from '@tg/core'

export type ArchiveRowMode = 'collapsed' | 'expanded' | 'hidden'

export const ARCHIVE_STORAGE_KEY = 'tg.archive.v1'

const MODES: readonly ArchiveRowMode[] = ['collapsed', 'expanded', 'hidden']

export function readArchiveRowMode(storage: CoreStorage): ArchiveRowMode {
  try {
    const parsed = JSON.parse(storage.getItem(ARCHIVE_STORAGE_KEY) ?? 'null') as { mode?: unknown } | null
    const mode = parsed?.mode
    return MODES.includes(mode as ArchiveRowMode) ? (mode as ArchiveRowMode) : 'collapsed'
  } catch {
    return 'collapsed'
  }
}

export function writeArchiveRowMode(storage: CoreStorage, mode: ArchiveRowMode): void {
  storage.setItem(ARCHIVE_STORAGE_KEY, JSON.stringify({ mode }))
}
