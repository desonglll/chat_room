/**
 * The panel's data: the header model (store rows + the peer's profile), the viewer's
 * notification preference with an optimistic toggle, and one pager per tab.
 *
 * Pagers are created per chatId and live as long as the panel shows that chat, so
 * switching tabs keeps every tab's loaded pages and cursor (independent pagination).
 */
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import type { ApiClient, ChatMembership, ConversationPreferences, User } from '@tg/core'
import { authStore, chatListStore, presenceStore, selectChatById, selectToken } from '@tg/core'
import { useStore } from 'zustand/react'
import {
  getConversationPreferences,
  getUserProfile,
  notificationsEnabled,
  notificationsPatch,
  updateConversationPreferences,
} from './chatInfoApi'
import type { InfoHeaderModel } from './chatInfoModel'
import { selectInfoHeader } from './chatInfoModel'
import { chatAdminApi } from '../chatAdmin/chatAdminApi'
import { createMemberPageSource } from '../chatAdmin/memberPageSource'
import { createMemberSource } from './memberSource'
import type { SharedPager } from './sharedPager'
import { createSharedPager } from './sharedPager'
import type { SharedFile, SharedLink, SharedSources } from './sharedSources'
import { createSharedSources } from './sharedSources'

export interface ChatInfoPagers {
  media: SharedPager<SharedFile>
  files: SharedPager<SharedFile>
  links: SharedPager<SharedLink>
  music: SharedPager<SharedFile>
  voice: SharedPager<SharedFile>
  gif: SharedPager<SharedFile>
  members: SharedPager<ChatMembership>
}

/** Test/screenshot injection: replaces the REST sources, keeps everything else real. */
export interface ChatInfoSourceOverrides {
  shared?: SharedSources
  loadMembers?: () => Promise<ChatMembership[]>
}

export function createChatInfoPagers(
  chatId: string,
  client: ApiClient,
  token: () => string,
  overrides: ChatInfoSourceOverrides = {},
): ChatInfoPagers {
  const shared = overrides.shared ?? createSharedSources(chatId, { client, token })
  const online = () => new Set(presenceStore.getState().chats[chatId]?.participants.map((p) => p.user_id) ?? [])
  // TG-201: the server-paged roster (keyset cursor, readable by every member) unless a test
  // injects a one-shot loader.
  const members = overrides.loadMembers
    ? createMemberSource(overrides.loadMembers, online)
    : createMemberPageSource(chatAdminApi, chatId)
  const byKey = (item: { key: string }) => item.key
  return {
    media: createSharedPager(shared.media, byKey),
    files: createSharedPager(shared.files, byKey),
    links: createSharedPager(shared.links, byKey),
    music: createSharedPager(shared.music, byKey),
    voice: createSharedPager(shared.voice, byKey),
    gif: createSharedPager(shared.gif, byKey),
    members: createSharedPager(members, (member) => member.user_id),
  }
}

export interface ChatInfo {
  header: InfoHeaderModel | null
  pagers: ChatInfoPagers
  notificationsOn: boolean
  notificationsBusy: boolean
  setNotifications(enabled: boolean): void
}

export function useChatInfo(chatId: string, client: ApiClient, overrides?: ChatInfoSourceOverrides): ChatInfo {
  const token = useStore(authStore, selectToken)
  const currentUserId = useStore(authStore, (state) => state.session?.user.id ?? '')
  const chat = useStore(chatListStore, selectChatById(chatId))
  const conversation = useStore(
    chatListStore,
    (state) => state.conversations.find((row) => row.room_id === chatId) ?? null,
  )
  const members = useStore(presenceStore, (state) => state.chats[chatId]?.members)
  const [peerProfile, setPeerProfile] = useState<User | null>(null)
  const [fetchedPreferences, setFetchedPreferences] = useState<ConversationPreferences | null>(null)
  const [busy, setBusy] = useState(false)

  const peerIdFallback = members?.find((member) => member.user_id !== currentUserId)?.user_id ?? ''
  const baseHeader = selectInfoHeader({ chatId, chat, conversation, peerProfile: null, peerIdFallback })
  const peerId = baseHeader?.variant === 'private' ? baseHeader.userId : ''

  useEffect(() => {
    setPeerProfile(null)
    if (!peerId || !token) return
    let live = true
    getUserProfile(client, token, peerId).then(
      (profile) => live && setPeerProfile(profile),
      () => undefined, // the header still renders from the sidebar row
    )
    return () => {
      live = false
    }
  }, [client, token, peerId])

  // The sidebar row usually carries the preference; fetch it only when it does not.
  const hasRow = conversation !== null
  useEffect(() => {
    setFetchedPreferences(null)
    if (hasRow || !token || !chatId) return
    let live = true
    getConversationPreferences(client, token, chatId).then(
      (preferences) => live && setFetchedPreferences(preferences),
      () => undefined,
    )
    return () => {
      live = false
    }
  }, [client, token, chatId, hasRow])

  const preferences = conversation?.preferences ?? fetchedPreferences
  const notificationsOn = notificationsEnabled(preferences, Date.now())

  const setNotifications = useCallback(
    (enabled: boolean) => {
      if (!token) return
      const patch = notificationsPatch(enabled)
      const apply = (next: ConversationPreferences | null, patchOnly = false) => {
        const store = chatListStore.getState()
        store.setConversations(
          store.conversations.map((row) =>
            row.room_id !== chatId
              ? row
              : { ...row, preferences: patchOnly || !next ? { ...row.preferences, ...patch } : next },
          ),
        )
        if (next) setFetchedPreferences(next)
      }
      const previous = preferences
      apply(null, true)
      setBusy(true)
      updateConversationPreferences(client, token, chatId, patch)
        .then((next) => apply(next))
        .catch(() => previous && apply(previous))
        .finally(() => setBusy(false))
    },
    [client, token, chatId, preferences],
  )

  const tokenRef = useRef(token)
  tokenRef.current = token
  const pagers = useMemo(
    () => createChatInfoPagers(chatId, client, () => tokenRef.current, overrides),
    [chatId, client, overrides],
  )

  const header = selectInfoHeader({ chatId, chat, conversation, peerProfile, peerIdFallback })
  return { header, pagers, notificationsOn, notificationsBusy: busy, setNotifications }
}
