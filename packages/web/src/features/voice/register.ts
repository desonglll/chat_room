/**
 * Plugs the voice bubble body into every bubble through TG-103's content registry. Imported
 * once for its side effect (`src/main.tsx`). A message is a voice message exactly when it
 * carries `voice` (the server sets it only with `media_kind: "voice"` and a visible
 * attachment).
 *
 * Priority 40: above text (0), file (10) and media (20) — an `audio/*` attachment would
 * otherwise draw as a file card — below polls (50) and the recalled placeholder (100).
 */
import type { BroadcastMessage } from '@tg/core'
import { registerMessageContent } from '../message'
import { VoiceContent } from './VoiceContent'

export const VOICE_CONTENT_PRIORITY = 40

export const isVoiceContent = (message: BroadcastMessage): boolean =>
  message.voice != null && message.attachment !== null

export const unregisterVoiceContent = registerMessageContent('voice', VoiceContent, {
  match: isVoiceContent,
  priority: VOICE_CONTENT_PRIORITY,
})
