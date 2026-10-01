/**
 * React lifecycle around `createChatSession`: one session per (chat, session token)
 * effect run — StrictMode's double-mount simply builds a second session after cleanly
 * stopping the first (store merges are idempotent, so the replay is harmless).
 */
import { useCallback, useEffect, useRef, useState } from 'react'
import type { ChatSocketStatus, ClientFrame, ServerFrame } from '@tg/core'
import { authStore, chatListStore, composerStore, messageStore, presenceStore, selectToken } from '@tg/core'
import { useStore } from 'zustand/react'
import { apiClient, draftsApi } from '../../app/client'
import { browserClock, chatSocketUrl, createBrowserSocket, onPageVisible, pageVisible } from '../../app/platform'
import type { ChatSession, ChatSessionTopicMode, SendMessageOptions } from './chatSession'
import { createChatSession } from './chatSession'
import { registerChatSession } from './chatSessionRegistry'
import { pollStore } from '../poll/pollStore'
import { channelStore } from '../channel/channelStore'

export interface ChatSessionHandle {
  connection: ChatSocketStatus
  /** TG-1208: the composer's gate — true while opening (sends are parked) and when online. */
  sendable: boolean
  sendMessage(text: string, options?: SendMessageOptions): boolean
  setDraftText(text: string): void
  sendFrame(frame: ClientFrame): boolean
}

/** TG-204: forum modes. `topic` must be memoized — a new object restarts the session. */
export interface ChatSessionHookOptions {
  topic?: ChatSessionTopicMode | null | undefined
  /** False: no chat-level `read` frame (the forum topic list). */
  readCursor?: boolean | undefined
  /** Every server frame; the latest callback is used without restarting the session. */
  onFrame?: ((frame: ServerFrame) => void) | undefined
}

export function useChatSession(chatId: string, hookOptions: ChatSessionHookOptions = {}): ChatSessionHandle {
  const { topic = null, readCursor = true } = hookOptions
  const onFrameRef = useRef(hookOptions.onFrame)
  useEffect(() => {
    onFrameRef.current = hookOptions.onFrame
  })
  const token = useStore(authStore, selectToken)
  const currentUserId = useStore(authStore, (state) => state.session?.user.id ?? '')
  const sessionRef = useRef<ChatSession | null>(null)
  const [connection, setConnection] = useState<ChatSocketStatus>('idle')
  const [sendable, setSendable] = useState(false)

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
      topic,
      readCursor,
      stores: {
        message: messageStore,
        presence: presenceStore,
        composer: composerStore,
        chatList: chatListStore,
        poll: pollStore,
        channel: channelStore,
      },
    })
    sessionRef.current = session
    const offStatus = session.onStatus(setConnection)
    const offSendable = session.onSendable(setSendable)
    const offFrame = session.onFrame((frame) => onFrameRef.current?.(frame))
    const unregister = registerChatSession(chatId, session)
    const offVisible = onPageVisible(() => session.markRead())
    session.start()
    setConnection(session.status())
    setSendable(session.sendable())
    return () => {
      offStatus()
      offSendable()
      offFrame()
      offVisible()
      unregister()
      session.stop()
      sessionRef.current = null
    }
  }, [chatId, token, currentUserId, topic, readCursor])

  const sendMessage = useCallback(
    (text: string, options?: SendMessageOptions) => sessionRef.current?.sendMessage(text, options) ?? false,
    [],
  )
  const setDraftText = useCallback((text: string) => {
    sessionRef.current?.setDraftText(text)
  }, [])

  const sendFrame = useCallback((frame: ClientFrame) => sessionRef.current?.sendFrame(frame) ?? false, [])

  return { connection, sendable, sendMessage, setDraftText, sendFrame }
}
