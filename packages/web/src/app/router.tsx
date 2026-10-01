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

function RequireSession({ children }: { children: ReactNode }) {
  const authenticated = useStore(authStore, (state) => state.session !== null)
  return authenticated ? children : <Navigate to="/login" replace />
}

function AnonymousOnly({ children }: { children: ReactNode }) {
  const authenticated = useStore(authStore, (state) => state.session !== null)
  return authenticated ? <Navigate to="/" replace /> : children
}

// TG-806: deep-link and secondary routes load on first visit, not on first paint.
const StickerSetLinkRoute = lazy(() =>
  import('../features/sticker/StickerSetLinkRoute').then((m) => ({ default: m.StickerSetLinkRoute })),
)
const JoinChatRoute = lazy(() =>
  import('../features/inviteLinks/JoinChatRoute').then((m) => ({ default: m.JoinChatRoute })),
)
const PublicChatRoute = lazy(() =>
  import('../features/chatPreview/PublicChatRoute').then((m) => ({ default: m.PublicChatRoute })),
)
const AddContactRoute = lazy(() =>
  import('../features/profile/AddContactRoute').then((m) => ({ default: m.AddContactRoute })),
)
const SavedMessagesRoute = lazy(() =>
  import('../features/savedMessages/SavedMessagesRoute').then((m) => ({ default: m.SavedMessagesRoute })),
)
const ContactsPage = lazy(() => import('../features/contacts/ContactsPage'))
const AdminPage = lazy(() => import('../features/admin/AdminPage'))
const NotificationsPage = lazy(() => import('../features/notifications/NotificationsPage'))

/** A lazy route element; nothing shows while its chunk loads (it is small and local). */
const suspended = (element: ReactNode) => <Suspense fallback={null}>{element}</Suspense>

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
      { path: '/addstickers/:shortName', element: suspended(<StickerSetLinkRoute />) },
      { path: '/joinchat/:token', element: suspended(<JoinChatRoute />) },
      { path: '/public/:username', element: suspended(<PublicChatRoute />) },
      { path: '/add/:username', element: suspended(<AddContactRoute />) },
      { path: '/saved', element: suspended(<SavedMessagesRoute />) },
      // TG-705: system administration.
      {
        path: '/admin',
        element: (
          <Suspense fallback={null}>
            <AdminPage />
          </Suspense>
        ),
      },
      // TG-703: notification center.
      {
        path: '/notifications',
        element: (
          <Suspense fallback={null}>
            <NotificationsPage />
          </Suspense>
        ),
      },
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
