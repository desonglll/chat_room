/**
 * Structure/ARIA assertions over server-rendered markup, the packages/ui pattern
 * (`bun test` has no DOM; behaviour lives in the pure controllers, tested separately —
 * the store-driven parts render their INITIAL state here because zustand's server
 * snapshot is `getInitialState`).
 */
import { expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { Composer } from '../src/features/chat/Composer'
import { MessageList } from '../src/features/chat/MessageList'
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

test('Composer exposes a named textarea and a disabled send while empty', () => {
  const html = renderToStaticMarkup(<Composer chatId="c1" onSend={() => true} onDraftChange={() => {}} />)
  expect(html).toContain('aria-label="消息内容"')
  expect(html).toContain('placeholder="写消息…"')
  expect(html).toContain('aria-label="发送"')
  expect(html).toContain('disabled')
})

test('MessageList before history_complete announces loading, not emptiness', () => {
  const html = renderToStaticMarkup(<MessageList chatId="c1" currentUserId="u1" />)
  expect(html).toContain('正在载入消息')
  expect(html).not.toContain('还没有消息')
})

test('resolveTheme: explicit wins, system follows the OS', () => {
  expect(resolveTheme('light', true)).toBe('day')
  expect(resolveTheme('dark', false)).toBe('night')
  expect(resolveTheme('system', true)).toBe('night')
  expect(resolveTheme('system', false)).toBe('day')
})
