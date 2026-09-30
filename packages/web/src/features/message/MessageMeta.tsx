/**
 * Timestamp, edited marker and delivery ticks — rendered twice by design:
 *
 *  - `MessageMeta` is the visible one, absolutely positioned at the bubble's bottom-right;
 *  - `MetaSpacer` is an invisible clone the content puts inline after its last character.
 *
 * Because the spacer is the same markup in the same font, it is exactly as wide as the
 * visible meta, so the text reflows around it: when the last line has room the time sits
 * beside it, when it does not the spacer wraps and the bubble grows a line. The two can
 * never overlap at any text length. This is Telegram's trick; `metaParts` is the single
 * source both render from, which is what the no-overlap test pins.
 */
import type { BroadcastMessage } from '@tg/core'
import type { DeliveryStatus } from './types'
import { ClockGlyph, FailedGlyph, TickGlyph } from './icons'
import { ChannelPostMeta } from '../channel/ChannelPostMeta'
import type { ChannelPostParts } from '../channel/channelModel'
import { channelPostLabel, channelPostOf } from '../channel/channelModel'

const timeFormat = new Intl.DateTimeFormat('zh-CN', { hour: '2-digit', minute: '2-digit', hour12: false })

export function formatMessageTime(timestamp: string): string {
  const parsed = new Date(timestamp)
  return Number.isNaN(parsed.getTime()) ? '' : timeFormat.format(parsed)
}

export function resolveDelivery(message: BroadcastMessage, explicit: DeliveryStatus | undefined): DeliveryStatus {
  if (explicit !== undefined) return explicit
  return message.delivery_state ?? 'sent'
}

const DELIVERY_LABEL: Record<DeliveryStatus, string> = {
  sending: '发送中',
  sent: '已发送',
  read: '已读',
  failed: '发送失败',
}

export interface MetaParts {
  edited: boolean
  time: string
  /** `null` for incoming messages: ticks are only ever shown on your own messages. */
  delivery: DeliveryStatus | null
  /** The whole meta as one sentence for assistive technology. */
  label: string
  /** TG-202: a channel post's views and signature, drawn before the time. */
  post: ChannelPostParts | null
}

export function metaParts(message: BroadcastMessage, outgoing: boolean, delivery: DeliveryStatus): MetaParts {
  const edited = message.edited_at !== null && message.recalled_at === null
  const time = formatMessageTime(message.timestamp)
  const shown = outgoing ? delivery : null
  const post = channelPostOf(message)
  const label = [
    post ? channelPostLabel(post) : '',
    edited ? '已编辑' : '',
    time,
    shown === null ? '' : DELIVERY_LABEL[shown],
  ]
    .filter(Boolean)
    .join(' ')
  return { edited, time, delivery: shown, label, post }
}

function DeliveryIcon({ delivery }: { delivery: DeliveryStatus }) {
  if (delivery === 'sending') return <ClockGlyph />
  if (delivery === 'failed') return <FailedGlyph />
  return <TickGlyph double={delivery === 'read'} />
}

function MetaInner({ parts, visible = false }: { parts: MetaParts; visible?: boolean }) {
  return (
    <>
      {parts.post ? <ChannelPostMeta post={parts.post} report={visible} /> : null}
      {parts.edited ? <span className="tg-bubble__edited">已编辑</span> : null}
      <time className="tg-bubble__time">{parts.time}</time>
      {parts.delivery === null ? null : (
        <span className="tg-bubble__delivery" data-delivery={parts.delivery}>
          <DeliveryIcon delivery={parts.delivery} />
        </span>
      )}
    </>
  )
}

export function MessageMeta({ parts, overlay, dateTime }: { parts: MetaParts; overlay: boolean; dateTime: string }) {
  return (
    <span
      className="tg-bubble__meta"
      data-overlay={overlay ? '' : undefined}
      role="note"
      aria-label={parts.label}
      title={dateTime}
    >
      <MetaInner parts={parts} visible />
    </span>
  )
}

export function MetaSpacer({ parts }: { parts: MetaParts }) {
  return (
    <span className="tg-bubble__meta-spacer" aria-hidden="true">
      <MetaInner parts={parts} />
    </span>
  )
}
