/**
 * TG-901: what the pinned bar shows. Telegram shows the newest pin first; each click jumps to
 * the shown message and moves the bar to the next older pin, wrapping around. Pure.
 */
import type { StoredMessage } from '@tg/core'
import { t } from '../../../i18n/index'
import type { ChatPin } from './pinnedApi'

export interface PinEntry {
  messageId: string
  text: string
  pinnedAt: string
}

/** One line for a pinned message: its text, else what kind of message it is. */
export function pinPreview(message: StoredMessage): string {
  const text = message.content.replace(/\s+/g, ' ').trim()
  if (text) return text
  if (message.poll) return t('w.chatList.media.poll')
  if (message.location) return t('w.chatList.media.location')
  if (message.contact) return t('w.chatList.media.contact')
  if (message.attachment) return message.attachment.file_name
  return t('w.chat.pinnedMessage')
}

/** Newest first; recalled messages drop out (the server keeps their pin rows). */
export function toPinEntries(pins: readonly ChatPin[]): PinEntry[] {
  return pins
    .filter((pin) => pin.message.recalled_at === null)
    .map((pin) => ({ messageId: pin.message.id, text: pinPreview(pin.message), pinnedAt: pin.pinned_at }))
    .sort((a, b) => Date.parse(b.pinnedAt) - Date.parse(a.pinnedAt))
}

/** The index shown after a click on `index`: the next older pin, wrapping to the newest. */
export function nextPinIndex(index: number, count: number): number {
  return count === 0 ? 0 : (index + 1) % count
}
