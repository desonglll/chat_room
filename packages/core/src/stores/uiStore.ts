/**
 * Cross-cutting UI state: sidebar width/collapse, the open panel, the modal stack.
 * Values only — layout math and animation live in `packages/web`/`packages/ui`.
 */
import { createStore } from 'zustand/vanilla'

export type WorkspacePanel = 'none' | 'chatInfo' | 'search' | 'settings'

export interface UiState {
  sidebarWidth: number
  sidebarCollapsed: boolean
  activeChatId: string
  activePanel: WorkspacePanel
  modalStack: string[]
  setSidebarWidth(width: number): void
  setSidebarCollapsed(collapsed: boolean): void
  setActiveChat(chatId: string): void
  openPanel(panel: WorkspacePanel): void
  closePanel(): void
  pushModal(key: string): void
  popModal(): void
}

export const DEFAULT_SIDEBAR_WIDTH = 320

export const createUiStore = () =>
  createStore<UiState>()((set) => ({
    sidebarWidth: DEFAULT_SIDEBAR_WIDTH,
    sidebarCollapsed: false,
    activeChatId: '',
    activePanel: 'none',
    modalStack: [],
    setSidebarWidth: (width) => set({ sidebarWidth: Math.max(64, Math.round(width)) }),
    setSidebarCollapsed: (collapsed) => set({ sidebarCollapsed: collapsed }),
    setActiveChat: (chatId) => set({ activeChatId: chatId }),
    openPanel: (panel) => set({ activePanel: panel }),
    closePanel: () => set({ activePanel: 'none' }),
    pushModal: (key) => set((state) => ({ modalStack: [...state.modalStack, key] })),
    popModal: () => set((state) => ({ modalStack: state.modalStack.slice(0, -1) })),
  }))

export type UiStore = ReturnType<typeof createUiStore>

export const selectTopModal = (state: UiState): string => state.modalStack[state.modalStack.length - 1] ?? ''

export const uiStore = createUiStore()
