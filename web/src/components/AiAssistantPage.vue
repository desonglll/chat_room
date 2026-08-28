<script setup lang="ts">
import { computed, onMounted, onUnmounted, watch } from 'vue'
import Message from 'primevue/message'
import {
  createCatchUpRun,
  createAiThread,
  deleteAiThread,
  listAiThreadMessages,
  listAiModels,
  listAiThreads,
  createAiRun,
  streamAiRunMessages,
  updateAiThread,
} from '../aiThreadApi'
import { parseAssistantPrompt } from '../assistantMentions'
import type { AiSelectedMessage } from '../aiSelectedContext'
import { hasActiveAiMessage, pollAiThreadMessages } from '../aiRunPolling'
import { useAiSourceDetails } from '../composables/useAiSourceDetails'
import { useAiAssistantState } from '../composables/useAiAssistantState'
import { useAiPromptMentions } from '../composables/useAiPromptMentions'
import { createRandomUuid } from '../randomUuid'
import { readRoomPassword } from '../roomPasswordVault'
import type { AiRuntimeStatus, AiThread, FavoriteItem, Room } from '../types'
import AiAssistantHeader from './AiAssistantHeader.vue'
import AiAssistantToolbar from './AiAssistantToolbar.vue'
import AiConversationMentionMenu from './AiConversationMentionMenu.vue'
import AiMessageList from './AiMessageList.vue'
import AiSelectedContextBar from './AiSelectedContextBar.vue'
import AiSourceDetailsPage from './AiSourceDetailsPage.vue'
import AiThreadSidebar from './AiThreadSidebar.vue'
import ComposerInput from './ComposerInput.vue'

const props = defineProps<{
  token: string
  rooms: Room[]
  aiStatus: AiRuntimeStatus
  rememberRoomPasswords: boolean
  embedded?: boolean
  initialRoomId?: string
  catchUpRequest?: number
  selectedMessages?: AiSelectedMessage[]
  saveFavorite: (title: string, content: string) => Promise<FavoriteItem>
}>()
const emit = defineEmits<{
  back: []
  error: [message: string]
  success: [message: string]
  catchUpFinished: []
  clearSelectedMessages: []
}>()

// prettier-ignore
const { activeThreadId, loading, loadingThreads, mentionIndex, mentionRange, messageList, messages, modelOptions, prompt, promptInput, roomPassword, selectedModelId, threads } = useAiAssistantState()
let pollGeneration = 0
let runStream: AbortController | null = null
let handledCatchUpRequest = 0

const availableRooms = computed(() => props.rooms.filter((room) => room.membership_status === 'active'))
const mentionableRooms = computed(() => availableRooms.value.map((room) => ({ roomId: room.id, title: room.name })))
const {
  candidates: mentionCandidates,
  choose: chooseConversation,
  handleInput: handlePromptInput,
  handleKeydown: handlePromptKeydown,
} = useAiPromptMentions({
  prompt,
  input: promptInput,
  range: mentionRange,
  activeIndex: mentionIndex,
  conversations: mentionableRooms,
  attachConversation: attachRoom,
  submit: () => submit(),
})
const activeThread = computed(() => threads.value.find((thread) => thread.id === activeThreadId.value) || null)
const activeRoom = computed(() => availableRooms.value.find((room) => room.id === activeThread.value?.room_id) || null)
const aiReady = computed(() => modelOptions.value.some((option) => option.ready))
const { closeSourceDetails, leaveSourceDetailsForThread, openSourceDetails, requestedThreadId, sourceMessage } =
  useAiSourceDetails(messages, activeThread)

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
    let initialThread = props.embedded
      ? threads.value.find((thread) => thread.room_id === props.initialRoomId)
      : threads.value.find((thread) => thread.id === requestedThreadId()) || threads.value[0]
    if (!initialThread && props.embedded && props.initialRoomId) {
      initialThread = await createAiThread(props.token, { room_id: props.initialRoomId })
      threads.value.unshift(initialThread)
    }
    if (initialThread) await selectSession(initialThread.id)
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
    const created = await createAiThread(
      props.token,
      props.embedded && props.initialRoomId ? { room_id: props.initialRoomId } : {},
    )
    await closeSourceDetails(true)
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
  if (threadId === activeThreadId.value) {
    await closeSourceDetails()
    return
  }
  await leaveSourceDetailsForThread(threadId)
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
    messageList.value?.scrollToLatestSoon()
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
    messageList.value?.scrollToLatestSoon()
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
      (props.selectedMessages || []).map((message) => message.messageId),
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

async function handleCatchUpRequest(): Promise<void> {
  const request = props.catchUpRequest || 0
  if (!props.embedded || !props.initialRoomId || !request || request === handledCatchUpRequest) return
  if (loadingThreads.value) return
  handledCatchUpRequest = request
  let accepted = false
  try {
    if (!aiReady.value) throw new Error('AI 助手当前不可用')
    if (loading.value) throw new Error('请等待当前 AI 回答完成后再总结未读')
    const session = await ensureSession()
    if (!session) return
    const room = availableRooms.value.find((candidate) => candidate.id === props.initialRoomId)
    if (!room) throw new Error('当前会话已不可访问')
    loading.value = true
    const run = await createCatchUpRun(
      props.token,
      session.id,
      room.id,
      room.has_password ? roomPassword.value : '',
      createRandomUuid(),
      selectedModelId.value || null,
    )
    if (!run) {
      emit('success', '当前没有需要总结的未读消息')
      return
    }
    accepted = true
    const generation = ++pollGeneration
    messages.value = await listAiThreadMessages(props.token, session.id)
    threads.value = await listAiThreads(props.token).catch(() => threads.value)
    await messageList.value?.scrollToLatest(true)
    void followRunStream(session.id, run.id, generation)
  } catch (caught) {
    report(caught, '总结未读失败')
  } finally {
    if (!accepted) loading.value = false
    emit('catchUpFinished')
  }
}
watch(
  () => props.catchUpRequest,
  () => void handleCatchUpRequest(),
  { flush: 'post' },
)
onMounted(async () => {
  await loadSessions()
  await handleCatchUpRequest()
})
onUnmounted(() => {
  runStream?.abort()
  pollGeneration += 1
})
</script>
<template>
  <main
    :id="embedded ? 'room-ai-panel' : 'workspace-main'"
    class="cr-page grid h-full min-h-0 min-w-0 flex-1 grid-rows-[auto_minmax(0,1fr)] overflow-hidden"
    :class="
      embedded
        ? 'absolute inset-0 z-40 border-l border-surface-200 bg-surface-0 md:relative md:inset-auto md:z-auto'
        : ''
    "
  >
    <AiAssistantHeader
      :title="activeRoom?.name || activeThread?.title || '新对话'"
      :ready="aiReady"
      :embedded="embedded"
      @back="emit('back')"
    />
    <div class="flex min-h-0 flex-col overflow-hidden">
      <Message
        v-if="!aiReady && aiStatus === 'missing_credentials'"
        severity="warn"
        :closable="false"
        class="m-4 sm:mx-7"
      >
        当前没有凭据完整的模型配置，请在系统后台检查 API key 环境变量。
      </Message>
      <Message
        v-else-if="!aiReady && aiStatus === 'disabled'"
        severity="secondary"
        :closable="false"
        class="m-4 sm:mx-7"
      >
        AI 功能当前已关闭。
      </Message>
      <section
        class="grid min-h-0 flex-1 grid-rows-[auto_minmax(0,1fr)] overflow-hidden"
        :class="embedded ? '' : 'md:grid-cols-[13rem_minmax(0,1fr)] md:grid-rows-1'"
      >
        <AiThreadSidebar
          v-if="!embedded"
          :threads="threads"
          :active-id="activeThreadId"
          :busy="loading || loadingThreads"
          @create="createSession"
          @select="selectSession"
          @delete="removeSession"
        />
        <AiSourceDetailsPage
          v-if="sourceMessage"
          :message="sourceMessage"
          :room-title="activeRoom?.name || ''"
          @back="closeSourceDetails"
        />
        <div v-else class="flex min-h-0 min-w-0 flex-col overflow-hidden">
          <AiAssistantToolbar
            v-model:password="roomPassword"
            :models="modelOptions"
            :model-id="selectedModelId"
            :room="activeRoom"
            :thinking-enabled="activeThread?.thinking_enabled || false"
            :ai-ready="aiReady"
            :loading="loading"
            :locked-room="embedded"
            :compact="embedded"
            @clear-room="clearRoom"
            @thinking="setThinking"
            @model="selectedModelId = $event"
            @quick="submit"
          />
          <AiSelectedContextBar
            v-if="selectedMessages?.length"
            :messages="selectedMessages"
            @clear="emit('clearSelectedMessages')"
          />
          <AiMessageList
            ref="messageList"
            class="min-h-0 flex-1 overflow-hidden"
            :messages="messages"
            :room-title="activeRoom?.name || ''"
            :save-favorite="saveFavorite"
            @sources="openSourceDetails"
          />
          <ComposerInput
            ref="promptInput"
            v-model="prompt"
            form-id="ai-assistant-query-form"
            :disabled="loading || !aiReady"
            :can-send="aiReady && Boolean(prompt.trim())"
            :loading="loading"
            :aria-expanded="Boolean(mentionRange)"
            :max-length="4000"
            aria-label="向 AI 助手提问"
            placeholder="发送消息，输入 @ 引用聊天会话"
            @caret="handlePromptInput"
            @keydown="handlePromptKeydown"
            @submit="submit()"
          >
            <template #popover>
              <AiConversationMentionMenu
                :range="mentionRange"
                :candidates="mentionCandidates"
                :active-index="mentionIndex"
                @choose="chooseConversation"
              />
            </template>
          </ComposerInput>
        </div>
      </section>
    </div>
  </main>
</template>
