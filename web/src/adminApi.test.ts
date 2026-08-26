import { afterEach, describe, expect, mock, test } from 'bun:test'
import { AdminApiError, exportAdminBackup, restoreAdminBackup } from './adminApi'

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

  test('uploads the archive as multipart data', async () => {
    const fetchMock = mock(async (_input: RequestInfo | URL, init?: RequestInit) => {
      expect(init?.method).toBe('POST')
      expect(init?.headers).toEqual({ Authorization: 'Bearer admin-token' })
      expect(init?.body).toBeInstanceOf(FormData)
      return Response.json({
        backup_created_at: '2026-08-26T00:00:00Z',
        included_files: false,
        previous_files_preserved: false,
        redis_keys_cleared: 0,
        chat_rooms_locked: true,
      })
    })
    globalThis.fetch = fetchMock as typeof fetch

    const result = await restoreAdminBackup('admin-token', new File(['archive'], 'backup.tar.gz'))

    expect(result.included_files).toBe(false)
    expect(result.chat_rooms_locked).toBe(true)
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
