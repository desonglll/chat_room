/**
 * Side-effect module: plugs the `entity_text` kind into TG-103's content registry. Imported
 * once at app start (`main.tsx`). Priority 1 beats plain text (0) and loses to every media
 * kind and the recalled placeholder, so captions and deleted messages are untouched.
 * TG-1208: also registers 设置 › 我的账号 › 表情状态, where the account sets its status.
 */
import { createElement, lazy } from 'react'
import { hasCustomEmoji, parseMessageEntities, type BroadcastMessage } from '@tg/core'
import { registerMessageContent } from '../message'
import { registerSettingsPage, SettingsIcon } from '../settings/shell'
import { t } from '../../i18n/index'
import { EntityTextContent } from './EntityText'

const EmojiStatusPicker = lazy(() => import('./EmojiStatusPicker').then((m) => ({ default: m.EmojiStatusPicker })))

export function isEntityTextMessage(message: BroadcastMessage): boolean {
  return (
    message.attachment === null &&
    message.recalled_at === null &&
    hasCustomEmoji(parseMessageEntities(message.entities, message.content))
  )
}

export const unregisterEntityText = registerMessageContent('entity_text', EntityTextContent, {
  match: isEntityTextMessage,
  priority: 1,
})

registerSettingsPage({
  id: 'account.emoji-status',
  section: 'account',
  get title() {
    return t('w.customEmoji.statusPage')
  },
  icon: createElement(SettingsIcon, { name: 'stickers' }),
  // After 头像 (20) and before 我的二维码 (30).
  order: 25,
  component: ({ onBack }) => createElement(EmojiStatusPicker, { onDone: onBack }),
})
