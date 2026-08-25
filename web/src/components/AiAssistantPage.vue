<script setup lang="ts">
import { computed, nextTick, ref } from 'vue'
import { ArrowLeft, Bot, CheckCheck, Hash, ListChecks, Send, Sparkles, X } from 'lucide-vue-next'
import Button from 'primevue/button'
import Message from 'primevue/message'
import Password from 'primevue/password'
import Textarea from 'primevue/textarea'
import { streamConversation } from '../assistantApi'
import {
  activeConversationMention,
  conversationMentionCandidates,
  insertConversationMention,
  parseAssistantPrompt,
  type ConversationMentionRange,
  type MentionableConversation,
} from '../assistantMentions'
import { readRoomPassword } from '../roomPasswordVault'
import type { AiConversationTurn, AiRuntimeStatus, Room } from '../types'

interface ThreadMessage extends AiConversationTurn {
  id: string
  roomTitle: string
  contextCount?: number
  streaming?: boolean
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
const prompt = ref('')
const thread = ref<ThreadMessage[]>([])
const loading = ref(false)
const threadElement = ref<HTMLElement | null>(null)
const promptInput = ref<{ $el?: HTMLTextAreaElement } | null>(null)
const mentionRange = ref<ConversationMentionRange | null>(null)
const mentionIndex = ref(0)
let pendingScrollFrame: number | null = null
const availableRooms = computed(() => props.rooms.filter((room) => room.membership_status === 'active'))
const selectedRoom = computed(() => availableRooms.value.find((room) => room.id === selectedRoomId.value) || null)
const mentionableRooms = computed(() => availableRooms.value.map((room) => ({ roomId: room.id, title: room.name })))
const mentionCandidates = computed(() => conversationMentionCandidates(mentionRange.value, mentionableRooms.value))
const aiReady = computed(() => props.aiStatus === 'ready')

function selectRoom(roomId: string): void {
  if (roomId === selectedRoomId.value) return
  selectedRoomId.value = roomId
  thread.value = []
  const room = availableRooms.value.find((candidate) => candidate.id === roomId)
  roomPassword.value = room?.has_password ? readRoomPassword(room.id, props.rememberRoomPasswords) : ''
}

function clearRoom(): void {
  selectedRoomId.value = ''
  roomPassword.value = ''
  thread.value = []
}

function updateMention(value: string, caret: number): void {
  mentionRange.value = activeConversationMention(value, caret, mentionableRooms.value)
  mentionIndex.value = Math.min(mentionIndex.value, Math.max(0, mentionCandidates.value.length - 1))
}

function handlePromptInput(event: Event): void {
  const textarea = event.target as HTMLTextAreaElement
  prompt.value = textarea.value
  updateMention(textarea.value, textarea.selectionStart)
}

function chooseConversation(conversation: MentionableConversation): void {
  if (!mentionRange.value) return
  const inserted = insertConversationMention(prompt.value, mentionRange.value, conversation)
  prompt.value = inserted.value
  mentionRange.value = null
  mentionIndex.value = 0
  selectRoom(conversation.roomId)
  void nextTick(() => {
    const textarea = promptInput.value?.$el
    textarea?.focus()
    textarea?.setSelectionRange(inserted.caret, inserted.caret)
  })
}

function handlePromptKeydown(event: KeyboardEvent): void {
  if (mentionRange.value && mentionCandidates.value.length) {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault()
      const offset = event.key === 'ArrowDown' ? 1 : -1
      mentionIndex.value =
        (mentionIndex.value + offset + mentionCandidates.value.length) % mentionCandidates.value.length
      return
    }
    if (event.key === 'Enter' || event.key === 'Tab') {
      event.preventDefault()
      chooseConversation(mentionCandidates.value[mentionIndex.value])
      return
    }
  }
  if (event.key === 'Escape' && mentionRange.value) {
    event.preventDefault()
    mentionRange.value = null
    return
  }
  if (event.key === 'Enter' && (event.metaKey || event.ctrlKey)) {
    event.preventDefault()
    void submit()
  }
}

function historyFor(roomId: string): AiConversationTurn[] {
  if (roomId !== selectedRoomId.value) return []
  return thread.value.slice(-12).map(({ role, content }) => ({ role, content }))
}

async function scrollToLatest(): Promise<void> {
  await nextTick()
  threadElement.value?.scrollTo({ top: threadElement.value.scrollHeight, behavior: 'smooth' })
}

function scrollToLatestSoon(): void {
  if (pendingScrollFrame !== null) return
  pendingScrollFrame = requestAnimationFrame(() => {
    pendingScrollFrame = null
    threadElement.value?.scrollTo({ top: threadElement.value.scrollHeight })
  })
}

async function submit(quickQuestion = ''): Promise<void> {
  if (loading.value || !aiReady.value) return
  const source = quickQuestion ? quickQuestion : prompt.value
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
  const assistantMessage: ThreadMessage = {
    id: crypto.randomUUID(),
    role: 'assistant',
    content: '',
    roomTitle: room.name,
    streaming: true,
  }
  thread.value.push(assistantMessage)
  prompt.value = ''
  mentionRange.value = null
  loading.value = true
  await scrollToLatest()
  try {
    const result = await streamConversation(
      room.id,
      parsed.question,
      history,
      props.token,
      roomPassword.value,
      (content) => {
        assistantMessage.content += content
        scrollToLatestSoon()
      },
    )
    assistantMessage.contextCount = result.context_message_count
  } catch (caught) {
    if (!assistantMessage.content) {
      thread.value = thread.value.filter((message) => message.id !== assistantMessage.id)
    }
    emit('error', caught instanceof Error ? caught.message : 'AI 请求失败')
  } finally {
    assistantMessage.streaming = false
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
        <div
          v-if="selectedRoom"
          class="mr-auto flex min-h-9 items-center gap-2 rounded-md bg-surface-100 px-2.5 text-sm"
        >
          <Hash :size="15" class="text-primary" />
          <span class="max-w-48 truncate">{{ selectedRoom.name }}</span>
          <Button
            text
            rounded
            severity="secondary"
            aria-label="清除当前会话"
            title="清除当前会话"
            class="size-7! p-0!"
            :disabled="loading"
            @click="clearRoom"
          >
            <X :size="14" />
          </Button>
        </div>
        <p v-else class="mr-auto text-xs text-muted-color">在输入框输入 @ 选择会话</p>
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
            <p class="mt-3 text-sm">{{ selectedRoom ? '可以开始提问' : '输入 @ 选择需要分析的会话' }}</p>
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
              <p v-if="message.content" class="whitespace-pre-wrap break-words">{{ message.content }}</p>
              <div v-else-if="message.streaming" class="flex min-h-6 items-center gap-2 text-muted-color">
                <span
                  class="size-3.5 animate-spin rounded-full border-2 border-surface-300 border-t-primary motion-reduce:animate-none"
                />
                正在分析
              </div>
              <p class="mt-2 text-[10px] opacity-65">
                {{ message.roomTitle
                }}<template v-if="message.contextCount !== undefined">
                  · {{ message.contextCount }} 条消息 · TOON</template
                >
              </p>
            </article>
          </li>
        </ol>
      </div>

      <form class="flex items-end gap-2 border-t border-surface-200 pt-3" @submit.prevent="submit()">
        <div class="relative min-w-0 flex-1">
          <div
            v-if="mentionRange"
            class="absolute bottom-[calc(100%+0.5rem)] left-0 z-20 w-[min(24rem,100%)] overflow-hidden rounded-md border border-surface-200 bg-surface-0 shadow-lg"
          >
            <p class="border-b border-surface-200 px-3 py-2 text-xs font-medium text-muted-color">选择会话</p>
            <ul v-if="mentionCandidates.length" role="listbox" class="max-h-64 overflow-y-auto p-1">
              <li v-for="(room, index) in mentionCandidates" :key="room.roomId" role="option">
                <button
                  type="button"
                  class="flex min-h-10 w-full items-center gap-2 rounded-sm px-2 text-left text-sm"
                  :class="index === mentionIndex ? 'bg-primary-50 text-primary' : 'hover:bg-surface-100'"
                  :aria-selected="index === mentionIndex"
                  @mousedown.prevent="chooseConversation(room)"
                >
                  <Hash :size="15" class="shrink-0" />
                  <span class="truncate">{{ room.title }}</span>
                </button>
              </li>
            </ul>
            <p v-else class="px-3 py-5 text-center text-sm text-muted-color">没有匹配的会话</p>
          </div>
          <Textarea
            ref="promptInput"
            v-model="prompt"
            auto-resize
            rows="2"
            maxlength="4000"
            fluid
            class="max-h-32 min-h-12"
            placeholder="输入 @ 选择会话，然后提出问题"
            :disabled="loading || !aiReady"
            aria-label="向 AI 助手提问"
            aria-autocomplete="list"
            :aria-expanded="Boolean(mentionRange)"
            @input="handlePromptInput"
            @click="handlePromptInput"
            @keydown="handlePromptKeydown"
          />
        </div>
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
