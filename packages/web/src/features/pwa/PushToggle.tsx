/**
 * TG-601: «推送通知» for this browser — subscribe/unsubscribe, with the message preview choice
 * the server applies per device (`show_details`). Explains why it is unavailable when it is.
 */
import { useEffect, useState } from 'react'
import { Toggle } from '@tg/ui'
import { t } from '../../i18n/index'
import type { PushState } from './webPush'
import { disablePush, enablePush, pushState } from './webPush'

const NOTE: Partial<Record<PushState, string>> = {
  unsupported: 'w.pwa.pushUnsupported',
  unavailable: 'w.pwa.pushUnavailable',
  denied: 'w.pwa.pushDenied',
}

export function PushToggle({ initial }: { initial?: PushState }) {
  const [state, setState] = useState<PushState | null>(initial ?? null)
  const [details, setDetails] = useState(false)
  const [busy, setBusy] = useState(false)
  useEffect(() => {
    if (initial === undefined) void pushState().then(setState, () => setState('unsupported'))
  }, [initial])
  if (state === null) return null
  const note = NOTE[state]
  const change = (on: boolean) => {
    setBusy(true)
    void (on ? enablePush(details) : disablePush())
      .then(setState, () => setState('unavailable'))
      .finally(() => setBusy(false))
  }
  return (
    <section className="tg-notify-settings__card" aria-label={t('w.pwa.push')}>
      <Toggle
        label={t('w.pwa.push')}
        description={note ? t(note) : t('w.pwa.pushDescription')}
        checked={state === 'on'}
        disabled={busy || note !== undefined}
        onCheckedChange={change}
      />
      {state === 'off' ? (
        <Toggle label={t('w.pwa.pushDetails')} checked={details} onCheckedChange={setDetails} />
      ) : null}
    </section>
  )
}
