/**
 * Pure channel presentation rules: view counts in Telegram's compact form, the subscriber
 * line, and what a message's meta shows when it is a channel post.
 */
import type { BroadcastMessage } from '@tg/core'
import { t } from '../../i18n/index'

const compact = (value: number, unit: number, suffix: string): string => {
  const scaled = value / unit
  // 1.2K, 12K, 123K — one decimal only while it still says something.
  const text = scaled < 10 ? (Math.floor(scaled * 10) / 10).toFixed(1).replace(/\.0$/, '') : String(Math.floor(scaled))
  return `${text}${suffix}`
}

/** 999 → "999", 1 234 → "1.2K", 12 345 → "12K", 1 234 567 → "1.2M". */
export function formatViews(views: number): string {
  if (!Number.isFinite(views) || views < 0) return '0'
  if (views < 1000) return String(Math.floor(views))
  if (views < 1_000_000) return compact(views, 1000, 'K')
  return compact(views, 1_000_000, 'M')
}

/** The header subtitle: "12,345 位订阅者". */
export function subscriberLine(count: number): string {
  const total = Math.max(0, count)
  // The grouped string fills {0}; the raw number (TG-905) is what picks the plural form.
  return t('w.channel.2b75a3', total.toLocaleString('zh-CN'), total)
}

/** The channel fields of one post, as the meta shows them. */
export interface ChannelPostParts {
  messageId: string
  views: number
  author: string
}

/** `null` unless the server marked `message` as a channel post (it carries `views`). */
export function channelPostOf(message: BroadcastMessage): ChannelPostParts | null {
  if (typeof message.views !== 'number') return null
  return { messageId: message.message_id, views: message.views, author: message.post_author ?? '' }
}

/** The post's part of the meta's accessible sentence. */
export function channelPostLabel(post: ChannelPostParts): string {
  return [t('w.channel.fdaf57', formatViews(post.views), post.views), post.author].filter(Boolean).join(' ')
}

/** Whether `myPermissions` (from `GET /permissions`) lets the viewer publish. */
export function canPublish(myPermissions: readonly string[] | null): boolean {
  return myPermissions?.includes('message.post') ?? false
}

/** TG-1203: whether a group's permission view still lets the viewer send (restrictions applied). */
export function canWriteInGroup(myPermissions: readonly string[]): boolean {
  return myPermissions.includes('message.send')
}
