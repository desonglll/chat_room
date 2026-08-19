import { onBeforeUnmount, watch, type Ref } from 'vue'
import type { ThemePreference } from '../types'

// main.ts wires PrimeVue's darkModeSelector to `[data-theme="dark"]`, so this
// is the single place that actually flips that attribute — 'system' tracks
// the OS preference live instead of resolving it once at load.
export function useTheme(theme: Ref<ThemePreference>): void {
  const media = window.matchMedia('(prefers-color-scheme: dark)')

  function apply(): void {
    const resolved = theme.value === 'system' ? (media.matches ? 'dark' : 'light') : theme.value
    if (resolved === 'dark') document.documentElement.setAttribute('data-theme', 'dark')
    else document.documentElement.removeAttribute('data-theme')
    document.documentElement.style.colorScheme = resolved
  }

  media.addEventListener('change', apply)
  watch(theme, apply, { immediate: true })
  onBeforeUnmount(() => media.removeEventListener('change', apply))
}
