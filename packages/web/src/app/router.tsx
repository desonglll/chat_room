/**
 * The routing skeleton (frozen interface §1): `/login`, `/` (shell, empty state) and
 * `/chat/:chatId` (shell with the chat open — route record name `chat`, matching
 * `domain/messageDeepLink`'s default). The server SPA-fallbacks every unmatched path to
 * `index.html`, so these history-mode URLs survive refresh.
 */
import type { ReactNode } from 'react'
import { authStore } from '@tg/core'
import { useStore } from 'zustand/react'
import { createBrowserRouter, Navigate } from 'react-router-dom'
import { LoginPage } from '../features/auth/LoginPage'
import { ForumChatRoute, ForumTopicRoute } from '../features/forum/ForumRoutes'
import { EmptyChatState } from '../features/shell/EmptyChatState'
import { WorkspaceShell } from '../features/shell/WorkspaceShell'
import { StickerSetLinkRoute } from '../features/sticker/StickerSetLinkRoute'
import { JoinChatRoute } from '../features/inviteLinks/JoinChatRoute'

function RequireSession({ children }: { children: ReactNode }) {
  const authenticated = useStore(authStore, (state) => state.session !== null)
  return authenticated ? children : <Navigate to="/login" replace />
}

function AnonymousOnly({ children }: { children: ReactNode }) {
  const authenticated = useStore(authStore, (state) => state.session !== null)
  return authenticated ? <Navigate to="/" replace /> : children
}

export const appRouter = createBrowserRouter([
  {
    path: '/login',
    element: (
      <AnonymousOnly>
        <LoginPage />
      </AnonymousOnly>
    ),
  },
  {
    element: (
      <RequireSession>
        <WorkspaceShell />
      </RequireSession>
    ),
    children: [
      { path: '/', element: <EmptyChatState /> },
      { path: '/chat/:chatId', element: <ForumChatRoute /> },
      { path: '/chat/:chatId/topic/:topicId', element: <ForumTopicRoute /> },
      { path: '/addstickers/:shortName', element: <StickerSetLinkRoute /> },
      { path: '/joinchat/:token', element: <JoinChatRoute /> },
    ],
  },
  { path: '*', element: <Navigate to="/" replace /> },
])
