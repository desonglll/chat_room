<script setup lang="ts">
import { Eye, EyeOff, Save, Trash2, X } from 'lucide-vue-next'
import { ref, watch } from 'vue'
import { deleteRoom, updateRoom } from '../api'
import type { Room, RoomUpdateResult } from '../types'

const props = defineProps<{
  open: boolean
  room: Room | null
  credential: string
}>()
const emit = defineEmits<{
  close: []
  updated: [result: RoomUpdateResult]
  deleted: [roomId: string]
}>()

const name = ref('')
const currentPassword = ref('')
const newPassword = ref('')
const removePassword = ref(false)
const currentVisible = ref(false)
const newVisible = ref(false)
const confirmingDelete = ref(false)
const error = ref('')
const busy = ref(false)

watch(() => props.open, (open) => {
  if (!open || !props.room) return
  name.value = props.room.name
  currentPassword.value = props.room.has_password ? props.credential : ''
  newPassword.value = ''
  removePassword.value = false
  currentVisible.value = false
  newVisible.value = false
  confirmingDelete.value = false
  error.value = ''
})

function validateCurrentPassword(message: string): boolean {
  if (props.room?.has_password && !currentPassword.value) {
    error.value = message
    return false
  }
  return true
}

async function save(): Promise<void> {
  if (!props.room) return
  const normalizedName = name.value.trim()
  if (!normalizedName) {
    error.value = '请输入房间名称'
    return
  }
  if (!validateCurrentPassword('请输入当前房间密码')) return

  const passwordChanged = removePassword.value || newPassword.value.length > 0
  const payload: { name: string; current_password?: string; new_password?: string } = {
    name: normalizedName,
  }
  if (props.room.has_password) payload.current_password = currentPassword.value
  if (passwordChanged) payload.new_password = removePassword.value ? '' : newPassword.value

  busy.value = true
  error.value = ''
  try {
    const room = await updateRoom(props.room.id, payload)
    emit('updated', {
      room,
      passwordChanged,
      password: room.has_password
        ? (passwordChanged ? newPassword.value : currentPassword.value)
        : '',
    })
  } catch (caught) {
    error.value = caught instanceof Error ? caught.message : '保存失败'
  } finally {
    busy.value = false
  }
}

function beginDelete(): void {
  if (!validateCurrentPassword('删除前请输入当前房间密码')) return
  confirmingDelete.value = true
  error.value = ''
}

async function confirmDelete(): Promise<void> {
  if (!props.room) return
  const room = props.room
  busy.value = true
  error.value = ''
  try {
    await deleteRoom(room.id, currentPassword.value)
    emit('deleted', room.id)
  } catch (caught) {
    error.value = caught instanceof Error ? caught.message : '删除失败'
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open && room" class="modal-backdrop" @mousedown.self="emit('close')">
      <section class="modal" role="dialog" aria-modal="true" aria-labelledby="manageTitle">
        <template v-if="!confirmingDelete">
          <header class="modal-header">
            <h2 id="manageTitle">管理聊天室</h2>
            <button class="icon-button" type="button" aria-label="关闭" title="关闭" @click="emit('close')">
              <X :size="18" />
            </button>
          </header>
          <form @submit.prevent="save">
            <div class="modal-body form-stack">
              <label for="manageRoomName">房间名称</label>
              <input id="manageRoomName" v-model="name" type="text" maxlength="80" required>

              <template v-if="room.has_password">
                <label for="currentRoomPassword">当前密码</label>
                <div class="password-input">
                  <input id="currentRoomPassword" v-model="currentPassword" :type="currentVisible ? 'text' : 'password'" maxlength="256" autocomplete="current-password">
                  <button type="button" :aria-label="currentVisible ? '隐藏密码' : '显示密码'" :title="currentVisible ? '隐藏密码' : '显示密码'" @click="currentVisible = !currentVisible">
                    <EyeOff v-if="currentVisible" :size="18" />
                    <Eye v-else :size="18" />
                  </button>
                </div>
              </template>

              <label for="newRoomPassword">新密码 <span>留空则不更改</span></label>
              <div class="password-input">
                <input id="newRoomPassword" v-model="newPassword" :disabled="removePassword" :type="newVisible ? 'text' : 'password'" maxlength="256" autocomplete="new-password">
                <button type="button" :disabled="removePassword" :aria-label="newVisible ? '隐藏密码' : '显示密码'" :title="newVisible ? '隐藏密码' : '显示密码'" @click="newVisible = !newVisible">
                  <EyeOff v-if="newVisible" :size="18" />
                  <Eye v-else :size="18" />
                </button>
              </div>

              <label v-if="room.has_password" class="checkbox-row">
                <input v-model="removePassword" type="checkbox">
                移除密码，改为公开房间
              </label>
              <p v-if="error" class="form-error" role="alert">{{ error }}</p>
            </div>
            <footer class="modal-footer split">
              <button class="danger-button" type="button" @click="beginDelete">
                <Trash2 :size="17" />
                删除聊天室
              </button>
              <div class="button-group">
                <button class="secondary-button" type="button" @click="emit('close')">取消</button>
                <button class="primary-button compact" type="submit" :disabled="busy">
                  <Save :size="17" />
                  {{ busy ? '正在保存' : '保存' }}
                </button>
              </div>
            </footer>
          </form>
        </template>

        <template v-else>
          <header class="modal-header">
            <h2 id="manageTitle">删除聊天室</h2>
          </header>
          <div class="modal-body">
            <p class="confirm-copy">确定删除“<strong>{{ room.name }}</strong>”吗？该房间的全部消息也会被删除。</p>
            <p v-if="error" class="form-error" role="alert">{{ error }}</p>
          </div>
          <footer class="modal-footer">
            <button class="secondary-button" type="button" @click="confirmingDelete = false">取消</button>
            <button class="danger-button filled" type="button" :disabled="busy" @click="confirmDelete">
              <Trash2 :size="17" />
              {{ busy ? '正在删除' : '确认删除' }}
            </button>
          </footer>
        </template>
      </section>
    </div>
  </Teleport>
</template>
