/** Test and screenshot fixtures for invite links. */
import type { InviteLink } from '@tg/core'

export const sampleLink = (id: string, extra: Partial<InviteLink> = {}): InviteLink => ({
  id,
  chat_id: 'c1',
  token: `tok-${id}-aaaaaaaaaaaaaaaaaaaa`,
  title: '',
  creator_id: 'owner',
  creator_name: '群主',
  expires_at: null,
  usage_limit: null,
  usage_count: 0,
  requires_approval: false,
  is_primary: false,
  revoked_at: null,
  created_at: '2026-10-01T08:00:00Z',
  pending_count: 0,
  state: 'active',
  ...extra,
})
