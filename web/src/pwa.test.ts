import { describe, expect, test } from 'bun:test'
import { clearPwaCaches, isPwaStaticCache } from './pwa'

describe('PWA cache privacy boundary', () => {
  test('recognizes only the static application cache namespace', () => {
    expect(isPwaStaticCache('echo-gate-static-release')).toBe(true)
    expect(isPwaStaticCache('api-responses')).toBe(false)
    expect(isPwaStaticCache('attachments')).toBe(false)
  })

  test('logout cleanup leaves unrelated browser caches untouched', async () => {
    const deleted: string[] = []
    await clearPwaCaches({
      keys: async () => ['echo-gate-static-old', 'other-origin-tooling'],
      delete: async (name) => {
        deleted.push(name)
        return true
      },
    })
    expect(deleted).toEqual(['echo-gate-static-old'])
  })
})
