import { ref } from 'vue'
import {
  createRoomTask,
  deleteRoomTask,
  listRoomTasks,
  RoomTaskApiError,
  updateRoomTask,
  type CreateRoomTaskPayload,
  type RoomTask,
  type UpdateRoomTaskPayload,
} from '../tasksApi'

export function useRoomTasks(options: { roomId: () => string; token: () => string; password: () => string }) {
  const tasks = ref<RoomTask[]>([])
  const loading = ref(false)
  const saving = ref('')
  const error = ref('')
  let loadVersion = 0

  async function load(): Promise<void> {
    if (!options.roomId()) return
    const version = ++loadVersion
    const roomId = options.roomId()
    const token = options.token()
    const password = options.password()
    loading.value = true
    error.value = ''
    try {
      const loaded = await listRoomTasks(roomId, token, password)
      if (version === loadVersion) tasks.value = loaded
    } catch (caught) {
      if (version === loadVersion) error.value = message(caught, '读取待办失败')
    } finally {
      if (version === loadVersion) loading.value = false
    }
  }

  async function create(payload: CreateRoomTaskPayload): Promise<RoomTask | null> {
    const roomId = options.roomId()
    saving.value = 'new'
    error.value = ''
    try {
      const created = await createRoomTask(roomId, options.token(), options.password(), payload)
      if (options.roomId() === roomId) tasks.value = [created, ...tasks.value]
      return created
    } catch (caught) {
      error.value = message(caught, '创建待办失败')
      return null
    } finally {
      saving.value = ''
    }
  }

  async function update(task: RoomTask, payload: UpdateRoomTaskPayload): Promise<RoomTask | null> {
    const roomId = options.roomId()
    saving.value = task.id
    error.value = ''
    try {
      const updated = await updateRoomTask(roomId, task.id, options.token(), options.password(), payload)
      if (options.roomId() === roomId) {
        tasks.value = tasks.value.map((item) => (item.id === updated.id ? updated : item))
      }
      return updated
    } catch (caught) {
      error.value = message(caught, '更新待办失败')
      if (caught instanceof RoomTaskApiError && caught.status === 409 && options.roomId() === roomId) await load()
      return null
    } finally {
      saving.value = ''
    }
  }

  async function remove(task: RoomTask): Promise<boolean> {
    const roomId = options.roomId()
    saving.value = task.id
    error.value = ''
    try {
      await deleteRoomTask(roomId, task.id, options.token(), options.password())
      if (options.roomId() === roomId) tasks.value = tasks.value.filter((item) => item.id !== task.id)
      return true
    } catch (caught) {
      error.value = message(caught, '删除待办失败')
      return false
    } finally {
      saving.value = ''
    }
  }

  return { tasks, loading, saving, error, load, create, update, remove }
}

function message(error: unknown, fallback: string): string {
  return error instanceof Error ? error.message : fallback
}
