import { describe, expect, test } from 'bun:test'
import { appendUploadMessage, completeUploadMessage, updateUploadMessage } from './chatUploadMessages'
import type { BroadcastMessage, UploadMessage } from './types'

const upload: UploadMessage = {
  type: 'upload',
  key: 'upload-1',
  room_id: 'room-1',
  file_name: 'video.mp4',
  mime_type: 'video/mp4',
  size_bytes: 100,
  preview_url: '',
  is_sensitive: true,
  content: '',
  phase: 'queued',
  processed_bytes: 0,
  total_bytes: 100,
  status: 'pending',
  error: '',
  timestamp: '2026-08-19T00:00:00Z',
}

const completed: BroadcastMessage = {
  type: 'broadcast',
  message_id: 'message-1',
  sender_id: 'user-1',
  sender: 'mike',
  sender_avatar: '',
  content: '',
  attachment: null,
  reply_to: null,
  recalled_at: null,
  edited_at: null,
  timestamp: '2026-08-19T00:00:01Z',
  forwarded_from: null,
  reactions: [],
}

describe('upload messages', () => {
  test('updates and replaces the placeholder in place', () => {
    const queued = appendUploadMessage([], upload)
    const progressing = updateUploadMessage(queued, upload.key, { phase: 'uploading', processed_bytes: 50 })
    const result = completeUploadMessage([...progressing, completed], upload.key, completed)

    expect(progressing[0]).toMatchObject({ type: 'upload', phase: 'uploading', processed_bytes: 50 })
    expect(progressing[0]).toMatchObject({ is_sensitive: true })
    expect(result?.[0]).toMatchObject({ type: 'broadcast', message_id: 'message-1', motion: 'outgoing' })
    expect(result).toHaveLength(1)
  })
})
