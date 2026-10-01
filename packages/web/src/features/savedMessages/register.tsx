/**
 * TG-503 side-effect module, imported once by `main.tsx`: «保存到收藏夹» in every live
 * message's context menu — the quick way into Saved Messages (Telegram: forward to Saved).
 * It writes a `favorites` row (the one source of truth); Saved Messages shows it.
 */
import { registerMessageMenuItem } from '../message'
import { BookmarkGlyph } from '../message/icons'
import { favoritesApi } from './savedMessagesApi'
import { t } from '../../i18n/index'
import './savedMessages.css'

registerMessageMenuItem('save-to-favorites', (message) =>
  message.message_id.startsWith('pending:')
    ? null
    : {
        id: 'save-to-favorites',
        label: t('w.savedMessages.dd6833'),
        icon: <BookmarkGlyph />,
        onSelect: () => void favoritesApi.saveMessages([message.message_id]).catch(() => undefined),
      },
)
