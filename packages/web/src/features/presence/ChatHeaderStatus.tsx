/**
 * The chat header's second line (TG-107): the typing summary when anyone is active, else
 * the member count for groups/channels or the peer's last-seen for a private chat.
 * Connection-state copy ("连接中…") stays with the header owner, which renders it
 * instead of this component while the socket is not online.
 */
import { useHeaderStatus, useTypingStatus } from './usePresenceText'
import type { HeaderStatus } from './presenceText'
import { TypingIndicator } from './TypingIndicator'
import './presence.css'

export function PresenceStatusLine({ status, className }: { status: HeaderStatus; className?: string | undefined }) {
  const classes = ['tg-presence-status', className].filter(Boolean).join(' ')
  const state = status.kind === 'typing' ? 'typing' : status.kind === 'last_seen' && status.online ? 'online' : 'idle'
  return (
    <span className={classes} data-state={state}>
      {status.kind === 'typing' ? <TypingIndicator action={status.action} /> : null}
      <span className="tg-presence-status__text">{status.text}</span>
    </span>
  )
}

export function ChatHeaderStatus({ chatId, className }: { chatId: string; className?: string | undefined }) {
  return <PresenceStatusLine status={useHeaderStatus(chatId)} className={className} />
}

/** Chat-list row preview replacement: renders only while someone in the chat is active. */
export function ChatTypingLine({ chatId, className }: { chatId: string; className?: string | undefined }) {
  const status = useTypingStatus(chatId)
  return status ? <PresenceStatusLine status={status} className={className} /> : null
}
