// TG-1203: a channel's admin panel was unreachable (the entry hid itself for channels), and the
// info panel asked every member for the audit log, which the server answers 403.
import { expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import type { ChatAdminApi, ChatPermissionsView } from '@tg/core'
import { canReadAuditLog } from '../chatInfo/chatInfoModel'
import { ChatAdminEntry, ChatAdminPanel } from './index'

const api = {} as ChatAdminApi
const view = (
  my: string[],
  chatType: ChatPermissionsView['chat_type'],
  role: ChatPermissionsView['my_role'],
): ChatPermissionsView => ({
  chat_id: 'c1',
  chat_type: chatType,
  member_count: 12,
  default_permissions: [],
  my_permissions: my,
  my_role: role,
  registry: [],
})
const audit = (v: ChatPermissionsView) => (canReadAuditLog(v) ? <span>AUDIT</span> : null)

test('a channel owner or administrator gets «管理频道»; a subscriber gets nothing', () => {
  const owner = renderToStaticMarkup(
    <ChatAdminEntry chatId="c1" api={api} initialView={view(['chat.info'], 'channel', 'owner')} />,
  )
  expect(owner).toContain('管理频道')
  expect(owner).toContain('频道 · 12 位订阅者')
  const admin = renderToStaticMarkup(
    <ChatAdminEntry chatId="c1" api={api} initialView={view(['message.post'], 'channel', 'admin')} />,
  )
  expect(admin).toContain('管理频道')
  expect(
    renderToStaticMarkup(<ChatAdminEntry chatId="c1" api={api} initialView={view([], 'channel', 'member')} />),
  ).toBe('')
})

test('the channel panel speaks of subscribers and drops member rules', () => {
  const html = renderToStaticMarkup(
    <ChatAdminPanel
      chatId="c1"
      api={api}
      onClose={() => {}}
      initial={{
        view: view(['chat.info', 'members.ban'], 'channel', 'owner'),
        admins: [],
        restricted: [],
        members: [],
        loading: false,
      }}
    />,
  )
  expect(html).toContain('管理频道')
  expect(html).toContain('订阅者')
  expect(html).toContain('已封禁的用户')
  expect(html).not.toContain('成员权限')
  expect(html).not.toContain('被限制的成员')
})

test('only holders of members.review mount the audit log', () => {
  const member = renderToStaticMarkup(
    <ChatAdminEntry
      chatId="c1"
      api={api}
      initialView={view(['message.send'], 'supergroup', 'member')}
      managerExtra={audit}
    />,
  )
  expect(member).toContain('群组权限')
  expect(member).not.toContain('AUDIT')
  const reviewer = renderToStaticMarkup(
    <ChatAdminEntry
      chatId="c1"
      api={api}
      initialView={view(['members.review'], 'supergroup', 'admin')}
      managerExtra={audit}
    />,
  )
  expect(reviewer).toContain('AUDIT')
})
