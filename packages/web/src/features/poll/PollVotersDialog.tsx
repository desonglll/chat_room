/**
 * Who voted for what, public polls only (the server refuses anonymous polls with 403, and
 * this dialog is never offered for them). Lazy-loaded: most viewers never open it.
 */
import { useEffect, useState } from 'react'
import { useStore } from 'zustand/react'
import type { PollState, PollVoterPage } from '@tg/core'
import { authStore, listPollVoters, selectToken } from '@tg/core'
import { Avatar, Modal, Spinner } from '@tg/ui'
import { apiClient } from '../../app/client'
import { pollErrorText } from './usePoll'

export interface PollVotersDialogProps {
  poll: PollState
  open: boolean
  onClose(): void
}

type Pages = Record<number, PollVoterPage>

export default function PollVotersDialog({ poll, open, onClose }: PollVotersDialogProps) {
  const token = useStore(authStore, selectToken)
  const [pages, setPages] = useState<Pages | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (!open) return
    let live = true
    const indexes = poll.options.map((_, index) => index).filter((index) => (poll.options[index]?.voters ?? 0) > 0)
    Promise.all(indexes.map((index) => listPollVoters(apiClient, token, poll.id, index, { limit: 100 })))
      .then((loaded) => {
        if (live) setPages(Object.fromEntries(loaded.map((page) => [page.option, page])))
      })
      .catch((caught: unknown) => {
        if (live) setError(pollErrorText(caught))
      })
    return () => {
      live = false
    }
  }, [open, poll.id, poll.options, token])

  return (
    <Modal open={open} onClose={() => onClose()} title="投票结果" description={poll.question} size="sm">
      {error ? <p className="tg-poll-voters__empty">{error}</p> : null}
      {!error && pages === null ? <Spinner /> : null}
      {pages
        ? poll.options.map((option, index) => {
            const page = pages[index]
            if (!page) return null
            return (
              <section key={index} className="tg-poll-voters__option">
                <h3 className="tg-poll-voters__title">
                  {option.text}
                  <span className="tg-poll-voters__total">{page.total} 票</span>
                </h3>
                <ul className="tg-poll-voters__list">
                  {page.voters.map((voter) => (
                    <li key={voter.user_id} className="tg-poll-voters__voter">
                      <Avatar label={voter.display_name || voter.username} size="sm" />
                      <span>{voter.display_name || voter.username}</span>
                    </li>
                  ))}
                </ul>
              </section>
            )
          })
        : null}
    </Modal>
  )
}
