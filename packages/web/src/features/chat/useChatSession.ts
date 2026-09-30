/**
 * React lifecycle around `createChatSession`: one session per (chat, session token)
 * effect run — StrictMode's double-mount simply builds a second session after cleanly
 * stopping the first (store merges are idempotent, so the replay is harmless).
 */
import { useCallback, useEffect, useRef, useState } from 'react'
import type { ChatSocketStatus } from '@tg/core'
import { authStore, chatListStore, composerStore, messageStore, presenceStore, selectToken } from '@tg/core'
import { useStore } from 'zustand/react'
import { apiClient, draftsApi } from '../../app/client'
import { browserClock, chatSocketUrl, createBrowserSocket } from '../../app/platform'
import type { ChatSession } from './chatSession'
import { createChatSession } from './chatSession'

export interface ChatSessionHandle {
  connection: ChatSocketStatus
  sendMessage(text: string): boolean
  setDraftText(text: string): void
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
      stores: {
        message: messageStore,
        presence: presenceStore,
        composer: composerStore,
        chatList: chatListStore,
      },
    })
    sessionRef.current = session
    const offStatus = session.onStatus(setConnection)
    session.start()
    setConnection(session.status())
    return () => {
      offStatus()
      session.stop()
      sessionRef.current = null
    }
  }, [chatId, token, currentUserId])

  const sendMessage = useCallback((text: string) => sessionRef.current?.sendMessage(text) ?? false, [])
  const setDraftText = useCallback((text: string) => {
    sessionRef.current?.setDraftText(text)
  }, [])

  return { connection, sendMessage, setDraftText }
}
