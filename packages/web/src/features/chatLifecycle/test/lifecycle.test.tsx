import { describe, expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { MemoryRouter } from 'react-router-dom'
import type { Chat } from '@tg/core'
import { ChatActions } from '../ChatActions'
import { canRemove, lifecycleActions, profileError } from '../lifecycleModel'

const chat = (extra: Partial<Chat>): Chat =>
  ({ id: 'c1', chat_type: 'group', title: 'Team', description: '', avatar_emoji: '', ...extra }) as Chat

describe('TG-701 chat lifecycle', () => {
  test('owners delete, admins edit and invite, members leave; private chats show nothing', () => {
    expect(lifecycleActions('owner', false)).toEqual({
      edit: true,
      invite: true,
      leave: false,
      delete: true,
      removeMembers: true,
    })
    expect(lifecycleActions('admin', false)).toEqual({
      edit: true,
      invite: true,
      leave: true,
      delete: false,
      removeMembers: true,
    })
    expect(lifecycleActions('member', false)).toEqual({
      edit: false,
      invite: false,
      leave: true,
      delete: false,
      removeMembers: false,
    })
    expect(lifecycleActions('owner', true).delete).toBe(false)
    expect(lifecycleActions(undefined, false).leave).toBe(false)
  })

  test('removal never targets the owner or yourself; admins remove only members', () => {
    expect(canRemove('owner', 'admin', false)).toBe(true)
    expect(canRemove('admin', 'member', false)).toBe(true)
    expect(canRemove('admin', 'admin', false)).toBe(false)
    expect(canRemove('owner', 'owner', false)).toBe(false)
    expect(canRemove('owner', 'member', true)).toBe(false)
    expect(canRemove('member', 'member', false)).toBe(false)
  })

  test('profile validation', () => {
    expect(profileError('', '')).toBe('w.lifecycle.titleRequired')
    expect(profileError('x'.repeat(65), '')).toBe('w.lifecycle.titleTooLong')
    expect(profileError('ok', 'y'.repeat(256))).toBe('w.lifecycle.descriptionTooLong')
    expect(profileError('ok', '')).toBe('')
  })

  test('the panel section lists the actions the role allows', () => {
    const render = (extra: Partial<Chat>) =>
      renderToStaticMarkup(
        <MemoryRouter>
          <ChatActions chat={chat(extra)} />
        </MemoryRouter>,
      )
    const owner = render({ membership_role: 'owner' })
    expect(owner).toContain('删除群组')
    expect(owner).not.toContain('退出群组')
    expect(render({ membership_role: 'member', chat_type: 'channel' })).toContain('退订并离开频道')
    expect(render({ membership_role: 'member', chat_type: 'private' })).toBe('')
  })
})
