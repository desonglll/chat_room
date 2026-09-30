/**
 * TG-107 public surface for the chat header and chat-list owners. The outbound half
 * (`createChatActionSender`) lives in `@tg/core` for the composer to call.
 */
export { ChatHeaderStatus, ChatTypingLine, PresenceStatusLine } from './ChatHeaderStatus'
export { TypingIndicator } from './TypingIndicator'
export { useHeaderStatus, useLastSeenText, useTypingStatus, useTypingSummary } from './usePresenceText'
export type { TypingStatus } from './usePresenceText'
export type { HeaderStatus } from './presenceText'
export { formatLastSeen } from '@tg/core'
