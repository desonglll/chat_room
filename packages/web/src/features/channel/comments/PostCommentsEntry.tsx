/**
 * TG-203: the «N 条评论» strip under a channel post that has a comment thread, opening the
 * thread in a side sheet. Rendered by the bubble only when the post carries `comments`.
 */
import { useState } from 'react'
import type { BroadcastMessage } from '@tg/core'
import { uiStore } from '@tg/core'
import { Sheet } from '@tg/ui'
import { useStore } from 'zustand/react'
import { commentsLabel } from './commentsModel'
import { CommentsPanel } from './CommentsPanel'
import { discussionApi } from './discussionApi'
import './comments.css'

export function PostCommentsEntry({ message }: { message: BroadcastMessage }) {
  const channelId = useStore(uiStore, (state) => state.activeChatId)
  const [open, setOpen] = useState(false)
  const [count, setCount] = useState<number | null>(null)
  if (message.comments === undefined || !channelId || message.recalled_at) return null
  const shown = count ?? message.comments
  return (
    <>
      <button type="button" className="tg-comments-entry" onClick={() => setOpen(true)}>
        <span aria-hidden="true">💬</span>
        <span>{commentsLabel(shown)}</span>
      </button>
      <Sheet open={open} side="right" title="评论" onClose={() => setOpen(false)} className="tg-comments__sheet">
        {open ? (
          <CommentsPanel channelId={channelId} postId={message.message_id} api={discussionApi} onCount={setCount} />
        ) : null}
      </Sheet>
    </>
  )
}
