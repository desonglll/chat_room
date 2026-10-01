/**
 * TG-510 «语言»: the interface language, applied at once (no reload). TG-1204: a settings card
 * with the shared radio rows (was bare native radios against the panel edge); each language is
 * named in itself, with its name in the current language underneath, as in Telegram.
 */
import type { Locale } from '@tg/core'
import { LOCALES, settingsStore } from '@tg/core'
import { RadioGroup } from '@tg/ui'
import { useStore } from 'zustand/react'
import { browserStorage } from '../../../app/platform'
import { t } from '../../../i18n/index'

const LOCALE_NAME: Record<Locale, () => string> = {
  'zh-CN': () => t('w.settings.localeZh'),
  en: () => t('w.settings.localeEn'),
}

export function LanguageSettingsPage() {
  const language = useStore(settingsStore, (state) => state.language)
  return (
    <div className="tg-language-settings">
      <section className="tg-settings__group">
        <RadioGroup
          name="language"
          aria-label={t('w.settings.cd99b2')}
          value={language}
          onValueChange={(value) => {
            const locale = LOCALES.find((item) => item.id === value)
            if (!locale) return
            settingsStore.getState().update({ language: locale.id })
            settingsStore.getState().persist(browserStorage)
          }}
          options={LOCALES.map((locale) => ({
            value: locale.id,
            label: <span lang={locale.id}>{locale.label}</span>,
            description: LOCALE_NAME[locale.id](),
          }))}
        />
      </section>
    </div>
  )
}
