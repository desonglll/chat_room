/**
 * TG-503 `/saved` — Saved Messages: the viewer's favorites read like a chat with themselves
 * (oldest first, a note input at the bottom). It is a projection of `favorites` (D-010), so
 * everything the old «收藏» screen did still works on the same rows; nothing is copied.
 * Each entry can be forwarded, deleted, or opened at its source when the viewer still can.
 */
import { useCallback, useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import type { FavoriteItem } from '@tg/core'
import { chatListStore, savedMessagesTimeline, savedSourceLine } from '@tg/core'
import { Button, Modal, ScrollArea, TextField } from '@tg/ui'
import { useStore } from 'zustand/react'
import { linkify } from '../message/content/linkify'
import { favoritesApi } from './savedMessagesApi'
import { t } from '../../i18n/index'

function time(iso: string): string {
  const date = new Date(iso)
  return Number.isNaN(date.getTime())
    ? ''
    : t(
        'w.savedMessages.584530',
        date.getMonth() + 1,
        date.getDate(),
        String(date.getHours()).padStart(2, '0'),
        String(date.getMinutes()).padStart(2, '0'),
      )
}

function Text({ text }: { text: string }) {
  return (
    <p className="tg-saved__text">
      {linkify(text).map((segment, index) =>
        segment.type === 'link' ? (
          <a key={index} href={segment.href} target="_blank" rel="noreferrer noopener">
            {segment.text}
          </a>
        ) : (
          <span key={index}>{segment.text}</span>
        ),
      )}
    </p>
  )
}

function SavedEntry({
  item,
  onDelete,
  onForward,
  onOpenSource,
}: {
  item: FavoriteItem
  onDelete(): void
  onForward(): void
  onOpenSource?: (() => void) | undefined
}) {
  const source = savedSourceLine(item)
  const attachment = item.attachment
  const image = attachment?.mime_type.startsWith('image/')
  return (
    <li className="tg-saved__entry">
      <article className="tg-saved__bubble">
        {source ? <p className="tg-saved__source">{source}</p> : null}
        {attachment ? (
          image ? (
            <img className="tg-saved__image" src={attachment.download_url} alt={attachment.file_name} loading="lazy" />
          ) : (
            <a className="tg-saved__file" href={attachment.download_url} download={attachment.file_name}>
              📎 {attachment.file_name}
            </a>
          )
        ) : null}
        {item.content ? <Text text={item.content} /> : null}
        <footer className="tg-saved__meta">
          <span>{time(item.created_at)}</span>
          <span className="tg-saved__actions">
            {onOpenSource ? (
              <button type="button" onClick={onOpenSource}>
                {t('w.savedMessages.8ac74f')}
              </button>
            ) : null}
            <button type="button" onClick={onForward}>
              {t('w.savedMessages.0d5a8a')}
            </button>
            <button type="button" onClick={onDelete}>
              {t('w.savedMessages.3755f5')}
            </button>
          </span>
        </footer>
      </article>
    </li>
  )
}

export function SavedMessagesRoute({ api = favoritesApi }: { api?: typeof favoritesApi }) {
  const navigate = useNavigate()
  const conversations = useStore(chatListStore, (state) => state.conversations)
  const [items, setItems] = useState<FavoriteItem[] | null>(null)
  const [note, setNote] = useState('')
  const [busy, setBusy] = useState(false)
  const [forwarding, setForwarding] = useState<FavoriteItem | null>(null)
  const [notice, setNotice] = useState('')

  const reload = useCallback(() => {
    api.list().then(
      (list) => setItems(savedMessagesTimeline(list)),
      () => setItems([]),
    )
  }, [api])
  useEffect(reload, [reload])

  const addNote = () => {
    const content = note.trim()
    if (!content || busy) return
    setBusy(true)
    api
      .addNote(content)
      .then((created) => {
        setNote('')
        setItems((current) => savedMessagesTimeline([...(current ?? []), created]))
      })
      .finally(() => setBusy(false))
  }

  const remove = (item: FavoriteItem) =>
    void api.remove(item.id).then(() => setItems((current) => (current ?? []).filter((entry) => entry.id !== item.id)))

  const forwardTo = (chatId: string) => {
    if (!forwarding) return
    const target = forwarding
    setForwarding(null)
    api.forward(target.id, [chatId]).then(
      (results) =>
        setNotice(
          results.some((result) => result.forwarded_message_id)
            ? t('w.savedMessages.35f8b3')
            : t('w.savedMessages.c3ef13'),
        ),
      () => setNotice(t('w.savedMessages.d27604')),
    )
  }

  return (
    <section className="tg-saved" aria-label={t('w.savedMessages.e6f497')}>
      <header className="tg-saved__header">
        <h2 className="tg-saved__title">{t('w.savedMessages.e6f497')}</h2>
        {notice ? (
          <span className="tg-saved__notice" role="status">
            {notice}
          </span>
        ) : null}
      </header>
      <ScrollArea className="tg-saved__scroll" orientation="vertical">
        {items === null ? null : items.length === 0 ? (
          <p className="tg-saved__empty">{t('w.savedMessages.18a6aa')}</p>
        ) : (
          <ul className="tg-saved__list">
            {items.map((item) => (
              <SavedEntry
                key={item.id}
                item={item}
                onDelete={() => remove(item)}
                onForward={() => setForwarding(item)}
                onOpenSource={
                  item.source_room_id && item.source_message_id
                    ? () =>
                        void navigate(
                          `/chat/${encodeURIComponent(item.source_room_id ?? '')}?message=${encodeURIComponent(item.source_message_id ?? '')}`,
                        )
                    : undefined
                }
              />
            ))}
          </ul>
        )}
      </ScrollArea>
      <form
        className="tg-saved__compose"
        onSubmit={(event) => {
          event.preventDefault()
          addNote()
        }}
      >
        <TextField
          label={t('w.savedMessages.089648')}
          value={note}
          onChange={(event) => setNote(event.currentTarget.value)}
          fullWidth
        />
        <Button variant="filled" type="submit" loading={busy} disabled={!note.trim()}>
          {t('w.savedMessages.fadf24')}
        </Button>
      </form>
      <Modal
        open={forwarding !== null}
        onClose={() => setForwarding(null)}
        title={t('w.savedMessages.3699f8')}
        size="sm"
      >
        <ul className="tg-saved__picker">
          {conversations.map((row) => (
            <li key={row.room_id}>
              <button type="button" onClick={() => forwardTo(row.room_id)}>
                {row.alias || row.title}
              </button>
            </li>
          ))}
        </ul>
      </Modal>
    </section>
  )
}
