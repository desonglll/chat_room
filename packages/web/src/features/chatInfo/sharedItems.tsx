/** One row/tile per shared-content kind. Presentational; data arrives from the pagers. */
import type { ReactNode } from 'react'
import { useState } from 'react'
import type { ChatMembership } from '@tg/core'
import { labelled } from '@tg/core'
import { Avatar } from '@tg/ui'
import { formatChatListTime } from '../chatList/chatTime'
import { openMediaViewer } from '../mediaViewer'
import { formatBytes } from '../message/content/attachmentKind'
import { useLastSeenText } from '../presence'
import { FileIcon, PlayIcon } from './icons'
import { memberName } from './memberSource'
import type { SharedFile, SharedLink } from './sharedSources'
import { t } from '../../i18n/index'
import { EmojiStatus } from '../customEmoji/EmojiStatus'

const when = (createdAt: string) => formatChatListTime(createdAt, new Date())

function extension(fileName: string): string {
  const dot = fileName.lastIndexOf('.')
  return dot > 0 && fileName.length - dot <= 5 ? fileName.slice(dot + 1).toUpperCase() : ''
}

export function MediaTile({ chatId, file }: { chatId: string; file: SharedFile }) {
  const { attachment } = file
  const [revealed, setRevealed] = useState(!attachment.is_sensitive)
  const video = attachment.mime_type.toLowerCase().startsWith('video/')
  return (
    <button
      type="button"
      className="tg-chatinfo__tile"
      data-veiled={revealed ? undefined : ''}
      aria-label={
        revealed
          ? labelled(video ? t('w.chatInfo.fa4e33') : t('w.chatInfo.48e9e5'), attachment.file_name)
          : t('w.chatInfo.976c5d')
      }
      onClick={(event) => {
        if (!revealed) {
          setRevealed(true)
          return
        }
        const box = event.currentTarget.getBoundingClientRect()
        openMediaViewer({
          chatId,
          attachmentId: attachment.id,
          sourceRect: { x: box.left, y: box.top, width: box.width, height: box.height },
        })
      }}
    >
      {video ? (
        <>
          <video src={attachment.download_url} preload="metadata" muted playsInline tabIndex={-1} />
          <span className="tg-chatinfo__tile-badge">
            <PlayIcon />
          </span>
        </>
      ) : (
        <img src={attachment.download_url} alt="" loading="lazy" decoding="async" draggable={false} />
      )}
    </button>
  )
}

export function GifTile({ file }: { file: SharedFile }) {
  return (
    <a
      className="tg-chatinfo__tile"
      href={file.attachment.download_url}
      target="_blank"
      rel="noopener noreferrer"
      aria-label={labelled('GIF', file.attachment.file_name)}
    >
      <img src={file.attachment.download_url} alt="" loading="lazy" decoding="async" draggable={false} />
    </a>
  )
}

export function FileRow({ file }: { file: SharedFile }) {
  const { attachment } = file
  const ext = extension(attachment.file_name)
  return (
    <li>
      <a className="tg-chatinfo__item" href={attachment.download_url} target="_blank" rel="noopener noreferrer">
        <span className="tg-chatinfo__item-thumb" data-kind="file">
          {ext || <FileIcon />}
        </span>
        <span className="tg-chatinfo__item-text">
          <span className="tg-chatinfo__item-title">{attachment.file_name}</span>
          <span className="tg-chatinfo__item-meta">
            {formatBytes(attachment.size_bytes)} · {when(file.createdAt)}
          </span>
        </span>
      </a>
    </li>
  )
}

export function LinkRow({ link }: { link: SharedLink }) {
  return (
    <li>
      <a className="tg-chatinfo__item" href={link.url} target="_blank" rel="noopener noreferrer">
        <span className="tg-chatinfo__item-thumb" data-kind="link">
          {link.host.slice(0, 1).toUpperCase()}
        </span>
        <span className="tg-chatinfo__item-text">
          <span className="tg-chatinfo__item-title">{link.host}</span>
          <span className="tg-chatinfo__item-link">{link.url}</span>
          <span className="tg-chatinfo__item-meta">
            {link.sender} · {when(link.createdAt)}
          </span>
        </span>
      </a>
    </li>
  )
}

export function VoiceRow({ file }: { file: SharedFile }) {
  return (
    <li className="tg-chatinfo__item" data-kind="voice">
      <span className="tg-chatinfo__item-text">
        <span className="tg-chatinfo__item-title">{file.sender}</span>
        <span className="tg-chatinfo__item-meta">
          {when(file.createdAt)} · {formatBytes(file.attachment.size_bytes)}
        </span>
        <audio className="tg-chatinfo__audio" src={file.attachment.download_url} preload="none" controls />
      </span>
    </li>
  )
}

const ROLE_COPY: Record<ChatMembership['role'], string> = {
  get owner() {
    return t('w.chatInfo.4fbe15')
  },
  get admin() {
    return t('w.chatInfo.ef84e7')
  },
  member: '',
}

export function MemberRow({ member, action }: { member: ChatMembership; action?: ReactNode }) {
  const lastSeen = useLastSeenText(member.user_id)
  const name = memberName(member)
  return (
    <li className="tg-chatinfo__item" data-kind="member">
      <Avatar
        label={name}
        initials={member.avatar_emoji || undefined}
        size="md"
        online={lastSeen === t('w.chatInfo.0373ff')}
      />
      <span className="tg-chatinfo__item-text">
        <span className="tg-chatinfo__item-title">
          {name}
          <EmojiStatus userId={member.user_id} size={16} />
        </span>
        <span className="tg-chatinfo__item-meta" data-online={lastSeen === t('w.chatInfo.0373ff') || undefined}>
          {lastSeen}
        </span>
      </span>
      {ROLE_COPY[member.role] ? <span className="tg-chatinfo__item-role">{ROLE_COPY[member.role]}</span> : null}
      {action}
    </li>
  )
}
