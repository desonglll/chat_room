import { afterEach, describe, expect, mock, test } from 'bun:test'
import { AdminApiError, executeAdminBackup, exportAdminBackup, syncAdminIndex, validateAdminBackup } from './adminApi'

const originalFetch = globalThis.fetch

afterEach(() => {
  globalThis.fetch = originalFetch
})

describe('admin backup API', () => {
  test('exports the selected backup scope and keeps the server filename', async () => {
    const fetchMock = mock(async (_input: RequestInfo | URL, init?: RequestInit) => {
      expect(init?.method).toBe('POST')
      expect(init?.headers).toEqual({
        Accept: 'application/json',
        Authorization: 'Bearer admin-token',
        'Content-Type': 'application/json',
      })
      expect(init?.body).toBe(JSON.stringify({ include_files: true }))
      return new Response('archive-bytes', {
        headers: { 'Content-Disposition': 'attachment; filename="chat-room-complete.tar.gz"' },
      })
    })
    globalThis.fetch = fetchMock as typeof fetch

    const result = await exportAdminBackup('admin-token', true)

    expect(result.filename).toBe('chat-room-complete.tar.gz')
    expect(await result.blob.text()).toBe('archive-bytes')
    expect(fetchMock).toHaveBeenCalledWith('/api/admin/backups/export', expect.any(Object))
  })

  test('validates before a separately confirmed restore', async () => {
    let request = 0
    const fetchMock = mock(async (_input: RequestInfo | URL, init?: RequestInit) => {
      expect(init?.method).toBe('POST')
      expect(init?.headers).toEqual({ Authorization: 'Bearer admin-token' })
      expect(init?.body).toBeInstanceOf(FormData)
      request += 1
      const form = init?.body as FormData
      if (request === 1) {
        expect(form.get('confirmation')).toBeNull()
        return Response.json({
          valid: true,
          backup_created_at: '2026-08-26T00:00:00Z',
          database_kind: 'sqlite',
          included_files: false,
          file_count: 1,
          total_bytes: 1024,
          checksum_status: 'verified',
          validation_duration_ms: 8,
        })
      }
      expect(form.get('confirmation')).toBe('RESTORE')
      return Response.json({
        backup_created_at: '2026-08-26T00:00:00Z',
        included_files: false,
        previous_database_preserved: true,
        previous_files_preserved: false,
        redis_keys_cleared: 0,
        vector_messages_queued: 0,
        chat_rooms_locked: true,
        restore_duration_ms: 250,
      })
    })
    globalThis.fetch = fetchMock as typeof fetch

    const file = new File(['archive'], 'backup.tar.gz')
    const validation = await validateAdminBackup('admin-token', file)
    const result = await executeAdminBackup('admin-token', file)

    expect(validation.valid).toBe(true)
    expect(result.included_files).toBe(false)
    expect(result.chat_rooms_locked).toBe(true)
    expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/admin/backups/restore', expect.any(Object))
    expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/admin/backups/restore/execute', expect.any(Object))
  })

  test('uses the server error message', async () => {
    globalThis.fetch = mock(async () =>
      Response.json({ error: '备份清单或文件校验失败' }, { status: 422 }),
    ) as typeof fetch

    try {
      await exportAdminBackup('admin-token', false)
      throw new Error('expected export to fail')
    } catch (error) {
      expect(error).toBeInstanceOf(AdminApiError)
      expect((error as AdminApiError).status).toBe(422)
      expect((error as Error).message).toBe('备份清单或文件校验失败')
    }
  })
})

test('queues an admin index synchronization', async () => {
  const fetchMock = mock(async (_input: RequestInfo | URL, init?: RequestInit) => {
    expect(init?.method).toBe('POST')
    expect(init?.body).toBe(JSON.stringify({ target: 'vector' }))
    return Response.json({ target: 'vector', queued_messages: 1043 })
  })
  globalThis.fetch = fetchMock as typeof fetch

  const result = await syncAdminIndex('vector', 'admin-token')

  expect(result).toEqual({ target: 'vector', queued_messages: 1043 })
  expect(fetchMock).toHaveBeenCalledWith('/api/admin/indexes/sync', expect.any(Object))
})
