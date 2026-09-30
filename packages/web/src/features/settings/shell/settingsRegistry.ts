/**
 * The settings page registry (TG-110). The shell owns the Telegram section list; feature tasks
 * plug pages into a section from their own `register.ts` (side-effect import in `main.tsx`):
 *
 *   registerSettingsPage({ id: 'appearance', section: 'appearance', title: '外观',
 *     component: lazy(() => import('./AppearancePage')) })
 *
 * A section with no page renders «即将推出»; with one page, opening the section opens that
 * page; with several, the section lists them. Registration is idempotent by `id` (a second
 * call replaces the first, so HMR and tests do not duplicate rows) and returns an unregister.
 *
 * Framework-light on purpose: React appears only as types, so M5 register modules stay cheap
 * to import at boot and the panel itself can be lazy-loaded.
 */
import type { ComponentType, ReactNode } from 'react'

export type SettingsSectionId =
  | 'account'
  | 'notifications'
  | 'privacy'
  | 'storage'
  | 'appearance'
  | 'folders'
  | 'language'
  | 'devices'

export interface SettingsSection {
  id: SettingsSectionId
  title: string
  /** Owning task, for the «即将推出» placeholder and for readers of this file. */
  owner: string
}

/** Telegram's order. `folders` is TG-501's slot (Telegram lists «聊天文件夹» at the root). */
export const SETTINGS_SECTIONS: readonly SettingsSection[] = [
  { id: 'account', title: '我的账号', owner: 'TG-110 / TG-511' },
  { id: 'notifications', title: '通知与声音', owner: 'TG-508' },
  { id: 'privacy', title: '隐私与安全', owner: 'TG-505 / TG-506' },
  { id: 'storage', title: '数据与存储', owner: 'TG-509' },
  { id: 'appearance', title: '外观', owner: 'TG-507' },
  { id: 'folders', title: '聊天文件夹', owner: 'TG-501' },
  { id: 'language', title: '语言', owner: 'TG-510' },
  { id: 'devices', title: '设备', owner: 'TG-110' },
]

export interface SettingsPageProps {
  /** One level up: the section list, or the root when the section has a single page. */
  onBack(): void
  /** Close the whole settings panel. */
  onClose(): void
}

export interface SettingsPageRegistration {
  /** Unique across all sections, e.g. `privacy.rules`. */
  id: string
  section: SettingsSectionId
  title: string
  /** Decorative glyph for the row in a multi-page section (`currentColor`). */
  icon?: ReactNode
  /** Rendered inside a `Suspense` boundary, so `lazy(() => import(...))` is welcome. */
  component: ComponentType<SettingsPageProps>
  /** Row order inside the section, ascending. Default 100. */
  order?: number | undefined
  /** The page draws its own header with a back button; the shell then draws none. */
  ownsHeader?: boolean | undefined
}

type Listener = () => void

let pages: readonly SettingsPageRegistration[] = []
const listeners = new Set<Listener>()

function publish(next: readonly SettingsPageRegistration[]): void {
  pages = next
  for (const listener of listeners) listener()
}

const orderOf = (page: SettingsPageRegistration) => page.order ?? 100

export function registerSettingsPage(page: SettingsPageRegistration): () => void {
  const others = pages.filter((existing) => existing.id !== page.id)
  publish([...others, page].sort((a, b) => orderOf(a) - orderOf(b)))
  return () => {
    if (pages.includes(page)) publish(pages.filter((existing) => existing !== page))
  }
}

/** Stable snapshot (a new array only after a registration change) for `useSyncExternalStore`. */
export function settingsPagesSnapshot(): readonly SettingsPageRegistration[] {
  return pages
}

export function subscribeSettingsPages(listener: Listener): () => void {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

export function pagesInSection(
  all: readonly SettingsPageRegistration[],
  section: SettingsSectionId,
): SettingsPageRegistration[] {
  return all.filter((page) => page.section === section)
}

export function findSettingsPage(
  all: readonly SettingsPageRegistration[],
  id: string,
): SettingsPageRegistration | undefined {
  return all.find((page) => page.id === id)
}
