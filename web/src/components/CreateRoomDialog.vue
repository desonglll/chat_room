<script setup lang="ts">
import { Eye, EyeOff, Plus, X } from 'lucide-vue-next'
import { ref, watch } from 'vue'
import { createRoom } from '../api'
import type { Room } from '../types'

const props = defineProps<{ open: boolean }>()
const emit = defineEmits<{
  close: []
  created: [room: Room, password: string]
}>()

const name = ref('')
const password = ref('')
const passwordVisible = ref(false)
const error = ref('')
const busy = ref(false)

watch(() => props.open, (open) => {
  if (!open) return
  name.value = ''
  password.value = ''
  passwordVisible.value = false
  error.value = ''
})

async function submit(): Promise<void> {
  const normalizedName = name.value.trim()
  if (!normalizedName) {
    error.value = '请输入房间名称'
    return
  }
  busy.value = true
  error.value = ''
  try {
    const room = await createRoom(normalizedName, password.value)
    emit('created', room, password.value)
  } catch (caught) {
    error.value = caught instanceof Error ? caught.message : '创建房间失败'
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open" class="modal-backdrop" @mousedown.self="emit('close')">
      <section class="modal" role="dialog" aria-modal="true" aria-labelledby="createTitle">
        <header class="modal-header">
          <h2 id="createTitle">新建聊天室</h2>
          <button class="icon-button" type="button" aria-label="关闭" title="关闭" @click="emit('close')">
            <X :size="18" />
          </button>
        </header>
        <form @submit.prevent="submit">
          <div class="modal-body form-stack">
            <label for="createRoomName">房间名称</label>
            <input id="createRoomName" v-model="name" type="text" maxlength="80" required autofocus placeholder="例如：产品讨论">

            <label for="createRoomPassword">密码 <span>可选</span></label>
            <div class="password-input">
              <input id="createRoomPassword" v-model="password" :type="passwordVisible ? 'text' : 'password'" maxlength="256" autocomplete="new-password">
              <button type="button" :aria-label="passwordVisible ? '隐藏密码' : '显示密码'" :title="passwordVisible ? '隐藏密码' : '显示密码'" @click="passwordVisible = !passwordVisible">
                <EyeOff v-if="passwordVisible" :size="18" />
                <Eye v-else :size="18" />
              </button>
            </div>
            <p v-if="error" class="form-error" role="alert">{{ error }}</p>
          </div>
          <footer class="modal-footer">
            <button class="secondary-button" type="button" @click="emit('close')">取消</button>
            <button class="primary-button compact" type="submit" :disabled="busy">
              <Plus :size="17" />
              {{ busy ? '正在创建' : '创建' }}
            </button>
          </footer>
        </form>
      </section>
    </div>
  </Teleport>
</template>
