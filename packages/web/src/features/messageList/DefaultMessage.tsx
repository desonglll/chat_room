/**
 * The fallback `renderMessage` that keeps the app usable until TG-103's bubble is wired
 * into the seam. It honours every `MessageRenderContext` field so the grouping, identity
 * and highlight contract is visible end to end, but it is deliberately plain: the real
 * anatomy (tails, ticks, context menu, reactions) is TG-103's.
 *
 * Height discipline (the list's anchor depends on it): an image attachment reserves a
 * fixed box before it loads, so a late image never changes the row height.
 */
import type { BroadcastMessage, DisplayMessage, UploadMessage } from '@tg/core'
import { Avatar } from '@tg/ui'
import { useMessageListActions } from './messageListActions'
import type { MessageRenderContext } from './renderContract'

const timeFormat = new Intl.DateTimeFormat('zh-CN', { hour: '2-digit', minute: '2-digit' })

function timeOf(timestamp: string): string {
  const parsed = new Date(timestamp)
  return Number.isNaN(parsed.getTime()) ? '' : timeFormat.format(parsed)
}

function deliveryMark(message: BroadcastMessage): string {
  if (message.delivery_state === 'sending') return '⋯'
  if (message.delivery_state === 'failed') return '未发送'
  return ''
}

function Attachment({ message }: { message: BroadcastMessage }) {
  const attachment = message.attachment
  if (!attachment) return null
  if (attachment.mime_type.startsWith('image/') && !attachment.is_sensitive) {
    return (
      <span className="tg-mlist-default__media">
        <img src={attachment.download_url} alt={attachment.file_name} loading="lazy" decoding="async" />
      </span>
    )
  }
  return (
    <a className="tg-mlist-default__file" href={attachment.download_url} target="_blank" rel="noreferrer">
      {attachment.file_name}
    </a>
  )
}

function BroadcastBubble({ message, ctx }: { message: BroadcastMessage; ctx: MessageRenderContext }) {
  const { jumpToMessage } = useMessageListActions()
  const mark = ctx.isOutgoing ? deliveryMark(message) : ''
  return (
    <div
      className={`tg-msg ${ctx.isOutgoing ? 'tg-msg--out' : 'tg-msg--in'} tg-mlist-default`}
      data-group={ctx.groupPosition}
      data-selected={ctx.selected || undefined}
    >
      {!ctx.isOutgoing ? (
        <span className="tg-mlist-default__avatar" aria-hidden={!ctx.showAvatar}>
          {ctx.showAvatar ? (
            <Avatar label={message.sender} initials={message.sender_avatar || undefined} size="sm" />
          ) : null}
        </span>
      ) : null}
      <div className="tg-msg__bubble">
        {ctx.showSenderName ? <span className="tg-msg__sender">{message.sender}</span> : null}
        {message.forwarded_from ? (
          <span className="tg-mlist-default__forward">转发自 {message.forwarded_from.sender}</span>
        ) : null}
        {message.reply_to ? (
          <button
            type="button"
            className="tg-msg__reply tg-mlist-default__reply"
            onClick={() => jumpToMessage(message.reply_to?.message_id ?? '')}
          >
            {message.reply_to.sender}：{message.reply_to.recalled ? '消息已撤回' : message.reply_to.content}
          </button>
        ) : null}
        {message.recalled_at ? (
          <span className="tg-msg__recalled">消息已撤回</span>
        ) : (
          <>
            <Attachment message={message} />
            {message.content ? <span className="tg-msg__content">{message.content}</span> : null}
          </>
        )}
        <span className="tg-msg__meta">
          {message.edited_at && !message.recalled_at ? <span>已编辑</span> : null}
          <time dateTime={message.timestamp}>{timeOf(message.timestamp)}</time>
          {mark ? <span data-state={message.delivery_state}>{mark}</span> : null}
        </span>
      </div>
    </div>
  )
}

function UploadRow({ message }: { message: UploadMessage }) {
  const percent = message.total_bytes > 0 ? Math.round((message.processed_bytes / message.total_bytes) * 100) : 0
  return (
    <div className="tg-msg tg-msg--out tg-mlist-default">
      <div className="tg-msg__bubble">
        <span className="tg-msg__content">{message.file_name}</span>
        <span className="tg-msg__meta">{message.status === 'failed' ? '上传失败' : `上传中 ${percent}%`}</span>
      </div>
    </div>
  )
}

export function DefaultMessage({ message, ctx }: { message: DisplayMessage; ctx: MessageRenderContext }) {
  if (message.type === 'system') {
    return (
      <div className="tg-mlist-service">
        <span className="tg-mlist-service__pill">{message.content}</span>
      </div>
    )
  }
  if (message.type === 'upload') return <UploadRow message={message} />
  return <BroadcastBubble message={message} ctx={ctx} />
}

export const renderDefaultMessage = (message: DisplayMessage, ctx: MessageRenderContext) => (
  <DefaultMessage message={message} ctx={ctx} />
)
