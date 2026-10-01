/** One roster row: avatar, name, role badge or title, and — when visible — the restriction. */
import type { ChatMemberEntry } from '@tg/core'
import { Avatar } from '@tg/ui'
import { memberName, roleBadge, untilText } from './chatAdminModel'
import { t } from '../../i18n/index'

export interface MemberRowProps {
  entry: ChatMemberEntry
  /** Opens the editor for this member; the row is static without it. */
  onOpen?: (() => void) | undefined
}

export function MemberRow({ entry, onOpen }: MemberRowProps) {
  const name = memberName(entry)
  const badge = roleBadge(entry)
  const restriction = entry.restrictions?.[0]
  const body = (
    <>
      <Avatar label={name} initials={entry.avatar_emoji || undefined} size="md" />
      <span className="tg-chatadmin__member-text">
        <span className="tg-chatadmin__member-name">{name}</span>
        {restriction ? (
          <span className="tg-chatadmin__member-sub" data-tone="danger">
            {t('w.chatAdmin.a7c55d')} {untilText(restriction.until)}
          </span>
        ) : (
          <span className="tg-chatadmin__member-sub">@{entry.username}</span>
        )}
      </span>
      {badge ? <span className="tg-chatadmin__badge">{badge}</span> : null}
    </>
  )
  return (
    <li className="tg-chatadmin__member">
      {onOpen ? (
        <button type="button" className="tg-chatadmin__member-button" onClick={onOpen}>
          {body}
        </button>
      ) : (
        <div className="tg-chatadmin__member-button">{body}</div>
      )}
    </li>
  )
}
