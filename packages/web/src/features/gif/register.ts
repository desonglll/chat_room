/**
 * TG-305 side-effect module, imported once by `main.tsx`:
 *  - GIF messages render through the TG-103 content registry: borderless like a photo,
 *    time overlaid, above image/video (20) and below stickers (30) and the recalled
 *    placeholder (100);
 *  - the media panel's GIF tab replaces TG-303's placeholder;
 *  - "保存 GIF" joins the message context menu for GIF messages.
 */
import { createElement } from 'react'
import { registerMessageContent, registerMessageMenuItem } from '../message'
import { registerMediaPanelTab } from '../sticker/panel/mediaPanelTabs'
import { saveGifMenuItem } from './message/saveGifMenuItem'
import { GifMessage } from './message/GifMessage'
import { isGifMessage } from './gifModel'
import { LazyGifTab } from './panel/LazyGifTab'
import './gif.css'

const hasCaption = (message: { content: string }) => message.content.trim() !== ''

registerMessageContent('gif', GifMessage, {
  match: isGifMessage,
  frame: (message) => (hasCaption(message) ? 'bubble' : 'media'),
  metaPlacement: (message) => (hasCaption(message) ? 'inline' : 'overlay'),
  leadingMedia: true,
  priority: 25,
})

registerMediaPanelTab({
  id: 'gif',
  label: 'GIF',
  order: 30,
  render: (context) => createElement(LazyGifTab, context),
})

registerMessageMenuItem('save-gif', saveGifMenuItem)
