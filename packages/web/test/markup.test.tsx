/**
 * Structure/ARIA assertions over server-rendered markup, the packages/ui pattern
 * (`bun test` has no DOM; behaviour lives in the pure controllers, tested separately —
 * the store-driven parts render their INITIAL state here because zustand's server
 * snapshot is `getInitialState`).
 */
import { expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { MemoryRouter } from 'react-router-dom'
import { ChatHeader } from '../src/features/chat/ChatHeader'
import { SelectionBar } from '../src/features/chat/SelectionBar'
import { MessageList } from '../src/features/messageList/MessageList'
import { LoginPage } from '../src/features/auth/LoginPage'
import { EmptyChatState } from '../src/features/shell/EmptyChatState'
import { resolveTheme } from '../src/app/theme'

test('LoginPage renders an accessible credential form in login mode', () => {
  const html = renderToStaticMarkup(<LoginPage />)
  expect(html).toContain('Echo Gate')
  expect(html).toContain('name="username"')
  // React 19's static renderer emits the camelCase spelling; browsers accept either.
  expect(html).toMatch(/autocomplete="username"/i)
  expect(html).toContain('type="password"')
  expect(html).toMatch(/autocomplete="current-password"/i)
  expect(html).toContain('type="submit"')
  expect(html).toContain('登录')
  expect(html).toContain('注册新账号')
  expect(html).not.toContain('role="alert"')
})

test('EmptyChatState is the quiet service pill', () => {
  const html = renderToStaticMarkup(<EmptyChatState />)
  expect(html).toContain('tg-empty-chat__pill')
  expect(html).toContain('选择一个会话开始聊天')
})

test('MessageList before history_complete announces loading, not emptiness', () => {
  const html = renderToStaticMarkup(<MessageList chatId="c1" currentUserId="u1" />)
  expect(html).toContain('正在载入消息')
  expect(html).not.toContain('还没有消息')
})

test('ChatHeader: inline mobile back button; connection copy replaces the presence line', () => {
  const at = (connection: 'connecting' | 'online') =>
    renderToStaticMarkup(
      <MemoryRouter>
        <ChatHeader chatId="c1" connection={connection} />
      </MemoryRouter>,
    )
  const connecting = at('connecting')
  expect(connecting).toContain('tg-mobile-back')
  expect(connecting).toContain('aria-label="返回会话列表"')
  expect(connecting).toContain('连接中…')
  const online = at('online')
  expect(online).not.toContain('连接中…')
  expect(online).toContain('tg-presence-status')
})

test('SelectionBar: count, forward, delete disabled unless every message is deletable', () => {
  const noop = () => {}
  const html = renderToStaticMarkup(
    <SelectionBar count={3} canDelete={false} onForward={noop} onDelete={noop} onCancel={noop} />,
  )
  expect(html).toContain('已选 3 条')
  expect(html).toContain('转发')
  expect(html).toMatch(/<button[^>]*disabled[^>]*>(?:(?!<\/button>).)*删除/)
  expect(html).toContain('aria-label="取消选择"')
})

test('resolveTheme: explicit wins, system follows the OS', () => {
  expect(resolveTheme('light', true)).toBe('day')
  expect(resolveTheme('dark', false)).toBe('night')
  expect(resolveTheme('system', true)).toBe('night')
  expect(resolveTheme('system', false)).toBe('day')
})
