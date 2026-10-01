/**
 * Pure poll presentation logic: merging snapshots from the three sources (the message's own
 * `poll`, `poll_updated` frames, the viewer's vote responses) and deriving what a bubble shows.
 * Framework-free so `bun test` covers every rule without a DOM.
 */
import type { PollState } from '@tg/core'
import { t } from '../../i18n/index'

/**
 * Merge an incoming snapshot over the held one.
 *
 * - An older `revision` never replaces a newer one (frames and responses race).
 * - Viewer-only fields (`chosen`, and a quiz's `correct_option`/`explanation` revealed by
 *   answering) are absent from chat-wide frames; absence means "unknown", so the held value
 *   survives. A viewer's own response (which always carries `chosen`) replaces them.
 */
export function mergePollState(held: PollState | undefined, incoming: PollState): PollState {
  if (!held || held.id !== incoming.id) return incoming
  if ((incoming.revision ?? 0) < (held.revision ?? 0)) return held
  const merged: PollState = { ...incoming }
  if (incoming.chosen === undefined && held.chosen !== undefined) merged.chosen = held.chosen
  if (incoming.correct_option === undefined && held.correct_option !== undefined) {
    merged.correct_option = held.correct_option
  }
  if (incoming.explanation === undefined && held.explanation !== undefined) merged.explanation = held.explanation
  return merged
}

/**
 * Whole-number percentages per option. Single choice uses the largest-remainder method so
 * the column sums to exactly 100 (Telegram does the same); multiple choice cannot sum to 100
 * and is rounded per option against the number of voters.
 */
export function pollPercentages(poll: PollState): number[] {
  const total = poll.total_voters
  if (total <= 0) return poll.options.map(() => 0)
  const exact = poll.options.map((option) => (option.voters * 100) / total)
  if (poll.multiple_choice) return exact.map((value) => Math.round(value))
  const floors = exact.map((value) => Math.floor(value))
  let remaining = 100 - floors.reduce((sum, value) => sum + value, 0)
  const byRemainder = exact
    .map((value, index) => ({ index, remainder: value - Math.floor(value) }))
    .filter((entry) => entry.remainder > 0)
    .sort((a, b) => b.remainder - a.remainder || a.index - b.index)
  for (const { index } of byRemainder) {
    if (remaining <= 0) break
    floors[index] = (floors[index] ?? 0) + 1
    remaining -= 1
  }
  return floors
}

export type OptionMark = 'none' | 'chosen' | 'correct' | 'wrong'

export interface PollOptionView {
  index: number
  text: string
  voters: number
  percent: number
  /** Bar length, 0..1: this option's share of all voters. */
  share: number
  mark: OptionMark
}

export interface PollView {
  /** Results replace the ballot once the viewer voted or the poll closed. */
  showResults: boolean
  hasVoted: boolean
  kindLabel: string
  footer: string
  options: PollOptionView[]
  /** The explanation, once revealed. */
  explanation: string | null
  canRetract: boolean
}

export function pollKindLabel(poll: PollState): string {
  if (poll.closed) return poll.quiz ? t('w.poll.18fa11') : t('w.poll.4852a8')
  const privacy = poll.public_voters ? t('w.poll.dfe5a3') : t('w.poll.9117f2')
  if (poll.quiz) return t('w.poll.298af6', privacy)
  return poll.multiple_choice ? t('w.poll.afa895', privacy) : t('w.poll.f815ca', privacy)
}

export function votersLabel(count: number, quiz = false): string {
  if (count === 0) return quiz ? t('w.poll.15e23e') : t('w.poll.aee9a5')
  return quiz ? t('w.poll.64304c', count) : t('w.poll.4d38b6', count)
}

export function describePoll(poll: PollState): PollView {
  const chosen = poll.chosen ?? []
  const hasVoted = chosen.length > 0
  const showResults = hasVoted || poll.closed
  const percents = pollPercentages(poll)
  const total = Math.max(1, poll.total_voters)
  const options = poll.options.map((option, index): PollOptionView => {
    const picked = chosen.includes(index)
    let mark: OptionMark = picked ? 'chosen' : 'none'
    if (poll.quiz && poll.correct_option !== undefined && showResults) {
      if (index === poll.correct_option) mark = 'correct'
      else if (picked) mark = 'wrong'
    }
    return {
      index,
      text: option.text,
      voters: option.voters,
      percent: percents[index] ?? 0,
      share: Math.min(1, option.voters / total),
      mark,
    }
  })
  return {
    showResults,
    hasVoted,
    kindLabel: pollKindLabel(poll),
    footer: votersLabel(poll.total_voters, poll.quiz),
    options,
    explanation: showResults && poll.explanation ? poll.explanation : null,
    canRetract: hasVoted && !poll.closed && !poll.quiz,
  }
}
