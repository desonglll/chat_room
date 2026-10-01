/** TG-1204: account isolation of the stores that are loaded once per session. */
import { afterEach, describe, expect, test } from 'bun:test'
import type { AuthSession, ChatFolder, Wallpaper } from '@tg/core'
import { authStore } from '@tg/core'
import { folderStore } from '../folders/folderStore'
import { wallpaperStore } from '../wallpaper/wallpaperStore'
import { recordSearch, searchStore } from './searchStore'

const memory = new Map<string, string>()
globalThis.localStorage = {
  getItem: (key: string) => memory.get(key) ?? null,
  setItem: (key: string, value: string) => void memory.set(key, value),
  removeItem: (key: string) => void memory.delete(key),
} as Storage
;(globalThis as { window?: unknown }).window ??= globalThis

const session = (id: string): AuthSession =>
  ({ token: `token-${id}`, user: { id, username: id }, expires_at: '2099-01-01T00:00:00Z' }) as AuthSession

afterEach(() => authStore.getState().clearSession())

describe('TG-1204 per-account stores', () => {
  test('another account signing in to the same tab does not see the folders or wallpapers', () => {
    authStore.getState().setSession(session('alice'))
    folderStore.setState({ folders: [{ id: 'f1' } as ChatFolder], activeId: 'f1', loaded: true })
    wallpaperStore.setState({ wallpapers: [{ scope: 'global' } as Wallpaper], loaded: true, images: {} })

    authStore.getState().clearSession()
    authStore.getState().setSession(session('bob'))

    expect(folderStore.getState()).toEqual({ folders: [], activeId: null, loaded: false })
    expect(wallpaperStore.getState()).toEqual({ wallpapers: [], loaded: false, images: {} })
  })

  test('recent searches belong to the account that made them', () => {
    authStore.getState().setSession(session('alice'))
    recordSearch('alice secret')
    expect(searchStore.getState().recent).toEqual(['alice secret'])

    authStore.getState().setSession(session('bob'))
    expect(searchStore.getState().recent).toEqual([])
    recordSearch('bob query')

    authStore.getState().setSession(session('alice'))
    expect(searchStore.getState().recent).toEqual(['alice secret'])
    authStore.getState().clearSession()
    expect(searchStore.getState().recent).toEqual([])
  })
})
