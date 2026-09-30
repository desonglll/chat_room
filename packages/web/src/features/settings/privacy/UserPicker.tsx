/**
 * Username search for adding an exception. Results come from `GET /api/users/search`;
 * the server requires 2–64 characters, so shorter queries do not hit the network.
 */
import { useEffect, useState } from 'react'
import { Avatar, Button, Spinner, TextField } from '@tg/ui'
import type { PrivacyApi, PrivacyUser } from '@tg/core'
import { privacyUserName } from './privacyCopy'

const SEARCH_DEBOUNCE_MS = 250

export interface UserPickerProps {
  api: Pick<PrivacyApi, 'searchUsers'>
  title: string
  excludeIds: readonly string[]
  onPick: (user: PrivacyUser) => void
  onCancel: () => void
}

export function UserPicker({ api, title, excludeIds, onPick, onCancel }: UserPickerProps) {
  const [query, setQuery] = useState('')
  const [results, setResults] = useState<PrivacyUser[]>([])
  const [state, setState] = useState<'idle' | 'loading' | 'error'>('idle')

  useEffect(() => {
    const trimmed = query.trim()
    if (trimmed.length < 2) {
      setResults([])
      setState('idle')
      return
    }
    let cancelled = false
    setState('loading')
    const timer = setTimeout(() => {
      api.searchUsers(trimmed).then(
        (users) => {
          if (cancelled) return
          setResults(users)
          setState('idle')
        },
        () => {
          if (!cancelled) setState('error')
        },
      )
    }, SEARCH_DEBOUNCE_MS)
    return () => {
      cancelled = true
      clearTimeout(timer)
    }
  }, [api, query])

  const visible = results.filter((user) => !excludeIds.includes(user.id))
  return (
    <section className="tg-privacy__picker" aria-label={title}>
      <h3 className="tg-privacy__section-title">{title}</h3>
      <TextField
        label="搜索用户名"
        value={query}
        autoFocus
        clearable
        clearLabel="清除"
        onClear={() => setQuery('')}
        onChange={(event) => setQuery(event.currentTarget.value)}
        error={state === 'error' ? '搜索失败，请稍后重试' : undefined}
      />
      {state === 'loading' ? <Spinner size="sm" label="正在搜索" /> : null}
      <ul className="tg-privacy__users">
        {visible.map((user) => {
          const name = privacyUserName(user)
          return (
            <li key={user.id}>
              <button type="button" className="tg-privacy__user tg-privacy__user--pick" onClick={() => onPick(user)}>
                <Avatar label={name} size="sm" />
                <span className="tg-privacy__user-name">{name}</span>
                <span className="tg-privacy__user-handle">@{user.username}</span>
              </button>
            </li>
          )
        })}
      </ul>
      <Button variant="text" size="sm" onClick={onCancel}>
        取消
      </Button>
    </section>
  )
}
