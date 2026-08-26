import { describe, expect, test } from 'bun:test'
import { readFileSync } from 'node:fs'

function source(path: string): string {
  return readFileSync(new URL(path, import.meta.url), 'utf8')
}

describe('mobile workspace layout contract', () => {
  test('keeps profile and global navigation reachable on workspace pages', () => {
    expect(source('./components/WorkspaceRail.vue')).toContain('cr-rail-mobile-profile')
    expect(source('./components/WorkspacePages.vue')).toContain('cr-workspace-pages')
    expect(source('./components/RoomSidebar.vue')).toContain('cr-sidebar--nav-only')
  })

  test('reserves mobile navigation space for chat and workspace surfaces', () => {
    const workspace = source('./workspace.css')
    expect(workspace).toContain('--cr-mobile-nav-height')
    expect(workspace).toContain('.cr-workspace-pages')
    expect(workspace).toContain('.cr-sidebar--nav-only')
  })

  test('does not let the mobile navigation shell cover an open room', () => {
    const workspace = source('./workspace.css')
    expect(workspace).toMatch(/\.cr-sidebar--nav-only\s*{[^}]*background:\s*transparent;/s)
  })

  test('uses compact scrollable AI controls instead of a fixed-height thread panel', () => {
    expect(source('./components/AiAssistantPage.vue')).toContain('grid-rows-[auto_minmax(0,1fr)]')
    expect(source('./components/AiAssistantToolbar.vue')).toContain('overflow-x-auto')
    expect(source('./components/AiThreadSidebar.vue')).toContain('overflow-x-auto')
  })

  test('hides the conversation list while another workspace owns the sidebar', () => {
    const app = source('./App.vue')
    expect(app).toContain(':collapsed="sidebarCollapsed || activePage !== \'chat\' || roomAiPanelVisible"')
    expect(app).toContain(":visible=\"activePage === 'chat' && mobileView === 'rooms'\"")
  })

  test('embeds the current room AI beside chat on desktop and as an overlay on mobile', () => {
    const app = source('./App.vue')
    const assistant = source('./components/AiAssistantPage.vue')
    const header = source('./components/ChatRoomHeader.vue')
    expect(app).toContain('60px minmax(20rem,1fr) minmax(22rem,32rem)')
    expect(app).toContain(':initial-room-id="selectedRoom.id"')
    expect(assistant).toContain('absolute inset-0 z-40')
    expect(assistant).toContain('md:relative md:inset-auto')
    expect(header).toContain("emit('assistant')")
  })
})
