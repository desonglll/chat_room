/**
 * Chrome floating over the virtual list: the sticky date pill, the older-page spinner, the
 * jump notice, and the scroll-to-bottom / back-to-where-you-were button. All absolutely
 * positioned so none of it takes part in Virtuoso's height measurement.
 */
import { Badge, Spinner } from '@tg/ui'
import { t } from '../../i18n/index'

export function FloatingDate({ text, visible }: { text: string; visible: boolean }) {
  if (!text) return null
  return (
    <div className="tg-mlist__sticky-date" data-visible={visible || undefined} aria-hidden="true">
      <span className="tg-mlist__date-pill">{text}</span>
    </div>
  )
}

export function TopLoader({ active }: { active: boolean }) {
  if (!active) return null
  return (
    <div className="tg-mlist__top-loader">
      <Spinner label={t('w.messageList.d9b3ce')} size="sm" />
    </div>
  )
}

export function JumpNotice({ text, onDismiss }: { text: string; onDismiss: () => void }) {
  if (!text) return null
  return (
    <div className="tg-mlist__notice" role="status">
      <button type="button" className="tg-mlist__date-pill" onClick={onDismiss}>
        {text}
      </button>
    </div>
  )
}

export interface ScrollButtonProps {
  visible: boolean
  returning: boolean
  unread: number
  busy: boolean
  onClick: () => void
}

/**
 * Telegram's single round button: while a return point exists it goes back there first;
 * otherwise it goes to the newest message. The badge counts unseen incoming messages.
 */
export function ScrollButton({ visible, returning, unread, busy, onClick }: ScrollButtonProps) {
  return (
    <button
      type="button"
      className="tg-mlist__fab"
      data-visible={visible || undefined}
      data-returning={returning || undefined}
      aria-label={returning ? t('w.messageList.71a148') : t('w.messageList.79ed68')}
      tabIndex={visible ? 0 : -1}
      aria-hidden={!visible}
      onClick={onClick}
    >
      {busy ? (
        <Spinner label={t('w.messageList.95b55a')} size="sm" />
      ) : (
        <svg width="24" height="24" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
          <path
            d="M6 9l6 6 6-6"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      )}
      {unread > 0 && !returning ? (
        <span className="tg-mlist__fab-badge">
          <Badge count={unread} max={999} />
        </span>
      ) : null}
    </button>
  )
}
