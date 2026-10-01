/**
 * TG-501 «聊天文件夹»: the viewer's folders in display order, with create / edit / delete and
 * move up / down (at most 10), and where the folder strip sits (above the list or on its left).
 */
import { useState } from 'react'
import type { ChatFolder, ChatFolderWrite } from '@tg/core'
import { chatListStore, folderConversations, MAX_CHAT_FOLDERS, settingsStore } from '@tg/core'
import { Button } from '@tg/ui'
import { useStore } from 'zustand/react'
import { browserStorage } from '../../app/platform'
import { EMPTY_FOLDER, FolderEditor } from './FolderEditor'
import type { FoldersApi } from '@tg/core'
import { folderStore, foldersApi, loadFolders } from './folderStore'
import { t } from '../../i18n/index'

export function FolderSettingsPage({ api = foldersApi }: { api?: FoldersApi }) {
  const { folders, loaded } = useStore(folderStore)
  const conversations = useStore(chatListStore, (state) => state.conversations)
  const layout = useStore(settingsStore, (state) => state.folderLayout)
  const [editing, setEditing] = useState<ChatFolder | 'new' | null>(null)
  const [failure, setFailure] = useState('')
  if (!loaded) void loadFolders(api)

  const run = async (action: () => Promise<unknown>) => {
    setFailure('')
    try {
      await action()
      await loadFolders(api)
    } catch {
      setFailure(t('w.folders.51d3cb'))
    }
  }
  const save = async (draft: ChatFolderWrite) => {
    await (editing === 'new' || !editing ? api.create(draft) : api.update(editing.id, draft))
    await loadFolders(api)
    setEditing(null)
  }
  const move = (index: number, by: -1 | 1) => {
    const ids = folders.map((folder) => folder.id)
    const [id] = ids.splice(index, 1)
    if (id === undefined) return
    ids.splice(index + by, 0, id)
    void run(() => api.reorder(ids))
  }
  const setLayout = (folderLayout: 'top' | 'left') => {
    settingsStore.getState().update({ folderLayout })
    settingsStore.getState().persist(browserStorage)
  }

  if (editing) {
    return (
      <FolderEditor
        initial={editing === 'new' ? EMPTY_FOLDER : editing}
        conversations={conversations}
        onSave={save}
        onCancel={() => setEditing(null)}
      />
    )
  }
  const now = Date.now()
  return (
    <div className="tg-folder-settings">
      <p>{t('w.folders.675049')}</p>
      <ul className="tg-folder-list" aria-label={t('w.folders.d7ce24')}>
        {folders.map((folder, index) => (
          <li key={folder.id} className="tg-folder-list__item">
            <span aria-hidden="true">{folder.emoji || '📁'}</span>
            <span className="tg-folder-list__title">
              {folder.title} · {folderConversations(folder, conversations, now).length} {t('w.folders.3bd0ab')}
            </span>
            <Button
              variant="text"
              size="sm"
              aria-label={t('w.folders.38e2f3', folder.title)}
              disabled={index === 0}
              onClick={() => move(index, -1)}
            >
              ↑
            </Button>
            <Button
              variant="text"
              size="sm"
              aria-label={t('w.folders.451b58', folder.title)}
              disabled={index === folders.length - 1}
              onClick={() => move(index, 1)}
            >
              ↓
            </Button>
            <Button variant="text" size="sm" onClick={() => setEditing(folder)}>
              {t('w.folders.a7f814')}
            </Button>
            <Button variant="danger" size="sm" onClick={() => void run(() => api.remove(folder.id))}>
              {t('w.folders.3755f5')}
            </Button>
          </li>
        ))}
      </ul>
      <Button disabled={folders.length >= MAX_CHAT_FOLDERS} onClick={() => setEditing('new')}>
        {t('w.folders.95cf3c')}
      </Button>
      {folders.length >= MAX_CHAT_FOLDERS ? (
        <p>
          {t('w.folders.4c5dfc')} {MAX_CHAT_FOLDERS} {t('w.folders.9a311c')}
        </p>
      ) : null}
      {failure ? <p role="alert">{failure}</p> : null}
      <fieldset className="tg-folder-editor__group">
        <legend>{t('w.folders.489b5a')}</legend>
        <label>
          <input type="radio" name="folder-layout" checked={layout === 'top'} onChange={() => setLayout('top')} />{' '}
          {t('w.folders.ad65cb')}
        </label>
        <label>
          <input type="radio" name="folder-layout" checked={layout === 'left'} onChange={() => setLayout('left')} />{' '}
          {t('w.folders.9bfcb6')}
        </label>
      </fieldset>
    </div>
  )
}
