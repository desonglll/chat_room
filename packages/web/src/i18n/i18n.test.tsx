import { afterEach, describe, expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { catalogKeys, setLocale, translate } from '@tg/core'
import { SearchTabs } from '../features/search/SearchTabs'
import { en } from './en/index'
import { zh } from './zh/index'
import './index'

afterEach(() => setLocale('zh-CN'))

const slots = (message: unknown) => JSON.stringify([...new Set(JSON.stringify(message).match(/\{\d+\}/g) ?? [])].sort())

describe('TG-510 web catalogs', () => {
  test('English has every source key, with the same slots', () => {
    expect(Object.keys(zh).length).toBeGreaterThan(1000)
    for (const [key, message] of Object.entries(zh)) {
      expect(en[key], key).toBeDefined()
      expect(slots(en[key]), key).toBe(slots(message))
    }
    expect(Object.keys(en).sort()).toEqual(Object.keys(zh).sort())
    expect(catalogKeys('en').length).toBeGreaterThanOrEqual(Object.keys(en).length)
  })

  test('switching the locale switches rendered copy, plurals included', () => {
    expect(renderToStaticMarkup(<SearchTabs query="" onPick={() => {}} />)).toContain('聊天')
    setLocale('en')
    const html = renderToStaticMarkup(<SearchTabs query="" onPick={() => {}} />)
    expect(html).toContain('Chats')
    expect(html).not.toContain('聊天')
    expect(translate('w.channel.764250', 1)).toBe('1 comment')
    expect(translate('w.channel.764250', 5)).toBe('5 comments')
  })
})
