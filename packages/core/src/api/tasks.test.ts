import { describe, expect, test } from 'bun:test'
import type { ApiClient } from './http'
import type { ChatTask } from './tasks'
import { createTasksApi } from './tasks'

describe('TG-706 tasks client', () => {
  test('an update resends the whole task with its version (optimistic concurrency)', async () => {
    const calls: unknown[] = []
    const client = {
      json: async (method: string, path: string, options: { body?: unknown } = {}) => {
        calls.push([method, path, options.body])
        return {}
      },
      request: async () => new Response(null, { status: 204 }),
    } as unknown as ApiClient
    const task = { id: 't1', title: 'Ship', status: 'open', assignee_id: null, due_at: null, version: 3 } as ChatTask
    await createTasksApi(client, () => 'x').update('c1', task, { status: 'done' })
    expect(calls).toEqual([
      [
        'PATCH',
        '/api/chats/c1/tasks/t1',
        { title: 'Ship', status: 'done', assignee_id: null, due_at: null, version: 3 },
      ],
    ])
  })
})
