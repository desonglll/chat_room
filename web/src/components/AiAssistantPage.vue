<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref } from 'vue'
import { Hash, Send } from 'lucide-vue-next'
import Button from 'primevue/button'
import Message from 'primevue/message'
import Textarea from 'primevue/textarea'
import {
  createAiThread,
  deleteAiThread,
  listAiThreadMessages,
  listAiModels,
  listAiThreads,
  createAiRun,
  streamAiRunMessages,
  updateAiThread,
} from '../aiThreadApi'
import {
  activeConversationMention,
  conversationMentionCandidates,
  insertConversationMention,
  parseAssistantPrompt,
  type ConversationMentionRange,
  type MentionableConversation,
} from '../assistantMentions'
import type { AiUiMessage } from '../aiUi'
import { hasActiveAiMessage, pollAiThreadMessages } from '../aiRunPolling'
import { shouldSubmitMessage } from '../composer'
import { createRandomUuid } from '../randomUuid'
import { readRoomPassword } from '../roomPasswordVault'
import type { AiModelChoice, AiRuntimeStatus, AiThread, Room } from '../types'
import AiAssistantHeader from './AiAssistantHeader.vue'
import AiAssistantToolbar from './AiAssistantToolbar.vue'
import AiMessageList from './AiMessageList.vue'
import AiThreadSidebar from './AiThreadSidebar.vue'

const props = defineProps<{
  token: string
  rooms: Room[]
  aiStatus: AiRuntimeStatus
  rememberRoomPasswords: boolean
}>()
const emit = defineEmits<{ back: []; error: [message: string] }>()

const threads = ref<AiThread[]>([])
const activeThreadId = ref('')
const messages = ref<AiUiMessage[]>([])
const modelOptions = ref<AiModelChoice[]>([])
const selectedModelId = ref('')
const roomPassword = ref('')
const prompt = ref('')
const loading = ref(false)
const loadingThreads = ref(false)
const promptInput = ref<{ $el?: HTMLTextAreaElement } | null>(null)
const messageList = ref<{ scrollToLatest: (smooth?: boolean) => Promise<void>; scrollToLatestSoon: () => void } | null>(
  null,
)
const mentionRange = ref<ConversationMentionRange | null>(null)
const mentionIndex = ref(0)
let pollGeneration = 0
let runStream: AbortController | null = null

const availableRooms = computed(() => props.rooms.filter((room) => room.membership_status === 'active'))
const mentionableRooms = computed(() => availableRooms.value.map((room) => ({ roomId: room.id, title: room.name })))
const mentionCandidates = computed(() => conversationMentionCandidates(mentionRange.value, mentionableRooms.value))
const activeThread = computed(() => threads.value.find((thread) => thread.id === activeThreadId.value) || null)
const activeRoom = computed(() => availableRooms.value.find((room) => room.id === activeThread.value?.room_id) || null)
const aiReady = computed(() => modelOptions.value.some((option) => option.ready))

function report(caught: unknown, fallback: string): void {
  emit('error', caught instanceof Error ? caught.message : fallback)
}

function replaceThread(updated: AiThread): void {
  const index = threads.value.findIndex((thread) => thread.id === updated.id)
  if (index === -1) threads.value.unshift(updated)
  else threads.value[index] = updated
}

function syncRoomPassword(): void {
  const room = activeRoom.value
  roomPassword.value = room?.has_password ? readRoomPassword(room.id, props.rememberRoomPasswords) : ''
}

async function loadSessions(): Promise<void> {
  loadingThreads.value = true
  try {
    const [loadedThreads, loadedModels] = await Promise.all([listAiThreads(props.token), listAiModels(props.token)])
    threads.value = loadedThreads
    modelOptions.value = loadedModels
    selectedModelId.value = loadedModels.find((option) => option.ready)?.id || ''
    if (threads.value.length) await selectSession(threads.value[0].id)
  } catch (caught) {
    report(caught, '加载 AI 对话失败')
  } finally {
    loadingThreads.value = false
  }
}

async function createSession(): Promise<AiThread | null> {
  if (loading.value || loadingThreads.value) return null
  loadingThreads.value = true
  try {
    const created = await createAiThread(props.token)
    threads.value.unshift(created)
    activeThreadId.value = created.id
    messages.value = []
    roomPassword.value = ''
    return created
  } catch (caught) {
    report(caught, '新建 AI 对话失败')
    return null
  } finally {
    loadingThreads.value = false
  }
}

async function selectSession(threadId: string): Promise<void> {
  if (threadId === activeThreadId.value) return
  runStream?.abort()
  runStream = null
  const generation = ++pollGeneration
  activeThreadId.value = threadId
  loadingThreads.value = true
  try {
    messages.value = await listAiThreadMessages(props.token, threadId)
    loading.value = hasActiveAiMessage(messages.value)
    syncRoomPassword()
    await messageList.value?.scrollToLatest()
    if (loading.value) void followPersistedRun(threadId, generation)
  } catch (caught) {
    report(caught, '加载 AI 对话消息失败')
  } finally {
    loadingThreads.value = false
  }
}

async function followRunStream(threadId: string, runId: string, generation: number): Promise<void> {
  const controller = new AbortController()
  runStream?.abort()
  runStream = controller
  try {
    await streamAiRunMessages(
      props.token,
      runId,
      (next) => {
        if (generation !== pollGeneration || activeThreadId.value !== threadId) return
        const index = messages.value.findIndex((message) => message.id === next.id)
        if (index === -1) messages.value.push(next)
        else messages.value[index] = next
        messageList.value?.scrollToLatestSoon()
      },
      controller.signal,
    )
  } catch (caught) {
    if (!controller.signal.aborted && generation === pollGeneration) report(caught, '读取 AI 回答失败')
  } finally {
    if (runStream === controller) runStream = null
    if (generation !== pollGeneration) return
    messages.value = await listAiThreadMessages(props.token, threadId).catch(() => messages.value)
    loading.value = false
    threads.value = await listAiThreads(props.token).catch(() => threads.value)
    await messageList.value?.scrollToLatest()
  }
}

async function followPersistedRun(threadId: string, generation: number): Promise<void> {
  try {
    await pollAiThreadMessages(
      () => listAiThreadMessages(props.token, threadId),
      (persisted) => {
        if (generation !== pollGeneration || activeThreadId.value !== threadId) return
        messages.value = persisted
        messageList.value?.scrollToLatestSoon()
      },
      { isCurrent: () => generation === pollGeneration && activeThreadId.value === threadId },
    )
  } catch (caught) {
    if (generation === pollGeneration) report(caught, '读取 AI 回答失败')
  } finally {
    if (generation !== pollGeneration) return
    loading.value = false
    threads.value = await listAiThreads(props.token).catch(() => threads.value)
    await messageList.value?.scrollToLatest()
  }
}

async function removeSession(thread: AiThread): Promise<void> {
  if (loading.value || loadingThreads.value || !window.confirm(`删除 AI 对话“${thread.title}”？`)) return
  try {
    await deleteAiThread(props.token, thread.id)
    threads.value = threads.value.filter((candidate) => candidate.id !== thread.id)
    if (activeThreadId.value === thread.id) {
      activeThreadId.value = ''
      messages.value = []
      if (threads.value[0]) await selectSession(threads.value[0].id)
    }
  } catch (caught) {
    report(caught, '删除 AI 对话失败')
  }
}

async function ensureSession(): Promise<AiThread | null> {
  return activeThread.value || createSession()
}

async function attachRoom(roomId: string): Promise<void> {
  const thread = await ensureSession()
  if (!thread || loading.value) return
  try {
    replaceThread(await updateAiThread(props.token, thread.id, { room_id: roomId }))
    syncRoomPassword()
  } catch (caught) {
    report(caught, '关联会话失败')
  }
}

async function clearRoom(): Promise<void> {
  if (!activeThread.value || loading.value) return
  try {
    replaceThread(await updateAiThread(props.token, activeThread.value.id, { clear_room: true }))
    roomPassword.value = ''
  } catch (caught) {
    report(caught, '清除会话关联失败')
  }
}

async function setThinking(enabled: boolean): Promise<void> {
  const thread = await ensureSession()
  if (!thread || loading.value) return
  try {
    replaceThread(await updateAiThread(props.token, thread.id, { thinking_enabled: enabled }))
  } catch (caught) {
    report(caught, '更新思考模式失败')
  }
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
  void attachRoom(conversation.roomId)
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
  if (shouldSubmitMessage(event, false)) {
    event.preventDefault()
    void submit()
  }
}

async function submit(quickQuestion = ''): Promise<void> {
  if (loading.value || !aiReady.value) return
  const parsed = parseAssistantPrompt(quickQuestion || prompt.value, mentionableRooms.value, activeRoom.value?.id || '')
  if (!parsed.question) return
  const room = parsed.roomId ? availableRooms.value.find((candidate) => candidate.id === parsed.roomId) : null
  if (parsed.roomId && !room) {
    emit('error', '引用的会话已不可访问')
    return
  }
  const session = await ensureSession()
  if (!session) return
  prompt.value = ''
  mentionRange.value = null
  loading.value = true
  try {
    const run = await createAiRun(
      props.token,
      session.id,
      parsed.question,
      room?.id || null,
      room?.has_password ? roomPassword.value : '',
      createRandomUuid(),
      selectedModelId.value || null,
    )
    const generation = ++pollGeneration
    messages.value = await listAiThreadMessages(props.token, session.id)
    threads.value = await listAiThreads(props.token).catch(() => threads.value)
    await messageList.value?.scrollToLatest(true)
    void followRunStream(session.id, run.id, generation)
  } catch (caught) {
    loading.value = false
    report(caught, 'AI 请求失败')
  }
}

onMounted(loadSessions)
onUnmounted(() => {
  runStream?.abort()
  pollGeneration += 1
})
</script>

<template>
  <main id="workspace-main" class="cr-page flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
    <AiAssistantHeader :title="activeThread?.title || '新对话'" :ready="aiReady" @back="emit('back')" />

    <Message v-if="!aiReady && aiStatus === 'missing_credentials'" severity="warn" :closable="false" class="m-4 sm:mx-7">
      当前没有凭据完整的模型配置，请在系统后台检查 API key 环境变量。
    </Message>
    <Message v-else-if="!aiReady && aiStatus === 'disabled'" severity="secondary" :closable="false" class="m-4 sm:mx-7">
      AI 功能当前已关闭。
    </Message>

    <section
      class="grid min-h-0 flex-1 grid-rows-[10rem_minmax(0,1fr)] md:grid-cols-[13rem_minmax(0,1fr)] md:grid-rows-1"
    >
      <AiThreadSidebar
        :threads="threads"
        :active-id="activeThreadId"
        :busy="loading || loadingThreads"
        @create="createSession"
        @select="selectSession"
        @delete="removeSession"
      />

      <div class="grid min-h-0 min-w-0 grid-rows-[auto_minmax(0,1fr)_auto]">
        <AiAssistantToolbar
          v-model:password="roomPassword"
          :models="modelOptions"
          :model-id="selectedModelId"
          :room="activeRoom"
          :thinking-enabled="activeThread?.thinking_enabled || false"
          :ai-ready="aiReady"
          :loading="loading"
          @clear-room="clearRoom"
          @thinking="setThinking"
          @model="selectedModelId = $event"
          @quick="submit"
        />

        <AiMessageList ref="messageList" :messages="messages" :room-title="activeRoom?.name || ''" />

        <form
          id="ai-assistant-query-form"
          class="mx-auto flex w-full max-w-4xl items-end gap-2 border-t border-surface-200 px-4 py-3 sm:px-7"
          @submit.prevent="submit()"
        >
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
                    <Hash :size="15" class="shrink-0" /><span class="truncate">{{ room.title }}</span>
                  </button>
                </li>
              </ul>
              <p v-else class="px-3 py-5 text-center text-sm text-muted-color">没有匹配的会话</p>
            </div>
            <Textarea
              ref="promptInput"
              v-model="prompt"
              auto-resize
              rows="1"
              maxlength="4000"
              fluid
              class="max-h-28 min-h-11 align-top"
              placeholder="发送消息，输入 @ 引用聊天会话"
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
            :disabled="!aiReady || !prompt.trim()"
          >
            <Send v-if="!loading" :size="18" />
          </Button>
        </form>
      </div>
    </section>
  </main>
</template>
