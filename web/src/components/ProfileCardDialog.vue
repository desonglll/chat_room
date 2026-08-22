<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { ExternalLink, Save } from 'lucide-vue-next'
import Avatar from 'primevue/avatar'
import Button from 'primevue/button'
import Dialog from 'primevue/dialog'
import InputText from 'primevue/inputtext'
import Message from 'primevue/message'
import Skeleton from 'primevue/skeleton'
import { getUserProfile, setRoomNickname } from '../api'
import { avatarColor } from '../avatarColor'
import type { User } from '../types'

const props = defineProps<{
  open: boolean
  userId: string
  token: string
  roomId?: string
  currentUserId?: string
}>()
const emit = defineEmits<{ close: [] }>()

const profile = ref<User | null>(null)
const loading = ref(false)
const error = ref('')
const nickname = ref('')
const savingNickname = ref(false)
const nicknameSaved = ref(false)
const visible = computed({
  get: () => props.open,
  set: (value: boolean) => {
    if (!value) emit('close')
  },
})
const isSelf = computed(() => Boolean(props.roomId) && props.userId === props.currentUserId)

watch(
  () => [props.open, props.userId],
  async ([open, userId]) => {
    if (!open || !userId) return
    profile.value = null
    error.value = ''
    nickname.value = ''
    nicknameSaved.value = false
    loading.value = true
    try {
      profile.value = await getUserProfile(userId as string, props.token)
    } catch (caught) {
      error.value = caught instanceof Error ? caught.message : '读取用户资料失败'
    } finally {
      loading.value = false
    }
  },
  { immediate: true },
)

async function saveNickname(): Promise<void> {
  if (!props.roomId) return
  savingNickname.value = true
  nicknameSaved.value = false
  error.value = ''
  try {
    await setRoomNickname(props.roomId, props.token, nickname.value.trim())
    nicknameSaved.value = true
  } catch (caught) {
    error.value = caught instanceof Error ? caught.message : '设置昵称失败'
  } finally {
    savingNickname.value = false
  }
}
</script>

<template>
  <Dialog v-model:visible="visible" modal header="用户资料" class="w-[min(92vw,380px)]" :draggable="false">
    <div v-if="loading" class="flex flex-col items-center gap-3 py-4">
      <Skeleton shape="circle" size="4rem" />
      <Skeleton width="60%" height="1rem" />
      <Skeleton width="80%" height="0.8rem" />
    </div>
    <Message v-else-if="error" severity="error" size="small" :closable="false">{{ error }}</Message>
    <div v-else-if="profile" class="flex flex-col items-center gap-3 py-2 text-center">
      <Avatar
        :label="profile.avatar_emoji || profile.username.slice(0, 1).toUpperCase()"
        shape="circle"
        class="size-16! text-3xl! text-white!"
        :style="{ backgroundColor: avatarColor(profile.id) }"
      />
      <div>
        <strong class="block text-base">{{ profile.display_name || profile.username }}</strong>
        <span class="text-xs text-muted-color">@{{ profile.username }}</span>
      </div>
      <p v-if="profile.signature" class="text-sm text-surface-600">{{ profile.signature }}</p>
      <a
        v-if="profile.homepage"
        :href="profile.homepage"
        target="_blank"
        rel="noopener noreferrer"
        class="inline-flex items-center gap-1 text-xs text-primary hover:underline"
      >
        {{ profile.homepage }} <ExternalLink :size="12" />
      </a>

      <div v-if="isSelf" class="mt-2 w-full border-t border-surface-200 pt-4 text-left">
        <label for="roomNickname" class="mb-2 block text-sm font-medium">本房间昵称</label>
        <div class="flex gap-2">
          <InputText
            id="roomNickname"
            v-model="nickname"
            name="room-nickname"
            autocomplete="off"
            maxlength="48"
            placeholder="留空则显示原名称…"
            class="min-w-0 flex-1"
            fluid
          />
          <Button type="button" :loading="savingNickname" @click="saveNickname"><Save :size="16" /></Button>
        </div>
        <small v-if="nicknameSaved" class="mt-1 block text-success">已保存</small>
      </div>
    </div>
  </Dialog>
</template>
