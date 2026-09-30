/**
 * Dev fixture page: a chat of 24 media where only the newest 6 are "loaded" (the seed); the
 * rest arrive through a fake `/files` backend with latency, exercising the pager.
 */
import '@tg/ui/styles.css'
import '../../message/message.css'
import '../mediaViewer.css'
import './fixtures.css'
import { useState } from 'react'
import { createRoot } from 'react-dom/client'
import { messageStore } from '@tg/core'
import { Button } from '@tg/ui'
import { MessageBubble } from '../../message/MessageBubble'
import { MediaViewer } from '../MediaViewer'
import { openMediaViewer } from '../mediaViewerStore'
import { toMediaPage } from '../chatMediaSource'
import type { FetchMediaPage } from '../mediaPager'
import { FIXTURE_CHAT, fixtureMedia } from './fixtureMedia'

const hasVideo = new URLSearchParams(location.search).has('video')
const ALL = fixtureMedia(24, new Set(hasVideo ? [21, 10] : []), new Set([19]))
const LOADED = ALL.slice(-6)
for (const { message } of LOADED) messageStore.getState().applyBroadcast(FIXTURE_CHAT, message, 'none')

const PAGE = 6
const createFetcher = (): FetchMediaPage => async (before) => {
  await new Promise((resolve) => setTimeout(resolve, 300))
  const newestFirst = [...ALL].reverse().map((entry) => entry.file)
  const start = before === null ? 0 : newestFirst.findIndex((file) => file.message_id === before) + 1
  const items = newestFirst.slice(start, start + PAGE)
  const more = start + PAGE < newestFirst.length
  return toMediaPage({ items, next_before: more ? (items.at(-1)?.message_id ?? null) : null })
}

function Page() {
  const [log, setLog] = useState('')
  const [theme, setTheme] = useState<'day' | 'night'>('day')
  return (
    <>
      <div className="fx-toolbar">
        <Button
          size="sm"
          variant="tonal"
          onClick={() => {
            const next = theme === 'day' ? 'night' : 'day'
            document.documentElement.setAttribute('data-tg-theme', next)
            setTheme(next)
          }}
        >
          {theme === 'day' ? '夜间' : '日间'}
        </Button>
        <span className="fx-log" data-testid="log">
          {log}
        </span>
      </div>
      <div className="fx-list">
        {LOADED.map(({ message }, index) => (
          <MessageBubble
            key={message.message_id}
            message={message}
            ctx={{
              groupPosition: 'single',
              isOutgoing: index % 2 === 0,
              showAvatar: false,
              showSenderName: false,
              highlighted: false,
              selected: false,
            }}
            actions={{
              onOpenMedia: (attachmentId) => openMediaViewer({ chatId: FIXTURE_CHAT, attachmentId }),
            }}
          />
        ))}
      </div>
      <MediaViewer
        createFetcher={createFetcher}
        actions={{
          onForward: (item) => setLog(`forward ${item.messageId}`),
          onDelete: async (item) => {
            setLog(`delete ${item.messageId}`)
            return true
          },
        }}
      />
    </>
  )
}

const host = document.getElementById('fixtures')
if (host === null) throw new Error('fixtures: #fixtures is missing from index.html')
createRoot(host).render(<Page />)
