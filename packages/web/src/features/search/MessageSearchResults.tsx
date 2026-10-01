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
import { t } from '../../i18n/index'

const KIND_GLYPH: Partial<Record<GlobalSearchResult['content_type'], string>> = {
  image: '🖼️',
  video: '🎬',
  audio: '🎵',
  voice: '🎤',
  file: '📄',
}

/**
 * `inline` (TG-904) is Telegram's default search page: message hits listed under the chat hits
 * with a «消息» heading, no filters, and nothing at all when there are none.
 */
export function MessageSearchResults({ query, inline = false }: { query: string; inline?: boolean }) {
  const tabId = useStore(searchStore, (state) => state.tab)
  const tab = SEARCH_TABS.find((item) => item.id === (inline ? 'messages' : tabId))
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

  if (inline && search.items.length === 0) return null
  return (
    <section className="tg-search-results" aria-label={t('w.search.bc89f9', tab?.label ?? '')}>
      {inline ? <h3 className="tg-search-results__section">{tab?.label}</h3> : null}
      {inline ? null : (
        <div className="tg-search-results__filters">
          <select
            aria-label={t('w.search.008629')}
            value={filters.senderId}
            onChange={(event) => setFilters({ ...filters, senderId: event.target.value })}
          >
            <option value="">{t('w.search.f40c84')}</option>
            {senders.map(([id, name]) => (
              <option key={id} value={id}>
                {name}
              </option>
            ))}
          </select>
          <input
            type="date"
            aria-label={t('w.search.1f2919')}
            value={filters.from}
            onChange={(event) => setFilters({ ...filters, from: event.target.value })}
          />
          <input
            type="date"
            aria-label={t('w.search.f4b9b2')}
            value={filters.to}
            onChange={(event) => setFilters({ ...filters, to: event.target.value })}
          />
        </div>
      )}
      {search.failed ? <p className="tg-search-results__empty">{t('w.search.75f73b')}</p> : null}
      {!search.loading && !search.failed && search.items.length === 0 ? (
        <p className="tg-search-results__empty">{t('w.search.a6fb2c')}</p>
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
          {t('w.search.3a0fab')}
        </Button>
      ) : null}
    </section>
  )
}
