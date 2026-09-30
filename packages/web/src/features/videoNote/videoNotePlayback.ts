/**
 * Which round video plays with sound right now (TG-402). Telegram plays one at a time: tapping
 * a video note makes it the active one (from the start, with sound, enlarged) and stops the
 * previous one; every other video note stays a muted, looping preview. A voice message and a
 * sounding video note exclude each other the same way.
 *
 * Watched reporting mirrors TG-401's listened reporting: the dot clears at once (the shared
 * `voiceStore`, which `voice_listened` frames also feed) and the server is told once.
 */
import { createStore } from 'zustand/vanilla'
import type { ApiClient } from '@tg/core'
import { voiceStore, type VoiceStore } from '../voice/voiceStore'
import { markVideoNoteListened } from './videoNoteApi'

export interface VideoNotePlaybackState {
  active: string | null
  activate(messageId: string): void
  /** Only clears when `messageId` is still the active one. */
  deactivate(messageId: string): void
}

export const createVideoNotePlayback = () =>
  createStore<VideoNotePlaybackState>()((set) => ({
    active: null,
    activate: (messageId) => set({ active: messageId }),
    deactivate: (messageId) => set((state) => (state.active === messageId ? { active: null } : state)),
  }))

export type VideoNotePlayback = ReturnType<typeof createVideoNotePlayback>

export const videoNotePlayback = createVideoNotePlayback()

export interface ReportWatchedDeps {
  client: ApiClient
  token: string
  store?: VoiceStore
}

const reported = new Set<string>()

/** Someone else's video note started with sound: clear its dot, tell the server once. */
export function reportVideoNoteWatched(messageId: string, deps: ReportWatchedDeps): void {
  const store = deps.store ?? voiceStore
  store.getState().markListened(messageId)
  if (reported.has(messageId)) return
  reported.add(messageId)
  markVideoNoteListened(deps.client, deps.token, messageId).catch(() => reported.delete(messageId))
}
