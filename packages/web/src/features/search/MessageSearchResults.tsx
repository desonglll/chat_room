/**
 * TG-504: a message tab's results with sender and date filters. Opening a result jumps to the
 * message (`/chat/:id?message=`) and records the query as a recent search.
 */
import { useMemo, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import type { GlobalSearchResult } from '@tg/core'
import { Button } from '@tg/ui'
import { useStore } from 'zustand/react'
import { resultDay, SEARCH_TABS } from './searchModel'
import { recordSearch, searchStore } from './searchStore'
import type { SearchFilterInput } from './useMessageSearch'
import { useMessageSearch } from './useMessageSearch'

const KIND_GLYPH: Partial<Record<GlobalSearchResult['content_type'], string>> = {
  image: '🖼️',
  video: '🎬',
  audio: '🎵',
  voice: '🎤',
  file: '📄',
}

export function MessageSearchResults({ query }: { query: string }) {
  const tabId = useStore(searchStore, (state) => state.tab)
  const tab = SEARCH_TABS.find((item) => item.id === tabId)
  const [filters, setFilters] = useState<SearchFilterInput>({ senderId: '', from: '', to: '' })
  const search = useMessageSearch(query, tab?.contentType ?? null, filters)
  const navigate = useNavigate()
  const senders = useMemo(() => {
    const seen = new Map<string, string>()
    for (const item of search.items) if (item.sender_id) seen.set(item.sender_id, item.sender)
    return [...seen]
  }, [search.items])

  const open = (item: GlobalSearchResult) => {
    recordSearch(query)
    void navigate(`/chat/${encodeURIComponent(item.room_id)}?message=${encodeURIComponent(item.message_id)}`)
  }

  return (
    <section className="tg-search-results" aria-label={`${tab?.label ?? ''}搜索结果`}>
      <div className="tg-search-results__filters">
        <select
          aria-label="发送者"
          value={filters.senderId}
          onChange={(event) => setFilters({ ...filters, senderId: event.target.value })}
        >
          <option value="">所有人</option>
          {senders.map(([id, name]) => (
            <option key={id} value={id}>
              {name}
            </option>
          ))}
        </select>
        <input
          type="date"
          aria-label="开始日期"
          value={filters.from}
          onChange={(event) => setFilters({ ...filters, from: event.target.value })}
        />
        <input
          type="date"
          aria-label="结束日期"
          value={filters.to}
          onChange={(event) => setFilters({ ...filters, to: event.target.value })}
        />
      </div>
      {search.failed ? <p className="tg-search-results__empty">搜索失败，请重试</p> : null}
      {!search.loading && !search.failed && search.items.length === 0 ? (
        <p className="tg-search-results__empty">没有找到结果</p>
      ) : null}
      <ul className="tg-search-results__list">
        {search.items.map((item) => (
          <li key={item.message_id}>
            <button type="button" className="tg-search-results__item" onClick={() => open(item)}>
              <span className="tg-search-results__head">
                <span className="tg-search-results__chat">{item.conversation_title}</span>
                <span className="tg-search-results__day">{resultDay(item.created_at)}</span>
              </span>
              <span className="tg-search-results__body">
                {KIND_GLYPH[item.content_type] ? (
                  <span aria-hidden="true">{KIND_GLYPH[item.content_type]} </span>
                ) : null}
                <span className="tg-search-results__sender">{item.sender}：</span>
                {item.excerpt || item.attachment_file_name || ''}
              </span>
            </button>
          </li>
        ))}
      </ul>
      {search.hasMore ? (
        <Button variant="text" loading={search.loading} onClick={search.loadMore}>
          加载更多
        </Button>
      ) : null}
    </section>
  )
}
