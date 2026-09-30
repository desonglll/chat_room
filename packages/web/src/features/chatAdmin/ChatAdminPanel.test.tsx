// TG-201 markup: the admin panel's pages and the info-panel entry, rendered from seeded state.
import { expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import type { ChatAdminApi, ChatMemberEntry, ChatPermissionsView, PermissionDescriptor } from '@tg/core'
import { ADMIN_ASSIGNABLE_KEYS, MEMBER_TOGGLEABLE_KEYS } from '@tg/core'
import { AdminEditor, ChatAdminEntry, ChatAdminPanel, RestrictionEditor } from './index'

const registry: PermissionDescriptor[] = [...MEMBER_TOGGLEABLE_KEYS, ...ADMIN_ASSIGNABLE_KEYS].map((key) => ({
  key,
  scope: 'member',
  label: `L:${key}`,
}))

const view = (my: string[], chatType: ChatPermissionsView['chat_type'] = 'supergroup'): ChatPermissionsView => ({
  chat_id: 'c1',
  chat_type: chatType,
  member_count: 201,
  default_permissions: ['message.send', 'message.send_media'],
  my_permissions: my,
  my_role: 'owner',
  registry,
})

const entry = (id: string, role: ChatMemberEntry['role'], extra: Partial<ChatMemberEntry> = {}): ChatMemberEntry => ({
  user_id: id,
  username: id,
  display_name: `名${id}`,
  avatar_emoji: '',
  nickname: '',
  role,
  status: 'active',
  joined_at: null,
  custom_title: '',
  ...extra,
})

const api = {} as ChatAdminApi
const owner = view(['members.ban', 'members.promote', 'members.roles'])
const seeded = {
  view: owner,
  admins: [entry('o', 'owner'), entry('a', 'admin', { custom_title: '版主', admin_rights: ['members.ban'] })],
  restricted: [entry('r', 'member', { restrictions: [{ permission_key: 'message.send', until: null }] })],
  members: [entry('m', 'member')],
  loading: false,
}

test('home shows the chat type, the automatic-upgrade note and every section', () => {
  const html = renderToStaticMarkup(<ChatAdminPanel chatId="c1" api={api} onClose={() => {}} initial={seeded} />)
  for (const text of ['管理群组', '超级群', '201 位成员', '不可撤销', '成员权限', '管理员', '被限制的成员']) {
    expect(html).toContain(text)
  }
  expect(html).toContain('2/9')
})

test('the admin list shows titles; the restricted list shows the restriction', () => {
  const admins = renderToStaticMarkup(
    <ChatAdminPanel chatId="c1" api={api} onClose={() => {}} initial={seeded} initialPage="admins" />,
  )
  expect(admins).toContain('版主')
  expect(admins).toContain('所有者')
  const restricted = renderToStaticMarkup(
    <ChatAdminPanel chatId="c1" api={api} onClose={() => {}} initial={seeded} initialPage="restricted" />,
  )
  expect(restricted).toContain('受限 · 永久')
})

test('the defaults page lists the nine member toggles, checked as the group allows', () => {
  const html = renderToStaticMarkup(
    <ChatAdminPanel chatId="c1" api={api} onClose={() => {}} initial={seeded} initialPage="defaults" />,
  )
  for (const key of MEMBER_TOGGLEABLE_KEYS) expect(html).toContain(`L:${key}`)
  expect(html).toMatch(/value="message\.send"[^>]*checked|checked[^>]*value="message\.send"/)
  expect(html).not.toMatch(/value="message\.pin"[^>]*checked|checked[^>]*value="message\.pin"/)
})

test('an administrator sees the rights they cannot grant disabled, and a title field', () => {
  const html = renderToStaticMarkup(
    <AdminEditor
      target={entry('m', 'member')}
      actor={view(['members.ban', 'members.promote'])}
      busy={false}
      onSave={() => {}}
    />,
  )
  expect(html).toContain('自定义头衔')
  expect(html).toMatch(/value="members\.ban"[^>]*checked|checked[^>]*value="members\.ban"/)
  expect(html).toMatch(
    /disabled=""[^>]*value="room\.delete"|value="chat\.info"[^>]*disabled|disabled[^>]*value="chat\.info"/,
  )
})

test('the restriction editor offers only what the group allows, with durations', () => {
  const html = renderToStaticMarkup(
    <RestrictionEditor target={entry('m', 'member')} view={owner} busy={false} onSave={() => {}} />,
  )
  expect(html).toContain('L:message.send')
  expect(html).not.toContain('L:message.pin')
  expect(html).toContain('限制 <strong>名m</strong>')
})

test('the entry names the chat type and hides itself in private chats', () => {
  const entryHtml = renderToStaticMarkup(<ChatAdminEntry chatId="c1" api={api} initialView={owner} />)
  expect(entryHtml).toContain('管理群组')
  expect(entryHtml).toContain('超级群 · 201 位成员')
  const member = renderToStaticMarkup(<ChatAdminEntry chatId="c1" api={api} initialView={view([], 'group')} />)
  expect(member).toContain('群组权限')
  expect(renderToStaticMarkup(<ChatAdminEntry chatId="c1" api={api} initialView={view([], 'private')} />)).toBe('')
})
