/**
 * TG-505 standalone privacy settings page. The integration lead mounts it into the settings
 * panel; until then it depends on nothing but its props and the shared API client.
 *
 * Every change is saved immediately (one PUT per edit, Telegram-style), optimistically: the
 * edited rule shows at once and is rolled back with an error line if the server refuses.
 */
import { useEffect, useState } from 'react'
import { IconButton, Spinner } from '@tg/ui'
import { PRIVACY_KEYS, privacyRuleWrite, type PrivacyApi, type PrivacyKey, type PrivacyRule } from '@tg/core'
import { SettingsIcon } from '../shell/settingsIcons'
import { PrivacyRuleEditor } from './PrivacyRuleEditor'
import { PRIVACY_KEY_COPY, privacyRuleSummary } from './privacyCopy'
import { replaceRule } from './privacyEditing'
import { privacyApi as defaultPrivacyApi } from './privacyApi'
import { t } from '../../../i18n/index'
import './privacy.css'

export interface PrivacySettingsPageProps {
  api?: PrivacyApi | undefined
  /** Skip the initial load (tests, previews, a lead-side prefetch). */
  initialRules?: PrivacyRule[] | undefined
  /** Open one dimension's editor directly. */
  initialKey?: PrivacyKey | null | undefined
  /** Back from the list; the page handles back from an editor itself. */
  onBack?: (() => void) | undefined
}

type Load = { state: 'loading' } | { state: 'failed' } | { state: 'ready'; rules: PrivacyRule[] }

export function PrivacySettingsPage({
  api = defaultPrivacyApi,
  initialRules,
  initialKey = null,
  onBack,
}: PrivacySettingsPageProps) {
  const [load, setLoad] = useState<Load>(initialRules ? { state: 'ready', rules: initialRules } : { state: 'loading' })
  const [openKey, setOpenKey] = useState<PrivacyKey | null>(initialKey)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (initialRules) return
    let cancelled = false
    api.get().then(
      (settings) => {
        if (!cancelled) setLoad({ state: 'ready', rules: settings.rules })
      },
      () => {
        if (!cancelled) setLoad({ state: 'failed' })
      },
    )
    return () => {
      cancelled = true
    }
  }, [api, initialRules])

  const rules = load.state === 'ready' ? load.rules : []
  const openRule = openKey ? rules.find((rule) => rule.key === openKey) : undefined

  const save = (next: PrivacyRule) => {
    const previous = rules
    setLoad({ state: 'ready', rules: replaceRule(rules, next) })
    setSaving(true)
    setError(null)
    api.put(next.key, privacyRuleWrite(next)).then(
      (stored) => {
        setLoad((current) =>
          current.state === 'ready' ? { state: 'ready', rules: replaceRule(current.rules, stored) } : current,
        )
        setSaving(false)
      },
      () => {
        setLoad({ state: 'ready', rules: previous })
        setSaving(false)
        setError(t('w.settings.b28580'))
      },
    )
  }

  const title = openRule ? PRIVACY_KEY_COPY[openRule.key].title : t('w.settings.4334e7')
  const back = openRule ? () => setOpenKey(null) : onBack
  return (
    <div className="tg-privacy" aria-busy={saving || load.state === 'loading'}>
      <header className="tg-privacy__header">
        {back ? (
          // TG-1204: the same arrow and button style as every other settings header.
          <IconButton label={t('w.settings.11d024')} variant="plain" onClick={back}>
            <SettingsIcon name="back" />
          </IconButton>
        ) : null}
        <h2 className="tg-privacy__title">{title}</h2>
      </header>
      {error ? (
        <p className="tg-privacy__error" role="alert">
          {error}
        </p>
      ) : null}
      {load.state === 'loading' ? <Spinner label={t('w.settings.9fec19')} /> : null}
      {load.state === 'failed' ? (
        <p className="tg-privacy__error" role="alert">
          {t('w.settings.69704d')}
        </p>
      ) : null}
      {load.state === 'ready' && openRule ? (
        <PrivacyRuleEditor rule={openRule} api={api} saving={saving} onChange={save} />
      ) : null}
      {load.state === 'ready' && !openRule ? (
        <section className="tg-privacy__section" aria-label={t('w.settings.4334e7')}>
          <ul className="tg-privacy__rows">
            {PRIVACY_KEYS.map((key) => {
              const rule = rules.find((candidate) => candidate.key === key)
              if (!rule) return null
              return (
                <li key={key}>
                  <button type="button" className="tg-privacy__row" onClick={() => setOpenKey(key)}>
                    <span className="tg-privacy__row-title">{PRIVACY_KEY_COPY[key].title}</span>
                    <span className="tg-privacy__row-value">{privacyRuleSummary(rule)}</span>
                  </button>
                </li>
              )
            })}
          </ul>
        </section>
      ) : null}
    </div>
  )
}
