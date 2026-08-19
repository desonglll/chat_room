import { describe, expect, test } from 'bun:test'
import { uploadPercent } from './attachmentUploadProgress'

describe('attachment upload progress', () => {
  test('reports measured bytes within hashing and upload phases', () => {
    expect(uploadPercent('queued', 0, 100)).toBe(0)
    expect(uploadPercent('hashing', 50, 100)).toBe(50)
    expect(uploadPercent('hashing', 100, 100)).toBe(100)
    expect(uploadPercent('uploading', 0, 100)).toBe(0)
    expect(uploadPercent('uploading', 50, 100)).toBe(50)
    expect(uploadPercent('uploading', 100, 100)).toBe(100)
    expect(uploadPercent('deduplicating', 100, 100)).toBe(100)
    expect(uploadPercent('finalizing', 100, 100)).toBe(100)
  })

  test('clamps invalid byte counts', () => {
    expect(uploadPercent('uploading', -1, 100)).toBe(0)
    expect(uploadPercent('uploading', 200, 100)).toBe(100)
  })
})
