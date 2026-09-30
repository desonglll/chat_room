/**
 * The middle pane with a chat open (TG-100 wiring of the M1 features): header (TG-107
 * status, TG-102 back button), the virtual list (TG-101) drawing TG-103 bubbles with every
 * action bound, and TG-104's composer — or the selection bar while messages are selected.
 * `?message=<id>` deep-links into history through the list's jump-to-message.
 */
import { useCallback, useEffect, useMemo, type ReactNode } from 'react'
import { useParams, useSearchParams } from 'react-router-dom'
import {
  authStore,
  chatListStore,
  composerStore,
  messageStore,
  peerReadThrough,
  presenceStore,
  selectChatById,
  selectPresence,
  selectTimeline,
  selectToken,
  uiStore,
} from '@tg/core'
import type { ServerFrame } from '@tg/core'
import { useStore } from 'zustand/react'
import { apiClient } from '../../app/client'
import { copyText } from '../../app/platform'
import { ChannelFooter, useChannelPublisher } from '../channel'
import { chatAdminApi } from '../chatAdmin'
import { Composer } from '../composer'
import { openMediaViewer } from '../mediaViewer'
import { MessageList } from '../messageList/MessageList'
import type { MessageListApi } from '../messageList/messageListController'
import type { ChatSessionTopicMode } from './chatSession'
import { requestDelete, requestForward, requestReplyElsewhere } from './chatDialogStore'
import { ChatHeader } from './ChatHeader'
import type { ChatRenderOptions } from './ChatMessage'
import { createChatRenderer } from './ChatMessage'
import { canDeleteAll } from './messageActions'
import { pinMessage } from './pinMessage'
import { SelectionBar } from './SelectionBar'
import { useChatSession } from './useChatSession'
import { useMessageSelection } from './useMessageSelection'
import { SlowModeNotice } from '../chatAdmin/slowMode/SlowModeNotice'
import { useSlowMode } from '../chatAdmin/slowMode/useSlowMode'
import { useNavigate } from 'react-router-dom'

/** TG-204: one forum topic's view of the chat, assembled by `features/forum`. */
export interface ChatPaneTopic {
  id: string
  mode: ChatSessionTopicMode
  header: ReactNode
  listApi: MessageListApi
  /** Replaces the composer (a closed topic for a non-manager). */
  composerLock?: ReactNode
  /** Every server frame of the chat socket (topic edits refresh the header). */
  onFrame?: ((frame: ServerFrame) => void) | undefined
}

export function ChatPane({ topic }: { topic?: ChatPaneTopic | undefined } = {}) {
  const { chatId = '' } = useParams()
  const [searchParams] = useSearchParams()
  const chat = useStore(chatListStore, selectChatById(chatId))
  const direct = useStore(
    chatListStore,
    (state) => state.conversations.find((row) => row.room_id === chatId)?.kind === 'direct',
  )
  const presence = useStore(presenceStore, selectPresence(chatId))
  const currentUserId = useStore(authStore, (state) => state.session?.user.id ?? '')
  const slowMode = useSlowMode(chatId, currentUserId)
  const navigate = useNavigate()
  const session = useChatSession(chatId, { topic: topic?.mode ?? null, onFrame: topic?.onFrame })
  const { connection, sendFrame } = session
  const selection = useMessageSelection(chatId)
  const { selected, toggle: toggleSelected, clear: clearSelection } = selection

  useEffect(() => {
    uiStore.getState().setActiveChat(chatId)
    return () => uiStore.getState().setActiveChat('')
  }, [chatId])

  const chatType = chat?.chat_type
  const groupIdentity = chatType === 'group' || chatType === 'supergroup'
  const role = chat?.membership_role
  // Server rule (`require_pin_permission`): any member of a private chat, else `message.pin`
  // (owner/admin). A private chat has no `/api/chats` descriptor, hence the sidebar kind.
  const canPin = direct || chatType === 'private' || role === 'owner' || role === 'admin'
  // Primitive selections: the pane re-renders when the read tick or the delete permission
  // changes, not on every message frame (the list subscribes to the timeline itself).
  const peerReadAt = useStore(messageStore, (state) => peerReadThrough(selectTimeline(chatId)(state), currentUserId))
  // TG-202: in a channel only `message.post` holders get the composer.
  const canPublish = useChannelPublisher(chatAdminApi, chat)
  const selectionMode = selected.size > 0
  const selectionDeletable = useStore(
    messageStore,
    (state) => selectionMode && canDeleteAll(selectTimeline(chatId)(state).messages, selected, currentUserId),
  )

  const deps = useMemo<ChatRenderOptions['deps']>(
    () => ({
      chatId,
      viewerId: currentUserId,
      canPin,
      sendFrame,
      dispatchMode: (target, event) => composerStore.getState().dispatchMode(target, event),
      toggleSelected,
      openMedia: (attachmentId) => openMediaViewer({ chatId, attachmentId }),
      requestDelete: (ids) => void requestDelete(chatId, ids),
      requestForward: (ids) => requestForward(chatId, ids),
      pin: (messageId) =>
        void pinMessage(apiClient, selectToken(authStore.getState()), chatId, messageId).catch(() => {}),
      copy: (text) => void copyText(text),
      quote: (messageId, content) => {
        // TG-409: the selection must be a slice of this message; a JS string index is the
        // UTF-16 offset the server expects. Without one this is a plain reply.
        const selected = (globalThis.getSelection?.()?.toString() ?? '').trim()
        const offset = selected ? content.indexOf(selected) : -1
        if (offset >= 0) {
          composerStore.getState().setReplyWithExtras(chatId, messageId, { quote: { text: selected, offset } })
        } else {
          composerStore.getState().dispatchMode(chatId, { type: 'reply', messageId })
        }
      },
      replyElsewhere: (source) => requestReplyElsewhere(chatId, source),
      openChatAt: (target, messageId) =>
        void navigate(`/chat/${encodeURIComponent(target)}?message=${encodeURIComponent(messageId)}`),
    }),
    [chatId, currentUserId, canPin, sendFrame, toggleSelected, navigate],
  )
  const renderMessage = useMemo(
    () => createChatRenderer({ deps, peerReadAt, selectionMode, groupIdentity }),
    [deps, peerReadAt, selectionMode, groupIdentity],
  )

  // Chronological, like Telegram forwards them — not click order.
  const selectedIds = useCallback(() => {
    const ordered = selectTimeline(chatId)(messageStore.getState()).messages.flatMap((row) =>
      row.type === 'broadcast' && selected.has(row.message_id) ? [row.message_id] : [],
    )
    return ordered.length === selected.size ? ordered : [...selected]
  }, [chatId, selected])

  return (
    <div className="tg-chat">
      {topic ? topic.header : <ChatHeader chatId={chatId} connection={connection} />}
      <MessageList
        key={topic ? `${chatId}:${topic.id}` : chatId}
        chatId={chatId}
        currentUserId={currentUserId}
        renderMessage={renderMessage}
        selectedMessageIds={selected}
        targetMessageId={searchParams.get('message')?.trim() ?? ''}
        {...(topic ? { api: topic.listApi } : {})}
      />
      {selectionMode ? (
        <SelectionBar
          count={selected.size}
          canDelete={selectionDeletable}
          onForward={() => {
            requestForward(chatId, selectedIds())
            clearSelection()
          }}
          onDelete={() => {
            void requestDelete(chatId, selectedIds()).then((deleted) => {
              if (deleted) clearSelection()
            })
          }}
          onCancel={clearSelection}
        />
      ) : !canPublish ? (
        <ChannelFooter chatId={chatId} />
      ) : topic?.composerLock ? (
        topic.composerLock
      ) : (
        <>
          <SlowModeNotice wait={slowMode.wait} />
          <Composer
            chatId={chatId}
            currentUserId={currentUserId}
            members={presence.participants}
            session={session}
            canSend={connection === 'online' && slowMode.wait === 0}
          />
        </>
      )}
    </div>
  )
}
