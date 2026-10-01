/**
 * The row the chat administration surface mounts (see docs/devlog/TG-205.md "Integration
 * patch list"): "邀请链接", which opens the invite-links page. It renders nothing for a viewer
 * who may not manage links.
 *
 *   <InviteLinksEntry chatId={chatId} variant="overlay" />   inside ChatAdminPanel's sheet
 *   <InviteLinksEntry chatId={chatId} />                     anywhere else (own right sheet)
 *
 * `overlay` exists because two open sheets means two focus traps pulling focus from each
 * other forever; inside a sheet the page covers the host sheet's surface instead.
 */
import { useEffect, useState } from 'react'
import { Sheet } from '@tg/ui'
import { inviteLinksApi, type InviteLinksService } from './inviteLinksApi'
import { InviteLinksPanel } from './InviteLinksPanel'
import { t } from '../../i18n/index'
import './inviteLinks.css'

export interface InviteLinksEntryProps {
  chatId: string
  /** Defaults to the app-wide service; tests inject a fake. */
  api?: InviteLinksService | undefined
  /** Skip the permission read (tests, or a host that already knows). */
  canManage?: boolean | undefined
  /** `sheet` (default): its own right sheet. `overlay`: cover the nearest positioned host. */
  variant?: 'sheet' | 'overlay' | undefined
}

function LinkIcon() {
  return (
    <svg viewBox="0 0 24 24" width="24" height="24" aria-hidden="true">
      <path
        d="M10 14a4 4 0 005.7 0l3-3a4 4 0 00-5.7-5.7l-1 1M14 10a4 4 0 00-5.7 0l-3 3a4 4 0 005.7 5.7l1-1"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
      />
    </svg>
  )
}

export function InviteLinksEntry({
  chatId,
  api = inviteLinksApi,
  canManage,
  variant = 'sheet',
}: InviteLinksEntryProps) {
  const [allowed, setAllowed] = useState<boolean>(canManage ?? false)
  const [open, setOpen] = useState(false)

  useEffect(() => {
    if (canManage !== undefined) return
    let alive = true
    setAllowed(false)
    api.canManage(chatId).then(
      (answer) => alive && setAllowed(answer),
      () => alive && setAllowed(false),
    )
    return () => {
      alive = false
    }
  }, [api, chatId, canManage])

  if (!allowed) return null
  return (
    <>
      <button type="button" className="tg-invite__entry" onClick={() => setOpen(true)}>
        <span className="tg-invite__entry-icon">
          <LinkIcon />
        </span>
        <span className="tg-invite__row-text">
          <span className="tg-invite__row-name">{t('w.inviteLinks.8a8f47')}</span>
          <span className="tg-invite__meta">{t('w.inviteLinks.5f1bb4')}</span>
        </span>
      </button>
      {variant === 'overlay' ? (
        open ? (
          <div className="tg-invite__overlay">
            <InviteLinksPanel chatId={chatId} api={api} onClose={() => setOpen(false)} />
          </div>
        ) : null
      ) : (
        <Sheet
          open={open}
          side="right"
          ariaLabel={t('w.inviteLinks.8a8f47')}
          showClose={false}
          className="tg-invite__sheet"
          onClose={() => setOpen(false)}
        >
          {open ? <InviteLinksPanel chatId={chatId} api={api} onClose={() => setOpen(false)} /> : null}
        </Sheet>
      )}
    </>
  )
}
