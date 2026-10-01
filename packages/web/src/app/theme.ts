/**
 * Applies the appearance settings to <html>: TG-009's `data-tg-theme` (day/night) and TG-507's
 * `data-tg-accent`. The token layers do the rest — no component changes with the theme.
 * `system` follows `prefers-color-scheme`; `scheduled` (TG-507) is re-evaluated every minute.
 */
import type { SettingsStore, ThemePreference } from '@tg/core'
import { resolveThemeAt } from '@tg/core'

export function resolveTheme(preference: ThemePreference, systemPrefersDark: boolean): 'day' | 'night' {
  return resolveThemeAt(preference, systemPrefersDark, { nightFrom: '22:00', nightTo: '07:00' }, new Date())
}

/** Subscribe the <html> theme and accent attributes to the store, the OS and the clock. */
export function bindTheme(store: SettingsStore): () => void {
  const media = window.matchMedia('(prefers-color-scheme: dark)')
  const root = document.documentElement

  const apply = () => {
    const settings = store.getState()
    root.setAttribute('data-tg-theme', resolveThemeAt(settings.theme, media.matches, settings, new Date()))
    root.setAttribute('data-tg-accent', settings.accent)
  }

  apply()
  const unsubscribe = store.subscribe(apply)
  media.addEventListener('change', apply)
  const clock = window.setInterval(apply, 60_000)
  return () => {
    unsubscribe()
    media.removeEventListener('change', apply)
    window.clearInterval(clock)
  }
}
