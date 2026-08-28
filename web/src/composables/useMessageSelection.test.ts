import { describe, expect, test } from 'bun:test'
import { useMessageSelection } from './useMessageSelection'

describe('message selection entry', () => {
  test('enters selection mode with the first chosen message selected', () => {
    const selection = useMessageSelection([], {
      download: () => {},
      favorite: () => {},
      forward: () => {},
      assistant: () => {},
    })

    selection.toggleSelection('message-1')

    expect(selection.selecting.value).toBe(true)
    expect(selection.selectedMessageIds.value).toEqual(['message-1'])
  })
})
