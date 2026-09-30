/**
 * Live poll snapshots, keyed by poll message id. The message timeline holds the snapshot the
 * message arrived with; this store holds everything newer (frames, the viewer's own votes),
 * merged by `mergePollState`. A bubble shows `merge(message.poll, store[id])`.
 *
 * Feature-local on purpose (one domain, one store file): the chat session feeds it through
 * `applyPollFrame`, the only entry point a `poll_updated` frame needs.
 */
import { createStore } from 'zustand/vanilla'
import type { PollState, ServerFrame } from '@tg/core'
import { mergePollState } from './pollView'

export interface PollStoreState {
  polls: Record<string, PollState>
  remember(poll: PollState): void
  clear(): void
}

export const createPollStore = () =>
  createStore<PollStoreState>()((set) => ({
    polls: {},
    remember: (poll) =>
      set((state) => {
        const merged = mergePollState(state.polls[poll.id], poll)
        return merged === state.polls[poll.id] ? state : { polls: { ...state.polls, [poll.id]: merged } }
      }),
    clear: () => set({ polls: {} }),
  }))

export type PollStore = ReturnType<typeof createPollStore>

export const pollStore = createPollStore()

/** Feed one `poll_updated` frame. Wire it in the chat session's frame fan-out. */
export function applyPollFrame(frame: Extract<ServerFrame, { type: 'poll_updated' }>, store: PollStore = pollStore) {
  store.getState().remember({ ...frame.poll, id: frame.message_id })
}

/** What a bubble should show: the message's snapshot merged with anything newer. */
export function effectivePoll(fromMessage: PollState, held: PollState | undefined): PollState {
  return held ? mergePollState(fromMessage, held) : fromMessage
}
