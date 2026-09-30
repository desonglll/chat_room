/**
 * The chat header (TG-100 wiring): mobile back button inline (TG-102), avatar + title,
 * and TG-107's status line (typing / members / last seen) under the title, with the
 * connection-state copy taking its place while the socket is not online.
 */
import type { ChatSocketStatus } from '@tg/core'
import { chatListStore, selectChatById, uiStore } from '@tg/core'
import { Avatar } from '@tg/ui'
import { useStore } from 'zustand/react'
import { ChatHeaderStatus } from '../presence'
import { MobileBackButton } from '../shell/MobileBackButton'

export const CONNECTION_COPY: Partial<Record<ChatSocketStatus, string>> = {
  connecting: '连接中…',
  offline: '连接已断开，正在重连…',
  failed: '无法进入会话',
}

export function ChatHeader({ chatId, connection }: { chatId: string; connection: ChatSocketStatus }) {
  const chat = useStore(chatListStore, selectChatById(chatId))
  // A private chat has no `/api/chats` descriptor; its sidebar row still names the peer.
  const row = useStore(chatListStore, (state) => state.conversations.find((entry) => entry.room_id === chatId))
  const title = chat?.title || row?.alias || row?.title || '…'
  const emoji = chat?.avatar_emoji || row?.avatar_emoji || undefined
  const connectionCopy = CONNECTION_COPY[connection]

  return (
    <header className="tg-chat__header">
      <MobileBackButton />
      <button
        type="button"
        className="tg-chat__identity"
        onClick={() => uiStore.getState().openPanel('chatInfo')}
        aria-label="查看会话信息"
      >
        <Avatar label={title} initials={emoji} />
        <span className="tg-chat__titles">
          <span className="tg-chat__title">{title}</span>
          {connectionCopy ? (
            <span className="tg-chat__subtitle">{connectionCopy}</span>
          ) : (
            <ChatHeaderStatus chatId={chatId} className="tg-chat__subtitle" />
          )}
        </span>
      </button>
    </header>
  )
}
