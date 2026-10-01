/**
 * TG-504: one tab's message results — first page on (query, tab, filters) change (debounced),
 * further pages on demand with that tab's own cursor. A stale response never overwrites a
 * newer one.
 */
import { useCallback, useEffect, useRef, useState } from 'react'
import type { GlobalSearchContentType, GlobalSearchFilters, GlobalSearchResult } from '@tg/core'
import { authStore, searchGlobalMessages, selectToken } from '@tg/core'
import { useStore } from 'zustand/react'
import { apiClient } from '../../app/client'

export interface SearchFilterInput {
  senderId: string
  from: string
  to: string
}

export interface MessageSearch {
  items: GlobalSearchResult[]
  loading: boolean
  failed: boolean
  hasMore: boolean
  loadMore: () => void
}

const DEBOUNCE_MS = 250

export function useMessageSearch(
  query: string,
  contentType: GlobalSearchContentType | null,
  filters: SearchFilterInput,
): MessageSearch {
  const token = useStore(authStore, selectToken)
  const [items, setItems] = useState<GlobalSearchResult[]>([])
  const [cursor, setCursor] = useState('')
  const [loading, setLoading] = useState(false)
  const [failed, setFailed] = useState(false)
  const generation = useRef(0)
  const q = query.trim()
  const request: GlobalSearchFilters | null =
    contentType && q && token ? { q, roomId: '', contentType, ...filters } : null
  const key = request ? JSON.stringify(request) : ''

  const fetchPage = useCallback(
    (page: string, append: boolean) => {
      if (!request || !token) return
      const mine = ++generation.current
      setLoading(true)
      searchGlobalMessages(apiClient, token, request, page).then(
        (result) => {
          if (mine !== generation.current) return
          setItems((current) => (append ? [...current, ...result.items] : result.items))
          setCursor(result.next_cursor ?? '')
          setFailed(false)
          setLoading(false)
        },
        () => {
          if (mine !== generation.current) return
          setFailed(true)
          setLoading(false)
        },
      )
    },
    // `key` captures every field of `request`.
    [key, token],
  )

  useEffect(() => {
    setItems([])
    setCursor('')
    if (!key) return
    const timer = setTimeout(() => fetchPage('', false), DEBOUNCE_MS)
    return () => clearTimeout(timer)
  }, [key, fetchPage])

  return {
    items,
    loading,
    failed,
    hasMore: cursor !== '',
    loadMore: () => {
      if (cursor && !loading) fetchPage(cursor, true)
    },
  }
}
