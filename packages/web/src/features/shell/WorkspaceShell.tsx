/**
 * The three-pane workspace: chat list | conversation | info panel. The middle pane is
 * the router outlet (`/` empty state, `/chat/:chatId` conversation); the right pane
 * mounts only while `uiStore.activePanel` asks for it, as in Telegram.
 */
import { useEffect } from 'react'
import { Outlet } from 'react-router-dom'
import { authStore, chatListStore, selectToken, uiStore } from '@tg/core'
import { useStore } from 'zustand/react'
import { apiClient } from '../../app/client'
import { ChatListPane } from '../chatList/ChatListPane'
import { loadChats } from '../chatList/chatListController'
import { InfoPane } from './InfoPane'

export function WorkspaceShell() {
  const token = useStore(authStore, selectToken)
  const infoOpen = useStore(uiStore, (state) => state.activePanel === 'chatInfo')

  useEffect(() => {
    if (token) void loadChats({ client: apiClient, token, store: chatListStore })
  }, [token])

  return (
    <div className="tg-shell" data-info-open={infoOpen || undefined}>
      <ChatListPane />
      <section className="tg-shell__main">
        <Outlet />
      </section>
      {infoOpen ? <InfoPane /> : null}
    </div>
  )
}
