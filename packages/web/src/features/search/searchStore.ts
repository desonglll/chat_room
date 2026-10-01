/** TG-504: the active search tab and this browser's recent searches. */
import { createStore } from 'zustand/vanilla'
import { browserStorage } from '../../app/platform'
import type { SearchTabId } from './searchModel'
import { readRecentSearches, RECENT_SEARCH_KEY, rememberSearch } from './searchModel'

export interface SearchState {
  tab: SearchTabId
  recent: string[]
}

export const searchStore = createStore<SearchState>()(() => ({
  tab: 'chats',
  recent: readRecentSearches(browserStorage),
}))

export function selectSearchTab(tab: SearchTabId): void {
  searchStore.setState({ tab })
}

export function recordSearch(query: string): void {
  const recent = rememberSearch(searchStore.getState().recent, query)
  searchStore.setState({ recent })
  browserStorage.setItem(RECENT_SEARCH_KEY, JSON.stringify(recent))
}

export function clearRecentSearches(): void {
  searchStore.setState({ recent: [] })
  browserStorage.removeItem(RECENT_SEARCH_KEY)
}
