/**
 * Everything the chat feature layers over the whole workspace, mounted once by the shell
 * (TG-100): the media viewer (TG-105) with forward / delete bound, the forward picker and
 * the delete confirmation. Download is the viewer's own `<a download>` default.
 */
import { useMemo } from 'react'
import { authStore } from '@tg/core'
import { useStore } from 'zustand/react'
import type { MediaViewerActions } from '../mediaViewer'
import { MediaViewer, mediaViewerStore } from '../mediaViewer'
import { requestDelete, requestForward } from './chatDialogStore'
import { DeleteConfirmDialog, ForwardDialog } from './ChatDialogs'
import { TranslationDialog } from '../contact/TranslationDialog'

const viewerChatId = () => mediaViewerStore.getState().request?.chatId ?? ''

export function ChatOverlays() {
  const viewerId = useStore(authStore, (state) => state.session?.user.id ?? '')
  const actions = useMemo<MediaViewerActions>(
    () => ({
      onForward: (item) => requestForward(viewerChatId(), [item.messageId]),
      onDelete: (item) => requestDelete(viewerChatId(), [item.messageId]),
      canDelete: (item) => item.senderId === viewerId,
    }),
    [viewerId],
  )
  return (
    <>
      <MediaViewer actions={actions} />
      <ForwardDialog />
      {/* TG-410: the translation of a message, when AI is configured. */}
      <TranslationDialog />
      <DeleteConfirmDialog />
    </>
  )
}
