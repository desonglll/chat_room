/**
 * Whether the viewer may publish in a channel: `message.post` in the server's permission view
 * (read-time half of the post gate; the server re-checks every send). While the answer is
 * loading, the owner/admin role decides, so an administrator's composer does not flicker.
 */
import { useEffect, useState } from 'react'
import type { Chat, ChatAdminApi } from '@tg/core'
import { canPublish } from './channelModel'

export function useChannelPublisher(api: ChatAdminApi, chat: Chat | null | undefined): boolean {
  const chatId = chat?.chat_type === 'channel' ? chat.id : ''
  const [permissions, setPermissions] = useState<string[] | null>(null)
  useEffect(() => {
    setPermissions(null)
    if (!chatId) return
    let live = true
    api.permissions(chatId).then(
      (view) => live && setPermissions(view.my_permissions),
      () => live && setPermissions([]),
    )
    return () => {
      live = false
    }
  }, [api, chatId])
  if (!chatId) return true
  if (permissions === null) return chat?.membership_role === 'owner' || chat?.membership_role === 'admin'
  return canPublish(permissions)
}
