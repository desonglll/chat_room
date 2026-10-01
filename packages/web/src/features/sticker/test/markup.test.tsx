/**
 * Static markup of the panel pieces (`bun test` has no DOM): structure, names, and that the
 * grid mounts only the rows near the viewport even with 300 stickers installed.
 */
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { stickerStore } from '@tg/core'
import { mediaPanelTabStore } from '../panel/mediaPanelTabs'
import { StickerSetsSettings } from '../manage/StickerSetsSettings'
import { MediaPanel } from '../panel/MediaPanel'
import { registerMediaPanelTab } from '../panel/mediaPanelTabs'
import { StickerSuggestions } from '../suggest/StickerSuggestions'
import { makeSet, makeSticker } from './fixtures'

const noop = () => undefined

function install(setCount: number, perSet: number) {
  const sets = Array.from({ length: setCount }, (_, s) =>
    makeSet(
      `set-${s}`,
      Array.from({ length: perSet }, (_, i) => makeSticker(`s${s}-${i}`, ['😀'], `set-${s}`)),
    ),
  )
  stickerStore.setState({ status: 'ready', revision: 1, sets, recent: [makeSticker('r1')], favorites: [] })
}

// Server rendering reads a zustand store's *initial* state (useSyncExternalStore's server
// snapshot), so for these static-markup tests the initial state follows the live one.
const initial = { sticker: stickerStore.getInitialState, tabs: mediaPanelTabStore.getInitialState }
beforeAll(() => {
  stickerStore.getInitialState = stickerStore.getState
  mediaPanelTabStore.getInitialState = mediaPanelTabStore.getState
})
afterAll(() => {
  stickerStore.getInitialState = initial.sticker
  mediaPanelTabStore.getInitialState = initial.tabs
})
afterEach(() => stickerStore.getState().reset())

// The tab registry is module-global and `gif/register` fills it on import. Bun loads every
// test file into one process in directory order, which differs between machines, so these
// tests start from an empty registry instead of whatever an earlier file left behind.
let savedTabs: ReturnType<typeof mediaPanelTabStore.getState>['tabs'] = []
beforeEach(() => {
  savedTabs = mediaPanelTabStore.getState().tabs
  mediaPanelTabStore.setState({ tabs: [] })
})
afterEach(() => mediaPanelTabStore.setState({ tabs: savedTabs }))

const panel = (tab: string) =>
  renderToStaticMarkup(
    <MediaPanel
      chatId="c1"
      emoji={<div id="emoji-body" />}
      onSendSticker={noop}
      canSend
      onClose={noop}
      initialTab={tab}
    />,
  )

describe('media panel markup', () => {
  test('three top tabs; the emoji tab shows the composer picker', () => {
    const html = panel('emoji')
    expect(html.match(/role="tab"/g)).toHaveLength(3)
    for (const label of ['表情', '贴纸', 'GIF']) expect(html).toContain(`>${label}<`)
    expect(html).toContain('id="emoji-body"')
    expect(html).not.toContain('tg-sticker-grid')
  })

  test('GIF is a placeholder until a tab registers for it', () => {
    expect(panel('gif')).toContain('GIF 即将推出')
    const undo = registerMediaPanelTab({ id: 'gif', label: 'GIF', order: 30, render: () => <b id="real-gif" /> })
    try {
      expect(panel('gif')).toContain('id="real-gif"')
    } finally {
      undo()
    }
  })

  test('300 installed stickers: rail lists every set, the grid mounts only visible rows', () => {
    install(12, 25)
    const html = panel('stickers')
    expect(html).toContain('aria-label="搜索贴纸"')
    expect(html.match(/tg-sticker-rail__item/g)?.length).toBe(1 + 12 + 1) // recents, sets, settings
    const cells = html.match(/data-sticker-id=/g)?.length ?? 0
    expect(cells).toBeGreaterThan(0)
    expect(cells).toBeLessThanOrEqual(32)
    expect(html).toContain('最近使用')
  })

  test('an empty library says so instead of drawing an empty grid', () => {
    stickerStore.setState({ status: 'ready', sets: [], recent: [], favorites: [] })
    expect(panel('stickers')).toContain('还没有贴纸包')
  })
})

describe('other surfaces', () => {
  test('suggestions render nothing until the controller has something to show', () => {
    install(1, 3)
    expect(renderToStaticMarkup(<StickerSuggestions draft="😀" onPick={noop} />)).toBe('')
  })

  test('management page lists sets in order with reorder, share, archive and remove', () => {
    install(2, 2)
    stickerStore.setState((state) => ({
      sets: [...state.sets, makeSet('old', [makeSticker('o1')], { archived: true, title: 'Old' })],
    }))
    const html = renderToStaticMarkup(<StickerSetsSettings />)
    expect(html.indexOf('Set set-0')).toBeLessThan(html.indexOf('Set set-1'))
    expect(html).toContain('aria-label="上移 Set set-0"')
    expect(html).toContain('aria-label="复制 Set set-1 的链接"')
    expect(html).toContain('>归档<')
    expect(html).toContain('已归档')
    expect(html).toContain('取消归档')
    expect(html).toContain('aria-label="移除 Old"')
  })
})
