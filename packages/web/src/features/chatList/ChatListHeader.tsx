/**
 * The sidebar's top bar, Telegram's shape: hamburger menu + search pill; in the archive
 * folder a back arrow + title; in the collapsed column only the hamburger.
 */
import { useRef, useState } from 'react'
import type { MenuItem } from '@tg/ui'
import { IconButton, Menu } from '@tg/ui'
import type { ChatListFolder } from './chatListFilters'
import { ChatListIcon } from './chatListIcons'

export interface ChatListHeaderProps {
  folder: ChatListFolder
  collapsed: boolean
  query: string
  onQueryChange: (query: string) => void
  onCloseFolder: () => void
  menuItems: readonly MenuItem[]
}

export function ChatListHeader({
  folder,
  collapsed,
  query,
  onQueryChange,
  onCloseFolder,
  menuItems,
}: ChatListHeaderProps) {
  const [menuOpen, setMenuOpen] = useState(false)
  const triggerRef = useRef<HTMLButtonElement | null>(null)
  const inputRef = useRef<HTMLInputElement | null>(null)

  const leading =
    folder === 'archive' ? (
      <IconButton label="返回会话列表" variant="plain" onClick={onCloseFolder}>
        <ChatListIcon name="back" size={22} />
      </IconButton>
    ) : (
      <IconButton
        ref={triggerRef}
        label="主菜单"
        variant="plain"
        aria-haspopup="menu"
        aria-expanded={menuOpen}
        onClick={() => setMenuOpen((open) => !open)}
      >
        <ChatListIcon name="menu" size={22} />
      </IconButton>
    )

  return (
    <header className="tg-chatlist__header">
      {leading}
      {collapsed ? null : folder === 'archive' ? (
        <h2 className="tg-chatlist__folder-title">已归档会话</h2>
      ) : (
        <label className="tg-chatlist__search">
          <ChatListIcon name="search" size={20} className="tg-chatlist__search-icon" />
          <input
            ref={inputRef}
            className="tg-chatlist__search-input"
            type="search"
            placeholder="搜索"
            aria-label="搜索会话"
            value={query}
            onChange={(event) => onQueryChange(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === 'Escape' && query) {
                event.preventDefault()
                onQueryChange('')
              }
            }}
          />
          {query ? (
            <button
              type="button"
              className="tg-chatlist__search-clear"
              aria-label="清除搜索"
              onClick={() => {
                onQueryChange('')
                inputRef.current?.focus()
              }}
            >
              <ChatListIcon name="close" size={16} />
            </button>
          ) : null}
        </label>
      )}
      <Menu
        open={menuOpen}
        onClose={() => setMenuOpen(false)}
        anchor={triggerRef}
        triggerRef={triggerRef}
        items={menuItems}
        aria-label="主菜单"
      />
    </header>
  )
}
