/**
 * TG-706 chat tasks (`/api/chats/:id/tasks`) and the chat audit log
 * (`/api/chats/:id/audit-events`, chat managers only). Updates carry the task's `version`:
 * the server refuses a stale one (409) instead of overwriting someone else's change.
 */
import { encodePathSegment, QueryParams, type ApiClient } from './http'

export type TaskStatus = 'open' | 'in_progress' | 'done' | 'cancelled'
export const TASK_STATUSES: readonly TaskStatus[] = ['open', 'in_progress', 'done', 'cancelled']
export const TASK_TITLE_MAX = 120

export interface ChatTask {
  id: string
  room_id: string
  title: string
  status: TaskStatus
  assignee_id: string | null
  assignee_name: string
  created_by_name: string
  due_at: string | null
  version: number
  can_update: boolean
  can_delete: boolean
  created_at: string
}

export interface AuditEvent {
  id: string
  actor_username: string
  event_type: string
  target_type: string | null
  target_id: string | null
  details: Record<string, string>
  created_at: string
}

export interface TasksApi {
  list(chatId: string): Promise<ChatTask[]>
  create(chatId: string, title: string): Promise<ChatTask>
  update(chatId: string, task: ChatTask, change: { title?: string; status?: TaskStatus }): Promise<ChatTask>
  remove(chatId: string, taskId: string): Promise<void>
  auditEvents(chatId: string, cursor?: string): Promise<{ items: AuditEvent[]; next_cursor: string | null }>
}

export function createTasksApi(client: ApiClient, token: () => string | null): TasksApi {
  const auth = () => {
    const value = token()
    return value ? { token: value } : {}
  }
  const tasks = (chatId: string) => `/api/chats/${encodePathSegment(chatId)}/tasks`
  return {
    list: (chatId) => client.json<ChatTask[]>('GET', tasks(chatId), auth()),
    create: (chatId, title) => client.json<ChatTask>('POST', tasks(chatId), { ...auth(), body: { title } }),
    update: (chatId, task, change) =>
      client.json<ChatTask>('PATCH', `${tasks(chatId)}/${encodePathSegment(task.id)}`, {
        ...auth(),
        body: {
          title: change.title ?? task.title,
          status: change.status ?? task.status,
          assignee_id: task.assignee_id,
          due_at: task.due_at,
          version: task.version,
        },
      }),
    remove: async (chatId, taskId) => {
      await client.request('DELETE', `${tasks(chatId)}/${encodePathSegment(taskId)}`, auth())
    },
    auditEvents: (chatId, cursor) =>
      client.json('GET', `/api/chats/${encodePathSegment(chatId)}/audit-events`, {
        ...auth(),
        query: new QueryParams({ limit: '50', ...(cursor ? { cursor } : {}) }),
      }),
  }
}
