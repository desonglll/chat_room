import type { FriendRequest, SocialUser, UserSummary } from './types'

export type ContactSection = 'friends' | 'requests' | 'blocked'
export type ContactEntryKind = 'friend' | 'incoming' | 'outgoing' | 'blocked'

export interface ContactEntry {
  key: string
  kind: ContactEntryKind
  user: UserSummary
  displayName: string
  subtitle: string
}

export function contactSection(value: unknown): ContactSection {
  return value === 'requests' || value === 'blocked' ? value : 'friends'
}

export function contactEntries(
  section: ContactSection,
  friends: SocialUser[],
  incoming: FriendRequest[],
  outgoing: FriendRequest[],
  blocked: SocialUser[],
): ContactEntry[] {
  if (section === 'friends') {
    return friends.map((user) => ({
      key: `friend:${user.id}`,
      kind: 'friend',
      user,
      displayName: user.remark || user.display_name || user.username,
      subtitle: user.signature,
    }))
  }
  if (section === 'blocked') {
    return blocked.map((user) => ({
      key: `blocked:${user.id}`,
      kind: 'blocked',
      user,
      displayName: user.display_name || user.username,
      subtitle: user.signature,
    }))
  }
  return [
    ...incoming.map((request) => ({
      key: `incoming:${request.user.id}`,
      kind: 'incoming' as const,
      user: request.user,
      displayName: request.user.display_name || request.user.username,
      subtitle: '希望添加你为好友',
    })),
    ...outgoing.map((request) => ({
      key: `outgoing:${request.user.id}`,
      kind: 'outgoing' as const,
      user: request.user,
      displayName: request.user.display_name || request.user.username,
      subtitle: '等待对方接受',
    })),
  ]
}

export function filterContactEntries(entries: ContactEntry[], query: string): ContactEntry[] {
  const needle = query.trim().toLowerCase()
  if (!needle) return entries
  return entries.filter((entry) =>
    `${entry.displayName} ${entry.user.display_name} ${entry.user.username} ${entry.subtitle}`
      .toLowerCase()
      .includes(needle),
  )
}
