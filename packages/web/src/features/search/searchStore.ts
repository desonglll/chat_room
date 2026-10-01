/** TG-504: the active search tab and this account's recent searches (TG-1204: per account). */
import { createStore } from 'zustand/vanilla'
import { authStore } from '@tg/core'
import { browserStorage } from '../../app/platform'
import type { SearchTabId } from './searchModel'
import { readRecentSearches, RECENT_SEARCH_KEY, recentSearchKey, rememberSearch } from './searchModel'

export interface SearchState {
  tab: SearchTabId
  recent: string[]
}

/** The signed-in account's storage key; `null` while anonymous (nothing is remembered then). */
function currentKey(): string | null {
  const userId = authStore.getState().session?.user.id
  return userId ? recentSearchKey(userId) : null
}

function readCurrent(): string[] {
  const key = currentKey()
  return key ? readRecentSearches(browserStorage, key) : []
}

// The pre-TG-1204 key was shared by every account on this browser; its owner is unknown.
browserStorage.removeItem(RECENT_SEARCH_KEY)

export const searchStore = createStore<SearchState>()(() => ({
  tab: 'chats',
  recent: readCurrent(),
}))

authStore.subscribe((next, previous) => {
  if (next.session?.user.id !== previous.session?.user.id) searchStore.setState({ tab: 'chats', recent: readCurrent() })
})

export function selectSearchTab(tab: SearchTabId): void {
  searchStore.setState({ tab })
}

export function recordSearch(query: string): void {
  const key = currentKey()
  if (!key) return
  const recent = rememberSearch(searchStore.getState().recent, query)
  searchStore.setState({ recent })
  browserStorage.setItem(key, JSON.stringify(recent))
}

export function clearRecentSearches(): void {
  searchStore.setState({ recent: [] })
  const key = currentKey()
  if (key) browserStorage.removeItem(key)
}
