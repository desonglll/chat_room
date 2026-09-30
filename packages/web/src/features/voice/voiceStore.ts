/**
 * Listened ("played") marks newer than the message snapshot, keyed by message id. A bubble
 * shows `message.voice.listened || listened[id]`. Fed by the viewer's own playback and by
 * `voice_listened` frames — which the server delivers only to the listener's and the
 * sender's own connections, so every frame that arrives here is about this viewer's dot.
 */
import { createStore } from 'zustand/vanilla'
import type { ApiClient, VoiceListenedFrame } from '@tg/core'
import { markVoiceListened } from '@tg/core'

export interface VoiceStoreState {
  listened: Record<string, true>
  markListened(messageId: string): void
  clear(): void
}

export const createVoiceStore = () =>
  createStore<VoiceStoreState>()((set) => ({
    listened: {},
    markListened: (messageId) =>
      set((state) => (state.listened[messageId] ? state : { listened: { ...state.listened, [messageId]: true } })),
    clear: () => set({ listened: {} }),
  }))

export type VoiceStore = ReturnType<typeof createVoiceStore>

export const voiceStore = createVoiceStore()

/** Feed one `voice_listened` frame. Wired in the chat session's frame fan-out. */
export function applyVoiceListenedFrame(frame: VoiceListenedFrame, store: VoiceStore = voiceStore): void {
  store.getState().markListened(frame.message_id)
}

export interface ReportListenDeps {
  client: ApiClient
  token: string
  store?: VoiceStore
}

const reported = new Set<string>()

/**
 * The viewer started playing someone else's voice message: clear the dot now and tell the
 * server once (it answers 204 and fans `voice_listened` out to the sender). A failed report
 * is retried on the next play.
 */
export function reportListened(messageId: string, deps: ReportListenDeps): void {
  const store = deps.store ?? voiceStore
  store.getState().markListened(messageId)
  if (reported.has(messageId)) return
  reported.add(messageId)
  markVoiceListened(deps.client, deps.token, messageId).catch(() => reported.delete(messageId))
}
