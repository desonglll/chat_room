/**
 * The link itself, with copy and share. Copy uses the async clipboard; share uses the Web
 * Share sheet where the browser has one and falls back to copying.
 */
import { useState } from 'react'
import { inviteLinkUrl } from '@tg/core'
import { Button } from '@tg/ui'
import { t } from '../../i18n/index'

export interface LinkShareBoxProps {
  token: string
  /** Hide the buttons for a link that no longer works. */
  live: boolean
  /** Defaults to this page's origin; tests pass one. */
  origin?: string | undefined
  children?: React.ReactNode
}

function currentOrigin(): string {
  return typeof window === 'undefined' ? '' : window.location.origin
}

export function LinkShareBox({ token, live, origin, children }: LinkShareBoxProps) {
  const url = inviteLinkUrl(origin ?? currentOrigin(), token)
  const [note, setNote] = useState<string | null>(null)

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(url)
      setNote(t('w.inviteLinks.3e1fac'))
    } catch {
      setNote(t('w.inviteLinks.7c71a9'))
    }
  }
  const share = async () => {
    if (typeof navigator.share === 'function') {
      try {
        await navigator.share({ title: t('w.inviteLinks.2b9bbf'), url })
        return
      } catch {
        // Dismissed or unsupported target: fall back to copying.
      }
    }
    await copy()
  }

  return (
    <div className="tg-invite__share">
      <output className="tg-invite__url" data-live={live ? 'true' : 'false'}>
        {url}
      </output>
      {live ? (
        <div className="tg-invite__share-actions">
          <Button variant="filled" onClick={() => void copy()}>
            {t('w.inviteLinks.abb22b')}
          </Button>
          <Button variant="tonal" onClick={() => void share()}>
            {t('w.inviteLinks.7a9243')}
          </Button>
          {children}
        </div>
      ) : null}
      {note ? (
        <p className="tg-invite__note" role="status">
          {note}
        </p>
      ) : null}
    </div>
  )
}
