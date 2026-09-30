/**
 * The right pane mount. The panel itself is TG-106's `features/chatInfo`; this file keeps
 * the shell's contract (`uiStore.activePanel === 'chatInfo'`) and adds the exit slide:
 * once the store says closed, the panel stays mounted until its exit animation ends.
 * (The exit only plays when the shell renders `<InfoPane />` unconditionally — see the
 * TG-106 integration patch list; with the conditional mount it simply unmounts.)
 */
import { useEffect, useState } from 'react'
import { uiStore } from '@tg/core'
import { useStore } from 'zustand/react'
import { apiClient } from '../../app/client'
import { ChatInfoPanel, watchPanelAnchor } from '../chatInfo'

// The message list keeps its anchor across every open/close, whoever triggers it (header
// click, close button, Escape). Module scope: the pin must be taken synchronously inside
// `openPanel`, before this pane even exists. A no-op outside the browser.
watchPanelAnchor(uiStore)

/** Safety net for a missed `animationend` (tab hidden mid-animation). */
const EXIT_FALLBACK_MS = 700

export interface InfoPaneProps {
  onSearchInChat?: ((chatId: string) => void) | undefined
}

export function InfoPane({ onSearchInChat }: InfoPaneProps) {
  const chatId = useStore(uiStore, (state) => state.activeChatId)
  const open = useStore(uiStore, (state) => state.activePanel === 'chatInfo')
  const closePanel = useStore(uiStore, (state) => state.closePanel)
  const [mounted, setMounted] = useState(open)
  if (open && !mounted) setMounted(true)

  useEffect(() => {
    if (open || !mounted) return
    const timer = setTimeout(() => setMounted(false), EXIT_FALLBACK_MS)
    return () => clearTimeout(timer)
  }, [open, mounted])

  if (!mounted || !chatId) return null
  return (
    <ChatInfoPanel
      chatId={chatId}
      client={apiClient}
      onClose={closePanel}
      onSearchInChat={onSearchInChat}
      closing={!open}
      onExited={() => setMounted(false)}
    />
  )
}
