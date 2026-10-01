/**
 * TG-404 «定时消息» — the author's scheduled messages in one chat, grouped by day, each with
 * «立即发送», «更改时间», «编辑» and «删除». Opened from the composer's calendar entry.
 */
import { useEffect, useState } from 'react'
import { useStore } from 'zustand/react'
import type { ScheduledMessage } from '@tg/core'
import { Button, Modal, Spinner, TextField } from '@tg/ui'
import { ScheduleDialog } from './ScheduleDialog'
import { scheduledErrorText } from './scheduledActions'
import { scheduledStore } from './scheduledStore'
import { formatScheduleClock, groupByDay } from './scheduleTime'
import { scheduledActions } from './useScheduled'
import { t } from '../../i18n/index'

export interface ScheduledMessagesDialogProps {
  open: boolean
  chatId: string
  onClose(): void
}

const EMPTY: readonly ScheduledMessage[] = []

interface RowProps {
  item: ScheduledMessage
  onError(message: string): void
  onReschedule(item: ScheduledMessage): void
}

function ScheduledRow({ item, onError, onReschedule }: RowProps) {
  const [editing, setEditing] = useState<string | null>(null)
  const [confirmDelete, setConfirmDelete] = useState(false)
  const [busy, setBusy] = useState(false)
  const run = (action: Promise<unknown>) => {
    setBusy(true)
    action.catch((caught: unknown) => onError(scheduledErrorText(caught))).finally(() => setBusy(false))
  }
  const saveEdit = () => {
    const content = (editing ?? '').trim()
    if (!content) return
    run(scheduledActions.update(item.chat_id, item.id, { content }).then(() => setEditing(null)))
  }

  return (
    <li className="tg-scheduled__item" aria-busy={busy || undefined}>
      <div className="tg-scheduled__meta">
        <time dateTime={item.scheduled_at}>{formatScheduleClock(new Date(item.scheduled_at))}</time>
        {item.silent ? <span className="tg-scheduled__silent">{t('w.scheduled.76b7a1')}</span> : null}
      </div>
      {editing === null ? (
        <p className="tg-scheduled__text">{item.content}</p>
      ) : (
        <TextField
          multiline
          rows={3}
          aria-label={t('w.scheduled.bb6278')}
          value={editing}
          fullWidth
          onChange={(event) => setEditing(event.target.value)}
        />
      )}
      <div className="tg-scheduled__actions">
        {editing !== null ? (
          <>
            <Button size="sm" variant="text" onClick={() => setEditing(null)}>
              {t('w.scheduled.4d0b46')}
            </Button>
            <Button size="sm" onClick={saveEdit} disabled={busy || !editing.trim()}>
              {t('w.scheduled.fadf24')}
            </Button>
          </>
        ) : confirmDelete ? (
          <>
            <Button size="sm" variant="text" onClick={() => setConfirmDelete(false)}>
              {t('w.scheduled.d046ac')}
            </Button>
            <Button size="sm" variant="danger" onClick={() => run(scheduledActions.remove(item.chat_id, item.id))}>
              {t('w.scheduled.3c06ab')}
            </Button>
          </>
        ) : (
          <>
            <Button size="sm" variant="text" onClick={() => run(scheduledActions.sendNow(item.chat_id, item.id))}>
              {t('w.scheduled.75e3ae')}
            </Button>
            <Button size="sm" variant="text" onClick={() => onReschedule(item)}>
              {t('w.scheduled.ef2853')}
            </Button>
            <Button size="sm" variant="text" onClick={() => setEditing(item.content)}>
              {t('w.scheduled.a7f814')}
            </Button>
            <Button size="sm" variant="text" onClick={() => setConfirmDelete(true)}>
              {t('w.scheduled.3755f5')}
            </Button>
          </>
        )}
      </div>
    </li>
  )
}

export function ScheduledMessagesDialog({ open, chatId, onClose }: ScheduledMessagesDialogProps) {
  const items = useStore(scheduledStore, (state) => state.byChat[chatId]) ?? EMPTY
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [rescheduling, setRescheduling] = useState<ScheduledMessage | null>(null)

  // Reopening refreshes: the scheduler may have delivered entries while this chat was closed.
  useEffect(() => {
    if (!open) return
    setError(null)
    setLoading(true)
    scheduledActions
      .load(chatId)
      .catch((caught: unknown) => setError(scheduledErrorText(caught)))
      .finally(() => setLoading(false))
  }, [open, chatId])

  const now = new Date()
  return (
    <>
      <Modal open={open} onClose={onClose} title={t('w.scheduled.6c0ad5')} size="md" className="tg-scheduled">
        {error ? (
          <p className="tg-scheduled__error" role="alert">
            {error}
          </p>
        ) : null}
        {loading && items.length === 0 ? <Spinner /> : null}
        {!loading && items.length === 0 ? <p className="tg-scheduled__empty">{t('w.scheduled.6d005f')}</p> : null}
        {groupByDay(items, now).map((group) => (
          <section key={group.day} className="tg-scheduled__day" aria-label={group.day}>
            <h3 className="tg-scheduled__day-title">{group.day}</h3>
            <ul className="tg-scheduled__list">
              {group.items.map((item) => (
                <ScheduledRow key={item.id} item={item} onError={setError} onReschedule={setRescheduling} />
              ))}
            </ul>
          </section>
        ))}
      </Modal>
      <ScheduleDialog
        open={rescheduling !== null}
        title={t('w.scheduled.ef2853')}
        initial={rescheduling ? new Date(rescheduling.scheduled_at) : undefined}
        onClose={() => setRescheduling(null)}
        onConfirm={async (at) => {
          if (!rescheduling) return
          try {
            await scheduledActions.update(chatId, rescheduling.id, { scheduled_at: at.toISOString() })
          } catch (caught) {
            throw new Error(scheduledErrorText(caught))
          }
        }}
      />
    </>
  )
}
