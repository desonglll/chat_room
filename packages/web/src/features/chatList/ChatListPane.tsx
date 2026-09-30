/**
 * The left pane (TG-102): top bar (hamburger menu + search), the archive entry row (TG-502:
 * collapsed / expanded / hidden into the menu), and the chat rows in Telegram order, each
 * archivable by context menu or touch swipe. `collapsed` renders the avatar-only column; the shell
 * owns width, collapse persistence and the mobile list ↔ chat switch.
 */
import type { MouseEvent } from 'react'
import { useEffect, useMemo, useState } from 'react'
import { useMatch, useNavigate } from 'react-router-dom'
import type { MenuItem } from '@tg/ui'
import { authStore, chatListStore, selectToken, settingsStore } from '@tg/core'
import { useStore } from 'zustand/react'
import { ScrollArea, Skeleton } from '@tg/ui'
import { apiClient } from '../../app/client'
import { openSettings } from '../settings/shell/settingsNavigation'
import { browserStorage } from '../../app/platform'
import { signOut } from '../../app/session'
import { ArchivableChatRow } from './ArchivableChatRow'
import { setChatArchived } from './archiveActions'
import { ARCHIVE_TITLE, ArchiveRow } from './ArchiveRow'
import type { ArchiveRowMode } from './archiveRowMode'
import { readArchiveRowMode, writeArchiveRowMode } from './archiveRowMode'
import { archiveBadge, archivePreviewChats } from './archiveRules'
import { ChatListHeader } from './ChatListHeader'
import type { ChatListFolder } from './chatListFilters'
import { selectChatListView } from './chatListFilters'
import { ConnectedChatRow } from './ConnectedChatRow'
import { NewChatDialog } from './NewChatDialog'
import { channelApi, CreateChannelDialog } from '../channel'
import { useMinuteClock } from './useMinuteClock'
import { PublicSearchResults } from '../chatPreview/PublicSearchResults'

export interface ChatListPaneProps {
  collapsed?: boolean | undefined
  onToggleCollapsed?: (() => void) | undefined
}

const isPlainClick = (event: MouseEvent) =>
  event.button === 0 && !event.metaKey && !event.ctrlKey && !event.shiftKey && !event.altKey

function toggleNightMode() {
  const settings = settingsStore.getState()
  const night =
    settings.theme === 'dark' ||
    (settings.theme === 'system' && document.documentElement.getAttribute('data-tg-theme') === 'night')
  settings.update({ theme: night ? 'light' : 'dark' })
  settingsStore.getState().persist(browserStorage)
}

export function ChatListPane({ collapsed = false, onToggleCollapsed }: ChatListPaneProps) {
  const currentUserId = useStore(authStore, (state) => state.session?.user.id ?? '')
  const token = useStore(authStore, selectToken)
  const conversations = useStore(chatListStore, (state) => state.conversations)
  const loading = useStore(chatListStore, (state) => state.loading)
  const failed = useStore(chatListStore, (state) => state.error !== '')
  const [folder, setFolder] = useState<ChatListFolder>('main')
  const [query, setQuery] = useState('')
  const [creating, setCreating] = useState(false)
  const [creatingChannel, setCreatingChannel] = useState(false)
  const navigate = useNavigate()
  const activeChatId = useMatch('/chat/:chatId/*')?.params.chatId ?? ''
  const now = useMinuteClock()
  const [archiveMode, setArchiveMode] = useState<ArchiveRowMode>(() => readArchiveRowMode(browserStorage))

  const view = useMemo(() => selectChatListView(conversations, folder, query), [conversations, folder, query])
  const badge = useMemo(() => archiveBadge(conversations, now.getTime()), [conversations, now])
  const previewChats = useMemo(() => archivePreviewChats(view.archived, 3), [view.archived])

  // Telegram leaves the archive once its last chat is unarchived.
  useEffect(() => {
    if (folder === 'archive' && view.archivedCount === 0) setFolder('main')
  }, [folder, view.archivedCount])

  const changeArchiveMode = (mode: ArchiveRowMode) => {
    setArchiveMode(mode)
    writeArchiveRowMode(browserStorage, mode)
  }
  const toggleArchive = (chatId: string, archived: boolean) => {
    void setChatArchived({ client: apiClient, token, store: chatListStore }, chatId, !archived)
  }

  const openChat = (chatId: string, event: MouseEvent<HTMLAnchorElement>) => {
    if (!isPlainClick(event)) return
    event.preventDefault()
    void navigate(`/chat/${encodeURIComponent(chatId)}`)
  }

  const menuItems: MenuItem[] = [
    { id: 'new-group', label: '新建群组', onSelect: () => setCreating(true) },
    { id: 'new-channel', label: '新建频道', onSelect: () => setCreatingChannel(true) },
    {
      id: 'archive',
      label: ARCHIVE_TITLE,
      hint: badge.count > 0 ? String(badge.count) : undefined,
      disabled: view.archivedCount === 0,
      onSelect: () => setFolder('archive'),
    },
    ...(archiveMode === 'hidden' && view.archivedCount > 0
      ? [{ id: 'archive-show', label: '在列表顶部显示归档', onSelect: () => changeArchiveMode('collapsed') }]
      : []),
    { id: 'settings', label: '设置', onSelect: () => openSettings() },
    { id: 'night', label: '夜间模式', onSelect: toggleNightMode },
    ...(onToggleCollapsed
      ? [{ id: 'collapse', label: collapsed ? '展开侧栏' : '收起侧栏', onSelect: onToggleCollapsed }]
      : []),
    {
      id: 'sign-out',
      label: '退出登录',
      danger: true,
      separatorBefore: true,
      onSelect: () => void signOut({ client: apiClient, storage: browserStorage, store: authStore }),
    },
  ]

  const showArchiveRow = folder === 'main' && !query.trim() && view.archivedCount > 0 && archiveMode !== 'hidden'
  const empty = !loading && !failed && view.rows.length === 0 && !showArchiveRow

  return (
    <nav className="tg-chatlist" aria-label="会话列表" data-collapsed={collapsed || undefined}>
      <ChatListHeader
        folder={folder}
        collapsed={collapsed}
        query={query}
        onQueryChange={setQuery}
        onCloseFolder={() => setFolder('main')}
        menuItems={menuItems}
      />
      <ScrollArea className="tg-chatlist__scroll" orientation="vertical" overlay>
        {loading && conversations.length === 0 ? (
          <div className="tg-chatlist__loading" aria-hidden="true">
            {[0, 1, 2, 3].map((key) => (
              <div key={key} className="tg-chatlist__skeleton-row">
                <Skeleton variant="circle" width="var(--tg-avatar-md)" height="var(--tg-avatar-md)" />
                {collapsed ? null : <Skeleton variant="text" lines={2} />}
              </div>
            ))}
          </div>
        ) : null}
        {!loading && failed && conversations.length === 0 ? (
          <p className="tg-chatlist__empty">会话列表加载失败，请刷新重试</p>
        ) : null}
        {empty && !collapsed ? (
          <p className="tg-chatlist__empty">
            {query.trim() ? '没有找到匹配的会话' : '还没有会话 — 建一个群，或等别人拉你进来'}
          </p>
        ) : null}
        <ul className="tg-chatlist__items">
          {showArchiveRow ? (
            <li>
              <ArchiveRow
                mode={archiveMode === 'expanded' ? 'expanded' : 'collapsed'}
                previewChats={previewChats}
                count={view.archivedCount}
                badge={badge}
                now={now.getTime()}
                collapsed={collapsed}
                onOpen={() => setFolder('archive')}
                onModeChange={changeArchiveMode}
              />
            </li>
          ) : null}
          {view.rows.map((conversation) => (
            <li key={conversation.room_id}>
              <ArchivableChatRow
                archived={conversation.preferences.is_archived}
                collapsed={collapsed}
                onToggleArchive={() => toggleArchive(conversation.room_id, conversation.preferences.is_archived)}
              >
                <ConnectedChatRow
                  conversation={conversation}
                  currentUserId={currentUserId}
                  activeChatId={activeChatId}
                  collapsed={collapsed}
                  now={now}
                  onOpen={openChat}
                />
              </ArchivableChatRow>
            </li>
          ))}
        </ul>
        {query.trim() && !collapsed ? <PublicSearchResults query={query} /> : null}
      </ScrollArea>
      <NewChatDialog
        open={creating}
        onClose={() => setCreating(false)}
        token={token}
        onCreated={(chat) => {
          setCreating(false)
          void navigate(`/chat/${chat.id}`)
        }}
      />
      <CreateChannelDialog
        open={creatingChannel}
        onClose={() => setCreatingChannel(false)}
        token={token}
        api={channelApi}
        onCreated={(chat) => {
          setCreatingChannel(false)
          void navigate(`/chat/${chat.id}`)
        }}
      />
    </nav>
  )
}
