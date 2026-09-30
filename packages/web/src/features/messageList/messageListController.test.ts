/**
 * The list state machine against a fake server that reproduces the real endpoints'
 * semantics (inclusive `before` cursor, chronological pages, `limit/2 + 1` older context).
 * The Virtuoso invariant under test: a row keeps its ABSOLUTE index (firstItemIndex + row
 * index) across every prepend and join, and changes only on an explicit remount (viewKey).
 */
import { describe, expect, test } from 'bun:test'
import type { BroadcastMessage, DisplayMessage } from '@tg/core'
import { computeMessageLayout, createMessageStore, selectTimeline } from '@tg/core'
import type { MessageListState } from './messageListController'
import {
  CONTEXT_WINDOW_SIZE,
  FIRST_ITEM_INDEX_BASE,
  MESSAGE_NOT_FOUND_NOTICE,
  OLDER_PAGE_SIZE,
  createMessageListController,
  visibleKeys,
  visibleWindow,
} from './messageListController'

const BASE = Date.parse('2026-01-01T00:00:00Z')

function message(n: number): BroadcastMessage {
  const sender = `u${Math.floor(n / 3) % 4}`
  return {
    type: 'broadcast',
    message_id: `m${String(n).padStart(6, '0')}`,
    sender_id: sender,
    sender,
    sender_avatar: '',
    content: `#${n}`,
    attachment: null,
    reply_to: null,
    recalled_at: null,
    edited_at: null,
    timestamp: new Date(BASE + n * 30_000).toISOString(),
    favorite_id: null,
    forwarded_from: null,
    reactions: [],
  }
}

function fakeServer(total: number) {
  const all = Array.from({ length: total }, (_, n) => message(n))
  const indexOf = (id: string) => all.findIndex((entry) => entry.message_id === id)
  const calls: string[] = []
  return {
    all,
    calls,
    api: {
      async loadOlder(beforeId: string, limit: number) {
        calls.push(`older:${beforeId}`)
        const at = indexOf(beforeId)
        return all.slice(Math.max(0, at + 1 - limit), at + 1).map((entry) => ({ ...entry }))
      },
      async loadAround(messageId: string, limit: number) {
        calls.push(`around:${messageId}`)
        const at = indexOf(messageId)
        if (at < 0) return []
        const older = all.slice(Math.max(0, at + 1 - (Math.floor(limit / 2) + 1)), at + 1)
        const newer = all.slice(at + 1, at + 1 + (limit - older.length))
        return [...older, ...newer].map((entry) => ({ ...entry }))
      },
    },
  }
}

function fakeTimers() {
  const pending = new Map<number, () => void>()
  let next = 1
  return {
    pending,
    runAll: () => {
      for (const [id, handler] of [...pending]) {
        pending.delete(id)
        handler()
      }
    },
    port: {
      setTimeout: (handler: () => void, _ms: number) => {
        pending.set(next, handler)
        return next++
      },
      clearTimeout: (handle: unknown) => void pending.delete(handle as number),
    },
  }
}

function setup(total = 1_000, liveCount = 100) {
  const server = fakeServer(total)
  const timers = fakeTimers()
  let live: DisplayMessage[] = server.all.slice(total - liveCount)
  const controller = createMessageListController({ api: server.api, getLive: () => live, timers: timers.port })
  const rows = () => visibleKeys(visibleWindow(controller.store.getState(), live))
  const absolute = (key: string) => controller.store.getState().firstItemIndex + rows().indexOf(key)
  return {
    server,
    timers,
    controller,
    rows,
    absolute,
    state: (): MessageListState => controller.store.getState(),
    setLive: (next: DisplayMessage[]) => (live = next),
    live: () => live,
  }
}

const id = (n: number) => message(n).message_id

describe('older pages in live mode', () => {
  test('the first live message is the hidden lead until the start is known', () => {
    const { rows } = setup()
    expect(rows()[0]).toBe(id(901))
  })

  test('a prepend keeps every visible row at its absolute index', async () => {
    const { controller, absolute, rows, state } = setup()
    const before = rows().map((key) => [key, absolute(key)] as const)
    await controller.loadOlder()
    expect(state().older).toHaveLength(OLDER_PAGE_SIZE - 1)
    for (const [key, index] of before) expect(absolute(key)).toBe(index)
    expect(state().firstItemIndex).toBe(FIRST_ITEM_INDEX_BASE - (OLDER_PAGE_SIZE - 1))
    expect(rows()[0]).toBe(id(900 - (OLDER_PAGE_SIZE - 1) + 1))
  })

  test('visible rows keep their layout across a prepend (hidden-lead contract)', async () => {
    const { controller, state, live } = setup()
    const layoutOf = () => {
      const view = visibleWindow(state(), live())
      const entries = computeMessageLayout(view.all, { currentUserId: 'u0', showGroupIdentity: true })
      return new Map(entries.slice(view.hidden).map((entry) => [entry.key, entry]))
    }
    const before = layoutOf()
    await controller.loadOlder()
    const after = layoutOf()
    for (const [key, entry] of before) expect(after.get(key)).toEqual(entry)
  })

  test('reaching the chat start reveals the lead with the same anchor math', async () => {
    const { controller, state, rows, absolute } = setup(120, 100)
    const firstVisible = rows()[0] as string
    const anchor = absolute(firstVisible)
    await controller.loadOlder()
    expect(state().olderHasMore).toBe(false)
    expect(rows()[0]).toBe(id(0))
    expect(absolute(firstVisible)).toBe(anchor)
    await controller.loadOlder()
    expect(state().older).toHaveLength(20)
  })

  test('concurrent loadOlder calls issue one request', async () => {
    const { controller, server } = setup()
    await Promise.all([controller.loadOlder(), controller.loadOlder()])
    expect(server.calls).toEqual([`older:${id(900)}`])
  })

  test('a failed page surfaces a notice and allows a retry', async () => {
    const { controller, state, server } = setup()
    server.api.loadOlder = async () => {
      throw new Error('offline')
    }
    await controller.loadOlder()
    expect(state().notice).not.toBe('')
    expect(state().loadingOlder).toBe(false)
  })
})

describe('jump to message', () => {
  test('a loaded target scrolls in place and flashes', async () => {
    const { controller, state, timers } = setup()
    await controller.jumpTo(id(950), { key: id(990), offsetPx: -12, atBottom: false })
    expect(state().mode).toBe('live')
    expect(state().scrollRequest).toMatchObject({ index: 950 - 901, align: 'center' })
    expect(state().highlightedId).toBe(id(950))
    expect(state().returnAnchor).toEqual({ key: id(990), offsetPx: -12, atBottom: false })
    timers.runAll()
    expect(state().highlightedId).toBe('')
  })

  test('a far target detaches into a fresh window around it', async () => {
    const { controller, state, rows } = setup(100_000, 100)
    const viewKey = state().viewKey
    await controller.jumpTo(id(50_000), { key: id(99_950), offsetPx: 0, atBottom: false })
    expect(state().mode).toBe('detached')
    expect(state().viewKey).toBe(viewKey + 1)
    expect(state().firstItemIndex).toBe(FIRST_ITEM_INDEX_BASE)
    const index = rows().indexOf(id(50_000))
    expect(state().initialLocation).toEqual({ index, align: 'center' })
    expect(state().highlightedId).toBe(id(50_000))
  })

  test('a missing target leaves the list alone and says so', async () => {
    const { controller, state } = setup()
    await controller.jumpTo('gone', null)
    expect(state().mode).toBe('live')
    expect(state().notice).toBe(MESSAGE_NOT_FOUND_NOTICE)
  })

  test('a target just above the loaded range merges without a remount', async () => {
    const { controller, state, rows, absolute } = setup()
    const anchorKey = rows()[0] as string
    const anchor = absolute(anchorKey)
    await controller.jumpTo(id(880), null)
    expect(state().mode).toBe('live')
    expect(state().viewKey).toBe(0)
    expect(absolute(anchorKey)).toBe(anchor)
    expect(state().scrollRequest?.index).toBe(rows().indexOf(id(880)))
  })

  test('paging newer from a detached window joins live without moving any row', async () => {
    const { controller, state, rows, absolute } = setup(2_000, 100)
    await controller.jumpTo(id(1_500), null)
    expect(state().mode).toBe('detached')
    let guard = 0
    while (state().mode === 'detached' && guard++ < 20) {
      const probe = rows()[0] as string
      const probeIndex = absolute(probe)
      await controller.loadNewer()
      expect(absolute(probe)).toBe(probeIndex)
    }
    expect(state().mode).toBe('live')
    expect(state().viewKey).toBe(1)
    expect(rows().at(-1)).toBe(id(1_999))
    expect(new Set(rows()).size).toBe(rows().length)
  })

  test('paging older in a detached window keeps the anchor too', async () => {
    const { controller, rows, absolute, state } = setup(5_000, 100)
    await controller.jumpTo(id(2_000), null)
    const probe = rows()[0] as string
    const probeIndex = absolute(probe)
    await controller.loadOlder()
    expect(state().detached.length).toBeGreaterThan(CONTEXT_WINDOW_SIZE)
    expect(absolute(probe)).toBe(probeIndex)
  })
})

describe('back to where you were', () => {
  test('from a detached window it remounts live at the anchor with its pixel offset', async () => {
    const { controller, state, rows } = setup(100_000, 100)
    await controller.jumpTo(id(50_000), { key: id(99_950), offsetPx: -17, atBottom: false })
    await controller.returnToAnchor()
    expect(state().mode).toBe('live')
    expect(state().detached).toEqual([])
    expect(state().returnAnchor).toBeNull()
    expect(state().initialLocation).toEqual({ index: rows().indexOf(id(99_950)), align: 'start', offset: 17 })
  })

  test('in live mode it scrolls back without a remount', async () => {
    const { controller, state, rows } = setup()
    await controller.jumpTo(id(920), { key: id(990), offsetPx: 4, atBottom: false })
    await controller.returnToAnchor()
    expect(state().viewKey).toBe(0)
    expect(state().scrollRequest).toMatchObject({ index: rows().indexOf(id(990)), align: 'start', offset: -4 })
  })

  test('an anchor taken at the bottom returns to the newest message', async () => {
    const { controller, state } = setup(100_000, 100)
    await controller.jumpTo(id(10), { key: id(99_999), offsetPx: 0, atBottom: true })
    await controller.returnToAnchor()
    expect(state().mode).toBe('live')
    expect(state().initialLocation).toBeNull()
  })

  test('goToLatest from detached drops the window and the return point', async () => {
    const { controller, state } = setup(100_000, 100)
    await controller.jumpTo(id(10), { key: id(99_999), offsetPx: 0, atBottom: false })
    controller.goToLatest()
    expect(state().mode).toBe('live')
    expect(state().returnAnchor).toBeNull()
    expect(state().viewKey).toBe(2)
  })
})

test('a disposed controller ignores late responses', async () => {
  const { controller, state } = setup()
  const pending = controller.loadOlder()
  controller.dispose()
  await pending
  expect(state().older).toEqual([])
})

test('a live timeline with only pending rows has nothing older to load', async () => {
  const { controller, state, setLive, server } = setup()
  setLive([{ ...message(5), message_id: 'pending:x', client_message_id: 'x' }])
  await controller.loadOlder()
  expect(server.calls).toEqual([])
  expect(state().olderHasMore).toBe(false)
})

test('a StrictMode dispose/resume cycle leaves the controller working', async () => {
  const { controller, state } = setup()
  controller.dispose()
  controller.resume()
  await controller.loadOlder()
  expect(state().older.length).toBeGreaterThan(0)
})

describe('store-backed live window (prependLive, TG-100)', () => {
  function storeSetup(total = 1_000, liveCount = 100) {
    const server = fakeServer(total)
    const timers = fakeTimers()
    const store = createMessageStore()
    for (const row of server.all.slice(total - liveCount)) store.getState().applyBroadcast('c', row, 'none')
    const getLive = () => selectTimeline('c')(store.getState()).messages
    const controller = createMessageListController({
      api: server.api,
      getLive,
      timers: timers.port,
      prependLive: (rows) => store.getState().prependHistory('c', rows),
    })
    const rows = () => visibleKeys(visibleWindow(controller.store.getState(), getLive()))
    const absolute = (key: string) => controller.store.getState().firstItemIndex + rows().indexOf(key)
    return { server, controller, store, rows, absolute, state: () => controller.store.getState() }
  }

  test('older pages go into the store, anchors hold, and live frames reach them', async () => {
    const { controller, store, rows, absolute, state } = storeSetup()
    const before = rows().map((key) => [key, absolute(key)] as const)
    await controller.loadOlder()
    expect(state().older).toHaveLength(0)
    expect(selectTimeline('c')(store.getState()).messages).toHaveLength(100 + OLDER_PAGE_SIZE - 1)
    for (const [key, index] of before) expect(absolute(key)).toBe(index)
    store.getState().applyEdit('c', { type: 'message_edited', message_id: id(860), content: 'edited', edited_at: 'e' })
    const edited = selectTimeline('c')(store.getState()).messages.find(
      (row) => row.type === 'broadcast' && row.message_id === id(860),
    )
    expect(edited).toMatchObject({ content: 'edited' })
  })

  test('a jump window that joins live is folded into the store', async () => {
    const { controller, store, state, rows } = storeSetup(1_000, 100)
    await controller.jumpTo(id(880), null)
    expect(state().mode).toBe('live')
    expect(selectTimeline('c')(store.getState()).messages[0]).toMatchObject({
      message_id: id(880 - CONTEXT_WINDOW_SIZE / 2),
    })
    expect(rows()).toContain(id(880))
  })
})
