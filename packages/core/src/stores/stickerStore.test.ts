import { describe, expect, test } from 'bun:test'
import type { Sticker, StickerSet } from '../api/stickers'
import {
  createStickerStore,
  normalizeStickerEmoji,
  selectActiveStickerSets,
  selectArchivedStickerSets,
  suggestStickers,
} from './stickerStore'

const sticker = (id: string, emojis: string[], setId = 's1'): Sticker => ({
  id,
  set_id: setId,
  emoji: emojis[0] ?? '',
  emojis,
  format: 'tgs',
  mime_type: 'application/x-tgsticker',
  width: 512,
  height: 512,
  duration_ms: 3000,
  size_bytes: 1000,
  file_url: `/api/stickers/${id}/file?key=k`,
})

const stickerSet = (id: string, stickers: Sticker[], extra: Partial<StickerSet> = {}): StickerSet => ({
  id,
  short_name: id,
  title: id,
  set_type: 'regular',
  owner_id: null,
  stickers,
  installed: true,
  archived: false,
  created_at: '',
  updated_at: '',
  ...extra,
})

describe('stickerStore', () => {
  test('keeps bounded recents, newest first, without duplicates', () => {
    const store = createStickerStore()
    const a = sticker('a', ['🦀'])
    store.getState().pushRecent(a, 2)
    store.getState().pushRecent(sticker('b', ['🎉']), 2)
    store.getState().pushRecent(a, 2)
    expect(store.getState().recent.map((entry) => entry.id)).toEqual(['a', 'b'])
    store.getState().removeRecent('a')
    expect(store.getState().recent.map((entry) => entry.id)).toEqual(['b'])
  })

  test('favorites mirror the server cap: newest first, oldest evicted', () => {
    const store = createStickerStore()
    for (const id of ['1', '2', '3', '4', '5', '6']) store.getState().setFavorite(sticker(id, ['😀']), true)
    expect(store.getState().favorites.map((entry) => entry.id)).toEqual(['6', '5', '4', '3', '2'])
    store.getState().setFavorite(sticker('4', ['😀']), false)
    expect(store.getState().favorites.map((entry) => entry.id)).toEqual(['6', '5', '3', '2'])
  })

  test('an older library revision never replaces a newer one', () => {
    const store = createStickerStore()
    store.getState().applyInstalled({ revision: 5, sets: [stickerSet('new', [])] })
    store.getState().applyInstalled({ revision: 4, sets: [stickerSet('old', [])] })
    expect(store.getState().sets.map((set) => set.id)).toEqual(['new'])
    store.getState().applyInstalled({ revision: 5, sets: [stickerSet('same-rev', [])] })
    expect(store.getState().sets.map((set) => set.id)).toEqual(['same-rev'])
  })

  test('active and archived views split the installed list and skip custom-emoji sets', () => {
    const sets = [
      stickerSet('a', []),
      stickerSet('b', [], { archived: true }),
      stickerSet('c', [], { set_type: 'custom_emoji' }),
    ]
    expect(selectActiveStickerSets({ sets }).map((set) => set.id)).toEqual(['a'])
    expect(selectArchivedStickerSets({ sets }).map((set) => set.id)).toEqual(['b'])
    expect(selectActiveStickerSets({ sets })).toBe(selectActiveStickerSets({ sets }))
  })
})

describe('suggestStickers', () => {
  const hearts = sticker('h1', ['❤️', '😍'])
  const plainHeart = sticker('h2', ['❤'], 's2')
  const thumbs = sticker('t1', ['👍'])
  const toned = sticker('t2', ['👍🏽'])
  const archivedHeart = sticker('h3', ['❤️'], 's3')
  const sets = [
    stickerSet('s1', [hearts, thumbs, toned]),
    stickerSet('s2', [plainHeart]),
    stickerSet('s3', [archivedHeart], { archived: true }),
  ]

  test('matches every emoji of a sticker, ignoring variation selectors', () => {
    const found = suggestStickers({ sets, recent: [], favorites: [] }, '❤')
    expect(found.map((entry) => entry.id)).toEqual(['h1', 'h2'])
    expect(suggestStickers({ sets, recent: [], favorites: [] }, '😍').map((entry) => entry.id)).toEqual(['h1'])
  })

  test('skin tones are distinct requests', () => {
    expect(suggestStickers({ sets, recent: [], favorites: [] }, '👍').map((entry) => entry.id)).toEqual(['t1'])
    expect(suggestStickers({ sets, recent: [], favorites: [] }, '👍🏽').map((entry) => entry.id)).toEqual(['t2'])
  })

  test('favorites, then recents, then sets; de-duplicated and bounded', () => {
    const found = suggestStickers({ sets, recent: [plainHeart], favorites: [archivedHeart] }, '❤️', 2)
    expect(found.map((entry) => entry.id)).toEqual(['h3', 'h2'])
  })

  test('archived sets and blank input suggest nothing extra', () => {
    expect(suggestStickers({ sets, recent: [], favorites: [] }, '❤️').some((entry) => entry.id === 'h3')).toBe(false)
    expect(suggestStickers({ sets, recent: [], favorites: [] }, '  ')).toEqual([])
    expect(normalizeStickerEmoji('❤️')).toBe('❤')
  })
})
