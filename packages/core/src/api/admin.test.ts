import { describe, expect, test } from 'bun:test'
import { createAdminApi, formatStorageBytes } from './admin'
import { ApiError, type ApiClient } from './http'

describe('TG-705 admin client', () => {
  test('isAdmin reads GET /api/admin/access; 401 is "no", other failures stay errors (TG-905)', async () => {
    const answering = (status: number, isAdmin = false) => {
      const paths: string[] = []
      const client = {
        json: async (_method: string, path: string) => {
          paths.push(path)
          if (status >= 400) throw new ApiError(status, path, 'Refused')
          return { is_admin: isAdmin }
        },
      } as unknown as ApiClient
      return { client, paths }
    }
    const admin = answering(200, true)
    expect(await createAdminApi(admin.client, () => 't').isAdmin()).toBe(true)
    expect(admin.paths).toEqual(['/api/admin/access'])
    expect(await createAdminApi(answering(200, false).client, () => 't').isAdmin()).toBe(false)
    expect(await createAdminApi(answering(401).client, () => null).isAdmin()).toBe(false)
    await expect(createAdminApi(answering(500).client, () => 't').isAdmin()).rejects.toBeInstanceOf(ApiError)
  })

  test('storage sizes read in binary units', () => {
    expect(formatStorageBytes(512)).toBe('512 B')
    expect(formatStorageBytes(1536)).toBe('1.5 KB')
    expect(formatStorageBytes(5 * 1024 ** 3)).toBe('5.0 GB')
  })
})
