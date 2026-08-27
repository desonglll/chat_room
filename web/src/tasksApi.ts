import { authHeaders, request } from './api'

export type TaskStatus = 'open' | 'in_progress' | 'done' | 'cancelled'
export type TaskFilter = 'all' | 'active' | 'done'

export interface RoomTaskSource {
  message_id: string
  sender: string
  excerpt: string
  recalled: boolean
  sent_at: string
}

export interface RoomTask {
  id: string
  room_id: string
  title: string
  status: TaskStatus
  assignee_id: string | null
  assignee_name: string
  assignee_active: boolean
  created_by_id: string | null
  created_by_name: string
  source: RoomTaskSource | null
  due_at: string | null
  version: number
  can_update: boolean
  can_delete: boolean
  created_at: string
  updated_at: string
}

export interface CreateRoomTaskPayload {
  title: string
  assignee_id: string | null
  due_at: string | null
  source_message_id: string | null
}

export interface UpdateRoomTaskPayload {
  title: string
  status: TaskStatus
  assignee_id: string | null
  due_at: string | null
  version: number
}

export class RoomTaskApiError extends Error {
  constructor(
    public readonly status: number,
    message: string,
  ) {
    super(message)
  }
}

function taskPath(roomId: string, taskId = ''): string {
  const base = `/api/rooms/${encodeURIComponent(roomId)}/tasks`
  return taskId ? `${base}/${encodeURIComponent(taskId)}` : base
}

function headers(token: string, password: string, json = false): Record<string, string> {
  const values = authHeaders(token)
  if (password) values['x-room-password'] = password
  if (json) values['Content-Type'] = 'application/json'
  return values
}

async function checked(response: Response, action: string): Promise<Response> {
  if (response.status === 400 || response.status === 422) {
    throw new RoomTaskApiError(response.status, '待办内容无效，请检查标题、负责人和截止时间')
  }
  if (response.status === 401) throw new RoomTaskApiError(401, '聊天室密码错误或登录已过期')
  if (response.status === 403) throw new RoomTaskApiError(403, '你没有执行此待办操作的权限')
  if (response.status === 404) throw new RoomTaskApiError(404, '待办或聊天室已不存在')
  if (response.status === 409) throw new RoomTaskApiError(409, '待办已被其他成员更新，列表已刷新')
  if (!response.ok) throw new RoomTaskApiError(response.status, `${action}失败：${response.status}`)
  return response
}

export function filterRoomTasks(tasks: RoomTask[], filter: TaskFilter): RoomTask[] {
  if (filter === 'active') return tasks.filter((task) => task.status === 'open' || task.status === 'in_progress')
  if (filter === 'done') return tasks.filter((task) => task.status === 'done' || task.status === 'cancelled')
  return tasks
}

export async function listRoomTasks(roomId: string, token: string, password: string): Promise<RoomTask[]> {
  return checked(await request(taskPath(roomId), { headers: headers(token, password) }), '读取待办').then(
    (response) => response.json() as Promise<RoomTask[]>,
  )
}

export async function createRoomTask(
  roomId: string,
  token: string,
  password: string,
  payload: CreateRoomTaskPayload,
): Promise<RoomTask> {
  const response = await request(taskPath(roomId), {
    method: 'POST',
    headers: headers(token, password, true),
    body: JSON.stringify(payload),
  })
  return checked(response, '创建待办').then((value) => value.json() as Promise<RoomTask>)
}

export async function updateRoomTask(
  roomId: string,
  taskId: string,
  token: string,
  password: string,
  payload: UpdateRoomTaskPayload,
): Promise<RoomTask> {
  const response = await request(taskPath(roomId, taskId), {
    method: 'PATCH',
    headers: headers(token, password, true),
    body: JSON.stringify(payload),
  })
  return checked(response, '更新待办').then((value) => value.json() as Promise<RoomTask>)
}

export async function deleteRoomTask(roomId: string, taskId: string, token: string, password: string): Promise<void> {
  await checked(
    await request(taskPath(roomId, taskId), { method: 'DELETE', headers: headers(token, password) }),
    '删除待办',
  )
}
