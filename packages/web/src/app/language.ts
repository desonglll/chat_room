/**
 * TG-510: keeps the i18n locale and `<html lang>` in step with `settingsStore.language`.
 * The app root subscribes to `localeStore` and re-renders the tree, so a switch takes effect
 * at once without reloading the page.
 *
 * TG-806: a catalog loads before its locale is applied, so the tree never renders keys. The
 * returned promise settles once the boot locale is ready (main renders after it).
 */
import type { SettingsStore } from '@tg/core'
import { isLocale, setLocale } from '@tg/core'
import { loadLocale } from '../i18n/index'

export function bindLanguage(store: SettingsStore): { ready: Promise<void>; unbind: () => void } {
  let latest = 0
  const apply = (): Promise<void> => {
    const { language } = store.getState()
    const locale = isLocale(language) ? language : 'zh-CN'
    const request = ++latest
    const ready = loadLocale(locale).then(
      () => {
        if (request !== latest) return
        setLocale(locale)
        document.documentElement.lang = locale
      },
      // Offline before the chunk was cached: stay in the current language.
      () => undefined,
    )
    return ready
  }
  const ready = apply()
  const unbind = store.subscribe(() => void apply())
  return { ready, unbind }
}
