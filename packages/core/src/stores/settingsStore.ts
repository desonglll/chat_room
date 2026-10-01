/**
 * Client settings: theme, shortcuts, notification toggles. Persistence goes through the
 * injected `CoreStorage` (`createStorage()`), explicitly passed to `hydrate`/`persist` —
 * core never touches a storage global. Key set is the skeleton's; TG-012/M5 extend it.
 */
import { createStore } from 'zustand/vanilla'
import type { CoreStorage } from '../types'
import type { AutoDownloadRules } from '../domain/autoDownload'
import { DEFAULT_AUTO_DOWNLOAD } from '../domain/autoDownload'

/** TG-507 adds `scheduled`: night between `nightFrom` and `nightTo` (local time). */
export type ThemePreference = 'light' | 'dark' | 'system' | 'scheduled'

/** TG-507: Telegram's accents (`packages/ui/src/tokens/themes/accents`). */
export type AccentId = 'blue' | 'cyan' | 'green' | 'orange' | 'pink' | 'purple' | 'red' | 'gray'

export type SendShortcut = 'enter' | 'shift-enter'

export interface SettingsSnapshot {
  theme: ThemePreference
  sendShortcut: SendShortcut
  notificationsEnabled: boolean
  notificationDetails: boolean
  /** TG-509: automatic media download per network type. */
  autoDownload: AutoDownloadRules
  /** TG-509: the offline media cache's ceiling (MB) and how long entries are kept (days; 0 = forever). */
  cacheLimitMb: number
  cacheRetentionDays: number
  /** TG-501: chat folders as a tab strip above the list, or a rail on the left (Telegram Desktop). */
  folderLayout: 'top' | 'left'
  /** TG-507: accent colour and the night schedule (`HH:MM`, local). */
  accent: AccentId
  nightFrom: string
  nightTo: string
}

export const DEFAULT_SETTINGS: SettingsSnapshot = {
  theme: 'system',
  sendShortcut: 'enter',
  notificationsEnabled: true,
  notificationDetails: true,
  autoDownload: DEFAULT_AUTO_DOWNLOAD,
  cacheLimitMb: 1024,
  cacheRetentionDays: 7,
  folderLayout: 'top',
  accent: 'blue',
  nightFrom: '22:00',
  nightTo: '07:00',
}

export const SETTINGS_STORAGE_KEY = 'tg.settings.v1'

export interface SettingsState extends SettingsSnapshot {
  update(change: Partial<SettingsSnapshot>): void
  /** Load from the injected storage; unknown/corrupt payloads fall back to defaults. */
  hydrate(storage: CoreStorage): void
  persist(storage: CoreStorage): void
}

function parseSnapshot(raw: string | null): Partial<SettingsSnapshot> {
  if (!raw) return {}
  try {
    const parsed = JSON.parse(raw) as Partial<SettingsSnapshot>
    return typeof parsed === 'object' && parsed !== null ? parsed : {}
  } catch {
    return {}
  }
}

export const createSettingsStore = () =>
  createStore<SettingsState>()((set, get) => ({
    ...DEFAULT_SETTINGS,
    update: (change) => set(change),
    hydrate: (storage) => set({ ...DEFAULT_SETTINGS, ...parseSnapshot(storage.getItem(SETTINGS_STORAGE_KEY)) }),
    persist: (storage) => {
      const {
        theme,
        sendShortcut,
        notificationsEnabled,
        notificationDetails,
        autoDownload,
        cacheLimitMb,
        cacheRetentionDays,
        folderLayout,
        accent,
        nightFrom,
        nightTo,
      } = get()
      storage.setItem(
        SETTINGS_STORAGE_KEY,
        JSON.stringify({
          theme,
          sendShortcut,
          notificationsEnabled,
          notificationDetails,
          autoDownload,
          cacheLimitMb,
          cacheRetentionDays,
          folderLayout,
          accent,
          nightFrom,
          nightTo,
        }),
      )
    },
  }))

export type SettingsStore = ReturnType<typeof createSettingsStore>

export const settingsStore = createSettingsStore()
