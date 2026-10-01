/**
 * The "话题" switch of the admin panel (TG-204): turns the chat into a forum (the server
 * upgrades a group to a supergroup) or back. Shown to holders of `chat.info`; the answered
 * chat descriptor goes into the chat list, which re-routes the open chat pane.
 */
import { useState } from 'react'
import type { ChatType, TopicsApi } from '@tg/core'
import { chatListStore, selectChatById } from '@tg/core'
import { Toggle } from '@tg/ui'
import { useStore } from 'zustand/react'
import { topicErrorText } from './topicListModel'
import { topicsApi } from './topicsApi'
import { t } from '../../i18n/index'
import './forum.css'

export interface ForumToggleProps {
  chatId: string
  chatType: ChatType
  myPermissions: readonly string[]
  api?: TopicsApi | undefined
  /** Test seed when the chat list holds no descriptor. */
  initialForum?: boolean | undefined
}

export function canToggleForum(chatType: ChatType, myPermissions: readonly string[]): boolean {
  return (chatType === 'group' || chatType === 'supergroup') && myPermissions.includes('chat.info')
}

export function ForumToggle({ chatId, chatType, myPermissions, api = topicsApi, initialForum }: ForumToggleProps) {
  const stored = useStore(chatListStore, (state) => selectChatById(chatId)(state)?.is_forum)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  if (!canToggleForum(chatType, myPermissions)) return null
  const enabled = stored ?? initialForum ?? false

  const change = (next: boolean) => {
    setBusy(true)
    setError('')
    api.setForum(chatId, next).then(
      (chat) => {
        chatListStore.getState().upsertChat(chat)
        setBusy(false)
      },
      (caught: unknown) => {
        setError(topicErrorText(caught))
        setBusy(false)
      },
    )
  }

  return (
    <div className="tg-forum-toggle">
      <Toggle
        label={t('w.forum.df999a')}
        description={t('w.forum.1dab4a')}
        checked={enabled}
        disabled={busy}
        onCheckedChange={change}
      />
      {error ? (
        <p className="tg-forum__error" role="alert">
          {error}
        </p>
      ) : null}
    </div>
  )
}
