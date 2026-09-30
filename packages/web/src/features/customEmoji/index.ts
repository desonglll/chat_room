/**
 * TG-304 public surface. Contract in docs/devlog/TG-304.md ("Frozen interface"); the
 * mount lines for the chat list, chat header, member list and composer are in its
 * "Integration patch list". Importing `./register` (once, at app start) turns on inline
 * custom emoji in message bubbles.
 */
export { EmojiStatus } from './EmojiStatus'
export type { EmojiStatusProps } from './EmojiStatus'
export { InlineCustomEmoji } from './InlineCustomEmoji'
export type { InlineCustomEmojiProps } from './InlineCustomEmoji'
export { EntityText, EntityTextContent } from './EntityText'
export { CustomEmojiTab } from './CustomEmojiTab'
export { CustomEmojiGrid } from './CustomEmojiGrid'
export type { PickedCustomEmoji } from './CustomEmojiGrid'
export { EmojiStatusPicker } from './EmojiStatusPicker'
export { registerAnimatedEmojiRenderer } from './animatedRenderers'
export type { AnimatedEmojiRendererProps } from './animatedRenderers'
export { handleCustomEmojiCopy, customEmojiPlainText } from './copyText'
export { useEntityDraft, createEntityDraft } from './useEntityDraft'
export type { EntityDraft } from './useEntityDraft'
export { configureCustomEmoji, customEmojiServices } from './services'
