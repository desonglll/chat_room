/**
 * TG-501: the folder strip — «全部» plus each folder with its unread count, as tabs above the
 * chat list or a rail on its left (设置 › 聊天文件夹). Counts come from the same conversations
 * the list shows (`folderUnread`), so they always agree with the chats' own badges.
 */
import { useEffect } from 'react'
import type { ChatFolder, ConversationSummary } from '@tg/core'
import { folderUnread, settingsStore } from '@tg/core'
import { useStore } from 'zustand/react'
import { folderStore, loadFolders, selectFolder } from './folderStore'

export function FolderTabs({ conversations }: { conversations: readonly ConversationSummary[] }) {
  const { folders, activeId, loaded } = useStore(folderStore)
  const layout = useStore(settingsStore, (state) => state.folderLayout)
  useEffect(() => {
    if (!loaded) void loadFolders()
  }, [loaded])
  return (
    <FolderStrip
      folders={folders}
      activeId={activeId}
      layout={layout}
      conversations={conversations}
      now={Date.now()}
      onSelect={selectFolder}
    />
  )
}

interface FolderStripProps {
  folders: readonly ChatFolder[]
  activeId: string | null
  layout: 'top' | 'left'
  conversations: readonly ConversationSummary[]
  now: number
  onSelect: (id: string | null) => void
}

export function FolderStrip({ folders, activeId, layout, conversations, now, onSelect }: FolderStripProps) {
  if (folders.length === 0) return null
  const tabs = [{ id: null as string | null, title: '全部', emoji: '', unread: 0 }].concat(
    folders.map((folder) => ({
      id: folder.id,
      title: folder.title,
      emoji: folder.emoji,
      unread: folderUnread(folder, conversations, now),
    })),
  )
  return (
    <div className="tg-folders" data-layout={layout} role="tablist" aria-label="聊天文件夹">
      {tabs.map((tab) => (
        <button
          key={tab.id ?? 'all'}
          type="button"
          role="tab"
          aria-selected={tab.id === activeId}
          className="tg-folders__tab"
          onClick={() => onSelect(tab.id)}
        >
          {tab.emoji ? <span className="tg-folders__emoji">{tab.emoji}</span> : null}
          <span className="tg-folders__title">{tab.title}</span>
          {tab.unread > 0 ? <span className="tg-folders__badge">{tab.unread}</span> : null}
        </button>
      ))}
    </div>
  )
}

/** The folder the chat list is narrowed to, or `undefined` for «全部». */
export function useActiveFolder(): ChatFolder | undefined {
  const { folders, activeId } = useStore(folderStore)
  return activeId ? folders.find((candidate) => candidate.id === activeId) : undefined
}
