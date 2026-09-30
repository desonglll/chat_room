/**
 * The three-pane workspace: chat list | conversation | info panel. The middle pane is
 * the router outlet (`/` empty state, `/chat/:chatId` conversation); the right pane
 * mounts only while `uiStore.activePanel` asks for it, as in Telegram.
 *
 * Layout ownership (TG-102): the sidebar width/collapse (drag + persisted), and the
 * mobile single-column switch. On mobile both columns stay mounted at full width and
 * the switch is a transform — no reflow, so no layout jump — and the hidden column is
 * `inert`. The info pane is a column on wide screens and an overlay below that.
 */
import type { CSSProperties } from 'react'
import { Outlet, useMatch } from 'react-router-dom'
import { authStore, selectToken, uiStore } from '@tg/core'
import { useStore } from 'zustand/react'
import { ChatListPane } from '../chatList/ChatListPane'
import { useChatListSync } from '../chatList/useChatListSync'
import { InfoPane } from './InfoPane'
import { MobileBackButton } from './MobileBackButton'
import { SIDEBAR_COLLAPSED_WIDTH } from './sidebarLayout'
import { SidebarResizer } from './SidebarResizer'
import { useMobileLayout } from './useMobileLayout'
import { useSidebarLayout } from './useSidebarLayout'

export function WorkspaceShell() {
  const token = useStore(authStore, selectToken)
  const infoOpen = useStore(uiStore, (state) => state.activePanel === 'chatInfo')
  const chatOpen = useMatch('/chat/:chatId') !== null
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
      </div>
      <section className="tg-shell__main" inert={mobile && !chatOpen}>
        {chatOpen ? <MobileBackButton className="tg-shell__back" /> : null}
        <Outlet />
      </section>
      {infoOpen ? <InfoPane /> : null}
    </div>
  )
}
