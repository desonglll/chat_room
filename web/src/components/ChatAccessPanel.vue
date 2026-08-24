<script setup lang="ts">
import { computed } from 'vue'
import { DoorOpen, LogIn, UserRound } from 'lucide-vue-next'
import Button from 'primevue/button'
import Message from 'primevue/message'
import Skeleton from 'primevue/skeleton'
import ScopedPasswordField from './ScopedPasswordField.vue'
import type { ChatStatus, Room, User } from '../types'

const props = defineProps<{
  room: Room | null
  user: User | null
  password: string
  status: ChatStatus
  error: string
  loading: boolean
}>()

const emit = defineEmits<{
  join: []
  requestJoin: []
  authenticate: []
  'update:password': [password: string]
}>()

const passwordModel = computed({
  get: () => props.password,
  set: (value: string) => emit('update:password', value),
})

function handleJoin(): void {
  if (props.room?.membership_status === 'active') emit('join')
  else emit('requestJoin')
}
</script>

<template>
  <section v-if="loading && !room" class="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto p-6">
    <div
      v-for="index in 4"
      :key="index"
      class="flex items-start gap-2.5"
      :class="{ 'flex-row-reverse': index % 2 === 0 }"
    >
      <Skeleton shape="circle" size="2rem" />
      <div class="space-y-2" :style="{ width: `${38 + ((index * 7) % 30)}%` }">
        <Skeleton height="0.8rem" width="40%" />
        <Skeleton height="2.4rem" border-radius="14px" />
      </div>
    </div>
  </section>

  <section v-else-if="!room" class="cr-chat-empty min-h-0 flex-1">
    <div class="cr-empty-state">
      <img src="/brand/echo-gate.svg" alt="" width="72" height="72" class="empty-mark" aria-hidden="true" />
      <small>ECHO GATE</small>
      <h2>选择一段会话</h2>
    </div>
  </section>

  <section v-else class="fade-in flex min-h-0 flex-1 items-center justify-center overflow-y-auto p-6">
    <form class="w-full max-w-[420px]" autocomplete="off" data-testid="join-form" @submit.prevent="handleJoin">
      <span class="grid size-12 place-items-center rounded-lg bg-primary-50 text-primary-700">
        <DoorOpen :size="23" />
      </span>
      <h3 class="mt-5 text-xl font-semibold">加入 {{ room.name }}</h3>
      <p class="mt-1.5 text-sm text-muted-color">
        {{
          room.membership_status === 'pending'
            ? '申请正在等待管理员审核'
            : room.membership_status === 'invited'
              ? '管理员已邀请你加入'
              : room.join_policy === 'approval'
                ? '提交申请后由管理员审核'
                : '验证后可直接加入'
        }}
      </p>

      <div
        v-if="user"
        class="mt-6 flex min-h-12 items-center gap-3 rounded-lg border border-surface-200 bg-surface-0 px-3 text-sm"
      >
        <UserRound :size="18" class="text-primary" />
        <span
          >以 <strong>{{ user.username }}</strong> 的身份加入</span
        >
      </div>

      <div v-if="room.has_password" class="mt-5 flex flex-col gap-2">
        <label for="joinPassword" class="text-sm font-medium">聊天室访问密码</label>
        <ScopedPasswordField
          v-model="passwordModel"
          input-id="joinPassword"
          name="room-access-password"
          scope="room-access"
        />
      </div>

      <Message v-if="room.membership_status === 'pending'" severity="info" size="small" :closable="false" class="mt-4">
        加入申请已提交
      </Message>
      <Message v-else-if="error" severity="error" size="small" :closable="false" class="mt-4">{{ error }}</Message>
      <Button
        v-if="user"
        class="mt-6 w-full"
        type="submit"
        :loading="status === 'connecting'"
        :disabled="room.membership_status === 'pending'"
      >
        <LogIn :size="18" />
        <span>{{
          room.membership_status === 'active'
            ? '进入聊天室'
            : room.membership_status === 'invited'
              ? '接受邀请并加入'
              : room.join_policy === 'approval'
                ? '申请加入聊天室'
                : '加入聊天室'
        }}</span>
      </Button>
      <Button v-else class="mt-6 w-full" type="button" @click="emit('authenticate')">
        <LogIn :size="18" />
        <span>登录或注册</span>
      </Button>
    </form>
  </section>
</template>

<style scoped>
@keyframes fade-in {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}

.fade-in {
  animation: fade-in 0.18s var(--cr-ease-out);
}

.cr-chat-empty {
  position: relative;
  display: grid;
  overflow: hidden;
  place-items: center;
  background: transparent;
}

.cr-chat-empty::before {
  position: absolute;
  width: min(34vw, 24rem);
  aspect-ratio: 0.78;
  border: 1px solid color-mix(in srgb, var(--cr-primary) 9%, transparent);
  border-radius: var(--cr-radius-lg);
  content: '';
  box-shadow:
    0 0 0 3.5rem color-mix(in srgb, var(--cr-primary) 2.5%, transparent),
    0 0 0 7rem color-mix(in srgb, var(--cr-primary) 1.8%, transparent);
}

.cr-empty-state {
  position: relative;
  display: grid;
  justify-items: center;
  color: var(--cr-text);
  text-align: center;
}

.cr-empty-state img {
  width: 4.5rem;
  height: 4.5rem;
  filter: drop-shadow(0 12px 22px rgba(23, 37, 33, 0.15));
}

.cr-empty-state small {
  margin-top: 1.25rem;
  color: var(--cr-primary);
  font-size: 0.65rem;
  font-weight: 800;
  letter-spacing: 0;
}

.cr-empty-state h2 {
  margin-top: 0.35rem;
  font-size: 1rem;
  font-weight: 650;
  letter-spacing: 0;
  text-wrap: balance;
}

.empty-mark {
  animation: empty-mark-in 0.24s var(--cr-ease-out);
}

@keyframes empty-mark-in {
  from {
    opacity: 0;
    transform: scale(0.92);
  }
}

@media (prefers-reduced-motion: reduce) {
  .fade-in,
  .empty-mark {
    animation: none;
  }
}

@media (max-width: 767px) {
  .cr-chat-empty::before {
    width: min(56vw, 15rem);
  }
}
</style>
