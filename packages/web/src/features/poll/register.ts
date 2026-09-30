/**
 * Plugs the poll body into every bubble through TG-103's content registry. Imported once for
 * its side effect (`src/main.tsx`). A message is a poll exactly when it carries `poll` — this
 * does not depend on TG-302's `media_kind` (see `docs/devlog/TG-406.md`).
 *
 * Priority 50: above text (0), file (10) and media (20), below the recalled placeholder (100),
 * so a recalled poll still shows «消息已撤回».
 */
import type { BroadcastMessage } from '@tg/core'
import { registerMessageContent } from '../message'
import { PollContent } from './PollContent'

export const POLL_CONTENT_PRIORITY = 50

export const isPollMessage = (message: BroadcastMessage): boolean => message.poll !== undefined && message.poll !== null

export const unregisterPollContent = registerMessageContent('poll', PollContent, {
  match: isPollMessage,
  priority: POLL_CONTENT_PRIORITY,
})
