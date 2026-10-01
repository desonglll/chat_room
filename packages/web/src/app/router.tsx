/**
 * The routing skeleton (frozen interface §1): `/login`, `/` (shell, empty state) and
 * `/chat/:chatId` (shell with the chat open — route record name `chat`, matching
 * `domain/messageDeepLink`'s default). The server SPA-fallbacks every unmatched path to
 * `index.html`, so these history-mode URLs survive refresh.
 */
import { lazy, Suspense } from 'react'
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
import { PublicChatRoute } from '../features/chatPreview/PublicChatRoute'
import { AddContactRoute } from '../features/profile/AddContactRoute'
import { SavedMessagesRoute } from '../features/savedMessages/SavedMessagesRoute'

function RequireSession({ children }: { children: ReactNode }) {
  const authenticated = useStore(authStore, (state) => state.session !== null)
  return authenticated ? children : <Navigate to="/login" replace />
}

function AnonymousOnly({ children }: { children: ReactNode }) {
  const authenticated = useStore(authStore, (state) => state.session !== null)
  return authenticated ? <Navigate to="/" replace /> : children
}

const ContactsPage = lazy(() => import('../features/contacts/ContactsPage'))

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
      { path: '/public/:username', element: <PublicChatRoute /> },
      { path: '/add/:username', element: <AddContactRoute /> },
      { path: '/saved', element: <SavedMessagesRoute /> },
      // TG-702: friends, requests, blocklist.
      {
        path: '/contacts',
        element: (
          <Suspense fallback={null}>
            <ContactsPage />
          </Suspense>
        ),
      },
    ],
  },
  { path: '*', element: <Navigate to="/" replace /> },
])
