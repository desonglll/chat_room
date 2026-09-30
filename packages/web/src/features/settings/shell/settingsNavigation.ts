/**
 * Where the settings panel is (TG-110): open or not, and the stack of views above the root.
 * Feature-local store (one domain, one file). The hamburger menu calls `openSettings()`; a
 * page may deep-link with `openSettings({ kind: 'page', id })`.
 *
 * The stack is kept while the panel slides out, so the exit animation shows the view the
 * user left; the next `openSettings()` starts from the root again.
 */
import { createStore } from 'zustand/vanilla'
import type { SettingsPageRegistration, SettingsSectionId } from './settingsRegistry'

export type SettingsView = { kind: 'section'; id: SettingsSectionId } | { kind: 'page'; id: string }

export interface SettingsNavigationState {
  open: boolean
  stack: readonly SettingsView[]
  openSettings(view?: SettingsView): void
  close(): void
  push(view: SettingsView): void
  /** Pop one view; from the root this closes the panel. */
  back(): void
}

export const createSettingsNavigation = () =>
  createStore<SettingsNavigationState>()((set, get) => ({
    open: false,
    stack: [],
    openSettings: (view) => set({ open: true, stack: view ? [view] : [] }),
    close: () => set({ open: false }),
    push: (view) => set((state) => ({ stack: [...state.stack, view] })),
    back: () => {
      const { stack } = get()
      if (stack.length === 0) set({ open: false })
      else set({ stack: stack.slice(0, -1) })
    },
  }))

export type SettingsNavigation = ReturnType<typeof createSettingsNavigation>

export const settingsNavigation = createSettingsNavigation()

export function openSettings(view?: SettingsView): void {
  settingsNavigation.getState().openSettings(view)
}

/** Opening a section with exactly one page opens that page directly (Telegram's 我的账号, 设备). */
export function viewForSection(
  section: SettingsSectionId,
  pagesOfSection: readonly SettingsPageRegistration[],
): SettingsView {
  const only = pagesOfSection.length === 1 ? pagesOfSection[0] : undefined
  return only ? { kind: 'page', id: only.id } : { kind: 'section', id: section }
}
