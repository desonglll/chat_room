/**
 * Side-effect module: plugs the `entity_text` kind into TG-103's content registry. Imported
 * once at app start (`main.tsx`). Priority 1 beats plain text (0) and loses to every media
 * kind and the recalled placeholder, so captions and deleted messages are untouched.
 */
import { hasCustomEmoji, parseMessageEntities, type BroadcastMessage } from '@tg/core'
import { registerMessageContent } from '../message'
import { EntityTextContent } from './EntityText'

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
