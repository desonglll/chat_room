import { describe, expect, test } from 'bun:test'
import type { ApiClient } from './http'
import { contactName, createSocialApi } from './social'

function recorder() {
  const calls: string[] = []
  const client = {
    json: async (method: string, path: string, options: { body?: unknown; query?: { toString(): string } } = {}) => {
      calls.push(`${method} ${path}${options.query ? `?${options.query}` : ''} ${JSON.stringify(options.body ?? null)}`)
      return path === '/api/direct-chats' ? { room_id: 'r1' } : []
    },
    request: async (method: string, path: string, options: { body?: unknown } = {}) => {
      calls.push(`${method} ${path} ${JSON.stringify(options.body ?? null)}`)
      return new Response(null, { status: 204 })
    },
  } as unknown as ApiClient
  return { client, calls }
}

describe('TG-702 social client', () => {
  test('speaks the server routes', async () => {
    const { client, calls } = recorder()
    const api = createSocialApi(client, () => 't')
    await api.requests('incoming')
    await api.sendRequest('u1')
    await api.respond('u1', true)
    await api.cancelRequest('u2')
    await api.setRemark('u1', 'Al')
    await api.block('u3')
    await api.unblock('u3')
    expect(await api.openPrivateChat('u1')).toBe('r1')
    expect(calls).toEqual([
      'GET /api/friend-requests?direction=incoming null',
      'POST /api/friend-requests {"user_id":"u1"}',
      'PATCH /api/friend-requests/u1 {"action":"accept"}',
      'DELETE /api/friend-requests/u2 null',
      'PUT /api/friends/u1/remark {"remark":"Al"}',
      'PUT /api/blocks/u3 null',
      'DELETE /api/blocks/u3 null',
      'POST /api/direct-chats {"user_id":"u1"}',
    ])
  })

  test('a contact shows under your remark first', () => {
    expect(contactName({ remark: 'Mum', display_name: 'Ann', username: 'ann' })).toBe('Mum')
    expect(contactName({ remark: '', display_name: '', username: 'ann' })).toBe('ann')
  })
})
