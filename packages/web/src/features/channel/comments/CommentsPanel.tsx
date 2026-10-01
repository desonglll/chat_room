/**
 * TG-203: one post's comment thread — read from the channel, written into its discussion group.
 * Non-members read; joining the group (one tap for an open group) unlocks the composer.
 */
import { useCallback, useEffect, useMemo, useState } from 'react'
import type { CommentThread, DiscussionApi } from '@tg/core'
import { Button, TextField } from '@tg/ui'
import { replyTarget } from './commentsModel'

export interface CommentsPanelProps {
  channelId: string
  postId: string
  api: DiscussionApi
  /** Reports the live count back to the post's entry. */
  onCount?: ((count: number) => void) | undefined
}

export function CommentsPanel({ channelId, postId, api, onCount }: CommentsPanelProps) {
  const [thread, setThread] = useState<CommentThread | null>(null)
  const [failed, setFailed] = useState(false)
  const [draft, setDraft] = useState('')
  const [replyTo, setReplyTo] = useState<{ id: string; sender: string } | null>(null)
  const [busy, setBusy] = useState(false)

  const load = useCallback(
    () =>
      api.thread(channelId, postId).then(
        (next) => {
          setThread(next)
          setFailed(false)
          onCount?.(next.count)
        },
        () => setFailed(true),
      ),
    [api, channelId, postId, onCount],
  )
  useEffect(() => void load(), [load])

  const byId = useMemo(
    () => new Map((thread?.messages ?? []).map((message) => [message.id, message] as const)),
    [thread],
  )

  const send = async () => {
    const content = draft.trim()
    if (!content || !thread) return
    setBusy(true)
    try {
      await api.comment(channelId, postId, {
        content,
        ...(replyTo ? { reply_to: replyTo.id } : {}),
        client_message_id: crypto.randomUUID(),
      })
      setDraft('')
      setReplyTo(null)
      await load()
    } catch {
      setFailed(true)
    } finally {
      setBusy(false)
    }
  }
  const join = async () => {
    if (!thread) return
    setBusy(true)
    try {
      await api.join(thread.discussion_chat_id)
      await load()
    } catch {
      setFailed(true)
    } finally {
      setBusy(false)
    }
  }

  if (thread === null) {
    return <p className="tg-comments__empty">{failed ? '评论加载失败' : '正在加载评论…'}</p>
  }
  return (
    <div className="tg-comments">
      {thread.messages.length === 0 ? (
        <p className="tg-comments__empty">还没有评论</p>
      ) : (
        <ul className="tg-comments__list" aria-label="评论">
          {thread.messages.map((message) => {
            const answers = replyTarget(message.reply_to?.message_id, thread.discussion_message_id, byId)
            return (
              <li key={message.id} className="tg-comments__item">
                <span className="tg-comments__sender">{message.sender}</span>
                {answers ? <span className="tg-comments__answers">回复 {answers}</span> : null}
                <p className="tg-comments__text">{message.recalled_at ? '评论已删除' : message.content}</p>
                {thread.can_comment && !message.recalled_at ? (
                  <button
                    type="button"
                    className="tg-comments__reply"
                    onClick={() => setReplyTo({ id: message.id, sender: message.sender })}
                  >
                    回复
                  </button>
                ) : null}
              </li>
            )
          })}
        </ul>
      )}
      {failed ? <p role="alert">操作失败，请重试</p> : null}
      {thread.can_comment ? (
        <form
          className="tg-comments__composer"
          onSubmit={(event) => {
            event.preventDefault()
            void send()
          }}
        >
          {replyTo ? (
            <p className="tg-comments__replying">
              回复 {replyTo.sender}
              <button type="button" className="tg-comments__reply" onClick={() => setReplyTo(null)}>
                取消
              </button>
            </p>
          ) : null}
          <TextField
            aria-label="写评论"
            placeholder="写评论…"
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
          />
          <Button type="submit" disabled={!draft.trim()} loading={busy}>
            发送
          </Button>
        </form>
      ) : (
        <Button variant="tonal" loading={busy} onClick={() => void join()}>
          加入讨论组以评论
        </Button>
      )}
    </div>
  )
}
