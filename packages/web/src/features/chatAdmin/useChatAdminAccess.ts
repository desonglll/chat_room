/**
 * Whether the viewer gets the "管理群组" entry for a chat, and the chat type to show beside
 * it. One `GET /permissions` per chat; the server decides, this only reads the answer.
 */
import { useEffect, useState } from 'react'
import type { ChatAdminApi, ChatPermissionsView } from '@tg/core'
import { adminCapabilities, type AdminCapabilities } from './chatAdminModel'

export interface ChatAdminAccess {
  view: ChatPermissionsView | null
  capabilities: AdminCapabilities
}

export function useChatAdminAccess(api: ChatAdminApi, chatId: string | null, reloadKey = 0): ChatAdminAccess {
  const [view, setView] = useState<ChatPermissionsView | null>(null)
  useEffect(() => {
    setView(null)
    if (!chatId) return
    let live = true
    api.permissions(chatId).then(
      (answer) => live && setView(answer),
      () => live && setView(null),
    )
    return () => {
      live = false
    }
  }, [api, chatId, reloadKey])
  return { view, capabilities: adminCapabilities(view) }
}
