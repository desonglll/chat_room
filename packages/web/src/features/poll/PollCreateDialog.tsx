/**
 * «新建投票» — the poll creation dialog (TG-406). Exported for the composer: TG-104's attach
 * menu has a disabled «投票» entry; the integration lead enables it to open this dialog
 * (hook-up in `docs/devlog/TG-406.md`, Integration patch list).
 *
 * The created message reaches the timeline through the normal message path; `onCreated`
 * only reports it (e.g. to scroll to it).
 */
import { useId, useMemo, useState } from 'react'
import { useStore } from 'zustand/react'
import type { StoredMessage } from '@tg/core'
import { POLL_LIMITS, authStore, createPoll, createRandomUuid, selectToken } from '@tg/core'
import { Button, IconButton, Modal, Radio, TextField, Toggle, VisuallyHidden } from '@tg/ui'
import { apiClient } from '../../app/client'
import { activeTopicId } from '../forum/activeTopic'
import { CrossGlyph } from './glyphs'
import {
  EMPTY_POLL_FORM,
  addOption,
  buildPollInput,
  canAddOption,
  removeOption,
  setMultipleChoice,
  setQuiz,
  type PollForm,
} from './pollForm'
import { pollStore } from './pollStore'
import { pollErrorText } from './usePoll'
import { t } from '../../i18n/index'

export interface PollCreateDialogProps {
  open: boolean
  chatId: string
  onClose(): void
  onCreated?: ((message: StoredMessage) => void) | undefined
}

export function PollCreateDialog({ open, chatId, onClose, onCreated }: PollCreateDialogProps) {
  const token = useStore(authStore, selectToken)
  const [form, setForm] = useState<PollForm>(EMPTY_POLL_FORM)
  const [error, setError] = useState<string | null>(null)
  const [sending, setSending] = useState(false)
  // One idempotency key per dialog session: a retried submit cannot create a second poll.
  const clientMessageId = useMemo(() => createRandomUuid(), [])
  const radioName = useId()

  const close = () => {
    setForm(EMPTY_POLL_FORM)
    setError(null)
    onClose()
  }

  const submit = () => {
    const built = buildPollInput(form)
    if (!built.ok) {
      setError(built.error)
      return
    }
    setSending(true)
    setError(null)
    const topicId = activeTopicId(chatId)
    createPoll(apiClient, token, chatId, {
      ...built.input,
      client_message_id: clientMessageId,
      ...(topicId ? { topic_id: topicId } : {}),
    })
      .then((message) => {
        if (message.poll) pollStore.getState().remember(message.poll)
        onCreated?.(message)
        close()
      })
      .catch((caught: unknown) => setError(pollErrorText(caught)))
      .finally(() => setSending(false))
  }

  const setOption = (index: number, text: string) =>
    setForm((current) => ({ ...current, options: current.options.map((old, at) => (at === index ? text : old)) }))

  return (
    <Modal
      open={open}
      onClose={close}
      title={form.quiz ? t('w.poll.5f530e') : t('w.poll.414afa')}
      size="sm"
      className="tg-poll-create"
      footer={
        <>
          <Button variant="text" onClick={close}>
            {t('w.poll.4d0b46')}
          </Button>
          <Button onClick={submit} loading={sending} disabled={sending}>
            {t('w.poll.fcbd09')}
          </Button>
        </>
      }
    >
      <TextField
        label={t('w.poll.2fb49e')}
        value={form.question}
        maxLength={POLL_LIMITS.questionChars}
        fullWidth
        onChange={(event) => setForm({ ...form, question: event.target.value })}
      />
      <div className="tg-poll-create__section">{t('w.poll.221ee0')}</div>
      <ol className="tg-poll-create__options">
        {form.options.map((text, index) => (
          <li key={index} className="tg-poll-create__option">
            {form.quiz ? (
              <Radio
                name={radioName}
                value={String(index)}
                checked={form.correctOption === index}
                onSelect={() => setForm({ ...form, correctOption: index })}
                size="sm"
                label={<VisuallyHidden>{t('w.poll.6a57fd', index + 1)}</VisuallyHidden>}
              />
            ) : null}
            <TextField
              aria-label={t('w.poll.9305ce', index + 1)}
              placeholder={t('w.poll.ec8639')}
              value={text}
              maxLength={POLL_LIMITS.optionChars}
              fullWidth
              onChange={(event) => setOption(index, event.target.value)}
            />
            {form.options.length > POLL_LIMITS.minOptions ? (
              <IconButton
                label={t('w.poll.1c40a1', index + 1)}
                size="sm"
                onClick={() => setForm(removeOption(form, index))}
              >
                <CrossGlyph />
              </IconButton>
            ) : null}
          </li>
        ))}
      </ol>
      {canAddOption(form) ? (
        <Button variant="text" size="sm" onClick={() => setForm(addOption(form))}>
          {t('w.poll.aff6de')}
        </Button>
      ) : (
        <div className="tg-poll-create__hint">
          {t('w.poll.4c5dfc')} {POLL_LIMITS.maxOptions} {t('w.poll.2ee9e7')}
        </div>
      )}
      {form.quiz ? <div className="tg-poll-create__hint">{t('w.poll.ead6aa')}</div> : null}
      <div className="tg-poll-create__section">{t('w.poll.7debf9')}</div>
      <Toggle
        label={t('w.poll.1df67e')}
        checked={form.anonymous}
        onCheckedChange={(anonymous) => setForm({ ...form, anonymous })}
      />
      <Toggle
        label={t('w.poll.83c680')}
        checked={form.multipleChoice}
        onCheckedChange={(value) => setForm(setMultipleChoice(form, value))}
      />
      <Toggle
        label={t('w.poll.0fda0d')}
        description={t('w.poll.2ee19b')}
        checked={form.quiz}
        onCheckedChange={(value) => setForm(setQuiz(form, value))}
      />
      {form.quiz ? (
        <div className="tg-poll-create__explanation">
          <TextField
            label={t('w.poll.471325')}
            value={form.explanation}
            maxLength={POLL_LIMITS.explanationChars}
            multiline
            rows={2}
            fullWidth
            onChange={(event) => setForm({ ...form, explanation: event.target.value })}
          />
        </div>
      ) : null}
      {error ? (
        <div className="tg-poll-create__error" role="alert">
          {error}
        </div>
      ) : null}
    </Modal>
  )
}

export default PollCreateDialog
