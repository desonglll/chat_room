/**
 * Actions a rendered message may ask of the list it sits in (reply-quote click → jump).
 * A React context rather than extra `renderMessage` arguments, so the frozen
 * `MessageRenderContext` stays exactly the six layout/state fields.
 */
import { createContext, useContext } from 'react'

export interface MessageListActions {
  /** Scroll to (loading if needed) and flash a message; the current position becomes the return point. */
  jumpToMessage(messageId: string): void
}

const NOOP_ACTIONS: MessageListActions = { jumpToMessage: () => {} }

export const MessageListActionsContext = createContext<MessageListActions>(NOOP_ACTIONS)

export function useMessageListActions(): MessageListActions {
  return useContext(MessageListActionsContext)
}
