import { afterEach, describe, expect, mock, test } from 'bun:test'
import {
  createRoomTask,
  filterRoomTasks,
  listRoomTasks,
  RoomTaskApiError,
  updateRoomTask,
  type RoomTask,
} from './tasksApi'

const originalFetch = globalThis.fetch

afterEach(() => {
  globalThis.fetch = originalFetch
})

function task(status: RoomTask['status']): RoomTask {
  return { id: status, status } as RoomTask
}

describe('room tasks API', () => {
  test('filters active and terminal work without changing server order', () => {
    const tasks = [task('open'), task('done'), task('in_progress'), task('cancelled')]
    expect(filterRoomTasks(tasks, 'active').map(({ status }) => status)).toEqual(['open', 'in_progress'])
    expect(filterRoomTasks(tasks, 'done').map(({ status }) => status)).toEqual(['done', 'cancelled'])
    expect(filterRoomTasks(tasks, 'all')).toEqual(tasks)
  })

  test('sends room authorization and full replacement version payloads', async () => {
    const fetchMock = mock(async (_path: string, options?: RequestInit) =>
      Response.json({ id: 'task-1', ...JSON.parse(String(options?.body || '{}')) }),
    )
    globalThis.fetch = fetchMock as typeof fetch
    await listRoomTasks('room/one', 'session-token', 'room-secret')
    await createRoomTask('room/one', 'session-token', 'room-secret', {
      title: 'Review launch notes',
      assignee_id: null,
      due_at: null,
      source_message_id: 'message-1',
    })
    await updateRoomTask('room/one', 'task/one', 'session-token', 'room-secret', {
      title: 'Review launch notes',
      status: 'done',
      assignee_id: 'user-1',
      due_at: null,
      version: 4,
    })

    expect(String(fetchMock.mock.calls[0]![0])).toContain('/api/rooms/room%2Fone/tasks')
    expect(fetchMock.mock.calls[0]![1]?.headers).toEqual({
      Accept: 'application/json',
      Authorization: 'Bearer session-token',
      'x-room-password': 'room-secret',
    })
    expect(String(fetchMock.mock.calls[2]![0])).toEndWith('/task%2Fone')
    expect(JSON.parse(String(fetchMock.mock.calls[2]![1]?.body))).toMatchObject({ status: 'done', version: 4 })
  })

  test('preserves optimistic conflict as a typed error', async () => {
    globalThis.fetch = mock(async () => new Response(null, { status: 409 })) as typeof fetch
    try {
      await updateRoomTask('room', 'task', 'token', '', {
        title: 'Current title',
        status: 'open',
        assignee_id: null,
        due_at: null,
        version: 1,
      })
      throw new Error('expected conflict')
    } catch (error) {
      expect(error).toBeInstanceOf(RoomTaskApiError)
      expect((error as RoomTaskApiError).status).toBe(409)
    }
  })

  test('turns malformed date payloads into a form error', async () => {
    globalThis.fetch = mock(async () => new Response(null, { status: 422 })) as typeof fetch
    expect(
      createRoomTask('room', 'token', '', {
        title: 'Schedule review',
        assignee_id: null,
        due_at: 'not-a-date',
        source_message_id: null,
      }),
    ).rejects.toThrow('待办内容无效')
  })
})
