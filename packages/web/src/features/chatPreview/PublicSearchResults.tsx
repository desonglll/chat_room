/**
 * TG-206: «全局搜索» under the chat list's search results — public chats found by handle
 * prefix or title (`GET /api/chats/discover?q=`). Only chats with a public handle are listed
 * (the others have no page a visitor may open); each row opens `/public/<handle>`.
 */
import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import type { Chat } from '@tg/core'
import { publicChatPath } from '@tg/core'
import { Avatar } from '@tg/ui'
import { publicHandlesApi } from './publicHandlesApi'

const SEARCH_DELAY_MS = 300
const MIN_QUERY = 2

export function PublicSearchResults({
  query,
  api = publicHandlesApi,
}: {
  query: string
  api?: typeof publicHandlesApi
}) {
  const navigate = useNavigate()
  const [results, setResults] = useState<Chat[]>([])
  const needle = query.trim()

  useEffect(() => {
    if (needle.replace(/^@/, '').length < MIN_QUERY) {
      setResults([])
      return
    }
    let alive = true
    const timer = setTimeout(() => {
      api
        .search(needle)
        .then((chats) => alive && setResults(chats.filter((chat) => chat.username)))
        .catch(() => alive && setResults([]))
    }, SEARCH_DELAY_MS)
    return () => {
      alive = false
      clearTimeout(timer)
    }
  }, [api, needle])

  if (results.length === 0) return null
  return (
    <section className="tg-publicsearch" aria-label="全局搜索">
      <h3 className="tg-publicsearch__title">全局搜索</h3>
      <ul className="tg-publicsearch__items">
        {results.map((chat) => (
          <li key={chat.id}>
            <button
              type="button"
              className="tg-publicsearch__row"
              onClick={() => navigate(publicChatPath(chat.username ?? ''))}
            >
              <Avatar label={chat.title} initials={chat.avatar_emoji || undefined} size="md" />
              <span className="tg-publicsearch__text">
                <span className="tg-publicsearch__name">{chat.title}</span>
                <span className="tg-publicsearch__meta">
                  @{chat.username} · {chat.member_count} {chat.chat_type === 'channel' ? '位订阅者' : '位成员'}
                </span>
              </span>
            </button>
          </li>
        ))}
      </ul>
    </section>
  )
}
