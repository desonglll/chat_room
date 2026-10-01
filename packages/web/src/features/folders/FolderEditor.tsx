/**
 * TG-501: create / edit one folder — name and emoji, whole chat types, chats added or excluded
 * by hand, and the three exclusion flags. Validation mirrors the server (`src/chats/folders.rs`):
 * a 1–12 character title and at least one type or chat.
 */
import { useState } from 'react'
import type { ChatFolder, ChatFolderWrite, ConversationSummary, FolderChatType } from '@tg/core'
import { Button, Checkbox, TextField, Toggle } from '@tg/ui'
import { t } from '../../i18n/index'

const TYPES: { id: FolderChatType; label: string }[] = [
  {
    id: 'private',
    get label() {
      return t('w.folders.3adfdb')
    },
  },
  {
    id: 'groups',
    get label() {
      return t('w.folders.4260ca')
    },
  },
  {
    id: 'channels',
    get label() {
      return t('w.folders.b76dfd')
    },
  },
]
export const FOLDER_TITLE_MAX = 12

export const EMPTY_FOLDER: ChatFolderWrite = {
  title: '',
  emoji: '',
  include_types: [],
  include_chat_ids: [],
  exclude_chat_ids: [],
  exclude_muted: false,
  exclude_read: false,
  exclude_archived: false,
}

/** Why the draft cannot be saved yet, or `''` when it can. */
export function folderDraftError(draft: ChatFolderWrite): string {
  const title = [...draft.title.trim()].length
  if (title === 0) return t('w.folders.0d58c8')
  if (title > FOLDER_TITLE_MAX) return t('w.folders.9a125a', FOLDER_TITLE_MAX)
  if (draft.include_types.length === 0 && draft.include_chat_ids.length === 0) return t('w.folders.98baa7')
  return ''
}

const toggleId = (ids: readonly string[], id: string, on: boolean) =>
  on ? [...ids.filter((value) => value !== id), id] : ids.filter((value) => value !== id)

interface FolderEditorProps {
  initial: ChatFolder | ChatFolderWrite
  conversations: readonly ConversationSummary[]
  onSave: (draft: ChatFolderWrite) => Promise<void>
  onCancel: () => void
}

export function FolderEditor({ initial, conversations, onSave, onCancel }: FolderEditorProps) {
  const [draft, setDraft] = useState<ChatFolderWrite>(() => {
    const { id: _id, ...rest } = { id: '', ...initial }
    return rest
  })
  const [saving, setSaving] = useState(false)
  const [failure, setFailure] = useState('')
  const error = folderDraftError(draft)
  const change = (patch: Partial<ChatFolderWrite>) => setDraft((current) => ({ ...current, ...patch }))

  const save = async () => {
    setSaving(true)
    setFailure('')
    try {
      await onSave({ ...draft, title: draft.title.trim() })
    } catch {
      setFailure(t('w.folders.18b4e2'))
    } finally {
      setSaving(false)
    }
  }

  return (
    <form
      className="tg-folder-editor"
      aria-label={t('w.folders.17b32d')}
      onSubmit={(event) => {
        event.preventDefault()
        if (!error) void save()
      }}
    >
      <div className="tg-folder-editor__row">
        <TextField
          label={t('w.folders.1f24c1')}
          value={draft.emoji}
          maxLength={8}
          onChange={(event) => change({ emoji: event.target.value })}
        />
        <TextField
          label={t('w.folders.6af274')}
          value={draft.title}
          onChange={(event) => change({ title: event.target.value })}
          error={draft.title && error.startsWith(t('w.folders.1be7ae')) ? error : undefined}
        />
      </div>
      <fieldset className="tg-folder-editor__group">
        <legend>{t('w.folders.72a2ac')}</legend>
        {TYPES.map((type) => (
          <Checkbox
            key={type.id}
            label={type.label}
            checked={draft.include_types.includes(type.id)}
            onCheckedChange={(on) =>
              change({
                include_types: on
                  ? [...draft.include_types.filter((value) => value !== type.id), type.id]
                  : draft.include_types.filter((value) => value !== type.id),
              })
            }
          />
        ))}
      </fieldset>
      <fieldset className="tg-folder-editor__group">
        <legend>{t('w.folders.7b37cc')}</legend>
        <Toggle
          label={t('w.folders.a074ec')}
          checked={draft.exclude_muted}
          onCheckedChange={(on) => change({ exclude_muted: on })}
        />
        <Toggle
          label={t('w.folders.642ec8')}
          checked={draft.exclude_read}
          onCheckedChange={(on) => change({ exclude_read: on })}
        />
        <Toggle
          label={t('w.folders.5cfbea')}
          checked={draft.exclude_archived}
          onCheckedChange={(on) => change({ exclude_archived: on })}
        />
      </fieldset>
      <fieldset className="tg-folder-editor__group">
        <legend>{t('w.folders.eb3785')}</legend>
        <ul className="tg-folder-editor__chats">
          {conversations.map((conversation) => {
            const id = conversation.room_id
            return (
              <li key={id} className="tg-folder-editor__row">
                <span className="tg-folder-list__title">{conversation.title || conversation.alias}</span>
                <Checkbox
                  label={t('w.folders.f896fb')}
                  checked={draft.include_chat_ids.includes(id)}
                  onCheckedChange={(on) =>
                    change({
                      include_chat_ids: toggleId(draft.include_chat_ids, id, on),
                      exclude_chat_ids: on ? toggleId(draft.exclude_chat_ids, id, false) : draft.exclude_chat_ids,
                    })
                  }
                />
                <Checkbox
                  label={t('w.folders.7b37cc')}
                  checked={draft.exclude_chat_ids.includes(id)}
                  onCheckedChange={(on) =>
                    change({
                      exclude_chat_ids: toggleId(draft.exclude_chat_ids, id, on),
                      include_chat_ids: on ? toggleId(draft.include_chat_ids, id, false) : draft.include_chat_ids,
                    })
                  }
                />
              </li>
            )
          })}
        </ul>
      </fieldset>
      {error && !error.startsWith(t('w.folders.1be7ae')) ? <p className="tg-folder-editor__hint">{error}</p> : null}
      {failure ? <p role="alert">{failure}</p> : null}
      <div className="tg-folder-editor__row">
        <Button type="button" variant="text" onClick={onCancel}>
          {t('w.folders.4d0b46')}
        </Button>
        <Button type="submit" disabled={Boolean(error)} loading={saving}>
          {t('w.folders.fadf24')}
        </Button>
      </div>
    </form>
  )
}
