import { describe, expect, test } from 'bun:test'
import type { ChatMemberEntry, ChatPermissionsView } from '@tg/core'
import {
  adminCapabilities,
  adminOptions,
  chatTypeNote,
  defaultAdminSelection,
  deniedFrom,
  memberAllowed,
  memberOptions,
  roleBadge,
  toggleMemberPermission,
  untilFromPreset,
  untilText,
} from './chatAdminModel'
import { createMemberPageSource } from './memberPageSource'

const registry = [
  { key: 'message.send', scope: 'member' as const, label: '发送消息' },
  { key: 'message.send_media', scope: 'member' as const, label: '发送媒体' },
  { key: 'members.ban', scope: 'admin' as const, label: '封禁与限制成员' },
]

function view(my: string[], defaults = ['message.send', 'message.send_media']): ChatPermissionsView {
  return {
    chat_id: 'c1',
    chat_type: 'supergroup',
    member_count: 3,
    default_permissions: defaults,
    my_permissions: my,
    my_role: 'admin',
    registry,
  }
}

const bob: ChatMemberEntry = {
  user_id: 'u-bob',
  username: 'bob',
  display_name: 'Bob',
  avatar_emoji: '',
  nickname: '',
  role: 'member',
  status: 'active',
  joined_at: '2026-10-01T00:00:00Z',
  custom_title: '',
  restrictions: [{ permission_key: 'message.send_media', until: null }],
}

describe('chat admin model', () => {
  test('capabilities follow the effective permissions', () => {
    expect(adminCapabilities(null).any).toBe(false)
    expect(adminCapabilities(view(['members.ban']))).toEqual({
      ban: true,
      promote: false,
      manageAdmins: false,
      any: true,
    })
  })

  test('content kinds are greyed out and switched off together with sending', () => {
    const off = toggleMemberPermission(new Set(['message.send', 'message.send_media']), 'message.send', false)
    expect([...off]).toEqual([])
    const media = memberOptions(registry, off).find((option) => option.key === 'message.send_media')
    expect(media).toMatchObject({ checked: false, disabled: true, label: '发送媒体' })
  })

  test('a member editor starts from defaults minus restrictions and denies what was switched off', () => {
    const defaults = ['message.send', 'message.send_media', 'message.pin']
    const allowed = memberAllowed(defaults, bob)
    expect([...allowed].sort()).toEqual(['message.pin', 'message.send'])
    expect(deniedFrom(defaults, toggleMemberPermission(allowed, 'message.pin', false))).toEqual([
      'message.send_media',
      'message.pin',
    ])
  })

  test('an administrator can only grant rights they hold; the owner anything', () => {
    const admin = view(['members.ban'])
    const options = adminOptions(registry, new Set(), admin)
    expect(options.find((option) => option.key === 'members.ban')?.disabled).toBe(false)
    expect(options.find((option) => option.key === 'members.promote')?.disabled).toBe(true)
    expect(adminOptions(registry, new Set(), view(['members.roles'])).every((option) => !option.disabled)).toBe(true)
    expect([...defaultAdminSelection(admin)]).toEqual(['members.ban'])
    expect(defaultAdminSelection(view(['members.roles'])).has('members.promote')).toBe(false)
  })

  test('copy: chat type note, until text, badges', () => {
    expect(chatTypeNote('group')).toContain('自动升级为超级群')
    expect(chatTypeNote('supergroup')).toContain('不可撤销')
    expect(untilText(null)).toBe('永久')
    expect(untilFromPreset('forever', 0)).toBeNull()
    expect(untilFromPreset('hour', 0)).toBe('1970-01-01T01:00:00.000Z')
    expect(roleBadge({ ...bob, role: 'admin', custom_title: '版主' })).toBe('版主')
    expect(roleBadge({ ...bob, role: 'owner' })).toBe('所有者')
  })

  test('the paged member source forwards the keyset cursor', async () => {
    const cursors: Array<string | null | undefined> = []
    const source = createMemberPageSource(
      {
        memberPage: async (_chat: string, options?: { cursor?: string | null }) => {
          cursors.push(options?.cursor)
          return { items: [bob], next_cursor: options?.cursor ? null : 'k1' }
        },
      } as never,
      'c1',
    )
    const first = await source(null)
    expect(first.next).toBe('k1')
    expect(first.items[0]).toMatchObject({ user_id: 'u-bob', status: 'active', role: 'member' })
    expect((await source('k1')).next).toBeNull()
    expect(cursors).toEqual([null, 'k1'])
  })
})
