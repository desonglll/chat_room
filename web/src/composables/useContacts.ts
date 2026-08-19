import { computed, ref, watch, type Ref } from 'vue'
import {
  blockUser,
  cancelFriendRequest,
  listBlockedUsers,
  listFriendRequests,
  listFriends,
  removeFriend,
  respondFriendRequest,
  sendFriendRequest,
  unblockUser,
} from '../socialApi'
import type { FriendRequest, SocialUser } from '../types'

export function useContacts(token: Ref<string>) {
  const friends = ref<SocialUser[]>([])
  const incoming = ref<FriendRequest[]>([])
  const outgoing = ref<FriendRequest[]>([])
  const blocked = ref<SocialUser[]>([])
  const loading = ref(false)
  const error = ref('')
  let requestVersion = 0

  async function refresh(): Promise<void> {
    const activeToken = token.value
    const version = ++requestVersion
    if (!activeToken) {
      friends.value = []
      incoming.value = []
      outgoing.value = []
      blocked.value = []
      return
    }
    loading.value = true
    try {
      const [nextFriends, nextIncoming, nextOutgoing, nextBlocked] = await Promise.all([
        listFriends(activeToken),
        listFriendRequests('incoming', activeToken),
        listFriendRequests('outgoing', activeToken),
        listBlockedUsers(activeToken),
      ])
      if (version !== requestVersion || activeToken !== token.value) return
      friends.value = nextFriends
      incoming.value = nextIncoming
      outgoing.value = nextOutgoing
      blocked.value = nextBlocked
      error.value = ''
    } catch (caught) {
      if (version === requestVersion) error.value = caught instanceof Error ? caught.message : '无法读取联系人'
    } finally {
      if (version === requestVersion) loading.value = false
    }
  }

  async function mutate(action: (activeToken: string) => Promise<void>): Promise<void> {
    if (!token.value) return
    await action(token.value)
    await refresh()
  }

  watch(token, () => void refresh(), { immediate: true })

  return {
    friends,
    incoming,
    outgoing,
    blocked,
    loading,
    error,
    incomingCount: computed(() => incoming.value.length),
    refresh,
    sendRequest: (userId: string) => mutate((value) => sendFriendRequest(userId, value)),
    cancelRequest: (userId: string) => mutate((value) => cancelFriendRequest(userId, value)),
    respond: (userId: string, action: 'accept' | 'decline') =>
      mutate((value) => respondFriendRequest(userId, action, value)),
    remove: (userId: string) => mutate((value) => removeFriend(userId, value)),
    block: (userId: string) => mutate((value) => blockUser(userId, value)),
    unblock: (userId: string) => mutate((value) => unblockUser(userId, value)),
  }
}
