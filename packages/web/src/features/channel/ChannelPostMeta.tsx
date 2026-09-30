/**
 * A channel post's part of the bubble meta: author signature and view count, before the time.
 * Rendered by TG-103's `MessageMeta` in both of its copies (visible and spacer), from the same
 * store values, so the two stay the same width. Only the visible copy reports the view.
 */
import { useEffect } from 'react'
import { useStore } from 'zustand/react'
import { uiStore } from '@tg/core'
import type { ChannelPostParts } from './channelModel'
import { formatViews } from './channelModel'
import { channelStore, effectiveViews } from './channelStore'
import { postViewReporter } from './postViews'
import './channel.css'

function EyeGlyph() {
  return (
    <svg className="tg-channel-post__eye" viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
      <path
        fill="currentColor"
        d="M8 3.5c3 0 5.4 2 6.4 4.5-1 2.5-3.4 4.5-6.4 4.5S2.6 10.5 1.6 8C2.6 5.5 5 3.5 8 3.5Zm0 1.6A2.9 2.9 0 1 0 8 10.9 2.9 2.9 0 0 0 8 5.1Zm0 1.5a1.4 1.4 0 1 1 0 2.8 1.4 1.4 0 0 1 0-2.8Z"
      />
    </svg>
  )
}

export function ChannelPostMeta({ post, report = false }: { post: ChannelPostParts; report?: boolean }) {
  const held = useStore(channelStore, (state) => state.views[post.messageId])
  const heldAuthor = useStore(channelStore, (state) => state.authors[post.messageId])
  const chatId = useStore(uiStore, (state) => state.activeChatId)

  useEffect(() => {
    if (report && chatId) postViewReporter.seen(chatId, post.messageId)
  }, [report, chatId, post.messageId])
  // A later copy of the frame may lack the signature (a direct broadcast); keep the first one.
  useEffect(() => {
    if (post.author) channelStore.getState().rememberAuthor(post.messageId, post.author)
  }, [post.messageId, post.author])

  const author = post.author || heldAuthor || ''
  return (
    <>
      {author ? <span className="tg-channel-post__author">{author}</span> : null}
      <span className="tg-channel-post__views">
        <EyeGlyph />
        {formatViews(effectiveViews(post.views, held))}
      </span>
    </>
  )
}
