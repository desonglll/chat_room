/**
 * The app-wide content registry, pre-filled with the built-in kinds. Priorities leave room
 * for later kinds: a recalled message always shows the placeholder (100), specific media
 * beats generic file (20 > 10), and text is the catch-all (0).
 */
import type { BroadcastMessage } from '@tg/core'
import { createContentRegistry } from './registry'
import { DeletedContent, FileContent, ImageContent, TextContent, VideoContent } from './builtinContents'
import { hasAttachmentOf, hasCaption } from './attachmentKind'

export const messageContent = createContentRegistry()

const mediaMeta = (message: BroadcastMessage) => (hasCaption(message) ? 'inline' : 'overlay')
const mediaFrame = (message: BroadcastMessage) => (hasCaption(message) ? 'bubble' : 'media')

messageContent.register('text', TextContent, { match: () => true, priority: 0 })
messageContent.register('file', FileContent, { match: (m) => m.attachment !== null, priority: 10 })
messageContent.register('image', ImageContent, {
  match: (m) => hasAttachmentOf(m, 'image'),
  frame: mediaFrame,
  metaPlacement: mediaMeta,
  leadingMedia: true,
  priority: 20,
})
messageContent.register('video', VideoContent, {
  match: (m) => hasAttachmentOf(m, 'video'),
  frame: mediaFrame,
  metaPlacement: mediaMeta,
  leadingMedia: true,
  priority: 20,
})
messageContent.register('deleted', DeletedContent, { match: (m) => m.recalled_at !== null, priority: 100 })

/** Plug a new message kind into every bubble. See `registry.ts`. */
export const registerMessageContent = messageContent.register
