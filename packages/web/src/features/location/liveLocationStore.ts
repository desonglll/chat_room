/**
 * TG-407: newer points of live locations, keyed by message id. A bubble shows the store's
 * point when it has one, else the message's own. Fed by the chat session's `location_updated`.
 */
import { createStore } from 'zustand/vanilla'
import type { MessageLocation, ServerFrame } from '@tg/core'

export interface LiveLocationState {
  points: Record<string, MessageLocation>
}

export const liveLocationStore = createStore<LiveLocationState>()(() => ({ points: {} }))

export function applyLocationFrame(frame: Extract<ServerFrame, { type: 'location_updated' }>): void {
  liveLocationStore.setState((state) => ({ points: { ...state.points, [frame.message_id]: frame.location } }))
}

/** The newer of the message's snapshot and the held point (by `updated_at`). */
export function effectiveLocation(fromMessage: MessageLocation, held: MessageLocation | undefined): MessageLocation {
  if (!held) return fromMessage
  return Date.parse(held.updated_at) >= Date.parse(fromMessage.updated_at) ? held : fromMessage
}
