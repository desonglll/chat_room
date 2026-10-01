import { describe, expect, test } from 'bun:test'
import { createAdminApi, formatStorageBytes } from './admin'
import { ApiError, type ApiClient } from './http'

const clientAnswering = (status: number) =>
  ({
    request: async (_method: string, path: string) => {
      if (status >= 400) throw new ApiError(status, path, 'Forbidden')
      return new Response('{}', { status })
    },
    json: async () => ({}),
  }) as unknown as ApiClient

describe('TG-705 admin client', () => {
  test('a 403 means "not an administrator", other failures stay errors', async () => {
    expect(await createAdminApi(clientAnswering(200), () => 't').isAdmin()).toBe(true)
    expect(await createAdminApi(clientAnswering(403), () => 't').isAdmin()).toBe(false)
    expect(await createAdminApi(clientAnswering(401), () => null).isAdmin()).toBe(false)
    await expect(createAdminApi(clientAnswering(500), () => 't').isAdmin()).rejects.toBeInstanceOf(ApiError)
  })

  test('storage sizes read in binary units', () => {
    expect(formatStorageBytes(512)).toBe('512 B')
    expect(formatStorageBytes(1536)).toBe('1.5 KB')
    expect(formatStorageBytes(5 * 1024 ** 3)).toBe('5.0 GB')
  })
})
