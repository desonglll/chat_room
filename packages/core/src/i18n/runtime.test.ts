import { afterEach, describe, expect, test } from 'bun:test'
import { localeStore, registerMessages, setLocale, translate } from './runtime'

registerMessages('zh-CN', { 'test.members': '{0} 位成员', 'test.msgs': { other: '{0} 条消息' } })
registerMessages('en', {
  'test.members': { one: '{0} member', other: '{0} members' },
  'test.msgs': { one: '{0} message', other: '{0} messages' },
})

afterEach(() => setLocale('zh-CN'))

describe('TG-510 i18n runtime', () => {
  test('slots are filled and missing keys fall back to the key', () => {
    expect(translate('test.members', 3)).toBe('3 位成员')
    expect(translate('missing.key')).toBe('missing.key')
  })

  test('plural forms follow each language’s own rule', () => {
    setLocale('en')
    expect(translate('test.members', 1)).toBe('1 member')
    expect(translate('test.members', 2)).toBe('2 members')
    expect(translate('test.msgs', 0)).toBe('0 messages')
    setLocale('zh-CN')
    expect(translate('test.msgs', 1)).toBe('1 条消息')
  })

  test('switching locale is a store change (hosts re-render, no reload)', () => {
    const seen: string[] = []
    const stop = localeStore.subscribe((state) => seen.push(state.locale))
    setLocale('en')
    setLocale('en')
    stop()
    expect(seen).toEqual(['en'])
  })
})

describe('TG-510 core catalogs', () => {
  test('English covers every core key, with the same slots', async () => {
    const { zh } = await import('./zh')
    const { en } = await import('./en')
    const slots = (message: unknown) =>
      JSON.stringify(
        [...new Set((typeof message === 'string' ? message : JSON.stringify(message)).match(/\{\d+\}/g) ?? [])].sort(),
      )
    for (const [key, message] of Object.entries(zh)) {
      expect(en[key], key).toBeDefined()
      expect(slots(en[key]), key).toBe(slots(message))
    }
  })
})
