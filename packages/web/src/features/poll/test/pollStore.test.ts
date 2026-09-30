import { describe, expect, test } from 'bun:test'
import { applyPollFrame, createPollStore, effectivePoll } from '../pollStore'
import { makePoll } from './fixtures'

describe('pollStore', () => {
  test('frames update counts but keep the ballot the viewer already has', () => {
    const store = createPollStore()
    store.getState().remember(makePoll({ revision: 2, chosen: [2] }))
    const { chosen: _drop, ...frame } = makePoll({ revision: 3, total_voters: 9 })
    applyPollFrame({ type: 'poll_updated', message_id: 'poll-1', poll: frame }, store)
    const held = store.getState().polls['poll-1']
    expect(held?.total_voters).toBe(9)
    expect(held?.chosen).toEqual([2])
  })

  test('a late, older frame is ignored', () => {
    const store = createPollStore()
    store.getState().remember(makePoll({ revision: 5, total_voters: 5 }))
    applyPollFrame(
      { type: 'poll_updated', message_id: 'poll-1', poll: makePoll({ revision: 4, total_voters: 4 }) },
      store,
    )
    expect(store.getState().polls['poll-1']?.total_voters).toBe(5)
  })

  test('a history reload newer than the held snapshot wins', () => {
    const fromHistory = makePoll({ revision: 8, total_voters: 8, chosen: [] })
    expect(effectivePoll(fromHistory, makePoll({ revision: 3, total_voters: 3 })).total_voters).toBe(8)
    expect(effectivePoll(fromHistory, undefined)).toBe(fromHistory)
  })
})
