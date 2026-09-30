import type { PollState } from '@tg/core'

export function makePoll(overrides: Partial<PollState> = {}): PollState {
  return {
    id: 'poll-1',
    question: '午饭吃什么？',
    closed: false,
    total_voters: 0,
    options: [
      { text: '面条', voters: 0 },
      { text: '米饭', voters: 0 },
      { text: '沙拉', voters: 0 },
    ],
    revision: 1,
    ...overrides,
  }
}

export function counted(voters: number[], overrides: Partial<PollState> = {}): PollState {
  return makePoll({
    total_voters: voters.reduce((sum, value) => sum + value, 0),
    options: voters.map((count, index) => ({ text: `选项${index + 1}`, voters: count })),
    ...overrides,
  })
}
