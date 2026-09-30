/**
 * Telegram's emoji / sticker / GIF panel: three tabs on top, one body below. The emoji body
 * is the composer's own picker (passed in as `emoji`), 贴纸 is `StickerTab`, and GIF is
 * whatever `registerMediaPanelTab({ id: 'gif' })` supplied — a placeholder until TG-305.
 *
 * Only the active tab is mounted, so opening the panel on 表情 never touches the sticker
 * library, and the last tab is remembered for the session like Telegram's.
 */
import { useState, type ReactNode } from 'react'
import type { Sticker } from '@tg/core'
import { Tabs, type TabItem } from '@tg/ui'
import { useStore } from 'zustand/react'
import { mediaPanelTabStore, type MediaPanelTabContext } from './mediaPanelTabs'
import { StickerTab } from './StickerTab'
import '../sticker.css'
import './panel.css'

export interface MediaPanelProps {
  chatId: string
  /** The emoji picker body. */
  emoji: ReactNode
  onSendSticker(sticker: Sticker): void
  canSend: boolean
  onClose(): void
  initialTab?: string | undefined
}

let lastTab = 'emoji'

function GifPlaceholder() {
  return <p className="tg-sticker-tab__state">GIF 即将推出</p>
}

export function MediaPanel({ chatId, emoji, onSendSticker, canSend, onClose, initialTab }: MediaPanelProps) {
  const extra = useStore(mediaPanelTabStore, (state) => state.tabs)
  const [tab, setTab] = useState(initialTab ?? lastTab)
  const context: MediaPanelTabContext = { chatId, canSend, close: onClose }

  const bodies: Record<string, ReactNode> = {
    emoji,
    stickers: <StickerTab onSend={onSendSticker} canSend={canSend} />,
  }
  const items: TabItem[] = [
    { id: 'emoji', label: '表情' },
    { id: 'stickers', label: '贴纸' },
  ]
  const tabs = extra.some((entry) => entry.id === 'gif')
    ? extra
    : [...extra, { id: 'gif', label: 'GIF', order: 30, render: () => <GifPlaceholder /> }]
  for (const entry of tabs) {
    if (entry.id === 'emoji' || entry.id === 'stickers') continue
    items.push({ id: entry.id, label: entry.label })
    if (entry.id === tab) bodies[entry.id] = entry.render(context)
  }
  const current = items.some((item) => item.id === tab) ? tab : 'emoji'

  return (
    <div className="tg-media-panel">
      <Tabs
        items={items}
        value={current}
        onValueChange={(id) => {
          lastTab = id
          setTab(id)
        }}
        stretch
        aria-label="表情与贴纸"
        className="tg-media-panel__tabs"
        panels={{ [current]: <div className="tg-media-panel__body">{bodies[current]}</div> }}
      />
    </div>
  )
}
