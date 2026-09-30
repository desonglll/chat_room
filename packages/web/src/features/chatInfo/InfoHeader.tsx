/**
 * The three info headers (private / group / channel) plus their detail rows. The
 * variant comes from `selectInfoHeader` (chat_type), never from ad-hoc checks here.
 */
import type { ReactNode } from 'react'
import { Avatar, Toggle } from '@tg/ui'
import { linkify } from '../message/content/linkify'
import { useLastSeenText } from '../presence'
import type { InfoHeaderModel } from './chatInfoModel'
import { memberCountText, subscriberCountText } from './chatInfoModel'
import { AutoDeleteRow } from './AutoDeleteRow'
import { AtIcon, BellIcon, InfoIcon } from './icons'

function PrivateStatus({ userId }: { userId: string }) {
  const text = useLastSeenText(userId)
  return (
    <p className="tg-chatinfo__status" data-online={text === '在线' || undefined}>
      {text}
    </p>
  )
}

export function InfoIdentity({ header, onlineCount = 0 }: { header: InfoHeaderModel; onlineCount?: number }) {
  let status: ReactNode
  if (header.variant === 'private') status = header.userId ? <PrivateStatus userId={header.userId} /> : null
  else if (header.variant === 'group')
    status = <p className="tg-chatinfo__status">{memberCountText(header.memberCount, onlineCount)}</p>
  else status = <p className="tg-chatinfo__status">{subscriberCountText(header.subscriberCount)}</p>

  return (
    <div className="tg-chatinfo__identity" data-variant={header.variant}>
      <Avatar label={header.title || '会话'} initials={header.avatarEmoji || undefined} size="xl" />
      <h3 className="tg-chatinfo__title">{header.title || '…'}</h3>
      {status}
    </div>
  )
}

/** Bio/description text with its http(s) URLs as links (the bubble's own linkify rule). */
function LinkifiedText({ text }: { text: string }) {
  return linkify(text).map((segment, index) =>
    segment.type === 'link' ? (
      <a key={index} className="tg-chatinfo__link" href={segment.href} target="_blank" rel="noopener noreferrer">
        {segment.text}
      </a>
    ) : (
      segment.text
    ),
  )
}

function DetailRow({ icon, value, label }: { icon: ReactNode; value: ReactNode; label: string }) {
  return (
    <li className="tg-chatinfo__row">
      <span className="tg-chatinfo__row-icon">{icon}</span>
      <span className="tg-chatinfo__row-text">
        <span className="tg-chatinfo__row-value">{value}</span>
        <span className="tg-chatinfo__row-label">{label}</span>
      </span>
    </li>
  )
}

export interface InfoDetailsProps {
  header: InfoHeaderModel
  notificationsOn: boolean
  notificationsBusy: boolean
  onNotificationsChange(enabled: boolean): void
  /** TG-405: shows the «自动删除消息» row when given. */
  chatId?: string | undefined
}

export function InfoDetails({
  header,
  notificationsOn,
  notificationsBusy,
  onNotificationsChange,
  chatId,
}: InfoDetailsProps) {
  const about = header.variant === 'private' ? header.bio : header.description
  return (
    <ul className="tg-chatinfo__details" aria-label="详细信息">
      {about ? <DetailRow icon={<InfoIcon />} value={<LinkifiedText text={about} />} label="简介" /> : null}
      {header.username ? (
        <DetailRow
          icon={<AtIcon />}
          value={`@${header.username}`}
          label={header.variant === 'private' ? '用户名' : '链接'}
        />
      ) : null}
      <li className="tg-chatinfo__row tg-chatinfo__row--toggle">
        <span className="tg-chatinfo__row-icon">
          <BellIcon />
        </span>
        <Toggle
          className="tg-chatinfo__toggle"
          label="通知"
          description={notificationsOn ? '已开启' : '已静音'}
          checked={notificationsOn}
          disabled={notificationsBusy}
          onCheckedChange={onNotificationsChange}
        />
      </li>
      {chatId ? <AutoDeleteRow chatId={chatId} /> : null}
    </ul>
  )
}
