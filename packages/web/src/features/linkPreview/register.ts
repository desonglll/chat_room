/**
 * TG-408 side-effect module, imported once by `main.tsx`: text messages with a link render
 * through `LinkPreviewContent` (cards may arrive after the message), and a sender can remove
 * the card from their own message.
 */
import { authStore, uiStore } from '@tg/core'
import { registerMessageContent, registerMessageMenuItem } from '../message'
import { linkPreviewApi } from './linkPreviewApi'
import { LinkPreviewContent } from './LinkPreviewContent'
import { effectiveCard, linkPreviewStore } from './linkPreviewStore'
import { t } from '../../i18n/index'
import './linkPreview.css'

const LINK = /https?:\/\//i

registerMessageContent('link', LinkPreviewContent, {
  match: (message) =>
    message.attachment === null &&
    (message.media_kind ?? '') === '' &&
    message.recalled_at === null &&
    LINK.test(message.content),
  // Above plain text (0), below every richer kind.
  priority: 1,
})

registerMessageMenuItem('hide-link-preview', (message) => {
  const mine = message.sender_id !== null && message.sender_id === authStore.getState().session?.user.id
  const held = linkPreviewStore.getState().cards[message.message_id]
  if (!mine || !effectiveCard(message.link_preview, held)) return null
  const chatId = uiStore.getState().activeChatId
  return {
    id: 'hide-link-preview',
    label: t('w.linkPreview.270391'),
    onSelect: () => void linkPreviewApi.hide(chatId, message.message_id).catch(() => undefined),
  }
})
