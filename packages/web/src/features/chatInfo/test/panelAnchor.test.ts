/** The anchor is pinned exactly on chat-info open/close transitions, synchronously. */
import { afterEach, expect, test } from 'bun:test'
import { createUiStore } from '@tg/core'
import { holdMessageListAnchor, watchPanelAnchor } from '../panelAnchor'

const globals = globalThis as { document?: unknown }
afterEach(() => {
  delete globals.document
})

test('holds on open and close only, releasing the previous hold first', () => {
  globals.document = {}
  const store = createUiStore()
  const events: string[] = []
  let holds = 0
  const stop = watchPanelAnchor(store, () => {
    const id = (holds += 1)
    events.push(`hold${id}`)
    return () => events.push(`release${id}`)
  })
  store.getState().openPanel('chatInfo')
  expect(events).toEqual(['hold1']) // synchronous: before any re-render could reflow the list
  store.getState().setActiveChat('c2')
  store.getState().setSidebarWidth(400)
  store.getState().openPanel('chatInfo')
  expect(events).toEqual(['hold1'])
  store.getState().openPanel('search') // chatInfo → search closes the info column
  expect(events).toEqual(['hold1', 'release1', 'hold2'])
  stop()
  expect(events).toEqual(['hold1', 'release1', 'hold2', 'release2'])
  store.getState().openPanel('chatInfo')
  expect(holds).toBe(2)
})

test('no DOM → no subscription; no scroller → a no-op hold', () => {
  const store = createUiStore()
  let held = false
  watchPanelAnchor(store, () => {
    held = true
    return () => undefined
  })
  store.getState().openPanel('chatInfo')
  expect(held).toBe(false)
  const release = holdMessageListAnchor({ querySelector: () => null } as unknown as ParentNode)
  expect(release()).toBeUndefined()
})
