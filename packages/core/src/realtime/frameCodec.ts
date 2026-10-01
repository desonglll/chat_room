/**
 * WS frame codec over the TG-007 frozen protocol (`types/realtime.ts`).
 *
 * Decoding implements the two tolerance rules the frozen interface REQUIRES of clients:
 * - an unknown frame `type` (or unparseable text) yields `null` — callers ignore it and
 *   keep the connection up, mirroring the server's own inbound rule;
 * - a `typing` frame with a missing or unknown `action` degrades to `'typing'`, so a newer
 *   server never gets its typing frames dropped by this client.
 */
import type { ClientFrame, ServerFrame, TypingAction } from '../types'
import { TYPING_ACTIONS } from '../types'

const KNOWN_SERVER_TYPES: ReadonlySet<string> = new Set([
  'auth_ok',
  'auth_fail',
  'history_complete',
  'broadcast',
  'read_receipt',
  'message_recalled',
  'messages_deleted',
  'message_edited',
  'reaction_changed',
  'typing',
  'presence',
  'system',
  'user_status',
  'chat_updated',
  'member_updated',
  'topic_updated',
  'message_views_updated',
  'poll_updated',
  'link_preview_updated',
  'location_updated',
  'draft_updated',
  'voice_listened',
])

/** Unknown wire strings degrade to `'typing'` (TG-007 §1 — mandatory client behaviour). */
export function normalizeTypingAction(value: unknown): TypingAction {
  return TYPING_ACTIONS.includes(value as TypingAction) ? (value as TypingAction) : 'typing'
}

export function parseServerFrame(raw: string): ServerFrame | null {
  let parsed: unknown
  try {
    parsed = JSON.parse(raw)
  } catch {
    return null
  }
  if (typeof parsed !== 'object' || parsed === null) return null
  const frame = parsed as Record<string, unknown>
  if (typeof frame.type !== 'string' || !KNOWN_SERVER_TYPES.has(frame.type)) return null
  if (frame.type === 'typing') {
    return { ...frame, action: normalizeTypingAction(frame.action) } as ServerFrame
  }
  return frame as unknown as ServerFrame
}

export function encodeClientFrame(frame: ClientFrame): string {
  return JSON.stringify(frame)
}
