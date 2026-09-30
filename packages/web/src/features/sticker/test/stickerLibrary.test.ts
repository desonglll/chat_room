import { describe, expect, test } from 'bun:test'
import { createMessageStore, createStickerStore } from '@tg/core'
import { createStickerLibrary } from '../stickerLibrary'
import { fakeApi, makeSet, makeSticker } from './fixtures'

function setup(overrides: Parameters<typeof fakeApi>[0] = {}) {
  const { api, calls } = fakeApi(overrides)
  const store = createStickerStore()
  const messages = createMessageStore()
  const library = createStickerLibrary({ api, store, messages, uuid: () => 'uuid-1' })
  return { api, calls, store, messages, library }
}

describe('sticker library', () => {
  test('concurrent loads share one request set', async () => {
    const { calls, store, library } = setup()
    await Promise.all([library.ensureLoaded(), library.ensureLoaded()])
    await library.ensureLoaded()
    expect(calls).toEqual(['installed()', 'recent()', 'favorites()'])
    expect(store.getState().status).toBe('ready')
  })

  test('a failed load reports error and the next call retries', async () => {
    let fail = true
    const { store, library } = setup({
      installed: async () => {
        if (fail) throw new Error('offline')
        return { revision: 1, sets: [] }
      },
    })
    await expect(library.ensureLoaded()).rejects.toThrow('offline')
    expect(store.getState().status).toBe('error')
    fail = false
    await library.ensureLoaded()
    expect(store.getState().status).toBe('ready')
  })

  test('send posts with a client id and reply, lands in the timeline and the recents', async () => {
    const sticker = makeSticker('st', ['🐱'])
    const { calls, store, messages, library } = setup()
    expect(await library.send({ chatId: 'c1', sticker, replyTo: 'm0' })).toBe(true)
    expect(calls).toEqual([`send("c1",{"sticker_id":"st","reply_to":"m0","client_message_id":"uuid-1"})`])
    const timeline = messages.getState().timelines.c1?.messages ?? []
    expect(timeline).toHaveLength(1)
    const sent = timeline[0]
    expect(sent?.type === 'broadcast' && sent.media_kind === 'sticker' && sent.sticker?.set_short_name).toBe('cats')
    expect(store.getState().recent.map((entry) => entry.id)).toEqual(['st'])
  })

  test('a failed send resolves false and changes nothing', async () => {
    const { store, messages, library } = setup({ send: async () => Promise.reject(new Error('403')) })
    expect(await library.send({ chatId: 'c1', sticker: makeSticker('st') })).toBe(false)
    expect(messages.getState().timelines.c1).toBeUndefined()
    expect(store.getState().recent).toEqual([])
  })

  test('reorder is optimistic, then the server answer replaces it', async () => {
    const a = makeSet('a', [])
    const b = makeSet('b', [])
    const { store, library } = setup({
      reorder: async () => ({ revision: 9, sets: [b, a] }),
    })
    store.getState().applyInstalled({ revision: 1, sets: [a, b] })
    const pending = library.reorder(['b', 'a'])
    expect(store.getState().sets.map((set) => set.id)).toEqual(['b', 'a'])
    await pending
    expect(store.getState().revision).toBe(9)
  })

  test('a failed library write re-reads the truth', async () => {
    const { library, calls } = setup({ uninstall: async () => Promise.reject(new Error('500')) })
    await expect(library.uninstall('a')).rejects.toThrow('500')
    await Promise.resolve()
    expect(calls).toContain('installed()')
  })

  test('favorite toggles optimistically and rolls back on failure', async () => {
    const sticker = makeSticker('f')
    const ok = setup()
    await ok.library.toggleFavorite(sticker)
    expect(ok.store.getState().favorites.map((entry) => entry.id)).toEqual(['f'])
    await ok.library.toggleFavorite(sticker)
    expect(ok.calls).toEqual(['setFavorite("f",true)', 'setFavorite("f",false)'])

    const broken = setup({ setFavorite: async () => Promise.reject(new Error('500')) })
    await expect(broken.library.toggleFavorite(sticker)).rejects.toThrow('500')
    expect(broken.store.getState().favorites).toEqual([])
  })
})
