import { describe, expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { globalSearchParams } from '@tg/core'
import {
  readRecentSearches,
  rememberSearch,
  RECENT_SEARCH_KEY,
  RECENT_SEARCH_MAX,
  resultDay,
  SEARCH_TABS,
  suggestSearches,
} from './searchModel'
import { SearchTabs } from './SearchTabs'

describe('TG-504 search tabs', () => {
  test('seven tabs; every message tab is its own server filter', () => {
    expect(SEARCH_TABS.map((tab) => tab.label)).toEqual(['聊天', '消息', '媒体', '链接', '文件', '音乐', '语音'])
    const types = SEARCH_TABS.map((tab) => tab.contentType).filter(Boolean)
    expect(new Set(types).size).toBe(6)
    const params = globalSearchParams(
      { q: 'x', roomId: '', senderId: 'u1', from: '', to: '', contentType: 'voice' },
      'c1',
    )
    expect(params.get('content_type')).toBe('voice')
    expect(params.get('sender_id')).toBe('u1')
    expect(params.get('cursor')).toBe('c1')
  })

  test('recent searches: newest first, deduplicated, capped, robust to bad storage', () => {
    let recent: string[] = []
    for (const query of ['a', 'B', 'b', '  ', 'c']) recent = rememberSearch(recent, query)
    expect(recent).toEqual(['c', 'b', 'a'])
    for (let index = 0; index < 20; index += 1) recent = rememberSearch(recent, `q${index}`)
    expect(recent).toHaveLength(RECENT_SEARCH_MAX)
    const storage = { getItem: () => '{oops', setItem() {}, removeItem() {} }
    expect(readRecentSearches(storage)).toEqual([])
    const good = {
      getItem: (key: string) => (key === RECENT_SEARCH_KEY ? '["x", 1, "y"]' : null),
      setItem() {},
      removeItem() {},
    }
    expect(readRecentSearches(good)).toEqual(['x', 'y'])
  })

  test('suggestions extend what is typed; with nothing typed they are the latest searches', () => {
    const recent = ['project plan', 'proj', 'photos', 'invoice']
    expect(suggestSearches(recent, 'proj')).toEqual(['project plan'])
    expect(suggestSearches(recent, '')).toEqual(['project plan', 'proj', 'photos'])
  })

  test('result day and the tab strip render', () => {
    expect(resultDay('not a date')).toBe('')
    expect(resultDay('2026-10-01T12:00:00')).toBe('2026-10-01')
    const html = renderToStaticMarkup(<SearchTabs query="" onPick={() => {}} />)
    expect(html).toContain('role="tablist"')
    expect(html).toContain('aria-selected="true"')
  })
})
