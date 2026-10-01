/**
 * TG-903: the Telegram contacts list — friends with their presence line, online first, then by
 * most recent activity (exact times before the privacy buckets), filtered in place. Pure.
 */
import type { SocialUser, UserStatus, UserStatusEntry } from '@tg/core'
import { contactName } from '@tg/core'

export interface ContactRow {
  user: SocialUser
  name: string
  status: UserStatus | undefined
}

const BUCKET_RANK: Record<UserStatus['kind'], number> = {
  online: 0,
  offline: 1,
  recently: 2,
  within_week: 3,
  within_month: 4,
  long_ago: 5,
  empty: 6,
}

function seenAt(status: UserStatus | undefined): number {
  return status?.kind === 'offline' ? Date.parse(status.last_seen) || 0 : 0
}

export function compareContacts(a: ContactRow, b: ContactRow): number {
  const rank = BUCKET_RANK[a.status?.kind ?? 'empty'] - BUCKET_RANK[b.status?.kind ?? 'empty']
  if (rank !== 0) return rank
  const seen = seenAt(b.status) - seenAt(a.status)
  if (seen !== 0) return seen
  return a.name.localeCompare(b.name)
}

/** Lower-cased match on the shown name, the remark, the display name and the @username. */
export function matchesContact(row: ContactRow, query: string): boolean {
  const needle = query.trim().replace(/^@/, '').toLowerCase()
  if (!needle) return true
  return [row.name, row.user.remark, row.user.display_name, row.user.username].some((value) =>
    value.toLowerCase().includes(needle),
  )
}

export function contactRows(
  friends: readonly SocialUser[],
  statuses: readonly UserStatusEntry[],
  query: string,
): ContactRow[] {
  const byId = new Map(statuses.map((entry) => [entry.user_id, entry.status]))
  return friends
    .map((user) => ({ user, name: contactName(user), status: byId.get(user.id) }))
    .filter((row) => matchesContact(row, query))
    .sort(compareContacts)
}
