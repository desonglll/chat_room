<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import {
  ArrowLeft,
  DoorOpen,
  EllipsisVertical,
  Eye,
  EyeOff,
  LogIn,
  LogOut,
  MessageCircle,
  Send,
  UserRound,
} from 'lucide-vue-next'
import { shouldSubmitMessage } from '../composer'
import type { ChatStatus, DisplayMessage, Room, User } from '../types'

const props = defineProps<{
  room: Room | null
  user: User | null
  password: string
  status: ChatStatus
  statusLabel: string
  authenticated: boolean
  error: string
  messages: DisplayMessage[]
  currentUserId: string
  visible: boolean
}>()

const emit = defineEmits<{
  back: []
  manage: []
  leave: []
  join: []
  authenticate: []
  send: [content: string]
  'update:password': [password: string]
}>()

const messageInput = ref<HTMLTextAreaElement | null>(null)
const messageList = ref<HTMLElement | null>(null)
const draft = ref('')
const passwordVisible = ref(false)
let composing: boolean = false
let suppressSubmitUntil = 0

const passwordModel = computed({
  get: () => props.password,
  set: (value: string) => emit('update:password', value),
})

watch(() => props.messages.length, async () => {
  await nextTick()
  if (messageList.value) messageList.value.scrollTop = messageList.value.scrollHeight
})

watch(() => props.room?.id, () => {
  draft.value = ''
  passwordVisible.value = false
})

function formatTime(value: string): string {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return ''
  return new Intl.DateTimeFormat('zh-CN', {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hour12: false,
  }).format(date)
}

function resizeComposer(): void {
  if (!messageInput.value) return
  messageInput.value.style.height = 'auto'
  messageInput.value.style.height = `${Math.min(messageInput.value.scrollHeight, 128)}px`
}

function submitMessage(): void {
  if (composing || performance.now() < suppressSubmitUntil) return
  const content = draft.value.trim()
  if (!content) return
  emit('send', content)
  draft.value = ''
  nextTick(resizeComposer)
}

function onComposerKeydown(event: KeyboardEvent): void {
  const compositionJustEnded = performance.now() < suppressSubmitUntil
  if (!shouldSubmitMessage(event, composing, compositionJustEnded)) return
  event.preventDefault()
  submitMessage()
}

function onCompositionEnd(): void {
  composing = false
  suppressSubmitUntil = performance.now() + 300
}

function onCompositionStart(): void {
  composing = true
}
</script>

<template>
  <main class="chat-panel" :class="{ 'mobile-hidden': !visible }">
    <header class="chat-header">
      <div class="chat-title-group">
        <button class="icon-button mobile-only" type="button" aria-label="返回房间列表" title="返回房间列表" @click="emit('back')">
          <ArrowLeft :size="21" />
        </button>
        <div class="min-width-zero">
          <h2>{{ room?.name || '选择聊天室' }}</h2>
          <div class="connection-line">
            <span class="status-dot" :class="status" />
            <span>{{ statusLabel }}</span>
            <span v-if="room" class="access-label">{{ room.has_password ? '私密房间' : '公开房间' }}</span>
          </div>
        </div>
      </div>
      <div v-if="room" class="toolbar">
        <button class="icon-button" type="button" aria-label="管理聊天室" title="管理聊天室" @click="emit('manage')">
          <EllipsisVertical :size="20" />
        </button>
        <button v-if="authenticated" class="icon-button danger-hover" type="button" aria-label="退出聊天室" title="退出聊天室" @click="emit('leave')">
          <LogOut :size="18" />
        </button>
      </div>
    </header>

    <section v-if="!room" class="empty-state">
      <span class="empty-icon"><MessageCircle :size="30" /></span>
      <strong>选择一个聊天室</strong>
      <p>从房间列表开始</p>
    </section>

    <section v-else-if="!authenticated" class="join-state">
      <form class="join-form" data-testid="join-form" @submit.prevent="emit('join')">
        <span class="join-icon"><DoorOpen :size="23" /></span>
        <h3>加入 {{ room.name }}</h3>
        <p>{{ room.has_password ? '验证账户与房间密码后加入' : '登录账户后加入' }}</p>

        <div v-if="user" class="join-account">
          <UserRound :size="18" />
          <span>以 <strong>{{ user.username }}</strong> 的身份加入</span>
        </div>

        <template v-if="room.has_password">
          <label for="joinPassword">房间密码</label>
          <div class="password-input">
            <input id="joinPassword" v-model="passwordModel" :type="passwordVisible ? 'text' : 'password'" autocomplete="current-password" required>
            <button type="button" :aria-label="passwordVisible ? '隐藏密码' : '显示密码'" :title="passwordVisible ? '隐藏密码' : '显示密码'" @click="passwordVisible = !passwordVisible">
              <EyeOff v-if="passwordVisible" :size="18" />
              <Eye v-else :size="18" />
            </button>
          </div>
        </template>

        <p v-if="error" class="form-error" role="alert">{{ error }}</p>
        <button v-if="user" class="primary-button wide" type="submit" :disabled="status === 'connecting'">
          <LogIn :size="18" />
          {{ status === 'connecting' ? '正在连接' : '加入聊天室' }}
        </button>
        <button v-else class="primary-button wide" type="button" @click="emit('authenticate')">
          <LogIn :size="18" />
          登录或注册
        </button>
      </form>
    </section>

    <section v-else class="conversation">
      <div ref="messageList" class="message-list" data-testid="message-list" aria-live="polite">
        <template v-for="message in messages" :key="message.type === 'broadcast' ? message.message_id : message.key">
          <div v-if="message.type === 'system'" class="system-message">
            <span />
            <p>{{ message.content }}</p>
            <span />
          </div>
          <div v-else class="message-row" :class="{ own: message.sender_id === currentUserId }">
            <div class="message-group">
              <div class="message-meta">
                <strong>{{ message.sender_id === currentUserId ? '你' : message.sender }}</strong>
                <time>{{ formatTime(message.timestamp) }}</time>
              </div>
              <p class="message-bubble">{{ message.content }}</p>
            </div>
          </div>
        </template>
      </div>
      <form class="composer" data-testid="chat-form" @submit.prevent="submitMessage">
        <label class="sr-only" for="messageInput">消息</label>
        <textarea
          id="messageInput"
          ref="messageInput"
          v-model="draft"
          rows="1"
          maxlength="4096"
          placeholder="输入消息"
          @input="resizeComposer"
          @compositionstart="onCompositionStart"
          @compositionend="onCompositionEnd"
          @keydown="onComposerKeydown"
        />
        <button class="send-button" type="submit" :disabled="!draft.trim()" aria-label="发送消息" title="发送消息">
          <Send :size="18" />
        </button>
      </form>
    </section>
  </main>
</template>
