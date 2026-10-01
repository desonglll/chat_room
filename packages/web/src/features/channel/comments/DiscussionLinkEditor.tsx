/**
 * TG-203 «讨论组»: a channel administrator links one of their groups as the channel's
 * discussion group (where every new post gets a comment thread) or unlinks it. The server
 * checks administration of both sides; the list here is only the viewer's groups.
 */
import { useState } from 'react'
import type { ChatType, DiscussionApi } from '@tg/core'
import { chatListStore } from '@tg/core'
import { Button } from '@tg/ui'
import { useStore } from 'zustand/react'
import { discussionApi } from './discussionApi'
import { t } from '../../../i18n/index'

export function DiscussionLinkEditor({
  chatId,
  chatType,
  myPermissions,
  api = discussionApi,
}: {
  chatId: string
  chatType: ChatType
  myPermissions: readonly string[]
  api?: DiscussionApi
}) {
  const linked = useStore(
    chatListStore,
    (state) => state.chats.find((chat) => chat.id === chatId)?.linked_chat_id ?? null,
  )
  const groups = useStore(chatListStore, (state) => state.chats).filter(
    (chat) => (chat.chat_type === 'group' || chat.chat_type === 'supergroup') && chat.id !== chatId,
  )
  const [choice, setChoice] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  if (chatType !== 'channel' || !myPermissions.includes('chat.info')) return null

  const apply = async (target: string | null) => {
    setBusy(true)
    setError('')
    try {
      await api.link(chatId, target)
      setChoice('')
    } catch {
      setError(target ? t('w.channel.dcee13') : t('w.channel.51d3cb'))
    } finally {
      setBusy(false)
    }
  }
  const linkedTitle = groups.find((group) => group.id === linked)?.title ?? t('w.channel.1efeec')

  return (
    <section className="tg-discussion-link" aria-label={t('w.channel.c6db6a')}>
      <h3 className="tg-discussion-link__title">{t('w.channel.c6db6a')}</h3>
      <p className="tg-discussion-link__note">{t('w.channel.590532')}</p>
      {linked ? (
        <div className="tg-discussion-link__row">
          <span>{linkedTitle}</span>
          <Button variant="danger" size="sm" loading={busy} onClick={() => void apply(null)}>
            {t('w.channel.e490ab')}
          </Button>
        </div>
      ) : (
        <div className="tg-discussion-link__row">
          <select aria-label={t('w.channel.b5f637')} value={choice} onChange={(event) => setChoice(event.target.value)}>
            <option value="">{t('w.channel.7299b0')}</option>
            {groups.map((group) => (
              <option key={group.id} value={group.id}>
                {group.title}
              </option>
            ))}
          </select>
          <Button size="sm" disabled={!choice} loading={busy} onClick={() => void apply(choice)}>
            {t('w.channel.dc91aa')}
          </Button>
        </div>
      )}
      {error ? <p role="alert">{error}</p> : null}
    </section>
  )
}
