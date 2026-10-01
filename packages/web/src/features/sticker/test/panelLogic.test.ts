import { afterEach, beforeEach, describe, expect, test } from 'bun:test'
import { moveId, shortNameFromLink, stickerSetLink } from '../manage/setOrder'
import { mediaPanelTabStore, registerMediaPanelTab } from '../panel/mediaPanelTabs'
import { searchStickers } from '../panel/stickerSearch'
import { makeSet, makeSticker } from './fixtures'

describe('sticker search', () => {
  const cats = makeSet('cats', [makeSticker('c1', ['🐱']), makeSticker('c2', ['😺', '🐱'])], { title: 'Cute Cats' })
  const dogs = makeSet('dogs', [makeSticker('d1', ['🐶'])], { title: '狗狗' })
  const library = { sets: [cats, dogs], recent: [], favorites: [] }

  test('an emoji finds every installed sticker answering to it', () => {
    const result = searchStickers('🐱', library, [cats, dogs])
    expect(result.sections[0]?.stickers.map((sticker) => sticker.id)).toEqual(['c1', 'c2'])
    expect(result.remoteShortName).toBeNull()
  })

  test('text matches set titles and short names, case-insensitively', () => {
    expect(searchStickers('CUTE', library, [cats, dogs]).sections.map((section) => section.id)).toEqual(['cats'])
    expect(searchStickers('狗', library, [cats, dogs]).sections.map((section) => section.id)).toEqual(['dogs'])
  })

  test('an unknown short-name-shaped query is looked up remotely; an installed one is not', () => {
    expect(searchStickers('Birds', library, [cats, dogs]).remoteShortName).toBe('birds')
    expect(searchStickers('cats', library, [cats, dogs]).remoteShortName).toBeNull()
    expect(searchStickers('two words', library, [cats, dogs]).remoteShortName).toBeNull()
    expect(searchStickers('  ', library, [cats, dogs])).toEqual({ sections: [], remoteShortName: null })
  })
})

describe('set management helpers', () => {
  test('moveId moves and clamps', () => {
    expect(moveId(['a', 'b', 'c'], 0, 2)).toEqual(['b', 'c', 'a'])
    expect(moveId(['a', 'b', 'c'], 2, -5)).toEqual(['c', 'a', 'b'])
    expect(moveId(['a', 'b'], 7, 0)).toEqual(['a', 'b'])
  })

  test('share links round-trip', () => {
    const link = stickerSetLink('https://chat.example/', 'cute_cats')
    expect(link).toBe('https://chat.example/addstickers/cute_cats')
    expect(shortNameFromLink(link)).toBe('cute_cats')
    expect(shortNameFromLink('/addstickers/Cats?x=1')).toBe('cats')
    expect(shortNameFromLink('Cats')).toBe('cats')
    expect(shortNameFromLink('not a name')).toBeNull()
  })
})

describe('media panel tab registry', () => {
  // Start empty: `gif/register` may already have filled this module-global registry if its
  // test file loaded first (bun's file order follows the directory listing, so CI differs).
  let savedTabs: ReturnType<typeof mediaPanelTabStore.getState>['tabs'] = []
  beforeEach(() => {
    savedTabs = mediaPanelTabStore.getState().tabs
    mediaPanelTabStore.setState({ tabs: [] })
  })
  afterEach(() => mediaPanelTabStore.setState({ tabs: savedTabs }))

  test('registration sorts by order, replaces by id, and undo restores', () => {
    const first = registerMediaPanelTab({ id: 'gif', label: 'GIF', order: 30, render: () => 'one' })
    registerMediaPanelTab({ id: 'extra', label: 'X', order: 25, render: () => null })
    const second = registerMediaPanelTab({ id: 'gif', label: 'GIF', order: 30, render: () => 'two' })
    expect(mediaPanelTabStore.getState().tabs.map((tab) => tab.id)).toEqual(['extra', 'gif'])
    second()
    expect(
      mediaPanelTabStore
        .getState()
        .tabs.find((tab) => tab.id === 'gif')
        ?.render({} as never),
    ).toBe('one')
    first()
    expect(mediaPanelTabStore.getState().tabs.map((tab) => tab.id)).toEqual(['extra'])
  })
})
