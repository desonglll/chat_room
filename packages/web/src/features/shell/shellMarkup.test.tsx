/** The shell and the pane render (initial store state) with the TG-102 structure. */
import { expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { ChatListPane } from '../chatList/ChatListPane'
import { WorkspaceShell } from './WorkspaceShell'

test('ChatListPane: hamburger, search, named list', () => {
  const html = renderToStaticMarkup(
    <MemoryRouter>
      <ChatListPane />
    </MemoryRouter>,
  )
  expect(html).toContain('aria-label="会话列表"')
  expect(html).toContain('aria-label="主菜单"')
  expect(html).toContain('aria-label="搜索会话"')
})

test('ChatListPane collapsed: no search field', () => {
  const html = renderToStaticMarkup(
    <MemoryRouter>
      <ChatListPane collapsed />
    </MemoryRouter>,
  )
  expect(html).toContain('data-collapsed')
  expect(html).not.toContain('aria-label="搜索会话"')
})

test('WorkspaceShell: sidebar width variable, resizer, mobile view follows the route', () => {
  const at = (path: string) =>
    renderToStaticMarkup(
      <MemoryRouter initialEntries={[path]}>
        <Routes>
          <Route element={<WorkspaceShell />}>
            <Route path="/" element={<p>empty</p>} />
            <Route path="/chat/:chatId" element={<p>chat</p>} />
          </Route>
        </Routes>
      </MemoryRouter>,
    )
  const list = at('/')
  expect(list).toContain('--tg-shell-sidebar-width:360px')
  expect(list).toContain('role="separator"')
  expect(list).toContain('data-mobile-view="list"')
  const chat = at('/chat/c1')
  expect(chat).toContain('data-mobile-view="chat"')
  // TG-100: the back button moved into the chat header (ChatHeader markup test).
  expect(chat).not.toContain('tg-mobile-back')
})

test('the chat header info button toggles the info panel (TG-106 → TG-110)', async () => {
  const { uiStore } = await import('@tg/core')
  const { toggleChatInfo } = await import('../chat/ChatHeader')
  uiStore.getState().closePanel()
  toggleChatInfo()
  expect(uiStore.getState().activePanel).toBe('chatInfo')
  toggleChatInfo()
  expect(uiStore.getState().activePanel).toBe('none')
})
