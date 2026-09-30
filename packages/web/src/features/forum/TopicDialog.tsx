/**
 * Create / edit a topic (TG-204), Telegram's sheet: a live preview of the icon and title,
 * the title field, the six colours (only meaningful without an emoji) and an emoji grid.
 * General edits its title only. Mount it fresh per open (the host keys it), so the form
 * starts from the topic being edited.
 */
import { useState, type CSSProperties, type FormEvent } from 'react'
import type { ForumTopic, TopicsApi } from '@tg/core'
import { MAX_TOPIC_TITLE_CHARS, TOPIC_COLORS, topicColorHex } from '@tg/core'
import { Button, Modal, TextField } from '@tg/ui'
import { TopicIcon } from './TopicIcon'
import { topicErrorText } from './topicListModel'
import type { TopicForm } from './topicForm'
import {
  initialTopicForm,
  titleLength,
  topicCreateInput,
  topicPatch,
  TOPIC_EMOJI,
  validateTopicForm,
} from './topicForm'

export interface TopicDialogProps {
  chatId: string
  api: TopicsApi
  /** The topic to edit; absent = create. */
  topic?: ForumTopic | null | undefined
  onClose(): void
  onSaved(topic: ForumTopic): void
}

export function TopicDialog({ chatId, api, topic = null, onClose, onSaved }: TopicDialogProps) {
  const [form, setForm] = useState<TopicForm>(() => initialTopicForm(topic))
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState('')
  const [touched, setTouched] = useState(false)
  const general = topic?.is_general ?? false
  const invalid = validateTopicForm(form)
  const set = (next: Partial<TopicForm>) => setForm((current) => ({ ...current, ...next }))

  const submit = (event?: FormEvent) => {
    event?.preventDefault()
    setTouched(true)
    if (invalid || saving) return
    const request = topic
      ? Object.keys(topicPatch(form, topic)).length === 0
        ? Promise.resolve(topic)
        : api.update(chatId, topic.id, topicPatch(form, topic))
      : api.create(chatId, topicCreateInput(form))
    setSaving(true)
    setError('')
    request.then(
      (saved) => {
        setSaving(false)
        onSaved(saved)
      },
      (caught: unknown) => {
        setSaving(false)
        setError(topicErrorText(caught))
      },
    )
  }

  const footer = (
    <>
      <Button variant="text" onClick={onClose} disabled={saving}>
        取消
      </Button>
      <Button onClick={() => submit()} loading={saving} disabled={touched && invalid !== null}>
        {topic ? '保存' : '创建'}
      </Button>
    </>
  )

  return (
    <Modal open onClose={() => !saving && onClose()} title={topic ? '编辑话题' : '新建话题'} footer={footer} size="sm">
      <form className="tg-topicdialog" onSubmit={submit}>
        <div className="tg-topicdialog__preview">
          <TopicIcon title={form.title || '话'} emoji={form.emoji} color={form.color} general={general} size={48} />
          <span className="tg-topicdialog__preview-title">{form.title.trim() || '话题名称'}</span>
        </div>
        <TextField
          label="话题名称"
          value={form.title}
          onChange={(event) => set({ title: event.target.value })}
          error={touched && invalid ? invalid : undefined}
          hint={`${titleLength(form.title)}/${MAX_TOPIC_TITLE_CHARS}`}
          disabled={saving}
          fullWidth
          autoFocus
        />
        {general ? null : (
          <>
            {form.emoji ? null : (
              <div className="tg-topicdialog__colors" role="group" aria-label="话题颜色">
                {TOPIC_COLORS.map((color, index) => (
                  <button
                    key={color}
                    type="button"
                    className="tg-topicdialog__swatch"
                    style={{ '--topic-color': topicColorHex(color) } as CSSProperties}
                    aria-label={`颜色 ${index + 1}`}
                    aria-pressed={form.color === color}
                    disabled={saving}
                    onClick={() => set({ color })}
                  />
                ))}
              </div>
            )}
            <div className="tg-topicdialog__emoji" role="group" aria-label="话题图标">
              <button
                type="button"
                className="tg-topicdialog__emoji-cell"
                aria-label="不使用表情"
                aria-pressed={form.emoji === ''}
                disabled={saving}
                onClick={() => set({ emoji: '' })}
              >
                <TopicIcon title={form.title || '话'} emoji="" color={form.color} size={26} />
              </button>
              {TOPIC_EMOJI.map((emoji) => (
                <button
                  key={emoji}
                  type="button"
                  className="tg-topicdialog__emoji-cell"
                  aria-label={emoji}
                  aria-pressed={form.emoji === emoji}
                  disabled={saving}
                  onClick={() => set({ emoji })}
                >
                  {emoji}
                </button>
              ))}
            </div>
          </>
        )}
        {error ? (
          <p className="tg-topicdialog__error" role="alert">
            {error}
          </p>
        ) : null}
      </form>
    </Modal>
  )
}
