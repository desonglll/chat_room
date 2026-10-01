/**
 * TG-1203: the «消息署名» switch of a channel's admin panel. TG-202 offered it only in the
 * creation dialog, so a channel created without signatures could never turn them on (Telegram
 * keeps it in the channel's settings). Shown to holders of `chat.info`, which is what the server
 * requires; the answered descriptor goes into the chat list.
 */
import { useState } from 'react'
import type { ChannelApi, ChatType } from '@tg/core'
import { chatListStore, selectChatById } from '@tg/core'
import { Toggle } from '@tg/ui'
import { useStore } from 'zustand/react'
import { channelApi } from './channelApi'
import { t } from '../../i18n/index'
import './channel.css'

export interface ChannelSignaturesToggleProps {
  chatId: string
  chatType: ChatType
  myPermissions: readonly string[]
  api?: Pick<ChannelApi, 'setSignatures'> | undefined
  /** Test seed when the chat list holds no descriptor. */
  initialEnabled?: boolean | undefined
}

export function canToggleSignatures(chatType: ChatType, myPermissions: readonly string[]): boolean {
  return chatType === 'channel' && myPermissions.includes('chat.info')
}

export function ChannelSignaturesToggle({
  chatId,
  chatType,
  myPermissions,
  api = channelApi,
  initialEnabled,
}: ChannelSignaturesToggleProps) {
  const stored = useStore(chatListStore, (state) => selectChatById(chatId)(state)?.signatures_enabled)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  if (!canToggleSignatures(chatType, myPermissions)) return null
  const enabled = stored ?? initialEnabled ?? false

  const change = (next: boolean) => {
    setBusy(true)
    setError('')
    api.setSignatures(chatId, next).then(
      (chat) => {
        chatListStore.getState().upsertChat(chat)
        setBusy(false)
      },
      () => {
        setError(t('w.channel.51d3cb'))
        setBusy(false)
      },
    )
  }

  return (
    <div className="tg-channel-signatures">
      <Toggle
        label={t('w.channel.b90326')}
        description={t('w.channel.f844d0')}
        checked={enabled}
        disabled={busy}
        onCheckedChange={change}
      />
      {error ? (
        <p className="tg-channel-footer__error" role="alert">
          {error}
        </p>
      ) : null}
    </div>
  )
}
