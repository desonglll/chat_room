/**
 * TG-606: installs the app-wide keyboard map (`keymap.ts`) on the workspace. Chat switching
 * walks the chat list as it is shown (folder, search and archive included), so the keys always
 * agree with what the user sees.
 */
import { useEffect } from 'react'
import { folderStore, selectFolder } from '../folders/folderStore'
import { openSettings } from '../settings/shell/settingsNavigation'
import { globalShortcut, neighbourChat } from './keymap'

function isEditable(target: EventTarget | null): boolean {
  const element = target as HTMLElement | null
  return !!element && (element.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(element.tagName))
}

function visibleChatIds(): string[] {
  return [...document.querySelectorAll<HTMLAnchorElement>('.tg-chatlist a[href^="/chat/"]')].map((link) =>
    decodeURIComponent(link.pathname.split('/')[2] ?? ''),
  )
}

export function useGlobalShortcuts(navigate: (path: string) => void): void {
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.defaultPrevented || event.isComposing) return
      const action = globalShortcut({
        key: event.key,
        code: event.code,
        ctrlKey: event.ctrlKey,
        metaKey: event.metaKey,
        altKey: event.altKey,
        shiftKey: event.shiftKey,
        editable: isEditable(event.target),
      })
      if (!action) return
      event.preventDefault()
      if (action.type === 'search') {
        document.querySelector<HTMLInputElement>('.tg-chatlist__search-input')?.focus()
      } else if (action.type === 'chat') {
        const current = decodeURIComponent(/^\/chat\/([^/]+)/.exec(window.location.pathname)?.[1] ?? '')
        const next = neighbourChat(visibleChatIds(), current, action.step)
        if (next) navigate(`/chat/${encodeURIComponent(next)}`)
      } else if (action.type === 'folder') {
        const folders = folderStore.getState().folders
        if (action.index === 1) selectFolder(null)
        else if (folders[action.index - 2]) selectFolder(folders[action.index - 2]!.id)
      } else {
        openSettings()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [navigate])
}
