/**
 * The one place a chat socket meets the stores (frozen interface §4). Framework-free:
 * every dependency is injected, so `chatSession.test.ts` drives the whole lifecycle —
 * connect, replay, live broadcast, reconnect catch-up, drafts — against a fake socket,
 * fake clock and fake APIs. M1 tasks extend the frame fan-out here rather than opening
 * second subscriptions elsewhere.
 */
import type {
  ApiClient,
  ChatDraft,
  ChatListStore,
  ChatSocketStatus,
  ComposerStore,
  CoreClock,
  CoreSocketFactory,
  CoreTimerHandle,
  DraftsApi,
  MessageStore,
  PresenceStore,
} from '@tg/core'
import {
  EMPTY_DRAFT,
  TYPING_TTL_MS,
  classifyMessageMotion,
  createChatSocket,
  createDraftSynchronizer,
  createRandomUuid,
  listChatMessages,
} from '@tg/core'

/** Server caps: message ≤ 4096 chars, typing preview ≤ 512 (`src/realtime/auth.rs`). */
export const MAX_MESSAGE_CHARS = 4096
export const TYPING_PREVIEW_CHARS = 512
export const TYPING_SEND_INTERVAL_MS = 1000

export interface ChatSessionStores {
  message: MessageStore
  presence: PresenceStore
  composer: ComposerStore
  chatList: ChatListStore
}

export interface ChatSessionOptions {
  chatId: string
  token: string
  currentUserId: string
  socketUrl: string
  createSocket: CoreSocketFactory
  clock: CoreClock
  client: ApiClient
  draftsApi: DraftsApi
  stores: ChatSessionStores
}

export interface ChatSession {
  start(): void
  /** Flushes a pending draft save, closes the socket, detaches every subscription. */
  stop(): void
  /** Optimistic append + WS send; false marks the row failed (offline). */
  sendMessage(text: string): boolean
  /** Composer edit: store + debounced cloud save + throttled typing preview. */
  setDraftText(text: string): void
  status(): ChatSocketStatus
  onStatus(handler: (status: ChatSocketStatus) => void): () => void
}

export function createChatSession(options: ChatSessionOptions): ChatSession {
  const { chatId, currentUserId, clock, stores } = options
  const unsubscribers: Array<() => void> = []
  let expiryTimer: CoreTimerHandle | null = null
  // -Infinity so the very first preview always goes out, whatever the clock's epoch.
  let lastTypingSentAt = Number.NEGATIVE_INFINITY
  let typingCleared = true
  // Whether the server may hold a draft row for this chat: set optimistically when a
  // commit starts, corrected by every accepted remote state. Decides whether sending a
  // message needs to flush an empty clear (PUT "") or has nothing to clean up.
  let serverDraftMayExist = false

  const socket = createChatSocket({
    url: options.socketUrl,
    token: options.token,
    createSocket: options.createSocket,
    clock,
    // Catch-up page, newest-first from REST, reversed to chronological order; the
    // socket filters by cursor and `mergeIncomingBroadcast` dedups by message_id.
    fetchMissed: async () => {
      const page = await listChatMessages(options.client, chatId, { token: options.token })
      return page.slice().reverse()
    },
  })

  const synchronizer = createDraftSynchronizer(
    async (id, draft) => {
      if (draft.text !== '' || draft.reply_to_message_id !== null) serverDraftMayExist = true
      const stored = await options.draftsApi.put(id, draft)
      acceptRemoteDraft(stored)
    },
    { clock },
  )

  /**
   * The draft policy («打字的键盘赢», frozen interface §4): every remote draft state —
   * WS frame, GET at open, PUT response — is accepted by the synchronizer FIRST and
   * applied to the composer only when no local save is pending. A differing local edit
   * stays queued and overwrites the server on commit (`domain/draftSync` semantics).
   */
  function acceptRemoteDraft(draft: ChatDraft): void {
    serverDraftMayExist = draft.text !== '' || draft.reply_to_message_id !== null
    synchronizer.accept(chatId, {
      text: draft.text,
      reply_to_message_id: draft.reply_to_message_id,
      topic_id: draft.topic_id,
    })
    if (synchronizer.pending(chatId)) return
    stores.composer.getState().applyDraftUpdated(chatId, { ...draft, type: 'draft_updated' })
  }

  function scheduleTypingExpiry(): void {
    if (expiryTimer !== null) clock.clearTimeout(expiryTimer)
    expiryTimer = clock.setTimeout(() => {
      expiryTimer = null
      stores.presence.getState().expireTyping(chatId, clock.now())
      if (stores.presence.getState().chats[chatId]?.typing.length) scheduleTypingExpiry()
    }, TYPING_TTL_MS + 50)
  }

  function maybeSendTyping(text: string): void {
    if (socket.status() !== 'online') return
    if (!text) {
      if (!typingCleared) {
        typingCleared = true
        socket.send({ type: 'typing', content: '', action: 'cancel' })
      }
      return
    }
    const now = clock.now()
    if (now - lastTypingSentAt < TYPING_SEND_INTERVAL_MS) return
    lastTypingSentAt = now
    typingCleared = false
    socket.send({ type: 'typing', content: text.slice(0, TYPING_PREVIEW_CHARS), action: 'typing' })
  }

  return {
    start() {
      unsubscribers.push(
        socket.on('auth_ok', (frame) => stores.presence.getState().applyAuthOk(chatId, frame)),
        socket.on('history_complete', () => stores.message.getState().setHistoryReady(chatId, true)),
        socket.on('broadcast', (frame) => {
          const ready = stores.message.getState().timelines[chatId]?.historyReady ?? false
          const motion = classifyMessageMotion(ready, frame.sender_id, currentUserId)
          stores.message.getState().applyBroadcast(chatId, frame, motion)
        }),
        socket.on('presence', (frame) => stores.presence.getState().applyPresence(chatId, frame)),
        socket.on('typing', (frame) => {
          stores.presence.getState().applyTyping(chatId, frame, clock.now())
          scheduleTypingExpiry()
        }),
        socket.on('user_status', (frame) => stores.presence.getState().applyUserStatus(chatId, frame)),
        socket.on('message_edited', (frame) => stores.message.getState().applyEdit(chatId, frame)),
        socket.on('message_recalled', (frame) => stores.message.getState().applyRecall(chatId, frame)),
        socket.on('reaction_changed', (frame) => stores.message.getState().applyReaction(chatId, frame)),
        socket.on('chat_updated', (frame) => stores.chatList.getState().applyChatUpdated(frame.chat)),
        socket.on('draft_updated', (frame) => acceptRemoteDraft(frame)),
      )
      socket.connect()
      options.draftsApi
        .get(chatId)
        .then((stored) => {
          if (stored) acceptRemoteDraft(stored)
        })
        .catch(() => {
          // No cloud draft is a fine draft; the next local edit will create one.
        })
    },

    stop() {
      synchronizer.flush(chatId)
      synchronizer.dispose()
      for (const unsubscribe of unsubscribers) unsubscribe()
      unsubscribers.length = 0
      if (expiryTimer !== null) {
        clock.clearTimeout(expiryTimer)
        expiryTimer = null
      }
      socket.close()
    },

    sendMessage(text) {
      const content = text.trim()
      if (!content || [...content].length > MAX_MESSAGE_CHARS) return false
      const clientMessageId = createRandomUuid()
      const draft = stores.composer.getState().drafts[chatId] ?? EMPTY_DRAFT
      const replyTo = draft.replyToMessageId ?? ''
      stores.message.getState().appendOptimistic(chatId, {
        clientMessageId,
        content,
        replyTo,
        currentUserId,
        participants: stores.presence.getState().chats[chatId]?.participants ?? [],
      })
      const sent = socket.send({
        type: 'message',
        content,
        ...(replyTo ? { reply_to: replyTo } : {}),
        client_message_id: clientMessageId,
      })
      if (!sent) stores.message.getState().markDelivery(chatId, clientMessageId, 'failed')
      stores.composer.getState().clearDraft(chatId)
      if (serverDraftMayExist) {
        // A draft row may exist server-side: flush an empty save, which the server
        // treats as an idempotent clear (TG-008 frozen interface).
        synchronizer.update(chatId, { text: '' })
        synchronizer.flush(chatId)
      } else {
        // Nothing ever reached the server: just drop any queued local edit so the
        // debounce cannot store a draft for a message that was already sent.
        synchronizer.dispose()
      }
      typingCleared = true // the server broadcasts the cancel itself on message store
      return sent
    },

    setDraftText(text) {
      stores.composer.getState().setDraftText(chatId, text)
      const draft = stores.composer.getState().drafts[chatId] ?? EMPTY_DRAFT
      synchronizer.update(chatId, {
        text,
        reply_to_message_id: draft.replyToMessageId,
        topic_id: draft.topicId,
      })
      maybeSendTyping(text)
    },

    status: () => socket.status(),
    onStatus: (handler) => socket.onStatus(handler),
  }
}
