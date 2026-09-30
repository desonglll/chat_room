// TG-205 markup: the invite-links page, a link's detail, the editor and the landing card,
// rendered from seeded state.
import { expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import type { InviteLinkMember } from '@tg/core'
import { MemoryRouter } from 'react-router-dom'
import type { InviteLinksService } from './inviteLinksApi'
import { InviteLinkEditor, InviteLinksEntry, InviteLinksPanel, JoinChatCard } from './index'
import { sampleLink } from './inviteLinksFixtures'

const api = { viewerId: () => 'me' } as unknown as InviteLinksService
const origin = 'https://chat.example'
const view = {
  links: [
    sampleLink('p', { is_primary: true, usage_count: 12 }),
    sampleLink('a', { title: '微博渠道', usage_count: 3, usage_limit: 10, expires_at: '2026-10-08T00:00:00Z' }),
    sampleLink('q', { title: '审核', requires_approval: true, pending_count: 2 }),
    sampleLink('r', { title: '旧链接', state: 'revoked', revoked_at: '2026-10-01T09:00:00Z' }),
  ],
  can_review: true,
  can_manage_others: false,
}
const person = (id: string, status: string): InviteLinkMember => ({
  user_id: id,
  username: id,
  display_name: `名${id}`,
  avatar_emoji: '',
  status,
  requested_at: '2026-10-01T09:00:00Z',
  joined_at: status === 'active' ? '2026-10-01T09:30:00Z' : null,
})

test('home lists the primary link, the other links and the revoked ones', () => {
  const html = renderToStaticMarkup(
    <InviteLinksPanel chatId="c1" api={api} onClose={() => {}} initial={{ view, loading: false }} origin={origin} />,
  )
  for (const text of [
    '主邀请链接',
    'https://chat.example/joinchat/tok-p-',
    '复制链接',
    '替换链接',
    '12 人已加入',
    '创建新链接',
    '微博渠道',
    '3/10 人已加入',
    '2 个待审核',
    '已撤销的链接',
    '旧链接',
  ]) {
    expect(html).toContain(text)
  }
})

test('detail shows requests with approve / decline and the joined list', () => {
  const html = renderToStaticMarkup(
    <InviteLinksPanel
      chatId="c1"
      api={api}
      onClose={() => {}}
      initial={{ view, loading: false }}
      initialPage={{ id: 'detail', linkId: 'q' }}
      initialPeople={{ joined: [person('j', 'active')], pending: [person('w', 'pending')] }}
      origin={origin}
    />,
  )
  for (const text of ['加入申请 · 1', '名w', '通过', '拒绝', '通过此链接加入 · 1', '名j', '由 群主 创建']) {
    expect(html).toContain(text)
  }
  // The owner's link, viewed by an administrator without members.promote: no edit / revoke.
  expect(html).not.toContain('撤销链接')
})

test('a revoked link offers delete to someone who may change it', () => {
  const html = renderToStaticMarkup(
    <InviteLinksPanel
      chatId="c1"
      api={api}
      onClose={() => {}}
      initial={{ view: { ...view, can_manage_others: true }, loading: false }}
      initialPage={{ id: 'detail', linkId: 'r' }}
      initialPeople={{ joined: [], pending: [] }}
      origin={origin}
    />,
  )
  expect(html).toContain('删除链接')
  expect(html).not.toContain('复制链接')
})

test('editor offers expiry presets, approval and limit presets', () => {
  const html = renderToStaticMarkup(<InviteLinkEditor busy={false} onSave={() => {}} />)
  for (const text of [
    '链接名称',
    '1 小时',
    '1 天',
    '1 周',
    '永不过期',
    '需要管理员审核',
    '人数上限',
    '不限',
    '创建链接',
  ]) {
    expect(html).toContain(text)
  }
})

test('the entry renders only for a link manager', () => {
  expect(renderToStaticMarkup(<InviteLinksEntry chatId="c1" api={api} canManage={false} />)).toBe('')
  expect(renderToStaticMarkup(<InviteLinksEntry chatId="c1" api={api} canManage />)).toContain('邀请链接')
  // The overlay variant (inside ChatAdminPanel's sheet) renders no second dialog.
  const overlay = renderToStaticMarkup(<InviteLinksEntry chatId="c1" api={api} canManage variant="overlay" />)
  expect(overlay).toContain('邀请链接')
  expect(overlay).not.toContain('role="dialog"')
})

test('landing card: preview, approval, pending and refusal', () => {
  const preview = {
    title: 'Rust 学习群',
    description: '每周一次分享',
    avatar_emoji: '🦀',
    chat_type: 'supergroup' as const,
    member_count: 321,
    requires_approval: false,
    membership_status: null,
    chat_id: null,
  }
  const render = (state: Parameters<typeof JoinChatCard>[0]['state']) =>
    renderToStaticMarkup(
      <MemoryRouter>
        <JoinChatCard state={state} onJoin={() => {}} onDismiss={() => {}} />
      </MemoryRouter>,
    )
  const open = render({ kind: 'preview', preview, busy: false })
  for (const text of ['Rust 学习群', '超级群 · 321 位成员', '每周一次分享', '加入群组']) expect(open).toContain(text)
  expect(render({ kind: 'preview', preview: { ...preview, requires_approval: true }, busy: false })).toContain(
    '申请加入',
  )
  expect(
    render({
      kind: 'preview',
      preview: { ...preview, requires_approval: true, membership_status: 'pending' },
      busy: false,
    }),
  ).toContain('等待管理员审核')
  expect(
    render({ kind: 'preview', preview: { ...preview, membership_status: 'active', chat_id: 'c1' }, busy: false }),
  ).toContain('打开聊天')
  expect(render({ kind: 'refused', reason: 'limit_reached' })).toContain('名额已用完')
})
