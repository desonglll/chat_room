/**
 * TG-510: keeps the i18n locale and `<html lang>` in step with `settingsStore.language`.
 * The app root subscribes to `localeStore` and re-renders the tree, so a switch takes effect
 * at once without reloading the page.
 */
import type { SettingsStore } from '@tg/core'
import { isLocale, setLocale } from '@tg/core'

export function bindLanguage(store: SettingsStore): () => void {
  const apply = () => {
    const { language } = store.getState()
    const locale = isLocale(language) ? language : 'zh-CN'
    setLocale(locale)
    document.documentElement.lang = locale
  }
  apply()
  return store.subscribe(apply)
}
