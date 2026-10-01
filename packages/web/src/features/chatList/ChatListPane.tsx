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
import { promptInstall, pwaStore } from '../../app/pwa'
import { apiClient } from '../../app/client'
import { openSettings } from '../settings/shell/settingsNavigation'
import { browserStorage } from '../../app/platform'
import { signOut } from '../../app/session'
import { ArchivableChatRow } from './ArchivableChatRow'
import { setChatArchived } from './archiveActions'
import { archiveTitle, ArchiveRow } from './ArchiveRow'
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
import { SavedMessagesRow } from '../savedMessages/SavedMessagesRow'
import { FolderTabs, useActiveFolder } from '../folders/FolderTabs'
import { MessageSearchResults, SearchTabs, searchStore } from '../search'
import { t } from '../../i18n/index'

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

  const activeFolder = useActiveFolder()
  // TG-504: while searching, a tab other than «聊天» replaces the chat rows with message results.
  const searchTab = useStore(searchStore, (state) => state.tab)
  // TG-601: «安装应用» while the browser offers installation.
  const canInstall = useStore(pwaStore, (state) => state.canInstall)
  const searching = query.trim() !== '' && !collapsed
  const messageTab = searching && searchTab !== 'chats'
  const custom = folder === 'main' ? activeFolder : undefined
  const view = useMemo(
    () => selectChatListView(conversations, folder, query, custom ? { folder: custom, now: now.getTime() } : undefined),
    [conversations, folder, query, custom, now],
  )
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
    { id: 'new-group', label: t('w.chatList.07285a'), onSelect: () => setCreating(true) },
    { id: 'new-channel', label: t('w.chatList.b0e811'), onSelect: () => setCreatingChannel(true) },
    {
      id: 'archive',
      label: archiveTitle(),
      hint: badge.count > 0 ? String(badge.count) : undefined,
      disabled: view.archivedCount === 0,
      onSelect: () => setFolder('archive'),
    },
    ...(archiveMode === 'hidden' && view.archivedCount > 0
      ? [{ id: 'archive-show', label: t('w.chatList.bb986d'), onSelect: () => changeArchiveMode('collapsed') }]
      : []),
    ...(canInstall ? [{ id: 'install', label: t('w.pwa.install'), onSelect: () => void promptInstall() }] : []),
    { id: 'contacts', label: t('w.contacts.menu'), onSelect: () => void navigate('/contacts') },
    { id: 'settings', label: t('w.chatList.7debf9'), onSelect: () => openSettings() },
    { id: 'night', label: t('w.chatList.e32be0'), onSelect: toggleNightMode },
    ...(onToggleCollapsed
      ? [
          {
            id: 'collapse',
            label: collapsed ? t('w.chatList.f4cbda') : t('w.chatList.ae1032'),
            onSelect: onToggleCollapsed,
          },
        ]
      : []),
    {
      id: 'sign-out',
      label: t('w.chatList.094774'),
      danger: true,
      separatorBefore: true,
      onSelect: () => void signOut({ client: apiClient, storage: browserStorage, store: authStore }),
    },
  ]

  const showArchiveRow =
    folder === 'main' && !custom && !query.trim() && view.archivedCount > 0 && archiveMode !== 'hidden'
  const empty = !loading && !failed && view.rows.length === 0 && !showArchiveRow

  return (
    <nav className="tg-chatlist" aria-label={t('w.chatList.b77b64')} data-collapsed={collapsed || undefined}>
      <ChatListHeader
        folder={folder}
        collapsed={collapsed}
        query={query}
        onQueryChange={setQuery}
        onCloseFolder={() => setFolder('main')}
        menuItems={menuItems}
      />
      <div className="tg-chatlist__body">
        {folder === 'main' && !collapsed && !searching ? <FolderTabs conversations={conversations} /> : null}
        {searching ? <SearchTabs query={query} onPick={setQuery} /> : null}
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
            <p className="tg-chatlist__empty">{t('w.chatList.c19516')}</p>
          ) : null}
          {empty && !collapsed && !messageTab ? (
            <p className="tg-chatlist__empty">{query.trim() ? t('w.chatList.39a166') : t('w.chatList.c40e5d')}</p>
          ) : null}
          {messageTab ? <MessageSearchResults query={query} /> : null}
          <ul className="tg-chatlist__items" hidden={messageTab}>
            {folder === 'main' && !custom && !query.trim() ? (
              <li>
                <SavedMessagesRow collapsed={collapsed} />
              </li>
            ) : null}
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
          {searching && !messageTab ? <PublicSearchResults query={query} /> : null}
        </ScrollArea>
      </div>
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
