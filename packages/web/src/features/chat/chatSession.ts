/**
 * The one place a chat socket meets the stores (frozen interface §4). Framework-free:
 * every dependency is injected, so `chatSession.test.ts` drives the whole lifecycle —
 * connect, replay, live broadcast, reconnect catch-up, drafts — against a fake socket,
 * fake clock and fake APIs. M1 tasks extend the frame fan-out here rather than opening
 * second subscriptions elsewhere.
 */
import { refreshPins } from './pinned/pinnedStore'
import type { ChatDraft, CoreTimerHandle, ReplyExtras } from '@tg/core'
import {
  EMPTY_DRAFT,
  TYPING_TTL_MS,
  classifyMessageMotion,
  createChatSocket,
  createDraftSynchronizer,
  createRandomUuid,
  getChat,
  listChatMessages,
  storedMessageToBroadcast,
} from '@tg/core'
import { applyPollFrame } from '../poll/pollStore'
import { applyViewsFrame } from '../channel/channelStore'
import { applyLocationFrame } from '../location/liveLocationStore'
import { applyLinkPreviewFrame } from '../linkPreview/linkPreviewStore'
import { applyVoiceListenedFrame } from '../voice/voiceStore'
import type { ChatSession, ChatSessionOptions } from './chatSessionTypes'
import { createOpeningSendQueue } from './openingSendQueue'
import { MAX_MESSAGE_CHARS } from './chatSessionTypes'

export type {
  ChatSession,
  ChatSessionOptions,
  ChatSessionStores,
  ChatSessionTopicMode,
  SendMessageOptions,
} from './chatSessionTypes'
export { MAX_MESSAGE_CHARS } from './chatSessionTypes'

/** TG-409: the reply's quote and source chat, on the wire (only alongside a `reply_to`). */
function replyExtrasFrame(
  extras: ReplyExtras | undefined,
  replyTo: string,
): { reply_quote?: { text: string; offset: number }; reply_to_chat_id?: string } {
  if (!replyTo || !extras) return {}
  return {
    ...(extras.quote ? { reply_quote: { text: extras.quote.text, offset: extras.quote.offset } } : {}),
    ...(extras.source ? { reply_to_chat_id: extras.source.chatId } : {}),
  }
}

export function createChatSession(options: ChatSessionOptions): ChatSession {
  const { chatId, currentUserId, clock, stores } = options
  const topic = options.topic ?? null
  let stopped = false
  const unsubscribers: Array<() => void> = []
  let expiryTimer: CoreTimerHandle | null = null
  let lastReadSent = ''
  let knownParticipants = -1
  // Whether the server may hold a draft row for this chat: set optimistically when a
  // commit starts, corrected by every accepted remote state. Decides whether sending a
  // message needs to flush an empty clear (PUT "") or has nothing to clean up.
  let serverDraftMayExist = false
  const openingSends = createOpeningSendQueue()
  const sendableHandlers = new Set<(sendable: boolean) => void>()

  const socket = createChatSocket({
    url: options.socketUrl,
    token: options.token,
    createSocket: options.createSocket,
    clock,
    // Catch-up page, newest-first from REST, reversed to chronological order; the
    // socket filters by cursor and `mergeIncomingBroadcast` dedups by message_id.
    fetchMissed: async () => {
      const page = topic
        ? await topic.latest()
        : await listChatMessages(options.client, chatId, { token: options.token })
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

  /**
   * The chat descriptor (member count, title …) is fetched once by the list sync; when
   * the participant set changes size (a join / leave while the chat is open), refetch it
   * so the header's member count follows. Private chats have no descriptor (404 → null).
   */
  function refreshDescriptor(participants: number): void {
    if (participants === knownParticipants) return
    const first = knownParticipants < 0
    knownParticipants = participants
    const known = stores.chatList.getState().chats.find((chat) => chat.id === chatId)
    if (first && known?.member_count === participants) return
    getChat(options.client, chatId, options.token)
      .then((chat) => {
        if (chat) stores.chatList.getState().upsertChat(chat)
      })
      .catch(() => {
        // The header keeps the last known descriptor.
      })
  }

  /** TG-1208: whether the composer may send now — parked while opening, live once ready. */
  function sendable(): boolean {
    if (stopped) return false
    if (openingSends.opening()) return socket.status() !== 'failed'
    return socket.status() === 'online'
  }

  function notifySendable(): void {
    const value = sendable()
    for (const handler of [...sendableHandlers]) handler(value)
  }

  function failParked(): void {
    openingSends.abandon((clientMessageId) => stores.message.getState().markDelivery(chatId, clientMessageId, 'failed'))
  }

  function markRead(): void {
    if (options.readCursor === false) return
    if (socket.status() !== 'online' || !(options.isVisible?.() ?? true)) return
    const timeline = stores.message.getState().timelines[chatId]
    if (!timeline?.historyReady) return
    let newest = ''
    for (let index = timeline.messages.length - 1; index >= 0; index -= 1) {
      const message = timeline.messages[index]
      if (message?.type !== 'broadcast' || message.message_id.startsWith('pending:')) continue
      newest = message.message_id
      break
    }
    if (!newest || newest === lastReadSent || newest === timeline.readCursors[currentUserId]) return
    lastReadSent = newest
    if (topic) topic.read(newest)
    else socket.send({ type: 'read', message_id: newest })
  }

  return {
    start() {
      unsubscribers.push(
        socket.on('auth_ok', (frame) => {
          stores.presence.getState().applyAuthOk(chatId, frame)
          stores.message.getState().applyReadReceipts(chatId, frame.read_receipts)
          refreshDescriptor(frame.participants.length)
        }),
        socket.on('history_complete', () => {
          const ready = () => {
            if (stopped) return
            stores.message.getState().setHistoryReady(chatId, true)
            // TG-1208: the sends made while opening go out now, after the replay, in order.
            openingSends.ready(({ clientMessageId, frame }) => {
              if (!socket.send(frame)) stores.message.getState().markDelivery(chatId, clientMessageId, 'failed')
            })
            notifySendable()
            markRead()
          }
          if (!topic) return ready()
          // The chat-wide replay may hold none of a quiet topic's messages: merge the
          // topic's newest page (older than the replay, deduped by id) before opening.
          topic
            .latest()
            .then((page) => {
              if (!stopped) stores.message.getState().prependHistory(chatId, page.map(storedMessageToBroadcast))
            })
            .catch(() => {
              // The replayed rows still open; older pages load on scroll.
            })
            .finally(ready)
        }),
        socket.on('broadcast', (frame) => {
          if (topic && !topic.accepts(frame)) return
          const ready = stores.message.getState().timelines[chatId]?.historyReady ?? false
          const motion = classifyMessageMotion(ready, frame.sender_id, currentUserId)
          stores.message.getState().applyBroadcast(chatId, frame, motion)
          if (ready && frame.sender_id !== currentUserId) markRead()
        }),
        socket.on('read_receipt', (frame) => stores.message.getState().applyReadReceipts(chatId, [frame])),
        socket.on('presence', (frame) => {
          stores.presence.getState().applyPresence(chatId, frame)
          refreshDescriptor(frame.participants.length)
        }),
        socket.on('typing', (frame) => {
          stores.presence.getState().applyTyping(chatId, frame, clock.now())
          scheduleTypingExpiry()
        }),
        socket.on('user_status', (frame) => stores.presence.getState().applyUserStatus(chatId, frame)),
        socket.on('message_edited', (frame) => stores.message.getState().applyEdit(chatId, frame)),
        socket.on('message_recalled', (frame) => stores.message.getState().applyRecall(chatId, frame)),
        socket.on('messages_deleted', (frame) => stores.message.getState().applyDeleted(chatId, frame)),
        socket.on('reaction_changed', (frame) => stores.message.getState().applyReaction(chatId, frame)),
        socket.on('chat_updated', (frame) => stores.chatList.getState().applyChatUpdated(frame.chat)),
        socket.on('draft_updated', (frame) => acceptRemoteDraft(frame)),
        socket.on('poll_updated', (frame) => {
          if (stores.poll) applyPollFrame(frame, stores.poll)
        }),
        socket.on('message_views_updated', (frame) => {
          if (stores.channel) applyViewsFrame(frame, stores.channel)
        }),
        // TG-408: link cards built after delivery (features/linkPreview).
        socket.on('link_preview_updated', (frame) => applyLinkPreviewFrame(frame)),
        // TG-407: live location points (features/location).
        socket.on('location_updated', (frame) => applyLocationFrame(frame)),
        // TG-401: the listener's and the sender's unlistened dots (features/voice).
        socket.on('voice_listened', (frame) => applyVoiceListenedFrame(frame)),
        // TG-901: someone pinned or unpinned here; the bar re-reads the pins.
        socket.on('pins_changed', () => void refreshPins(chatId)),
        socket.onStatus((status) => {
          if (status === 'failed') failParked()
          notifySendable()
        }),
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
      stopped = true
      failParked()
      sendableHandlers.clear()
      synchronizer.flush(chatId)
      synchronizer.dispose()
      for (const unsubscribe of unsubscribers) unsubscribe()
      unsubscribers.length = 0
      if (expiryTimer !== null) {
        clock.clearTimeout(expiryTimer)
        expiryTimer = null
      }
      socket.close()
      // The next open replays from the server: a kept timeline could silently skip every
      // message that arrived while the chat was closed (the replay appends after it).
      stores.message.getState().clearChat(chatId)
    },

    sendMessage(text, options) {
      const content = text.trim()
      if (!content || [...content].length > MAX_MESSAGE_CHARS) return false
      const clientMessageId = createRandomUuid()
      const draft = stores.composer.getState().drafts[chatId] ?? EMPTY_DRAFT
      const replyTo = draft.replyToMessageId ?? ''
      const entities = options?.entities ?? []
      stores.message.getState().appendOptimistic(chatId, {
        clientMessageId,
        content,
        replyTo,
        currentUserId,
        participants: stores.presence.getState().chats[chatId]?.participants ?? [],
        ...(entities.length ? { entities } : {}),
      })
      const frame = {
        type: 'message' as const,
        content,
        ...(replyTo ? { reply_to: replyTo } : {}),
        ...replyExtrasFrame(stores.composer.getState().replyExtras[chatId], replyTo),
        client_message_id: clientMessageId,
        ...(topic?.sendTopicId ? { topic_id: topic.sendTopicId } : {}),
        ...(options?.silent ? { silent: true } : {}),
        ...(options?.noLinkPreview ? { no_link_preview: true } : {}),
        ...(entities.length ? { entities } : {}),
      }
      // TG-1208: while the chat is still opening the send is parked, not refused.
      const parked = openingSends.opening() && sendable()
      if (parked) openingSends.push(clientMessageId, frame)
      const sent = parked || socket.send(frame)
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
    },

    sendFrame: (frame) => socket.send(frame),
    markRead,

    status: () => socket.status(),
    onStatus: (handler) => socket.onStatus(handler),
    sendable,
    onSendable(handler) {
      sendableHandlers.add(handler)
      return () => sendableHandlers.delete(handler)
    },
    onFrame: (handler) => socket.onAnyFrame(handler),
  }
}
