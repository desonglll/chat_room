/**
 * TG-303 side-effect module, imported once by `main.tsx`: sticker messages render through
 * the TG-103 content registry — no bubble, time overlaid, above the generic image/file
 * kinds (20/10) and below the recalled placeholder (100).
 */
import { registerMessageContent } from '../message'
import { StickerMessage } from './message/StickerMessage'
import { isStickerMessage } from './message/stickerMessageModel'
import './sticker.css'

registerMessageContent('sticker', StickerMessage, {
  match: isStickerMessage,
  frame: 'bare',
  metaPlacement: 'overlay',
  priority: 30,
})
