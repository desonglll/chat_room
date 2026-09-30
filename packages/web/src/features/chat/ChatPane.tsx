/**
 * The middle pane with a chat open (TG-100 wiring of the M1 features): header (TG-107
 * status, TG-102 back button), the virtual list (TG-101) drawing TG-103 bubbles with every
 * action bound, and TG-104's composer — or the selection bar while messages are selected.
 * `?message=<id>` deep-links into history through the list's jump-to-message.
 */
import { useCallback, useEffect, useMemo } from 'react'
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
import { useStore } from 'zustand/react'
import { apiClient } from '../../app/client'
import { copyText } from '../../app/platform'
import { Composer } from '../composer'
import { openMediaViewer } from '../mediaViewer'
import { MessageList } from '../messageList/MessageList'
import { requestDelete, requestForward } from './chatDialogStore'
import { ChatHeader } from './ChatHeader'
import type { ChatRenderOptions } from './ChatMessage'
import { createChatRenderer } from './ChatMessage'
import { canDeleteAll } from './messageActions'
import { pinMessage } from './pinMessage'
import { SelectionBar } from './SelectionBar'
import { useChatSession } from './useChatSession'
import { useMessageSelection } from './useMessageSelection'

export function ChatPane() {
  const { chatId = '' } = useParams()
  const [searchParams] = useSearchParams()
  const chat = useStore(chatListStore, selectChatById(chatId))
  const direct = useStore(
    chatListStore,
    (state) => state.conversations.find((row) => row.room_id === chatId)?.kind === 'direct',
  )
  const presence = useStore(presenceStore, selectPresence(chatId))
  const currentUserId = useStore(authStore, (state) => state.session?.user.id ?? '')
  const session = useChatSession(chatId)
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
    }),
    [chatId, currentUserId, canPin, sendFrame, toggleSelected],
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
      <ChatHeader chatId={chatId} connection={connection} />
      <MessageList
        key={chatId}
        chatId={chatId}
        currentUserId={currentUserId}
        renderMessage={renderMessage}
        selectedMessageIds={selected}
        targetMessageId={searchParams.get('message')?.trim() ?? ''}
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
      ) : (
        <Composer
          chatId={chatId}
          currentUserId={currentUserId}
          members={presence.participants}
          session={session}
          canSend={connection === 'online'}
        />
      )}
    </div>
  )
}
