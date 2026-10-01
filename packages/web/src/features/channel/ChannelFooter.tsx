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
import { t } from '../../i18n/index'
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
    <div className="tg-channel-footer" role="group" aria-label={t('w.channel.7e1aaf')}>
      {pending ? (
        <p className="tg-channel-footer__note">{t('w.channel.915b94')}</p>
      ) : subscribed ? (
        <>
          <Button variant="text" className="tg-channel-footer__main" loading={busy} onClick={props.onToggleMute}>
            {muted ? t('w.channel.59bc75') : t('w.channel.afdbd1')}
          </Button>
          <Button variant="text" className="tg-channel-footer__leave" disabled={busy} onClick={props.onLeave}>
            {t('w.channel.fc6c92')}
          </Button>
        </>
      ) : (
        <Button variant="text" fullWidth loading={busy} onClick={props.onSubscribe}>
          {t('w.channel.5319af')}
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
        run(async () => setPending((await subscribeToChannel(deps, chatId)) === 'pending'), t('w.channel.3ac829'))
      }
      onToggleMute={() =>
        run(() => setChannelMuted(deps, chatId, !(row && isConversationMuted(row, Date.now()))), t('w.channel.51d3cb'))
      }
      onLeave={() =>
        run(async () => {
          await leaveChannel(deps, chatId)
          void navigate('/')
        }, t('w.channel.c87b60'))
      }
    />
  )
}
