/**
 * Whether the viewer may publish in a channel: `message.post` in the server's permission view
 * (read-time half of the post gate; the server re-checks every send). While the answer is
 * loading, the owner/admin role decides, so an administrator's composer does not flicker.
 *
 * TG-1203: in a group or supergroup the same answer decides `message.send`, with the viewer's
 * own restrictions applied by the server — a restricted member used to get a live composer
 * whose sends the server silently dropped. While loading, a group member may write.
 */
import { useEffect, useState } from 'react'
import type { Chat, ChatAdminApi } from '@tg/core'
import { canPublish, canWriteInGroup } from './channelModel'

export function useChannelPublisher(api: ChatAdminApi, chat: Chat | null | undefined): boolean {
  const kind = chat?.chat_type
  const gated = kind === 'channel' || kind === 'group' || kind === 'supergroup'
  const chatId = gated && chat ? chat.id : ''
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
  if (kind !== 'channel') return permissions === null || canWriteInGroup(permissions)
  if (permissions === null) return chat?.membership_role === 'owner' || chat?.membership_role === 'admin'
  return canPublish(permissions)
}
