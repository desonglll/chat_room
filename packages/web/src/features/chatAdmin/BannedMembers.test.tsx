// TG-1203: the admin panel's ban entry points, the channel signature switch, and the owner-row
// layout fix — each found missing or broken in the M12 walkthrough.
import { expect, test } from 'bun:test'
import { readFileSync } from 'node:fs'
import { renderToStaticMarkup } from 'react-dom/server'
import type { ChatAdminApi, ChatPermissionsView } from '@tg/core'
import { canToggleSignatures } from '../channel/ChannelSignaturesToggle'
import { BanMemberButton } from './BannedMembers'
import { ChatAdminPanel } from './index'

const view = (my: string[], chatType: ChatPermissionsView['chat_type']): ChatPermissionsView => ({
  chat_id: 'c1',
  chat_type: chatType,
  member_count: 3,
  default_permissions: ['message.send'],
  my_permissions: my,
  my_role: 'admin',
  registry: [],
})
const seeded = (v: ChatPermissionsView) => ({ view: v, admins: [], restricted: [], members: [], loading: false })
const panel = (v: ChatPermissionsView) =>
  renderToStaticMarkup(<ChatAdminPanel chatId="c1" api={{} as ChatAdminApi} onClose={() => {}} initial={seeded(v)} />)

test('a holder of members.ban reaches the removed-users list; others do not', () => {
  expect(panel(view(['members.ban'], 'supergroup'))).toContain('已封禁的用户')
  expect(panel(view(['members.promote'], 'supergroup'))).not.toContain('已封禁的用户')
})

test('the member page offers «封禁并移出群组»', () => {
  const html = renderToStaticMarkup(<BanMemberButton chatId="c1" userId="u1" onBanned={() => {}} />)
  expect(html).toContain('封禁并移出群组')
})

test('a channel admin with chat.info gets the «消息署名» switch after creation', () => {
  expect(panel(view(['chat.info'], 'channel'))).toContain('消息署名')
  expect(panel(view(['chat.info'], 'supergroup'))).not.toContain('消息署名')
  expect(canToggleSignatures('channel', ['message.post'])).toBe(false)
  expect(canToggleSignatures('channel', ['chat.info'])).toBe(true)
})

test('roster rows are border-box, so the static owner row does not clip its badge', () => {
  const css = readFileSync(new URL('./chatAdmin.css', import.meta.url), 'utf8')
  const rule = css.slice(
    css.indexOf('.tg-chatadmin__member-button,'),
    css.indexOf('}', css.indexOf('.tg-chatadmin__member-button,')),
  )
  expect(rule).toContain('inline-size: 100%')
  expect(rule).toContain('box-sizing: border-box')
})
