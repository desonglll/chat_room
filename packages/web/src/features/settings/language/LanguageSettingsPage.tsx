/** TG-510 «语言»: the interface language, applied at once (no reload). */
import { LOCALES, settingsStore } from '@tg/core'
import { useStore } from 'zustand/react'
import { browserStorage } from '../../../app/platform'
import { t } from '../../../i18n/index'

export function LanguageSettingsPage() {
  const language = useStore(settingsStore, (state) => state.language)
  return (
    <div className="tg-language-settings" role="radiogroup" aria-label={t('w.settings.cd99b2')}>
      {LOCALES.map((locale) => (
        <label key={locale.id} className="tg-language-settings__option">
          <input
            type="radio"
            name="language"
            checked={language === locale.id}
            onChange={() => {
              settingsStore.getState().update({ language: locale.id })
              settingsStore.getState().persist(browserStorage)
            }}
          />{' '}
          <span lang={locale.id}>{locale.label}</span>
        </label>
      ))}
    </div>
  )
}
