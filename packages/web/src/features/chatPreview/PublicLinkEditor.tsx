/**
 * TG-206: «公开链接» in the chat admin panel (Telegram: Group Type › Public). An admin with
 * `chat.info` picks a handle; availability is checked as they type (debounced), saving turns a
 * group into a supergroup, and an empty handle makes the chat private again.
 */
import { useEffect, useState } from 'react'
import type { ChatType } from '@tg/core'
import { ApiError, publicChatPath, usernameReasonText } from '@tg/core'
import { Button, TextField } from '@tg/ui'
import { publicHandlesApi } from './publicHandlesApi'
import { t } from '../../i18n/index'

const CHECK_DELAY_MS = 400

type Check = { kind: 'idle' } | { kind: 'checking' } | { kind: 'ok' } | { kind: 'bad'; text: string }

export function PublicLinkEditor({
  chatId,
  chatType,
  current,
  myPermissions,
  api = publicHandlesApi,
}: {
  chatId: string
  chatType: ChatType
  current: string | null
  myPermissions: readonly string[]
  api?: typeof publicHandlesApi
}) {
  const [draft, setDraft] = useState(current ?? '')
  const [saved, setSaved] = useState(current ?? '')
  const [check, setCheck] = useState<Check>({ kind: 'idle' })
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const handle = draft.trim().replace(/^@/, '')

  useEffect(() => {
    if (!handle || handle.toLowerCase() === saved.toLowerCase()) {
      setCheck({ kind: 'idle' })
      return
    }
    setCheck({ kind: 'checking' })
    const timer = setTimeout(() => {
      api
        .check(handle)
        .then((result) =>
          setCheck(result.available ? { kind: 'ok' } : { kind: 'bad', text: usernameReasonText(result.reason) }),
        )
        .catch(() => setCheck({ kind: 'idle' }))
    }, CHECK_DELAY_MS)
    return () => clearTimeout(timer)
  }, [api, handle, saved])

  if (chatType === 'private' || !myPermissions.includes('chat.info')) return null

  const save = (value: string | null) => {
    setBusy(true)
    setError('')
    api
      .set(chatId, value)
      .then((chat) => {
        setSaved(chat.username ?? '')
        setDraft(chat.username ?? '')
      })
      .catch((failure: unknown) => {
        setError(usernameReasonText(failure instanceof ApiError ? failure.serverMessage : undefined))
      })
      .finally(() => setBusy(false))
  }

  return (
    <section className="tg-publiclink" aria-label={t('w.chatPreview.093bb5')}>
      <h3 className="tg-publiclink__title">{t('w.chatPreview.093bb5')}</h3>
      <TextField
        label={t('w.chatPreview.a1aaf3')}
        value={draft}
        placeholder={t('w.chatPreview.928162')}
        onChange={(event) => setDraft(event.currentTarget.value)}
        disabled={busy}
      />
      <p className="tg-publiclink__status" role="status">
        {check.kind === 'checking'
          ? t('w.chatPreview.481ee2')
          : check.kind === 'ok'
            ? t('w.chatPreview.e33f2f', handle.toLowerCase())
            : check.kind === 'bad'
              ? check.text
              : saved
                ? t('w.chatPreview.4e56ef', publicChatPath(saved))
                : t('w.chatPreview.238533')}
      </p>
      <div className="tg-publiclink__actions">
        <Button
          variant="filled"
          loading={busy}
          disabled={!handle || check.kind === 'bad' || check.kind === 'checking' || handle.toLowerCase() === saved}
          onClick={() => save(handle)}
        >
          {t('w.chatPreview.fadf24')}
        </Button>
        {saved ? (
          <Button variant="text" disabled={busy} onClick={() => save(null)}>
            {t('w.chatPreview.6ea556')}
          </Button>
        ) : null}
      </div>
      {error ? (
        <p className="tg-publiclink__status" role="alert">
          {error}
        </p>
      ) : null}
    </section>
  )
}
