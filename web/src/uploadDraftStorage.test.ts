import { describe, expect, test } from 'bun:test'
import { loadUploadDraft, removeUploadDraft, saveUploadDraft } from './uploadDraftStorage'

class MemoryStorage implements Storage {
  private readonly values = new Map<string, string>()
  get length() { return this.values.size }
  clear() { this.values.clear() }
  getItem(key: string) { return this.values.get(key) ?? null }
  key(index: number) { return [...this.values.keys()][index] ?? null }
  removeItem(key: string) { this.values.delete(key) }
  setItem(key: string, value: string) { this.values.set(key, value) }
}

describe('upload draft persistence', () => {
  test('restores message metadata for a resumed upload', () => {
    const storage = new MemoryStorage()
    const draft = { content: 'caption', replyTo: 'message-id', isSensitive: true }
    saveUploadDraft(storage, 'upload-id', draft)
    expect(loadUploadDraft(storage, 'upload-id')).toEqual(draft)
    removeUploadDraft(storage, 'upload-id')
    expect(loadUploadDraft(storage, 'upload-id')).toEqual({ content: '', replyTo: '', isSensitive: false })
  })
})
