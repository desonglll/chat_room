/**
 * The three-pane workspace: chat list | conversation | info panel. The middle pane is
 * the router outlet (`/` empty state, `/chat/:chatId` conversation); the right pane is
 * always mounted and renders only while `uiStore.activePanel` asks for it (or while its
 * exit slide plays), as in Telegram.
 *
 * Layout ownership (TG-102): the sidebar width/collapse (drag + persisted), and the
 * mobile single-column switch. On mobile both columns stay mounted at full width and
 * the switch is a transform — no reflow, so no layout jump — and the hidden column is
 * `inert`; the way back is the chat header's own back button. The info pane is a column
 * on wide screens and an overlay below that. The settings panel (TG-110) slides over the
 * sidebar from inside it. `ChatOverlays` (media viewer, forward
 * picker, delete confirmation) is mounted once here, above every pane.
 */
import type { CSSProperties } from 'react'
import { Outlet, useMatch } from 'react-router-dom'
import { authStore, selectToken, uiStore } from '@tg/core'
import { useStore } from 'zustand/react'
import { ChatOverlays } from '../chat/ChatOverlays'
import { ChatListPane } from '../chatList/ChatListPane'
import { useChatListSync } from '../chatList/useChatListSync'
import { SettingsHost } from '../settings/shell'
import { StickerOverlays } from '../sticker/StickerOverlays'
import { ChatWallpaper } from '../wallpaper/ChatWallpaper'
import { UpdateBanner } from '../pwa/UpdateBanner'
import { InfoPane } from './InfoPane'
import { SIDEBAR_COLLAPSED_WIDTH } from './sidebarLayout'
import { SidebarResizer } from './SidebarResizer'
import { useMobileLayout } from './useMobileLayout'
import { useSidebarLayout } from './useSidebarLayout'

export function WorkspaceShell() {
  const token = useStore(authStore, selectToken)
  const infoOpen = useStore(uiStore, (state) => state.activePanel === 'chatInfo')
  const chatMatch = useMatch('/chat/:chatId/*')
  const chatOpen = chatMatch !== null
  const mobile = useMobileLayout()
  const { layout, preview, commit, toggleCollapsed } = useSidebarLayout()

  useChatListSync(token)

  const collapsed = layout.collapsed && !mobile
  const style = {
    '--tg-shell-sidebar-width': `${collapsed ? SIDEBAR_COLLAPSED_WIDTH : layout.width}px`,
  } as CSSProperties

  return (
    <div
      className="tg-shell"
      style={style}
      data-info-open={infoOpen || undefined}
      data-sidebar-collapsed={collapsed || undefined}
      data-mobile-view={chatOpen ? 'chat' : 'list'}
    >
      <div className="tg-shell__sidebar" inert={mobile && chatOpen}>
        <ChatListPane collapsed={collapsed} onToggleCollapsed={mobile ? undefined : toggleCollapsed} />
        {mobile ? null : <SidebarResizer layout={layout} onPreview={preview} onCommit={commit} />}
        <SettingsHost />
      </div>
      <section className="tg-shell__main" inert={mobile && !chatOpen}>
        {/* TG-507: the chat's (or the global) wallpaper, on its own layer behind the chat. */}
        <ChatWallpaper chatId={chatMatch?.params.chatId ?? ''} />
        <Outlet />
      </section>
      <InfoPane />
      <ChatOverlays />
      <StickerOverlays />
      {/* TG-601: «有新版本» when a new service worker is waiting. */}
      <UpdateBanner />
    </div>
  )
}
