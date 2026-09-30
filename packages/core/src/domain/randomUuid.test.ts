// New in TG-011 (the old module shipped without a test): pins the RFC-4122 v4 shape of
// the fallback path and the preference for a native randomUUID when the source has one.
import { describe, expect, test } from 'bun:test'
import { createRandomUuid } from './randomUuid'

describe('random uuids', () => {
  test('prefers the injected source randomUUID', () => {
    const uuid = createRandomUuid({
      getRandomValues: () => {
        throw new Error('must not be called')
      },
      randomUUID: () => 'fixed-uuid',
    })
    expect(uuid).toBe('fixed-uuid')
  })

  test('builds a version-4 variant-1 uuid from raw random bytes', () => {
    const uuid = createRandomUuid({
      getRandomValues: (bytes) => {
        bytes.fill(0xff)
        return bytes
      },
    })
    expect(uuid).toBe('ffffffff-ffff-4fff-bfff-ffffffffffff')
  })

  test('defaults to the standards-level crypto global', () => {
    expect(createRandomUuid()).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/)
  })
})
