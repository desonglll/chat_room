<script setup lang="ts">
import { computed, ref } from 'vue'
import { CheckCheck } from 'lucide-vue-next'
import Avatar from 'primevue/avatar'
import Popover from 'primevue/popover'
import { avatarColor } from '../avatarColor'
import type { RoomMember } from '../types'

const props = defineProps<{
  read: RoomMember[]
  unread: RoomMember[]
}>()

const receiptPopover = ref()
const label = computed(() => {
  if (!props.unread.length) return '全部已读'
  if (!props.read.length) return '未读'
  return `${props.read.length} 人已读`
})

function avatarLabel(member: RoomMember): string {
  return member.avatar_emoji || member.username.slice(0, 1).toUpperCase()
}
</script>

<template>
  <button
    type="button"
    class="mt-1 ml-auto flex min-h-6 items-center gap-1 rounded px-1.5 text-[11px] transition-colors hover:bg-surface-200"
    :class="unread.length ? 'text-muted-color' : 'text-success'"
    :aria-label="`${label}，查看详情`"
    @click="receiptPopover.toggle($event)"
  >
    <CheckCheck :size="13" />
    <span>{{ label }}</span>
  </button>
  <Popover ref="receiptPopover">
    <div class="max-h-[min(70vh,420px)] w-64 overflow-y-auto">
      <section>
        <div class="mb-2 flex items-center justify-between">
          <strong class="text-sm">已读</strong>
          <span class="text-xs text-muted-color">{{ read.length }}</span>
        </div>
        <ul v-if="read.length" class="max-h-40 space-y-1 overflow-y-auto p-0">
          <li v-for="member in read" :key="member.user_id" class="flex min-h-9 items-center gap-2 rounded-md px-1.5 text-sm hover:bg-surface-50">
            <Avatar
              :label="avatarLabel(member)"
              shape="circle"
              size="small"
              class="shrink-0 text-white!"
              :style="{ backgroundColor: avatarColor(member.user_id) }"
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
          <li v-for="member in unread" :key="member.user_id" class="flex min-h-9 items-center gap-2 rounded-md px-1.5 text-sm hover:bg-surface-50">
            <Avatar
              :label="avatarLabel(member)"
              shape="circle"
              size="small"
              class="shrink-0 text-white!"
              :style="{ backgroundColor: avatarColor(member.user_id) }"
            />
            <span class="truncate">{{ member.username }}</span>
          </li>
        </ul>
        <p v-else class="py-2 text-xs text-muted-color">暂无</p>
      </section>
    </div>
  </Popover>
</template>
