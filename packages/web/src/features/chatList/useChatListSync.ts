/**
 * The chat list's live lifecycle, mounted once by the workspace shell: initial load,
 * the account socket (other members' messages + unread snapshot, with resync on
 * reconnect) and the open-timeline mirror (own sends, recalls). Everything it wires is
 * framework-free and tested on its own; this hook only owns the React lifetime.
 */
import { useEffect } from 'react'
import { chatListStore, messageStore, uiStore } from '@tg/core'
import { apiClient } from '../../app/client'
import { accountSocketUrl, browserClock, createBrowserSocket } from '../../app/platform'
import { startAccountFeed } from './accountFeed'
import { loadChatList, loadConversations } from './chatListController'
import { mirrorTimelines } from './timelineMirror'

export function useChatListSync(token: string): void {
  useEffect(() => {
    if (!token) return undefined
    const deps = { client: apiClient, token, store: chatListStore }
    void loadChatList(deps)
    let resyncQueued = false
    const resync = () => {
      if (resyncQueued) return
      resyncQueued = true
      void loadConversations(deps).finally(() => {
        resyncQueued = false
      })
    }
    const stopFeed = startAccountFeed({
      url: accountSocketUrl(),
      token,
      socketFactory: createBrowserSocket,
      clock: browserClock,
      store: chatListStore,
      activeChatId: () => uiStore.getState().activeChatId,
      resync,
    })
    const stopMirror = mirrorTimelines(messageStore, chatListStore)
    return () => {
      stopFeed()
      stopMirror()
    }
  }, [token])
}
