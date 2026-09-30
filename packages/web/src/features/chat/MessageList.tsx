/**
 * The minimal message list: plain rows, no virtualization — TG-101 owns that, and
 * TG-103 owns the real bubble anatomy. What is already right and stays: rows come from
 * `messageStore` timelines, in/out sides, recall placeholders, delivery marks, and the
 * stick-to-bottom rule (follow only when the reader is already at the bottom).
 */
import { useEffect, useLayoutEffect, useRef } from 'react'
import type { BroadcastMessage } from '@tg/core'
import { messageStore, selectTimeline } from '@tg/core'
import { useStore } from 'zustand/react'
import { Spinner } from '@tg/ui'

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

const FOLLOW_THRESHOLD_PX = 80

export function MessageList({ chatId, currentUserId }: { chatId: string; currentUserId: string }) {
  const timeline = useStore(messageStore, selectTimeline(chatId))
  const viewportRef = useRef<HTMLDivElement>(null)
  const followRef = useRef(true)

  // Track whether the reader sits at the bottom BEFORE new rows change the height.
  const handleScroll = () => {
    const viewport = viewportRef.current
    if (!viewport) return
    followRef.current = viewport.scrollHeight - viewport.scrollTop - viewport.clientHeight < FOLLOW_THRESHOLD_PX
  }

  useLayoutEffect(() => {
    const viewport = viewportRef.current
    if (viewport && followRef.current) viewport.scrollTop = viewport.scrollHeight
  }, [timeline.messages.length, chatId])

  useEffect(() => {
    followRef.current = true
  }, [chatId])

  const rows = timeline.messages.filter((message): message is BroadcastMessage => message.type === 'broadcast')

  return (
    <div className="tg-messages" ref={viewportRef} onScroll={handleScroll}>
      {!timeline.historyReady && rows.length === 0 ? (
        <div className="tg-messages__loading">
          <Spinner label="正在载入消息" />
        </div>
      ) : null}
      {timeline.historyReady && rows.length === 0 ? (
        <div className="tg-messages__empty">
          <p className="tg-messages__empty-pill">还没有消息，说点什么吧</p>
        </div>
      ) : null}
      <ol className="tg-messages__list">
        {rows.map((message) => {
          const outgoing = message.sender_id === currentUserId
          return (
            <li key={message.message_id} className={`tg-msg ${outgoing ? 'tg-msg--out' : 'tg-msg--in'}`}>
              <div className="tg-msg__bubble">
                {!outgoing ? <span className="tg-msg__sender">{message.sender}</span> : null}
                {message.reply_to ? (
                  <span className="tg-msg__reply">
                    {message.reply_to.sender}：{message.reply_to.recalled ? '消息已撤回' : message.reply_to.content}
                  </span>
                ) : null}
                {message.recalled_at ? (
                  <span className="tg-msg__recalled">消息已撤回</span>
                ) : (
                  <span className="tg-msg__content">{message.content}</span>
                )}
                <span className="tg-msg__meta">
                  {message.edited_at && !message.recalled_at ? <span>已编辑</span> : null}
                  <time dateTime={message.timestamp}>{timeOf(message.timestamp)}</time>
                  {outgoing && deliveryMark(message) ? (
                    <span data-state={message.delivery_state}>{deliveryMark(message)}</span>
                  ) : null}
                </span>
              </div>
            </li>
          )
        })}
      </ol>
    </div>
  )
}
