/**
 * TG-206 `/public/:username` — Telegram's `t.me/<username>` for a public group or channel: the
 * preview a visitor may see (title, type, member count, description) and one button — join,
 * request to join, or open the chat for a member. A non-member never learns the chat's id;
 * joining goes through the handle, and the preview is re-read to open the chat.
 */
import { useEffect, useState } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import type { PublicChatPreview } from '@tg/core'
import { authStore, chatListStore, selectToken } from '@tg/core'
import { Avatar, Button } from '@tg/ui'
import { apiClient } from '../../app/client'
import { loadChatList } from '../chatList/chatListController'
import { publicHandlesApi } from './publicHandlesApi'
import { t } from '../../i18n/index'

export interface PublicChatRouteDeps {
  preview(username: string): Promise<PublicChatPreview>
  join(username: string): Promise<unknown>
  refreshChats(): Promise<void>
}

const defaultDeps: PublicChatRouteDeps = {
  preview: (username) => publicHandlesApi.preview(username),
  join: (username) => publicHandlesApi.join(username),
  refreshChats: () =>
    loadChatList({ client: apiClient, token: selectToken(authStore.getState()), store: chatListStore }),
}

type State =
  | { kind: 'loading' }
  | { kind: 'missing' }
  | { kind: 'preview'; preview: PublicChatPreview; busy: boolean; requested: boolean; error: string }

const TYPE_LABEL: Record<string, string> = {
  get channel() {
    return t('w.chatPreview.b76dfd')
  },
  get supergroup() {
    return t('w.chatPreview.a06237')
  },
  get group() {
    return t('w.chatPreview.4260ca')
  },
}

export function PublicChatRoute({ deps = defaultDeps }: { deps?: PublicChatRouteDeps }) {
  const { username = '' } = useParams()
  const navigate = useNavigate()
  const [state, setState] = useState<State>({ kind: 'loading' })

  useEffect(() => {
    let alive = true
    setState({ kind: 'loading' })
    deps.preview(username).then(
      (preview) => alive && setState({ kind: 'preview', preview, busy: false, requested: false, error: '' }),
      () => alive && setState({ kind: 'missing' }),
    )
    return () => {
      alive = false
    }
  }, [deps, username])

  if (state.kind === 'loading') return <div className="tg-public" aria-busy="true" />
  if (state.kind === 'missing') {
    return (
      <div className="tg-public">
        <section className="tg-public__card" aria-label={t('w.chatPreview.093bb5')}>
          <p className="tg-public__title">
            {t('w.chatPreview.57811c')}
            {username}
          </p>
          <p className="tg-public__meta">{t('w.chatPreview.ff5181')}</p>
        </section>
      </div>
    )
  }

  const { preview } = state
  const open = async () => {
    if (preview.is_member && preview.chat_id) {
      navigate(`/chat/${preview.chat_id}`, { replace: true })
      return
    }
    setState({ ...state, busy: true, error: '' })
    try {
      await deps.join(username)
      const joined = await deps.preview(username)
      if (joined.is_member && joined.chat_id) {
        await deps.refreshChats().catch(() => undefined)
        navigate(`/chat/${joined.chat_id}`, { replace: true })
      } else {
        setState({ kind: 'preview', preview: joined, busy: false, requested: true, error: '' })
      }
    } catch {
      setState({ ...state, busy: false, error: t('w.chatPreview.954537') })
    }
  }
  const members =
    preview.chat_type === 'channel'
      ? t('w.chatPreview.2b75a3', preview.member_count)
      : t('w.chatPreview.0ece20', preview.member_count)
  const action = preview.is_member
    ? preview.chat_type === 'channel'
      ? t('w.chatPreview.6a20f5')
      : t('w.chatPreview.004846')
    : preview.requires_approval
      ? t('w.chatPreview.527fa6')
      : preview.chat_type === 'channel'
        ? t('w.chatPreview.5319af')
        : t('w.chatPreview.9542b3')

  return (
    <div className="tg-public">
      <section className="tg-public__card" aria-label={`@${preview.username}`}>
        <Avatar label={preview.title} initials={preview.avatar_emoji || undefined} size={96} />
        <p className="tg-public__title">{preview.title}</p>
        <p className="tg-public__meta">
          {TYPE_LABEL[preview.chat_type] ?? t('w.chatPreview.4260ca')} · {members} · @{preview.username}
        </p>
        {preview.description ? <p className="tg-public__description">{preview.description}</p> : null}
        {state.requested ? (
          <p className="tg-public__meta" role="status">
            {t('w.chatPreview.ac52a2')}
          </p>
        ) : (
          <Button variant="filled" loading={state.busy} onClick={() => void open()}>
            {action}
          </Button>
        )}
        {state.error ? (
          <p className="tg-public__meta" role="alert">
            {state.error}
          </p>
        ) : null}
      </section>
    </div>
  )
}
