import {
  DeleteOutlined,
  DownloadOutlined,
  EditOutlined,
  FileOutlined,
  LockOutlined,
  MoreOutlined,
  RollbackOutlined,
} from '@ant-design/icons'
import { Button, Dropdown, Empty, Image, Popover, Spin, Tooltip } from 'antd'
import { useEffect, useRef, useState } from 'react'
import type { MenuProps } from 'antd'
import { UserAvatar } from '../../components/UserAvatar'
import { formatBytes, formatTime } from '../../lib/format'
import type { StoredMessage } from '../../types'

const REACTIONS = ['👍', '❤️', '😂', '😮', '😢', '👏']

interface MessageListProps {
  messages: StoredMessage[]
  currentUserId: string
  loading: boolean
  canLoadOlder: boolean
  onLoadOlder: () => void
  onReply: (message: StoredMessage) => void
  onEdit: (message: StoredMessage) => void
  onRecall: (message: StoredMessage) => void
  onReact: (message: StoredMessage, emoji: string, active: boolean) => void
  onRead: (messageId: string) => void
}

export function MessageList({
  messages,
  currentUserId,
  loading,
  canLoadOlder,
  onLoadOlder,
  onReply,
  onEdit,
  onRecall,
  onReact,
  onRead,
}: MessageListProps) {
  const viewportRef = useRef<HTMLDivElement>(null)
  const previousCountRef = useRef(0)
  const stayAtBottomRef = useRef(true)

  useEffect(() => {
    const newest = messages.at(-1)
    if (newest) onRead(newest.id)
    if (messages.length >= previousCountRef.current && viewportRef.current && stayAtBottomRef.current) {
      viewportRef.current.scrollTop = viewportRef.current.scrollHeight
    }
    previousCountRef.current = messages.length
  }, [messages, onRead])

  if (loading && messages.length === 0) {
    return (
      <div className="flex min-h-0 flex-1 items-center justify-center">
        <Spin tip="正在连接" />
      </div>
    )
  }

  if (messages.length === 0) {
    return (
      <div className="flex min-h-0 flex-1 items-center justify-center">
        <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="这里还没有消息" />
      </div>
    )
  }

  return (
    <div
      ref={viewportRef}
      className="scrollbar-thin min-h-0 flex-1 overflow-y-auto bg-[#f8faf9] px-3 py-5 sm:px-7"
      onScroll={(event) => {
        const viewport = event.currentTarget
        stayAtBottomRef.current = viewport.scrollHeight - viewport.scrollTop - viewport.clientHeight < 120
      }}
    >
      <div className="mx-auto w-full max-w-3xl">
        {canLoadOlder && (
          <div className="mb-5 text-center">
            <Button type="link" size="small" loading={loading} onClick={onLoadOlder}>
              查看更早的消息
            </Button>
          </div>
        )}
        <div className="space-y-4">
          {messages.map((message) => (
            <MessageRow
              key={message.id}
              message={message}
              own={message.sender_id === currentUserId}
              currentUserId={currentUserId}
              onReply={onReply}
              onEdit={onEdit}
              onRecall={onRecall}
              onReact={onReact}
            />
          ))}
        </div>
      </div>
    </div>
  )
}

function MessageRow({
  message,
  own,
  currentUserId,
  onReply,
  onEdit,
  onRecall,
  onReact,
}: {
  message: StoredMessage
  own: boolean
  currentUserId: string
  onReply: (message: StoredMessage) => void
  onEdit: (message: StoredMessage) => void
  onRecall: (message: StoredMessage) => void
  onReact: (message: StoredMessage, emoji: string, active: boolean) => void
}) {
  const menu: MenuProps['items'] = [
    { key: 'reply', label: '回复', icon: <RollbackOutlined /> },
    ...(own && !message.recalled_at
      ? [
          { key: 'edit', label: '编辑', icon: <EditOutlined /> },
          { key: 'recall', label: '撤回', danger: true, icon: <DeleteOutlined /> },
        ]
      : []),
  ]

  return (
    <article className={`group flex items-start gap-2.5 ${own ? 'flex-row-reverse' : ''}`}>
      <UserAvatar emoji={message.sender_avatar} name={message.sender} size={36} />
      <div className={`min-w-0 max-w-[82%] sm:max-w-[72%] ${own ? 'items-end' : 'items-start'} flex flex-col`}>
        <div className={`mb-1 flex items-center gap-2 text-[11px] text-[#7b8782] ${own ? 'flex-row-reverse' : ''}`}>
          <span>{message.sender}</span>
          <time>{formatTime(message.created_at)}</time>
          {message.edited_at && <span>已编辑</span>}
        </div>
        <div className={`flex items-start gap-1 ${own ? 'flex-row-reverse' : ''}`}>
          <div
            className={`min-w-0 rounded-lg px-3.5 py-2.5 text-sm leading-6 shadow-sm ${
              own ? 'bg-[#087f5b] text-white' : 'border border-[#e1e7e4] bg-white text-[#1d2924]'
            }`}
          >
            {message.forwarded_from && (
              <p className={`mb-1 text-xs ${own ? 'text-white/70' : 'text-[#718078]'}`}>
                转发自 {message.forwarded_from.sender} · {message.forwarded_from.room_name}
              </p>
            )}
            {message.reply_to && (
              <div className={`mb-2 border-l-2 pl-2 text-xs ${own ? 'border-white/50 text-white/75' : 'border-[#58a98d] text-[#66746e]'}`}>
                <strong className="block font-medium">{message.reply_to.sender}</strong>
                <span className="line-clamp-2">
                  {message.reply_to.recalled
                    ? '原消息已撤回'
                    : message.reply_to.content || message.reply_to.attachment_file_name}
                </span>
              </div>
            )}
            {message.recalled_at ? (
              <span className={own ? 'text-white/65' : 'text-[#8a9590]'}>消息已撤回</span>
            ) : (
              <>
                {message.content && <div className="message-body">{message.content}</div>}
                {message.attachment && <MessageAttachment attachment={message.attachment} own={own} />}
              </>
            )}
          </div>
          <Dropdown
            menu={{
              items: menu,
              onClick: ({ key }) => {
                if (key === 'reply') onReply(message)
                if (key === 'edit') onEdit(message)
                if (key === 'recall') onRecall(message)
              },
            }}
            trigger={['click']}
          >
            <Button
              type="text"
              size="small"
              className="opacity-0 group-hover:opacity-100 group-focus-within:opacity-100"
              icon={<MoreOutlined />}
              aria-label="消息操作"
            />
          </Dropdown>
        </div>
        {!message.recalled_at && (
          <div className={`mt-1.5 flex flex-wrap items-center gap-1 ${own ? 'justify-end' : ''}`}>
            {message.reactions.map((reaction) => (
              <button
                key={reaction.emoji}
                type="button"
                className={`rounded-full border px-2 py-0.5 text-xs ${
                  reaction.user_ids.includes(currentUserId)
                    ? 'border-[#7ebca5] bg-[#e5f4ed] text-[#076b4d]'
                    : 'border-[#dce3e0] bg-white text-[#596761]'
                }`}
                onClick={() =>
                  onReact(message, reaction.emoji, !reaction.user_ids.includes(currentUserId))
                }
              >
                {reaction.emoji} {reaction.user_ids.length}
              </button>
            ))}
            <Popover
              trigger="click"
              content={
                <div className="flex gap-1">
                  {REACTIONS.map((emoji) => (
                    <Button
                      key={emoji}
                      type="text"
                      size="small"
                      onClick={() =>
                        onReact(
                          message,
                          emoji,
                          !message.reactions
                            .find((reaction) => reaction.emoji === emoji)
                            ?.user_ids.includes(currentUserId),
                        )
                      }
                    >
                      {emoji}
                    </Button>
                  ))}
                </div>
              }
            >
              <button
                type="button"
                className="rounded-full px-1.5 py-0.5 text-xs text-[#7c8983] opacity-0 hover:bg-[#edf1ef] group-hover:opacity-100 group-focus-within:opacity-100"
                aria-label="添加回应"
              >
                +☺
              </button>
            </Popover>
          </div>
        )}
      </div>
    </article>
  )
}

function MessageAttachment({ attachment, own }: { attachment: StoredMessage['attachment']; own: boolean }) {
  const [revealed, setRevealed] = useState(false)
  if (!attachment) return null
  const sensitive = attachment.is_sensitive && !revealed
  const image = attachment.mime_type.startsWith('image/')

  if (sensitive) {
    return (
      <button
        type="button"
        className={`mt-2 flex w-full items-center gap-2 rounded-md border px-3 py-3 text-left text-xs ${
          own ? 'border-white/25 bg-black/10' : 'border-[#dce3e0] bg-[#f5f7f6]'
        }`}
        onClick={() => setRevealed(true)}
      >
        <LockOutlined />
        敏感内容，点击查看
      </button>
    )
  }

  if (image) {
    return (
      <div className="mt-2 overflow-hidden rounded-md bg-black/5">
        <Image
          src={attachment.download_url}
          alt={attachment.file_name}
          className="max-h-80 object-contain"
        />
      </div>
    )
  }

  return (
    <a
      href={attachment.download_url}
      className={`mt-2 flex min-w-52 items-center gap-3 rounded-md border px-3 py-2.5 no-underline ${
        own ? 'border-white/25 text-white' : 'border-[#dce3e0] text-[#24312c]'
      }`}
      download
    >
      <FileOutlined className="text-xl" />
      <span className="min-w-0 flex-1">
        <span className="block truncate text-xs font-medium">{attachment.file_name}</span>
        <span className={`text-[11px] ${own ? 'text-white/65' : 'text-[#7a8781]'}`}>
          {formatBytes(attachment.size_bytes)}
        </span>
      </span>
      <Tooltip title="下载">
        <DownloadOutlined />
      </Tooltip>
    </a>
  )
}
