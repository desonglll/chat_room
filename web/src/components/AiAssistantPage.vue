<script setup lang="ts">
import { computed, nextTick, ref } from 'vue'
import { ArrowLeft, Bot, CheckCheck, ListChecks, Send, Sparkles } from 'lucide-vue-next'
import Button from 'primevue/button'
import Message from 'primevue/message'
import Password from 'primevue/password'
import Select from 'primevue/select'
import Textarea from 'primevue/textarea'
import { queryConversation } from '../assistantApi'
import { parseAssistantPrompt } from '../assistantMentions'
import { readRoomPassword } from '../roomPasswordVault'
import type { AiConversationTurn, AiRuntimeStatus, Room } from '../types'

interface ThreadMessage extends AiConversationTurn {
  id: string
  roomTitle: string
  contextCount?: number
}

const props = defineProps<{
  token: string
  rooms: Room[]
  aiStatus: AiRuntimeStatus
  rememberRoomPasswords: boolean
}>()
const emit = defineEmits<{ back: []; error: [message: string] }>()

const selectedRoomId = ref('')
const roomPassword = ref('')
const prompt = ref('@AI助手 ')
const thread = ref<ThreadMessage[]>([])
const loading = ref(false)
const threadElement = ref<HTMLElement | null>(null)
const availableRooms = computed(() => props.rooms.filter((room) => room.membership_status === 'active'))
const selectedRoom = computed(() => availableRooms.value.find((room) => room.id === selectedRoomId.value) || null)
const mentionableRooms = computed(() => availableRooms.value.map((room) => ({ roomId: room.id, title: room.name })))
const aiReady = computed(() => props.aiStatus === 'ready')

function selectRoom(roomId: string): void {
  selectedRoomId.value = roomId
  thread.value = []
  const room = availableRooms.value.find((candidate) => candidate.id === roomId)
  roomPassword.value = room?.has_password ? readRoomPassword(room.id, props.rememberRoomPasswords) : ''
  prompt.value = room ? `@AI助手 @${room.name} ` : '@AI助手 '
}

function historyFor(roomId: string): AiConversationTurn[] {
  if (roomId !== selectedRoomId.value) return []
  return thread.value.slice(-12).map(({ role, content }) => ({ role, content }))
}

async function scrollToLatest(): Promise<void> {
  await nextTick()
  threadElement.value?.scrollTo({ top: threadElement.value.scrollHeight, behavior: 'smooth' })
}

async function submit(quickQuestion = ''): Promise<void> {
  if (loading.value || !aiReady.value) return
  const source = quickQuestion
    ? `@AI助手 ${selectedRoom.value ? `@${selectedRoom.value.name} ` : ''}${quickQuestion}`
    : prompt.value
  const parsed = parseAssistantPrompt(source, mentionableRooms.value, selectedRoomId.value)
  const room = availableRooms.value.find((candidate) => candidate.id === parsed.roomId)
  if (!room) {
    emit('error', '请先选择一个可访问的会话')
    return
  }
  if (!parsed.question) return
  const history = historyFor(room.id)
  if (room.id !== selectedRoomId.value) {
    selectedRoomId.value = room.id
    thread.value = []
    roomPassword.value = room.has_password ? readRoomPassword(room.id, props.rememberRoomPasswords) : ''
  }
  thread.value.push({ id: crypto.randomUUID(), role: 'user', content: parsed.question, roomTitle: room.name })
  prompt.value = `@AI助手 @${room.name} `
  loading.value = true
  await scrollToLatest()
  try {
    const result = await queryConversation(room.id, parsed.question, history, props.token, roomPassword.value)
    thread.value.push({
      id: crypto.randomUUID(),
      role: 'assistant',
      content: result.answer,
      roomTitle: room.name,
      contextCount: result.context_message_count,
    })
  } catch (caught) {
    emit('error', caught instanceof Error ? caught.message : 'AI 请求失败')
  } finally {
    loading.value = false
    await scrollToLatest()
  }
}
</script>

<template>
  <main id="workspace-main" class="cr-page flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
    <header class="cr-page-header flex shrink-0 items-center gap-3 px-4 sm:px-7">
      <Button text rounded severity="secondary" aria-label="返回聊天" title="返回聊天" @click="emit('back')">
        <ArrowLeft :size="19" />
      </Button>
      <span class="grid size-9 shrink-0 place-items-center rounded-md bg-primary-50 text-primary">
        <Bot :size="20" />
      </span>
      <div class="min-w-0 flex-1">
        <h1 class="text-base font-semibold">AI 助手</h1>
        <p class="mt-0.5 truncate text-xs text-muted-color">{{ selectedRoom?.name || '未选择会话' }}</p>
      </div>
      <span
        class="rounded-sm px-2 py-1 text-[11px] font-medium"
        :class="aiReady ? 'bg-green-50 text-green-700' : 'bg-surface-100 text-muted-color'"
        >{{ aiReady ? '可用' : '不可用' }}</span
      >
    </header>

    <Message v-if="aiStatus === 'missing_credentials'" severity="warn" :closable="false" class="m-4 sm:mx-7">
      服务端未设置 CHAT_ROOM_AI_API_KEY，配置后重启服务即可启用。
    </Message>
    <Message v-else-if="aiStatus === 'disabled'" severity="secondary" :closable="false" class="m-4 sm:mx-7">
      AI 功能当前已关闭。
    </Message>

    <section
      class="mx-auto grid min-h-0 w-full max-w-5xl flex-1 grid-rows-[auto_minmax(0,1fr)_auto] px-4 pb-4 sm:px-7 sm:pb-6"
    >
      <div class="flex flex-wrap items-center gap-2 border-b border-surface-200 py-3">
        <Select
          :model-value="selectedRoomId"
          :options="availableRooms"
          option-label="name"
          option-value="id"
          filter
          placeholder="选择会话"
          class="min-w-52 flex-1 sm:max-w-80"
          :disabled="loading"
          @update:model-value="selectRoom($event)"
        />
        <Password
          v-if="selectedRoom?.has_password"
          v-model="roomPassword"
          :feedback="false"
          toggle-mask
          autocomplete="off"
          placeholder="聊天室密码"
          input-class="w-full"
          class="min-w-44 flex-1 sm:max-w-60"
          :disabled="loading"
        />
        <Button
          text
          severity="secondary"
          size="small"
          :disabled="!selectedRoom || !aiReady || loading"
          @click="submit('总结这段对话')"
        >
          <Sparkles :size="16" /><span>总结</span>
        </Button>
        <Button
          text
          severity="secondary"
          size="small"
          :disabled="!selectedRoom || !aiReady || loading"
          @click="submit('提取对话中的待办事项')"
        >
          <ListChecks :size="16" /><span>待办</span>
        </Button>
        <Button
          text
          severity="secondary"
          size="small"
          :disabled="!selectedRoom || !aiReady || loading"
          @click="submit('梳理这段对话已经形成的结论')"
        >
          <CheckCheck :size="16" /><span>结论</span>
        </Button>
      </div>

      <div ref="threadElement" class="min-h-0 overflow-y-auto py-5" aria-live="polite">
        <div v-if="!thread.length" class="grid min-h-full place-items-center text-center text-muted-color">
          <div>
            <Bot :size="34" class="mx-auto opacity-35" />
            <p class="mt-3 text-sm">{{ selectedRoom ? '可以开始提问' : '尚未选择会话' }}</p>
          </div>
        </div>
        <ol v-else class="space-y-5">
          <li
            v-for="message in thread"
            :key="message.id"
            class="flex"
            :class="message.role === 'user' ? 'justify-end' : 'justify-start'"
          >
            <article
              class="max-w-[min(82%,42rem)] rounded-md px-3.5 py-3 text-sm leading-6"
              :class="
                message.role === 'user'
                  ? 'bg-primary text-primary-contrast'
                  : 'border border-surface-200 bg-surface-0 text-surface-900'
              "
            >
              <p class="whitespace-pre-wrap break-words">{{ message.content }}</p>
              <p class="mt-2 text-[10px] opacity-65">
                {{ message.roomTitle
                }}<template v-if="message.contextCount !== undefined">
                  · {{ message.contextCount }} 条消息 · TOON</template
                >
              </p>
            </article>
          </li>
          <li v-if="loading" class="flex justify-start">
            <div
              class="flex items-center gap-2 rounded-md border border-surface-200 bg-surface-0 px-3.5 py-3 text-sm text-muted-color"
            >
              <span
                class="size-3.5 animate-spin rounded-full border-2 border-surface-300 border-t-primary motion-reduce:animate-none"
              />
              正在分析
            </div>
          </li>
        </ol>
      </div>

      <form class="flex items-end gap-2 border-t border-surface-200 pt-3" @submit.prevent="submit()">
        <Textarea
          v-model="prompt"
          auto-resize
          rows="2"
          maxlength="4000"
          fluid
          class="max-h-32 min-h-12 flex-1"
          placeholder="@AI助手 @会话名称 输入问题"
          :disabled="loading || !aiReady"
          @keydown.meta.enter.prevent="submit()"
          @keydown.ctrl.enter.prevent="submit()"
        />
        <Button
          type="submit"
          rounded
          aria-label="发送给 AI 助手"
          title="发送"
          class="size-11! shrink-0 p-0!"
          :loading="loading"
          :disabled="!aiReady"
        >
          <Send v-if="!loading" :size="18" />
        </Button>
      </form>
    </section>
  </main>
</template>
