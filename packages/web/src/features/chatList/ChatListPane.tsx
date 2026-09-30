/**
 * The left pane: account header, minimal chat list, new-group entry. TG-102 replaces
 * the row anatomy (previews, pins, mute, drag width); the pane boundary and the
 * `chatListStore` source stay.
 */
import { useState } from 'react'
import { NavLink, useNavigate } from 'react-router-dom'
import { authStore, chatListStore, selectToken } from '@tg/core'
import { useStore } from 'zustand/react'
import { Avatar, Badge, IconButton, ScrollArea, Skeleton } from '@tg/ui'
import { apiClient } from '../../app/client'
import { browserStorage } from '../../app/platform'
import { signOut } from '../../app/session'
import { SpriteIcon } from '../shell/SpriteIcon'
import { NewChatDialog } from './NewChatDialog'

export function ChatListPane() {
  const user = useStore(authStore, (state) => state.session?.user ?? null)
  const token = useStore(authStore, selectToken)
  const chats = useStore(chatListStore, (state) => state.chats)
  const loading = useStore(chatListStore, (state) => state.loading)
  const failed = useStore(chatListStore, (state) => state.error !== '')
  const [creating, setCreating] = useState(false)
  const navigate = useNavigate()

  return (
    <nav className="tg-chatlist" aria-label="会话列表">
      <header className="tg-chatlist__header">
        {user ? <Avatar label={user.display_name || user.username} initials={user.avatar_emoji || undefined} /> : null}
        <span className="tg-chatlist__account">{user?.display_name || user?.username}</span>
        <IconButton label="新建群组" variant="plain" onClick={() => setCreating(true)}>
          <SpriteIcon name="room-add" size={22} />
        </IconButton>
        <IconButton
          label="退出登录"
          variant="plain"
          onClick={() => void signOut({ client: apiClient, storage: browserStorage, store: authStore })}
        >
          <SpriteIcon name="back" size={20} />
        </IconButton>
      </header>
      <ScrollArea className="tg-chatlist__scroll" orientation="vertical" overlay>
        {loading && chats.length === 0 ? (
          <div className="tg-chatlist__loading" aria-hidden="true">
            <Skeleton variant="rect" height="var(--tg-menu-row-height)" radius="var(--tg-radius-md)" />
            <Skeleton variant="rect" height="var(--tg-menu-row-height)" radius="var(--tg-radius-md)" />
            <Skeleton variant="rect" height="var(--tg-menu-row-height)" radius="var(--tg-radius-md)" />
          </div>
        ) : null}
        {!loading && failed ? <p className="tg-chatlist__empty">会话列表加载失败，请刷新重试</p> : null}
        {!loading && !failed && chats.length === 0 ? (
          <p className="tg-chatlist__empty">还没有会话 — 建一个群，或等别人拉你进来</p>
        ) : null}
        <ul className="tg-chatlist__items">
          {chats.map((chat) => (
            <li key={chat.id}>
              <NavLink
                to={`/chat/${chat.id}`}
                className={({ isActive }) => `tg-chatlist__row${isActive ? ' tg-chatlist__row--active' : ''}`}
              >
                <Avatar label={chat.title} initials={chat.avatar_emoji || undefined} />
                <span className="tg-chatlist__title">{chat.title}</span>
                {chat.unread_count > 0 ? <Badge count={chat.unread_count} /> : null}
              </NavLink>
            </li>
          ))}
        </ul>
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
    </nav>
  )
}
