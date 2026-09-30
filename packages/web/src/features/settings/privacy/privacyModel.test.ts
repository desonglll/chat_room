import { describe, expect, test } from 'bun:test'
import type { PrivacyRule } from '@tg/core'
import { privacyRuleSummary, visibleExceptionLists } from './privacyCopy'
import { replaceRule, withException, withoutException, withTier } from './privacyEditing'

const user = (id: string) => ({ id, username: id, avatar_emoji: '', display_name: '' })
const rule = (over: Partial<PrivacyRule> = {}): PrivacyRule => ({
  key: 'last_seen',
  tier: 'contacts',
  allow_users: [],
  deny_users: [],
  ...over,
})

describe('privacy copy', () => {
  test('the summary counts only the lists the tier shows, Telegram-style', () => {
    const lists = { allow_users: [user('a')], deny_users: [user('b'), user('c')] }
    expect(privacyRuleSummary(rule({ tier: 'contacts', ...lists }))).toBe('我的联系人 (-2, +1)')
    expect(privacyRuleSummary(rule({ tier: 'everybody', ...lists }))).toBe('所有人 (-2)')
    expect(privacyRuleSummary(rule({ tier: 'nobody', ...lists }))).toBe('没有人 (+1)')
    expect(privacyRuleSummary(rule({ tier: 'nobody' }))).toBe('没有人')
  })

  test('each tier shows the lists that can change its outcome', () => {
    expect(visibleExceptionLists('everybody')).toEqual({ allow: false, deny: true })
    expect(visibleExceptionLists('contacts')).toEqual({ allow: true, deny: true })
    expect(visibleExceptionLists('nobody')).toEqual({ allow: true, deny: false })
  })
})

describe('privacy editing', () => {
  test('adding an account to one list moves it out of the other, once', () => {
    const start = rule({ deny_users: [user('a')] })
    const moved = withException(start, 'allow', user('a'))
    expect(moved.allow_users.map((u) => u.id)).toEqual(['a'])
    expect(moved.deny_users).toEqual([])
    expect(withException(moved, 'allow', user('a'))).toBe(moved)
  })

  test('removal and tier changes are pure', () => {
    const start = rule({ allow_users: [user('a'), user('b')] })
    const removed = withoutException(start, 'allow', 'a')
    expect(removed.allow_users.map((u) => u.id)).toEqual(['b'])
    expect(start.allow_users).toHaveLength(2)
    expect(withTier(start, 'nobody').tier).toBe('nobody')
    const replaced = replaceRule([start, rule({ key: 'forwards' })], withTier(start, 'nobody'))
    expect(replaced.map((r) => r.tier)).toEqual(['nobody', 'contacts'])
  })
})
