/**
 * Applies `settingsStore.theme` to the document as TG-009's `data-tg-theme` attribute.
 * `day` is the `:root` default, so only `night` needs the attribute; `system` follows
 * `prefers-color-scheme`. The settings UI arrives in M5 — this only honours the stored
 * preference and the OS.
 */
import type { SettingsStore, ThemePreference } from '@tg/core'

export function resolveTheme(preference: ThemePreference, systemPrefersDark: boolean): 'day' | 'night' {
  if (preference === 'dark') return 'night'
  if (preference === 'light') return 'day'
  return systemPrefersDark ? 'night' : 'day'
}

/** Subscribe the <html> theme attribute to the store and the OS. Returns unsubscribe. */
export function bindTheme(store: SettingsStore): () => void {
  const media = window.matchMedia('(prefers-color-scheme: dark)')

  const apply = () => {
    const theme = resolveTheme(store.getState().theme, media.matches)
    document.documentElement.setAttribute('data-tg-theme', theme)
  }

  apply()
  const unsubscribe = store.subscribe(apply)
  media.addEventListener('change', apply)
  return () => {
    unsubscribe()
    media.removeEventListener('change', apply)
  }
}
