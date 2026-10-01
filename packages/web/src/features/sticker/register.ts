/**
 * TG-303 side-effect module, imported once by `main.tsx`: sticker messages render through
 * the TG-103 content registry — no bubble, time overlaid, above the generic image/file
 * kinds (20/10) and below the recalled placeholder (100).
 */
import { lazy } from 'react'
import { registerMessageContent } from '../message'
import { registerSettingsPage } from '../settings/shell'
import { StickerMessage } from './message/StickerMessage'
import { isStickerMessage } from './message/stickerMessageModel'
import { t } from '../../i18n/index'
import './sticker.css'

registerMessageContent('sticker', StickerMessage, {
  match: isStickerMessage,
  frame: 'bare',
  metaPlacement: 'overlay',
  priority: 30,
})

// Sticker-set management in 设置 › 外观 (Telegram: Chat Settings › Stickers and Emoji).
registerSettingsPage({
  id: 'appearance.stickers',
  section: 'appearance',
  get title() {
    return t('w.sticker.4be31b')
  },
  order: 50,
  component: lazy(() => import('./manage/StickerSetsSettings').then((m) => ({ default: m.StickerSetsSettings }))),
})
