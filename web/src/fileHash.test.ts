import { describe, expect, test } from 'bun:test'
import { hashFile } from './composables/useChunkedUpload'

describe('file hashing', () => {
  test('computes the standard SHA-256 digest and reports read progress', async () => {
    const file = new File(['abc'], 'abc.txt', { type: 'text/plain' })
    const progress: number[] = []
    const digest = await hashFile(file, (bytes) => progress.push(bytes))

    expect(digest).toBe('ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad')
    expect(progress).toEqual([3])
  })
})
