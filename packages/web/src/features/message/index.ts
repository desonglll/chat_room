/**
 * TG-103 public surface. TG-101's list renders every row through `MessageBubble`; later
 * message kinds plug in through `registerMessageContent`. See docs/devlog/TG-103.md
 * "Frozen interface".
 */
export { MessageBubble } from './MessageBubble'
export type { DeliveryStatus, GroupPosition, MessageActions, MessageBubbleProps, MessageRenderContext } from './types'
export { messageContent, registerMessageContent } from './content/messageContent'
export { createContentRegistry } from './content/registry'
export type { ContentRegistry } from './content/registry'
export type {
  ContentFrame,
  ContentKind,
  MessageContentProps,
  MetaPlacement,
  RegisterOptions,
} from './content/contentTypes'
export { MessageText } from './content/MessageText'
