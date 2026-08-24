<script setup lang="ts">
import { computed, ref } from 'vue'
import { CheckCheck } from 'lucide-vue-next'
import Popover from 'primevue/popover'
import type { RoomMember } from '../types'
import AppAvatar from './AppAvatar.vue'

const props = defineProps<{
  read: RoomMember[]
  unread: RoomMember[]
  direct: boolean
}>()

const receiptPopover = ref()
const label = computed(() => {
  if (!props.unread.length) return props.direct ? '已读' : '全部已读'
  if (!props.read.length) return '未读'
  return `${props.read.length} 人已读`
})
</script>

<template>
  <button
    type="button"
    class="cr-read-receipt ml-auto flex min-h-5 items-center gap-1 rounded px-1 text-[10px] hover:bg-surface-200"
    :class="unread.length ? 'text-muted-color' : 'text-success'"
    :aria-label="direct ? label : `${label}，查看详情`"
    @click="!direct && receiptPopover.toggle($event)"
  >
    <CheckCheck :size="13" aria-hidden="true" />
    <span>{{ label }}</span>
  </button>
  <Popover v-if="!direct" ref="receiptPopover">
    <div class="max-h-[min(70vh,420px)] w-64 overflow-y-auto">
      <section>
        <div class="mb-2 flex items-center justify-between">
          <strong class="text-sm">已读</strong>
          <span class="text-xs text-muted-color">{{ read.length }}</span>
        </div>
        <ul v-if="read.length" class="max-h-40 space-y-1 overflow-y-auto p-0">
          <li
            v-for="member in read"
            :key="member.user_id"
            class="flex min-h-9 items-center gap-2 rounded-md px-1.5 text-sm hover:bg-surface-50"
          >
            <AppAvatar
              :avatar="member.avatar_emoji"
              :fallback="member.username"
              :color-key="member.user_id"
              size="small"
              class="shrink-0 text-white!"
            />
            <span class="truncate">{{ member.username }}</span>
          </li>
        </ul>
        <p v-else class="py-2 text-xs text-muted-color">暂无</p>
      </section>
      <section class="mt-3 border-t border-surface-200 pt-3">
        <div class="mb-2 flex items-center justify-between">
          <strong class="text-sm">未读</strong>
          <span class="text-xs text-muted-color">{{ unread.length }}</span>
        </div>
        <ul v-if="unread.length" class="max-h-40 space-y-1 overflow-y-auto p-0">
          <li
            v-for="member in unread"
            :key="member.user_id"
            class="flex min-h-9 items-center gap-2 rounded-md px-1.5 text-sm hover:bg-surface-50"
          >
            <AppAvatar
              :avatar="member.avatar_emoji"
              :fallback="member.username"
              :color-key="member.user_id"
              size="small"
              class="shrink-0 text-white!"
            />
            <span class="truncate">{{ member.username }}</span>
          </li>
        </ul>
        <p v-else class="py-2 text-xs text-muted-color">暂无</p>
      </section>
    </div>
  </Popover>
</template>
