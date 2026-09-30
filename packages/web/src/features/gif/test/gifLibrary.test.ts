import { describe, expect, test } from 'bun:test'
import { ApiError, createMessageStore } from '@tg/core'
import { createGifLibrary, createGifStore, gifErrorMessage } from '../gifLibrary'
import { fakeGifsApi, savedGif } from './fixtures'

function setup(overrides: Parameters<typeof fakeGifsApi>[0] = {}) {
  const { api, calls } = fakeGifsApi(overrides)
  const store = createGifStore()
  const messages = createMessageStore()
  const library = createGifLibrary({ api, store, messages, uuid: () => 'uuid-1' })
  return { calls, store, messages, library }
}

describe('gif library', () => {
  test('concurrent loads share one request pair; reset forgets and reloads', async () => {
    const { calls, store, library } = setup()
    await Promise.all([library.ensureLoaded(), library.ensureLoaded()])
    expect(calls).toEqual(['saved()', 'recent()'])
    expect(store.getState().saved.map((gif) => gif.id)).toEqual(['g1'])
    library.reset()
    expect(store.getState()).toMatchObject({ status: 'idle', saved: [], recent: [] })
    await library.ensureLoaded()
    expect(calls).toHaveLength(4)
  })

  test('sending a saved GIF lands in the timeline and moves it to the front', async () => {
    const { calls, store, messages, library } = setup()
    store.setState({ saved: [savedGif('a'), savedGif('b')] })
    expect(await library.send('c1', { saved_gif_id: 'b' }, 'm0')).toEqual({ ok: true })
    expect(calls).toEqual([`send("c1",{"saved_gif_id":"b"},{"reply_to":"m0","client_message_id":"uuid-1"})`])
    const sent = messages.getState().timelines.c1?.messages[0]
    expect(sent?.type === 'broadcast' && sent.media_kind).toBe('gif')
    expect(store.getState().saved.map((gif) => gif.id)).toEqual(['b', 'a'])
  })

  test('a refused upload reports the server code and changes nothing', async () => {
    const { messages, library } = setup({
      upload: async () => Promise.reject(new ApiError(400, '/x', 'audio_not_allowed')),
    })
    const result = await library.upload('c1', 'bytes')
    expect(result).toEqual({ ok: false, code: 'audio_not_allowed' })
    expect(messages.getState().timelines.c1).toBeUndefined()
    expect(gifErrorMessage('audio_not_allowed')).toBe('GIF 不能包含声音')
  })

  test('save puts the GIF first without duplicating it; remove rolls back on failure', async () => {
    const { store, library } = setup({ remove: async () => Promise.reject(new Error('offline')) })
    store.setState({ saved: [savedGif('saved-m1'), savedGif('x')] })
    await library.save('m1')
    expect(store.getState().saved.map((gif) => gif.id)).toEqual(['saved-m1', 'x'])
    await library.remove('x')
    expect(store.getState().saved.map((gif) => gif.id)).toEqual(['saved-m1', 'x'])
  })

  test('media-reported aspect ratios are kept per file', () => {
    const { store, library } = setup()
    library.reportAspect('/f', 1.5)
    library.reportAspect('/f', Number.NaN)
    expect(store.getState().aspects).toEqual({ '/f': 1.5 })
  })
})
