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
  <main class="min-h-0 min-w-0 flex-1 overflow-y-auto overscroll-contain bg-surface-0">
    <header
      class="sticky top-0 z-10 flex h-16 items-center gap-3 border-b border-surface-200 bg-surface-0/95 px-3 backdrop-blur sm:px-5"
    >
      <Button text rounded severity="secondary" aria-label="返回聊天" title="返回聊天" @click="emit('back')"
        ><ArrowLeft :size="19"
      /></Button>
      <div class="min-w-0 flex-1">
        <h2 class="text-base font-semibold">发现聊天室</h2>
        <p class="mt-0.5 text-xs text-muted-color">浏览并加入公开聊天室</p>
      </div>
    </header>

    <div class="mx-auto w-full max-w-3xl px-4 py-5 sm:px-6">
      <IconField class="w-full">
        <InputIcon class="text-surface-500"><Search :size="16" aria-hidden="true" /></InputIcon>
        <InputText
          v-model="query"
          name="room-discovery-search"
          autocomplete="off"
          placeholder="搜索聊天室名称…"
          variant="filled"
          fluid
          aria-label="搜索聊天室"
          class="h-10 rounded-lg! border-transparent! bg-surface-100! pl-10! hover:bg-surface-100! focus:border-primary! focus:bg-surface-0!"
        />
      </IconField>

      <Message v-if="error" severity="error" :closable="false" class="mt-4">{{ error }}</Message>

      <div v-if="loading" class="mt-4 divide-y divide-surface-100 border-y border-surface-200">
        <div v-for="index in 4" :key="index" class="h-[68px] animate-pulse bg-surface-50 motion-reduce:animate-none" />
      </div>

      <div v-else-if="discoverable.length === 0" class="mt-16 flex flex-col items-center text-center text-muted-color">
        <span class="grid size-12 place-items-center rounded-full bg-primary-50 text-primary"
          ><Compass :size="22" aria-hidden="true"
        /></span>
        <strong class="mt-3 text-sm text-color">{{ query ? '没有匹配的聊天室' : '暂无可发现的公开聊天室' }}</strong>
      </div>

      <ul v-else class="mt-4 divide-y divide-surface-100 border-y border-surface-200">
        <li
          v-for="room in discoverable"
          :key="room.id"
          class="flex min-h-[68px] items-center gap-3 px-2 py-2 transition-colors [contain-intrinsic-size:68px] [content-visibility:auto] hover:bg-surface-50 motion-reduce:transition-none sm:px-3"
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
