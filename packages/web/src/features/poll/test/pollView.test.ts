import { describe, expect, test } from 'bun:test'
import { describePoll, mergePollState, pollKindLabel, pollPercentages } from '../pollView'
import { counted, makePoll } from './fixtures'

describe('mergePollState', () => {
  test('a stale revision never replaces a newer snapshot', () => {
    const newer = makePoll({ revision: 5, total_voters: 3 })
    const older = makePoll({ revision: 4, total_voters: 1 })
    expect(mergePollState(newer, older)).toBe(newer)
    expect(mergePollState(older, newer).total_voters).toBe(3)
  })

  test('a chat-wide frame keeps the viewer-only fields it does not carry', () => {
    const mine = makePoll({ revision: 2, chosen: [1], correct_option: 1, explanation: '因为' })
    const frame = makePoll({ revision: 3, total_voters: 7 })
    const merged = mergePollState(mine, frame)
    expect(merged.total_voters).toBe(7)
    expect(merged.chosen).toEqual([1])
    expect(merged.correct_option).toBe(1)
    expect(merged.explanation).toBe('因为')
  })

  test("the viewer's own response replaces the ballot, retraction included", () => {
    const mine = makePoll({ revision: 2, chosen: [1] })
    expect(mergePollState(mine, makePoll({ revision: 3, chosen: [] })).chosen).toEqual([])
  })

  test('snapshots of different polls do not merge', () => {
    const other = makePoll({ id: 'poll-2' })
    expect(mergePollState(makePoll({ revision: 9 }), other)).toBe(other)
  })
})

describe('pollPercentages', () => {
  test('single choice sums to exactly 100 by largest remainder', () => {
    expect(pollPercentages(counted([1, 1, 1]))).toEqual([34, 33, 33])
    expect(pollPercentages(counted([2, 1]))).toEqual([67, 33])
    const sum = pollPercentages(counted([3, 3, 1, 5, 7])).reduce((a, b) => a + b, 0)
    expect(sum).toBe(100)
  })

  test('multiple choice is per option against voters, so it may exceed 100', () => {
    expect(pollPercentages(counted([2, 2], { total_voters: 2, multiple_choice: true }))).toEqual([100, 100])
  })

  test('no voters, no percentages', () => {
    expect(pollPercentages(makePoll())).toEqual([0, 0, 0])
  })
})

describe('describePoll', () => {
  test('the ballot shows until the viewer votes', () => {
    expect(describePoll(makePoll({ chosen: [] })).showResults).toBe(false)
    const voted = describePoll(counted([1, 0], { chosen: [0] }))
    expect(voted.showResults).toBe(true)
    expect(voted.options[0]?.mark).toBe('chosen')
    expect(voted.canRetract).toBe(true)
    expect(voted.footer).toBe('1 人已投票')
  })

  test('a closed poll shows results to everyone and allows no retraction', () => {
    const closed = describePoll(counted([1, 0], { closed: true, chosen: [0] }))
    expect(closed.showResults).toBe(true)
    expect(closed.canRetract).toBe(false)
    expect(closed.kindLabel).toBe('投票已结束')
  })

  test('a quiz marks the correct answer and a wrong pick, never allows retraction', () => {
    const quiz = describePoll(
      counted([1, 2, 0], { quiz: true, chosen: [0], correct_option: 1, explanation: '二加二等于四' }),
    )
    expect(quiz.options.map((option) => option.mark)).toEqual(['wrong', 'correct', 'none'])
    expect(quiz.canRetract).toBe(false)
    expect(quiz.explanation).toBe('二加二等于四')
    expect(quiz.footer).toBe('3 人已作答')
  })

  test('bars are the share of all voters', () => {
    const view = describePoll(counted([3, 1], { chosen: [0] }))
    expect(view.options.map((option) => option.share)).toEqual([0.75, 0.25])
  })

  test('kind labels follow Telegram', () => {
    expect(pollKindLabel(makePoll())).toBe('匿名投票')
    expect(pollKindLabel(makePoll({ public_voters: true, multiple_choice: true }))).toBe('公开投票 · 多选')
    expect(pollKindLabel(makePoll({ quiz: true }))).toBe('匿名测验')
    expect(pollKindLabel(makePoll({ quiz: true, closed: true }))).toBe('测验已结束')
  })
})
