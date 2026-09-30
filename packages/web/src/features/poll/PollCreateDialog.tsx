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
    createPoll(apiClient, token, chatId, { ...built.input, client_message_id: clientMessageId })
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
      title={form.quiz ? '新建测验' : '新建投票'}
      size="sm"
      className="tg-poll-create"
      footer={
        <>
          <Button variant="text" onClick={close}>
            取消
          </Button>
          <Button onClick={submit} loading={sending} disabled={sending}>
            创建
          </Button>
        </>
      }
    >
      <TextField
        label="问题"
        value={form.question}
        maxLength={POLL_LIMITS.questionChars}
        fullWidth
        onChange={(event) => setForm({ ...form, question: event.target.value })}
      />
      <div className="tg-poll-create__section">选项</div>
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
                label={<VisuallyHidden>{`设选项 ${index + 1} 为正确答案`}</VisuallyHidden>}
              />
            ) : null}
            <TextField
              aria-label={`选项 ${index + 1}`}
              placeholder="添加一个选项"
              value={text}
              maxLength={POLL_LIMITS.optionChars}
              fullWidth
              onChange={(event) => setOption(index, event.target.value)}
            />
            {form.options.length > POLL_LIMITS.minOptions ? (
              <IconButton label={`删除选项 ${index + 1}`} size="sm" onClick={() => setForm(removeOption(form, index))}>
                <CrossGlyph />
              </IconButton>
            ) : null}
          </li>
        ))}
      </ol>
      {canAddOption(form) ? (
        <Button variant="text" size="sm" onClick={() => setForm(addOption(form))}>
          添加选项
        </Button>
      ) : (
        <div className="tg-poll-create__hint">最多 {POLL_LIMITS.maxOptions} 个选项</div>
      )}
      {form.quiz ? <div className="tg-poll-create__hint">点击选项前的圆点标记正确答案</div> : null}
      <div className="tg-poll-create__section">设置</div>
      <Toggle
        label="匿名投票"
        checked={form.anonymous}
        onCheckedChange={(anonymous) => setForm({ ...form, anonymous })}
      />
      <Toggle
        label="多选"
        checked={form.multipleChoice}
        onCheckedChange={(value) => setForm(setMultipleChoice(form, value))}
      />
      <Toggle
        label="测验模式"
        description="只有一个正确答案，作答后不可更改"
        checked={form.quiz}
        onCheckedChange={(value) => setForm(setQuiz(form, value))}
      />
      {form.quiz ? (
        <div className="tg-poll-create__explanation">
          <TextField
            label="解析（可选）"
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
