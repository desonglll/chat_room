// Migrated from web/src/fileHash.test.ts (TG-011). The digest primitive is injected
// (dependency grant: zustand only), so the test injects Bun's own SHA-256 — the digest is
// still checked against the standard 'abc' vector, not a mock.
import { describe, expect, test } from 'bun:test'
import type { FileChunkSource, Sha256StreamFactory } from '../types'
import { hashFile } from './fileHash'

const bunSha256: Sha256StreamFactory = () => {
  const hasher = new Bun.CryptoHasher('sha256')
  return {
    update: (bytes) => hasher.update(bytes),
    digestHex: () => hasher.digest('hex'),
  }
}

const sourceOf = (text: string): FileChunkSource => {
  const bytes = new TextEncoder().encode(text)
  return {
    size: bytes.length,
    readChunk: async (start, end) => bytes.slice(start, end),
  }
}

describe('file hashing', () => {
  test('computes the standard SHA-256 digest and reports read progress', async () => {
    const progress: number[] = []
    const digest = await hashFile(sourceOf('abc'), bunSha256, { onProgress: (bytes) => progress.push(bytes) })

    expect(digest).toBe('ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad')
    expect(progress).toEqual([3])
  })

  test('chunks large sources and reports progress per chunk', async () => {
    const progress: number[] = []
    const digest = await hashFile(sourceOf('abcdef'), bunSha256, {
      chunkSize: 2,
      onProgress: (bytes) => progress.push(bytes),
    })

    expect(digest).toBe('bef57ec7f53a6d40beb640a780a639c83bc29ac8a9816f1fc6c5c6dcd93c4721')
    expect(progress).toEqual([2, 4, 6])
  })

  test('aborts between chunks with an AbortError', async () => {
    let reads = 0
    const source: FileChunkSource = {
      size: 4,
      readChunk: async (start, end) => {
        reads += 1
        return new Uint8Array(end - start)
      },
    }
    const promise = hashFile(source, bunSha256, { chunkSize: 2, isCancelled: () => reads >= 1 })
    await expect(promise).rejects.toMatchObject({ name: 'AbortError' })
    expect(reads).toBe(1)
  })
})
