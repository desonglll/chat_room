<script setup lang="ts">
import { computed, ref } from 'vue'
import { ArrowLeft, Compass, LogIn, Search } from 'lucide-vue-next'
import Button from 'primevue/button'
import IconField from 'primevue/iconfield'
import InputIcon from 'primevue/inputicon'
import InputText from 'primevue/inputtext'
import Message from 'primevue/message'
import { avatarColor } from '../avatarColor'
import type { Room, User } from '../types'

const props = defineProps<{
  rooms: Room[]
  user: User | null
  loading: boolean
  joiningId: string
  error: string
}>()

const emit = defineEmits<{
  back: []
  join: [room: Room]
  authenticate: []
}>()

const query = ref('')

// Every room here is guaranteed password-free — private rooms the caller
// hasn't joined never appear in the list the backend returns at all.
const discoverable = computed(() => {
  const needle = query.value.trim().toLowerCase()
  return props.rooms
    .filter((room) => !room.membership_status)
    .filter((room) => !needle || room.name.toLowerCase().includes(needle))
})

function joinLabel(room: Room): string {
  return room.join_policy === 'approval' ? '申请加入' : '加入'
}
</script>

<template>
  <main class="min-h-0 min-w-0 flex-1 overflow-y-auto bg-surface-0">
    <header
      class="sticky top-0 z-10 flex h-[72px] items-center gap-3 border-b border-surface-200 bg-surface-0/95 px-4 backdrop-blur sm:px-7"
    >
      <Button text rounded severity="secondary" aria-label="返回聊天" title="返回聊天" @click="emit('back')"
        ><ArrowLeft :size="19"
      /></Button>
      <div class="min-w-0 flex-1">
        <h2 class="text-base font-semibold">发现聊天室</h2>
        <p class="mt-0.5 text-xs text-muted-color">浏览并加入公开聊天室</p>
      </div>
    </header>

    <div class="mx-auto w-full max-w-2xl px-5 py-6 sm:px-8">
      <IconField>
        <InputIcon><Search :size="16" /></InputIcon>
        <InputText v-model="query" placeholder="搜索聊天室名称" fluid />
      </IconField>

      <Message v-if="error" severity="error" :closable="false" class="mt-4">{{ error }}</Message>

      <div v-if="loading" class="mt-6 space-y-2">
        <div v-for="index in 4" :key="index" class="h-16 animate-pulse rounded-lg bg-surface-100" />
      </div>

      <div v-else-if="discoverable.length === 0" class="mt-16 flex flex-col items-center text-center text-muted-color">
        <span
          class="grid size-14 place-items-center rounded-xl bg-gradient-to-br from-primary-50 to-surface-0 shadow-sm"
          ><Compass :size="23"
        /></span>
        <strong class="mt-3 text-sm text-color">{{ query ? '没有匹配的聊天室' : '暂无可发现的公开聊天室' }}</strong>
      </div>

      <ul v-else class="mt-4 space-y-2">
        <li
          v-for="room in discoverable"
          :key="room.id"
          class="flex items-center gap-3 rounded-xl bg-surface-0 px-4 py-3 shadow-xs transition-shadow hover:shadow-sm"
        >
          <span
            class="grid size-10 shrink-0 place-items-center rounded-full text-base text-white"
            :style="{ backgroundColor: avatarColor(room.id) }"
          >
            <template v-if="room.avatar_emoji">{{ room.avatar_emoji }}</template>
            <Compass v-else :size="17" />
          </span>
          <div class="min-w-0 flex-1">
            <strong class="block truncate text-sm font-semibold">{{ room.name }}</strong>
            <small v-if="room.description" class="mt-0.5 block truncate text-xs text-muted-color">{{
              room.description
            }}</small>
          </div>
          <Button
            v-if="user"
            size="small"
            :loading="joiningId === room.id"
            :disabled="joiningId !== '' && joiningId !== room.id"
            @click="emit('join', room)"
          >
            {{ joinLabel(room) }}
          </Button>
          <Button v-else size="small" outlined @click="emit('authenticate')">
            <LogIn :size="15" /><span>登录后加入</span>
          </Button>
        </li>
      </ul>
    </div>
  </main>
</template>
