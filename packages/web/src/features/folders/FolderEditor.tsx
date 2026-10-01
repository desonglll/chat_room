/**
 * TG-501: create / edit one folder — name and emoji, whole chat types, chats added or excluded
 * by hand, and the three exclusion flags. Validation mirrors the server (`src/chats/folders.rs`):
 * a 1–12 character title and at least one type or chat.
 */
import { useState } from 'react'
import type { ChatFolder, ChatFolderWrite, ConversationSummary, FolderChatType } from '@tg/core'
import { Button, Checkbox, TextField, Toggle } from '@tg/ui'

const TYPES: { id: FolderChatType; label: string }[] = [
  { id: 'private', label: '私聊' },
  { id: 'groups', label: '群组' },
  { id: 'channels', label: '频道' },
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
  if (title === 0) return '请输入文件夹名称'
  if (title > FOLDER_TITLE_MAX) return `名称最多 ${FOLDER_TITLE_MAX} 个字`
  if (draft.include_types.length === 0 && draft.include_chat_ids.length === 0) return '至少选择一种聊天类型或一个聊天'
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
      setFailure('保存失败，请重试')
    } finally {
      setSaving(false)
    }
  }

  return (
    <form
      className="tg-folder-editor"
      aria-label="编辑文件夹"
      onSubmit={(event) => {
        event.preventDefault()
        if (!error) void save()
      }}
    >
      <div className="tg-folder-editor__row">
        <TextField
          label="图标"
          value={draft.emoji}
          maxLength={8}
          onChange={(event) => change({ emoji: event.target.value })}
        />
        <TextField
          label="文件夹名称"
          value={draft.title}
          onChange={(event) => change({ title: event.target.value })}
          error={draft.title && error.startsWith('名称') ? error : undefined}
        />
      </div>
      <fieldset className="tg-folder-editor__group">
        <legend>包含的聊天类型</legend>
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
        <legend>排除</legend>
        <Toggle label="已静音" checked={draft.exclude_muted} onCheckedChange={(on) => change({ exclude_muted: on })} />
        <Toggle label="已读" checked={draft.exclude_read} onCheckedChange={(on) => change({ exclude_read: on })} />
        <Toggle
          label="已归档"
          checked={draft.exclude_archived}
          onCheckedChange={(on) => change({ exclude_archived: on })}
        />
      </fieldset>
      <fieldset className="tg-folder-editor__group">
        <legend>单独添加或排除的聊天</legend>
        <ul className="tg-folder-editor__chats">
          {conversations.map((conversation) => {
            const id = conversation.room_id
            return (
              <li key={id} className="tg-folder-editor__row">
                <span className="tg-folder-list__title">{conversation.title || conversation.alias}</span>
                <Checkbox
                  label="包含"
                  checked={draft.include_chat_ids.includes(id)}
                  onCheckedChange={(on) =>
                    change({
                      include_chat_ids: toggleId(draft.include_chat_ids, id, on),
                      exclude_chat_ids: on ? toggleId(draft.exclude_chat_ids, id, false) : draft.exclude_chat_ids,
                    })
                  }
                />
                <Checkbox
                  label="排除"
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
      {error && !error.startsWith('名称') ? <p className="tg-folder-editor__hint">{error}</p> : null}
      {failure ? <p role="alert">{failure}</p> : null}
      <div className="tg-folder-editor__row">
        <Button type="button" variant="text" onClick={onCancel}>
          取消
        </Button>
        <Button type="submit" disabled={Boolean(error)} loading={saving}>
          保存
        </Button>
      </div>
    </form>
  )
}
