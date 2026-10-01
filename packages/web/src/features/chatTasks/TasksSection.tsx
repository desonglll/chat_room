/**
 * TG-706 «任务» in a group's info panel: the chat's tasks with their status, add one, change its
 * status, delete it — each control only where the server says the viewer may (`can_update`,
 * `can_delete`). A stale edit (someone changed it first) reloads instead of overwriting.
 */
import { useCallback, useEffect, useState } from 'react'
import type { ChatTask, TasksApi, TaskStatus } from '@tg/core'
import { ApiError, TASK_STATUSES, TASK_TITLE_MAX } from '@tg/core'
import { Button, TextField } from '@tg/ui'
import { t } from '../../i18n/index'
import { tasksApi } from './tasksApi'

const STATUS_KEY: Record<TaskStatus, string> = {
  open: 'w.tasks.open',
  in_progress: 'w.tasks.inProgress',
  done: 'w.tasks.done',
  cancelled: 'w.tasks.cancelled',
}

export function TasksSection({ chatId, api = tasksApi }: { chatId: string; api?: TasksApi }) {
  const [tasks, setTasks] = useState<ChatTask[] | null>(null)
  const [title, setTitle] = useState('')
  const [note, setNote] = useState('')
  const reload = useCallback(() => api.list(chatId).then(setTasks, () => setTasks([])), [api, chatId])
  useEffect(() => void reload(), [reload])

  const failed = (error: unknown) => {
    setNote(error instanceof ApiError && error.status === 409 ? t('w.tasks.stale') : t('w.tasks.failed'))
    void reload()
  }
  const trimmed = title.trim()

  return (
    <section className="tg-tasks" aria-label={t('w.tasks.title')}>
      <h3 className="tg-tasks__title">{t('w.tasks.title')}</h3>
      {tasks && tasks.length === 0 ? <p className="tg-tasks__empty">{t('w.tasks.empty')}</p> : null}
      <ul className="tg-tasks__list">
        {(tasks ?? []).map((task) => (
          <li key={task.id} className="tg-tasks__row" data-status={task.status}>
            <span className="tg-tasks__text">
              {task.title}
              {task.assignee_name ? <span className="tg-tasks__meta"> · {task.assignee_name}</span> : null}
            </span>
            {task.can_update ? (
              <select
                aria-label={t('w.tasks.status', task.title)}
                value={task.status}
                onChange={(event) =>
                  void api
                    .update(chatId, task, { status: event.target.value as TaskStatus })
                    .then(() => void reload(), failed)
                }
              >
                {TASK_STATUSES.map((status) => (
                  <option key={status} value={status}>
                    {t(STATUS_KEY[status])}
                  </option>
                ))}
              </select>
            ) : (
              <span className="tg-tasks__meta">{t(STATUS_KEY[task.status])}</span>
            )}
            {task.can_delete ? (
              <button
                type="button"
                className="tg-tasks__delete"
                aria-label={t('w.tasks.delete', task.title)}
                onClick={() => void api.remove(chatId, task.id).then(() => void reload(), failed)}
              >
                ×
              </button>
            ) : null}
          </li>
        ))}
      </ul>
      <form
        className="tg-tasks__add"
        onSubmit={(event) => {
          event.preventDefault()
          if (!trimmed || [...trimmed].length > TASK_TITLE_MAX) return
          void api.create(chatId, trimmed).then(() => {
            setTitle('')
            void reload()
          }, failed)
        }}
      >
        <TextField
          aria-label={t('w.tasks.new')}
          placeholder={t('w.tasks.new')}
          value={title}
          maxLength={TASK_TITLE_MAX}
          onChange={(event) => setTitle(event.target.value)}
        />
        <Button type="submit" size="sm" disabled={!trimmed}>
          {t('w.tasks.add')}
        </Button>
      </form>
      {note ? <p role="status">{note}</p> : null}
    </section>
  )
}
