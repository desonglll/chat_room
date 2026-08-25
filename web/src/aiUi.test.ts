import { describe, expect, test } from 'bun:test'
import { aiContextUsage } from './aiUi'

describe('AI context usage', () => {
  test('separates recent context from full-room RAG matches', () => {
    expect(aiContextUsage(130, 12)).toEqual({ recent: 118, retrieved: 12 })
  })

  test('does not claim RAG when no semantic evidence was injected', () => {
    expect(aiContextUsage(118, null)).toEqual({ recent: 118, retrieved: 0 })
  })
})
