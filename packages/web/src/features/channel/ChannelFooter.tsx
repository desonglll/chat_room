/**
 * What a channel shows in place of the composer for anyone who may not post (Telegram's
 * bottom bar): «订阅» for a visitor, «静音 / 取消静音» plus «退订» for a subscriber.
 */
import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { useStore } from 'zustand/react'
import { authStore, chatListStore, isConversationMuted, selectChatById, selectToken } from '@tg/core'
import { Button } from '@tg/ui'
import { apiClient } from '../../app/client'
import { channelApi } from './channelApi'
import { leaveChannel, setChannelMuted, subscribeToChannel } from './channelActions'
import './channel.css'

export interface ChannelFooterViewProps {
  subscribed: boolean
  muted: boolean
  pending: boolean
  busy: boolean
  error: string
  onSubscribe: () => void
  onToggleMute: () => void
  onLeave: () => void
}

export function ChannelFooterView(props: ChannelFooterViewProps) {
  const { subscribed, muted, pending, busy, error } = props
  return (
    <div className="tg-channel-footer" role="group" aria-label="频道操作">
      {pending ? (
        <p className="tg-channel-footer__note">订阅申请已提交，等待管理员审核</p>
      ) : subscribed ? (
        <>
          <Button variant="text" className="tg-channel-footer__main" loading={busy} onClick={props.onToggleMute}>
            {muted ? '取消静音' : '静音'}
          </Button>
          <Button variant="text" className="tg-channel-footer__leave" disabled={busy} onClick={props.onLeave}>
            退订
          </Button>
        </>
      ) : (
        <Button variant="text" fullWidth loading={busy} onClick={props.onSubscribe}>
          订阅
        </Button>
      )}
      {error ? (
        <p className="tg-channel-footer__error" role="alert">
          {error}
        </p>
      ) : null}
    </div>
  )
}

export function ChannelFooter({ chatId }: { chatId: string }) {
  const token = useStore(authStore, selectToken)
  const chat = useStore(chatListStore, selectChatById(chatId))
  const row = useStore(chatListStore, (state) => state.conversations.find((entry) => entry.room_id === chatId))
  const [busy, setBusy] = useState(false)
  const [pending, setPending] = useState(false)
  const [error, setError] = useState('')
  const navigate = useNavigate()
  const deps = { api: channelApi, client: apiClient, token, store: chatListStore }
  const run = (action: () => Promise<unknown>, failure: string) => {
    setBusy(true)
    setError('')
    action()
      .catch(() => setError(failure))
      .finally(() => setBusy(false))
  }

  return (
    <ChannelFooterView
      subscribed={chat?.membership_status === 'active'}
      muted={row ? isConversationMuted(row, Date.now()) : false}
      pending={pending}
      busy={busy}
      error={error}
      onSubscribe={() =>
        run(async () => setPending((await subscribeToChannel(deps, chatId)) === 'pending'), '订阅失败，请稍后再试')
      }
      onToggleMute={() =>
        run(() => setChannelMuted(deps, chatId, !(row && isConversationMuted(row, Date.now()))), '操作失败，请重试')
      }
      onLeave={() =>
        run(async () => {
          await leaveChannel(deps, chatId)
          void navigate('/')
        }, '退订失败，请稍后再试')
      }
    />
  )
}
