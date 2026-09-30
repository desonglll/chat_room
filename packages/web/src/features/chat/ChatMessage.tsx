/**
 * The chat's `renderMessage` (TG-101 seam → TG-103 bubble, TG-100 wiring): every row is a
 * `MessageBubble` with its actions bound to this chat. A component (not a bare function)
 * so it can read the list's jump action from context.
 */
import { memo, useMemo } from 'react'
import type { DisplayMessage } from '@tg/core'
import { MessageBubble } from '../message'
import type { MessageRenderContext, RenderMessage } from '../messageList/renderContract'
import { useMessageListActions } from '../messageList/messageListActions'
import type { MessageActionDeps } from './messageActions'
import { bindMessageActions, deliveryFor } from './messageActions'

export interface ChatRenderOptions {
  deps: Omit<MessageActionDeps, 'jumpTo'>
  peerReadAt: string
  selectionMode: boolean
  /** Group chats keep the avatar column on every incoming row, so a run stays aligned. */
  groupIdentity: boolean
}

interface ChatMessageProps {
  message: DisplayMessage
  ctx: MessageRenderContext
  options: ChatRenderOptions
}

function ChatMessageImpl({ message, ctx, options }: ChatMessageProps) {
  const { jumpToMessage } = useMessageListActions()
  const actions = useMemo(
    () => bindMessageActions(message, { ...options.deps, jumpTo: jumpToMessage }),
    [message, options.deps, jumpToMessage],
  )
  return (
    <MessageBubble
      message={message}
      ctx={ctx}
      actions={actions}
      viewerId={options.deps.viewerId}
      delivery={deliveryFor(message, options.deps.viewerId, options.peerReadAt)}
      selectionMode={options.selectionMode}
      reserveAvatar={options.groupIdentity && !ctx.isOutgoing}
    />
  )
}

const ChatMessage = memo(ChatMessageImpl)

export function createChatRenderer(options: ChatRenderOptions): RenderMessage {
  return (message, ctx) => <ChatMessage message={message} ctx={ctx} options={options} />
}
