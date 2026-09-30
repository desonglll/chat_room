/** TG-110: the settings registry, navigation and the pure page models. */
import { afterEach, describe, expect, test } from 'bun:test'
import type { DeviceSession, User } from '@tg/core'
import { createSettingsNavigation, viewForSection } from '../settingsNavigation'
import {
  pagesInSection,
  registerSettingsPage,
  SETTINGS_SECTIONS,
  settingsPagesSnapshot,
  subscribeSettingsPages,
  type SettingsPageRegistration,
} from '../settingsRegistry'
import { profileHeaderModel, profilePatch, validateProfileDraft } from '../profileModel'
import { describeLastActive, deviceSubtitle, orderDeviceSessions } from '../pages/devicesModel'

const Page = () => null
const cleanups: Array<() => void> = []
const register = (page: SettingsPageRegistration) => cleanups.push(registerSettingsPage(page))
afterEach(() => {
  while (cleanups.length) cleanups.pop()!()
})

describe('registry', () => {
  test('pages land in their section in order; the same id replaces instead of duplicating', () => {
    register({ id: 't.b', section: 'appearance', title: 'B', component: Page, order: 20 })
    register({ id: 't.a', section: 'appearance', title: 'A', component: Page, order: 10 })
    register({ id: 't.a', section: 'appearance', title: 'A2', component: Page, order: 10 })
    const titles = pagesInSection(settingsPagesSnapshot(), 'appearance')
      .filter((page) => page.id.startsWith('t.'))
      .map((page) => page.title)
    expect(titles).toEqual(['A2', 'B'])
  })

  test('subscribers hear registrations; unregister removes the page and the snapshot is stable', () => {
    let calls = 0
    const off = subscribeSettingsPages(() => (calls += 1))
    const before = settingsPagesSnapshot()
    expect(settingsPagesSnapshot()).toBe(before)
    const unregister = registerSettingsPage({ id: 't.lang', section: 'language', title: '语言', component: Page })
    expect(calls).toBe(1)
    expect(pagesInSection(settingsPagesSnapshot(), 'language')).toHaveLength(1)
    unregister()
    expect(pagesInSection(settingsPagesSnapshot(), 'language')).toHaveLength(0)
    expect(calls).toBe(2)
    off()
  })

  test('the section list is Telegram-shaped and includes every M5 slot', () => {
    expect(SETTINGS_SECTIONS.map((section) => section.title)).toEqual([
      '我的账号',
      '通知与声音',
      '隐私与安全',
      '数据与存储',
      '外观',
      '聊天文件夹',
      '语言',
      '设备',
    ])
  })
})

describe('navigation', () => {
  test('open → push → back → back closes; reopening starts at the root', () => {
    const nav = createSettingsNavigation()
    nav.getState().openSettings()
    nav.getState().push({ kind: 'section', id: 'privacy' })
    nav.getState().push({ kind: 'page', id: 'privacy.rules' })
    expect(nav.getState().stack).toHaveLength(2)
    nav.getState().back()
    nav.getState().back()
    expect(nav.getState().open).toBeTrue()
    nav.getState().back()
    expect(nav.getState().open).toBeFalse()
    nav.getState().push({ kind: 'section', id: 'storage' })
    nav.getState().openSettings()
    expect(nav.getState().stack).toEqual([])
  })

  test('a single-page section opens its page; empty and multi-page sections open the section', () => {
    const one: SettingsPageRegistration = { id: 'x', section: 'devices', title: '设备', component: Page }
    expect(viewForSection('devices', [one])).toEqual({ kind: 'page', id: 'x' })
    expect(viewForSection('storage', [])).toEqual({ kind: 'section', id: 'storage' })
    expect(viewForSection('privacy', [one, { ...one, id: 'y' }])).toEqual({ kind: 'section', id: 'privacy' })
  })
})

const user: User = {
  id: 'u1',
  username: 'alice',
  avatar_emoji: '🦊',
  display_name: 'Alice',
  signature: '',
  homepage: '',
  created_at: '2026-01-01T00:00:00Z',
}

describe('profile', () => {
  test('header: display name or username, @handle, emoji or uploaded avatar', () => {
    expect(profileHeaderModel(user)).toEqual({ name: 'Alice', handle: '@alice', avatarSrc: undefined, initials: '🦊' })
    const uploaded = { ...user, display_name: ' ', avatar_emoji: '/api/users/u1/avatar' }
    expect(profileHeaderModel(uploaded)).toMatchObject({ name: 'alice', avatarSrc: '/api/users/u1/avatar' })
  })

  test('validation mirrors the server limits', () => {
    expect(validateProfileDraft({ displayName: 'x'.repeat(49), signature: '', homepage: '' }).displayName).toBeString()
    expect(validateProfileDraft({ displayName: '', signature: 'a\nb', homepage: '' }).signature).toBeString()
    expect(validateProfileDraft({ displayName: '', signature: '', homepage: 'ftp://x' }).homepage).toBeString()
    expect(validateProfileDraft({ displayName: 'Al', signature: 'hi', homepage: 'https://a.b' })).toEqual({})
  })

  test('patch sends only trimmed changes, null when nothing changed', () => {
    expect(profilePatch(user, { displayName: 'Alice', signature: '', homepage: '' })).toBeNull()
    expect(profilePatch(user, { displayName: ' Al ', signature: '', homepage: '' })).toEqual({ display_name: 'Al' })
  })
})

const session = (id: string, current: boolean, lastUsed: string): DeviceSession => ({
  id,
  device_name: id,
  ip_hint: '203.0.113.x',
  created_at: lastUsed,
  last_used_at: lastUsed,
  expires_at: '2027-01-01T00:00:00Z',
  current,
})

describe('devices', () => {
  test('this device first, others by most recent use', () => {
    const { current, others } = orderDeviceSessions([
      session('old', false, '2026-09-01T00:00:00Z'),
      session('me', true, '2026-10-01T00:00:00Z'),
      session('new', false, '2026-09-30T00:00:00Z'),
    ])
    expect(current?.id).toBe('me')
    expect(others.map((entry) => entry.id)).toEqual(['new', 'old'])
  })

  test('last-active copy', () => {
    const now = new Date(2026, 9, 1, 12, 0)
    expect(describeLastActive(new Date(2026, 9, 1, 11, 59, 30).toISOString(), now)).toBe('刚刚')
    expect(describeLastActive(new Date(2026, 9, 1, 9, 5).toISOString(), now)).toBe('09:05')
    expect(describeLastActive(new Date(2026, 8, 3, 9, 5).toISOString(), now)).toBe('9月3日')
    expect(describeLastActive(new Date(2025, 0, 2).toISOString(), now)).toBe('2025/1/2')
    expect(deviceSubtitle(session('me', true, now.toISOString()), now)).toBe('203.0.113.x · 在线')
  })
})
