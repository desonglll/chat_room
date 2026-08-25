<script setup lang="ts">
import { computed } from 'vue'
import { DoorOpen, LogIn, UserRound } from 'lucide-vue-next'
import Button from 'primevue/button'
import Message from 'primevue/message'
import Skeleton from 'primevue/skeleton'
import ToggleSwitch from 'primevue/toggleswitch'
import ScopedPasswordField from './ScopedPasswordField.vue'
import type { ChatStatus, Room, User } from '../types'

const props = defineProps<{
  room: Room | null
  user: User | null
  password: string
  rememberRoomPasswords: boolean
  status: ChatStatus
  error: string
  loading: boolean
}>()

const emit = defineEmits<{
  join: []
  requestJoin: []
  authenticate: []
  'update:password': [password: string]
  'update:rememberRoomPasswords': [remember: boolean]
}>()

const passwordModel = computed({
  get: () => props.password,
  set: (value: string) => emit('update:password', value),
})
const rememberPasswordModel = computed({
  get: () => props.rememberRoomPasswords,
  set: (value: boolean) => emit('update:rememberRoomPasswords', value),
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
      <div class="cr-empty-mark-wrap">
        <img src="/brand/echo-gate.svg" alt="" width="72" height="72" class="empty-mark" aria-hidden="true" />
      </div>
      <small>YOUR CONVERSATIONS</small>
      <h2>选择一段会话</h2>
    </div>
  </section>

  <section v-else class="fade-in cr-access-stage flex min-h-0 flex-1 items-center justify-center overflow-y-auto p-6">
    <form
      class="cr-access-form w-full max-w-[420px]"
      autocomplete="off"
      data-testid="join-form"
      @submit.prevent="handleJoin"
    >
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

      <div v-if="room.has_password" class="mt-5 flex flex-col gap-3">
        <label for="joinPassword" class="text-sm font-medium">聊天室访问密码</label>
        <ScopedPasswordField
          v-model="passwordModel"
          input-id="joinPassword"
          name="room-access-password"
          scope="room-access"
        />
        <label class="flex min-h-10 cursor-pointer items-center justify-between gap-4 text-sm">
          <span>切换会话时记住密码</span>
          <ToggleSwitch v-model="rememberPasswordModel" aria-label="切换会话时记住聊天室密码" />
        </label>
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
  background: var(--cr-chat-canvas);
}

.cr-chat-empty::before {
  position: absolute;
  inset: 14% 12%;
  border: 1px solid color-mix(in srgb, var(--cr-border) 42%, transparent);
  content: '';
  clip-path: polygon(0 0, 16% 0, 16% 1px, 0 1px, 0 100%, 1px 100%, 1px 84%, 0 84%);
}

.cr-empty-state {
  position: relative;
  display: grid;
  justify-items: center;
  color: var(--cr-text);
  text-align: center;
}

.cr-empty-state img {
  width: 4rem;
  height: 4rem;
  filter: drop-shadow(0 12px 22px rgba(23, 37, 33, 0.15));
}

.cr-empty-mark-wrap {
  display: grid;
  width: 6.5rem;
  height: 6.5rem;
  place-items: center;
  border: 1px solid color-mix(in srgb, var(--cr-border) 78%, transparent);
  border-radius: 50%;
  background: color-mix(in srgb, var(--cr-surface) 52%, transparent);
}

.cr-empty-state small {
  margin-top: 1.5rem;
  color: var(--cr-primary);
  font-size: 0.65rem;
  font-weight: 800;
  letter-spacing: 0;
}

.cr-empty-state h2 {
  margin-top: 0.45rem;
  font-size: 1.125rem;
  font-weight: 700;
  text-wrap: balance;
}

.cr-empty-state > span {
  margin-top: 0.35rem;
  color: var(--cr-text-muted);
  font-size: 0.75rem;
}

.cr-empty-state > small,
.cr-empty-state > h2,
.cr-empty-state > span {
  animation: empty-copy-in var(--cr-motion-slow) var(--cr-ease-out) both;
}

.cr-empty-state > h2 {
  animation-delay: 40ms;
}

.cr-empty-state > span {
  animation-delay: 80ms;
}

.cr-access-stage {
  background: var(--cr-chat-canvas);
}

.cr-access-form {
  padding: 1.5rem;
  border: 1px solid var(--cr-border);
  border-radius: var(--cr-radius-lg);
  background: color-mix(in srgb, var(--cr-surface) 88%, transparent);
  box-shadow: var(--cr-shadow-md);
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

@keyframes empty-copy-in {
  from {
    opacity: 0;
    transform: translateY(6px);
  }
}

@media (prefers-reduced-motion: reduce) {
  .fade-in,
  .empty-mark,
  .cr-empty-state > small,
  .cr-empty-state > h2,
  .cr-empty-state > span {
    animation: none;
  }
}

@media (max-width: 767px) {
  .cr-chat-empty::before {
    inset: 10% 7%;
  }
}
</style>
