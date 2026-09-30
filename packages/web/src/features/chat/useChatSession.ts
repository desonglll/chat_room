/**
 * React lifecycle around `createChatSession`: one session per (chat, session token)
 * effect run — StrictMode's double-mount simply builds a second session after cleanly
 * stopping the first (store merges are idempotent, so the replay is harmless).
 */
import { useCallback, useEffect, useRef, useState } from 'react'
import type { ChatSocketStatus, ClientFrame } from '@tg/core'
import { authStore, chatListStore, composerStore, messageStore, presenceStore, selectToken } from '@tg/core'
import { useStore } from 'zustand/react'
import { apiClient, draftsApi } from '../../app/client'
import { browserClock, chatSocketUrl, createBrowserSocket, onPageVisible, pageVisible } from '../../app/platform'
import type { ChatSession, SendMessageOptions } from './chatSession'
import { createChatSession } from './chatSession'
import { registerChatSession } from './chatSessionRegistry'
import { pollStore } from '../poll/pollStore'

export interface ChatSessionHandle {
  connection: ChatSocketStatus
  sendMessage(text: string, options?: SendMessageOptions): boolean
  setDraftText(text: string): void
  sendFrame(frame: ClientFrame): boolean
}

export function useChatSession(chatId: string): ChatSessionHandle {
  const token = useStore(authStore, selectToken)
  const currentUserId = useStore(authStore, (state) => state.session?.user.id ?? '')
  const sessionRef = useRef<ChatSession | null>(null)
  const [connection, setConnection] = useState<ChatSocketStatus>('idle')

  useEffect(() => {
    if (!chatId || !token || !currentUserId) return
    const session = createChatSession({
      chatId,
      token,
      currentUserId,
      socketUrl: chatSocketUrl(chatId),
      createSocket: createBrowserSocket,
      clock: browserClock,
      client: apiClient,
      draftsApi,
      isVisible: pageVisible,
      stores: {
        message: messageStore,
        presence: presenceStore,
        composer: composerStore,
        chatList: chatListStore,
        poll: pollStore,
      },
    })
    sessionRef.current = session
    const offStatus = session.onStatus(setConnection)
    const unregister = registerChatSession(chatId, session)
    const offVisible = onPageVisible(() => session.markRead())
    session.start()
    setConnection(session.status())
    return () => {
      offStatus()
      offVisible()
      unregister()
      session.stop()
      sessionRef.current = null
    }
  }, [chatId, token, currentUserId])

  const sendMessage = useCallback(
    (text: string, options?: SendMessageOptions) => sessionRef.current?.sendMessage(text, options) ?? false,
    [],
  )
  const setDraftText = useCallback((text: string) => {
    sessionRef.current?.setDraftText(text)
  }, [])

  const sendFrame = useCallback((frame: ClientFrame) => sessionRef.current?.sendFrame(frame) ?? false, [])

  return { connection, sendMessage, setDraftText, sendFrame }
}
