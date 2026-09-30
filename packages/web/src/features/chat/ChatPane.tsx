/**
 * The middle pane with a chat open: header (title, member count, typing line,
 * connection state), the minimal message list, the minimal composer.
 */
import { useEffect } from 'react'
import { useParams } from 'react-router-dom'
import { authStore, chatListStore, presenceStore, selectChatById, selectPresence, uiStore } from '@tg/core'
import { useStore } from 'zustand/react'
import { Avatar } from '@tg/ui'
import { Composer } from './Composer'
import { MessageList } from './MessageList'
import { useChatSession } from './useChatSession'

const CONNECTION_COPY: Record<string, string> = {
  connecting: '连接中…',
  offline: '连接已断开，正在重连…',
  failed: '无法进入会话',
}

export function ChatPane() {
  const { chatId = '' } = useParams()
  const chat = useStore(chatListStore, selectChatById(chatId))
  const presence = useStore(presenceStore, selectPresence(chatId))
  const currentUserId = useStore(authStore, (state) => state.session?.user.id ?? '')
  const { connection, sendMessage, setDraftText } = useChatSession(chatId)

  useEffect(() => {
    uiStore.getState().setActiveChat(chatId)
    return () => uiStore.getState().setActiveChat('')
  }, [chatId])

  const typingNames = presence.typing
    .filter((indicator) => indicator.user_id !== currentUserId)
    .map((indicator) => indicator.username)
    .filter(Boolean)

  const subtitle =
    CONNECTION_COPY[connection] ??
    (typingNames.length > 0 ? `${typingNames.join('、')} 正在输入…` : `${chat?.member_count ?? 0} 位成员`)

  return (
    <div className="tg-chat">
      <header className="tg-chat__header">
        <button
          type="button"
          className="tg-chat__identity"
          onClick={() => uiStore.getState().openPanel('chatInfo')}
          aria-label="查看会话信息"
        >
          <Avatar label={chat?.title ?? '会话'} initials={chat?.avatar_emoji || undefined} />
          <span className="tg-chat__titles">
            <span className="tg-chat__title">{chat?.title ?? '…'}</span>
            <span className="tg-chat__subtitle" data-typing={typingNames.length > 0 || undefined}>
              {subtitle}
            </span>
          </span>
        </button>
      </header>
      <MessageList chatId={chatId} currentUserId={currentUserId} />
      <Composer chatId={chatId} onSend={sendMessage} onDraftChange={setDraftText} />
    </div>
  )
}
