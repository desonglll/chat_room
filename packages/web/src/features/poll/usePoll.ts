/**
 * React glue for one poll bubble: the effective snapshot plus the viewer's actions. Every
 * response is the poll *as the viewer sees it*, so it goes straight into the store.
 */
import { useCallback, useState } from 'react'
import { useStore } from 'zustand/react'
import type { ApiClient, PollState } from '@tg/core'
import { ApiError, authStore, closePoll, retractPollVote, selectToken, votePoll } from '@tg/core'
import { apiClient } from '../../app/client'
import { effectivePoll, pollStore, type PollStore } from './pollStore'

export interface PollController {
  poll: PollState
  viewerId: string
  busy: boolean
  error: string | null
  vote(options: number[]): void
  retract(): void
  close(): void
}

export interface PollDeps {
  client: ApiClient
  store: PollStore
}

const DEFAULT_DEPS: PollDeps = { client: apiClient, store: pollStore }

export function pollErrorText(error: unknown): string {
  if (error instanceof ApiError) {
    if (error.status === 409) return '投票已结束或答案已提交'
    if (error.status === 403) return '没有权限执行此操作'
    if (error.status === 404) return '投票已不可用'
  }
  return '操作失败，请重试'
}

export function usePoll(fromMessage: PollState, deps: PollDeps = DEFAULT_DEPS): PollController {
  const held = useStore(deps.store, (state) => state.polls[fromMessage.id])
  const token = useStore(authStore, selectToken)
  const viewerId = useStore(authStore, (state) => state.session?.user.id ?? '')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const poll = effectivePoll(fromMessage, held)

  const run = useCallback(
    (request: (client: ApiClient, token: string, id: string) => Promise<PollState>) => {
      setBusy(true)
      setError(null)
      request(deps.client, token, fromMessage.id)
        .then((next) => deps.store.getState().remember(next))
        .catch((caught: unknown) => setError(pollErrorText(caught)))
        .finally(() => setBusy(false))
    },
    [deps, token, fromMessage.id],
  )

  return {
    poll,
    viewerId,
    busy,
    error,
    vote: (options) => run((client, auth, id) => votePoll(client, auth, id, options)),
    retract: () => run(retractPollVote),
    close: () => run(closePoll),
  }
}
