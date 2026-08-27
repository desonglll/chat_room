<script setup lang="ts">
import { computed, defineAsyncComponent, ref, watch } from 'vue'
import { UploadCloud } from 'lucide-vue-next'
import { useRouter } from 'vue-router'
import ChatAccessPanel from './ChatAccessPanel.vue'
import ChatPanelDialogs from './ChatPanelDialogs.vue'
import ChatRoomHeader from './ChatRoomHeader.vue'
import MessageComposer from './MessageComposer.vue'
import MessageList from './MessageList.vue'
import MessageSelectionBar from './MessageSelectionBar.vue'
import RoomConnectingView from './RoomConnectingView.vue'
import UploadStatusPanel from './UploadStatusPanel.vue'
import { useChatPanelInteractions } from '../composables/useChatPanelInteractions'
import { useChatPanelMessageActions } from '../composables/useChatPanelMessageActions'
import { useRoomMessageNavigation } from '../composables/useRoomMessageNavigation'
import { useMessageSelection } from '../composables/useMessageSelection'
import { resolveRoomViewState } from '../roomViewState'
import type { ChatPanelEmits, ChatPanelProps } from '../chatPanelContract'
import type { BroadcastMessage } from '../types'
const RoomPinsBar = defineAsyncComponent(() => import('./RoomPinsBar.vue'))
const props = defineProps<ChatPanelProps>()
const emit = defineEmits<ChatPanelEmits>()
const filesOpen = ref(false)
const extractionOpen = ref(false)
const taskPanel = ref<true | BroadcastMessage | null>(null)
const viewProfileUserId = ref('')
const composerRef = ref<InstanceType<typeof MessageComposer> | null>(null)
const messageListRef = ref<InstanceType<typeof MessageList> | null>(null)
const roomPinsRef = ref<{ toggle: (messageId: string) => Promise<void> } | null>(null)
const pinnedMessageIds = ref<string[]>([])
const viewState = computed(() =>
  resolveRoomViewState({
    room: props.room,
    status: props.status,
    authenticated: props.authenticated,
    loading: props.loading,
    messageCount: props.messages.length,
  }),
)
const router = useRouter()
const canPin = computed(
  () => props.conversation?.kind === 'direct' || ['owner', 'admin'].includes(props.room?.membership_role || ''),
)
const { searchOpen, locateMessage, locateSearchResult } = useRoomMessageNavigation({
  roomId: () => props.room?.id || '',
  ready: () => viewState.value === 'conversation' && props.historyReady,
  visible: () => props.visible,
  authenticated: () => props.authenticated,
  messageList: () => messageListRef.value,
  closeFiles: () => (filesOpen.value = false),
})
const {
  closeSelection,
  downloadSelected,
  favoriteSelected,
  forwardSelected,
  selectedAttachments,
  selectedMessageIds,
  selecting,
  toggleSelection,
} = useMessageSelection(() => props.messages, {
  download: (attachments) => emit('download', attachments),
  favorite: (messageIds) => emit('favorite', messageIds),
  forward: (messageIds) => emit('forward', messageIds),
})
const {
  editMessage,
  editingTo,
  recallMessage,
  replyingTo,
  resetTargets,
  sendMessage,
  startEdit,
  startReply,
  uploadFiles,
} = useChatPanelMessageActions(router, {
  edit: (messageId, content) => emit('edit', messageId, content),
  recall: (messageId) => emit('recall', messageId),
  send: (content, replyTo) => emit('send', content, replyTo),
  upload: (files, content, replyTo, isSensitive) => emit('upload', files, content, replyTo, isSensitive),
})
const { dragActive, galleryImages, handleDragEnter, handleDragLeave, handleDrop, previewImageId, shaking } =
  useChatPanelInteractions({
    messages: () => props.messages,
    pokedAt: () => props.pokedAt,
    visible: () => props.visible,
    authenticated: () => props.authenticated,
    focusShortcut: () => props.focusShortcut,
    focusComposer: () => composerRef.value?.focus(),
    addFiles: (files) => composerRef.value?.addFiles(files),
  })

watch(
  () => props.room?.id,
  () => {
    resetTargets()
    filesOpen.value = false
    extractionOpen.value = false
    taskPanel.value = null
    pinnedMessageIds.value = []
    previewImageId.value = ''
    selecting.value = false
    selectedMessageIds.value = []
  },
)
</script>

<template>
  <main
    id="workspace-main"
    class="cr-chat-panel cr-chat-canvas absolute inset-0 flex min-h-0 min-w-0 flex-col transition-[transform,opacity,visibility] duration-[var(--cr-motion-enter)] [transition-timing-function:var(--cr-ease-drawer)] motion-reduce:transition-none md:relative md:inset-auto md:visible md:translate-x-0 md:opacity-100"
    :class="[
      visible
        ? 'visible translate-x-0 opacity-100'
        : 'invisible pointer-events-none translate-x-4 opacity-0 md:pointer-events-auto',
      { 'animate-poke-shake': shaking },
    ]"
  >
    <ChatRoomHeader
      v-if="room"
      :room="room"
      :alias="conversation?.alias || ''"
      :original-title="conversation?.title || room.name"
      :kind="conversation?.kind || 'group'"
      :peer="conversation?.peer || null"
      :status="status"
      :status-label="statusLabel"
      :authenticated="authenticated"
      :members="members"
      :current-user-id="currentUserId"
      :token="token"
      :ai-enabled="aiEnabled"
      :ai-panel-open="aiPanelOpen"
      @back="emit('back')"
      @manage="emit('manage')"
      @leave="emit('leave')"
      @files="filesOpen = true"
      @search="searchOpen = true"
      @view-profile="viewProfileUserId = $event"
      @toggle-selection="selecting = !selecting"
      @remove-friend="emit('removeFriend')"
      @block-user="emit('blockUser')"
      @assistant="emit('assistant')"
      @catch-up="emit('catchUp')"
      @extraction="extractionOpen = true"
      @tasks="taskPanel = true"
    />
    <ChatAccessPanel
      v-if="viewState === 'loading' || viewState === 'empty' || viewState === 'access'"
      :room="room"
      :user="user"
      :password="password"
      :remember-room-passwords="rememberRoomPasswords"
      :status="status"
      :error="error"
      :loading="loading"
      @join="emit('join')"
      @request-join="emit('requestJoin')"
      @authenticate="emit('authenticate')"
      @update:password="emit('update:password', $event)"
      @update:remember-room-passwords="emit('update:rememberRoomPasswords', $event)"
    />
    <RoomConnectingView v-else-if="viewState === 'connecting'" />
    <section
      v-else
      class="cr-conversation-stage relative flex min-h-0 flex-1 flex-col"
      @dragenter.prevent="handleDragEnter"
      @dragover.prevent
      @dragleave="handleDragLeave"
      @drop.prevent="handleDrop"
    >
      <Transition
        enter-active-class="transition-[opacity] duration-[var(--cr-motion-normal)] [transition-timing-function:var(--cr-ease-out)] motion-reduce:transition-none"
        enter-from-class="opacity-0"
        leave-active-class="transition-[opacity] duration-[var(--cr-motion-fast)] [transition-timing-function:var(--cr-ease-out)] motion-reduce:transition-none"
        leave-to-class="opacity-0"
      >
        <div
          v-if="dragActive"
          class="pointer-events-none absolute inset-3 z-30 grid place-items-center rounded-lg border-2 border-dashed border-primary bg-primary-50/95 text-primary shadow-lg"
        >
          <div class="text-center">
            <UploadCloud class="mx-auto" :size="34" />
            <strong class="mt-2 block text-sm">释放以添加附件</strong>
          </div>
        </div>
      </Transition>
      <RoomPinsBar
        v-if="conversation"
        ref="roomPinsRef"
        :room-id="room?.id || ''"
        :token="token"
        :can-pin="canPin"
        @update:message-ids="pinnedMessageIds = $event"
        @locate="locateMessage"
        @edit-favorite="router.push({ name: 'favorites', query: { edit: $event } })"
      />
      <MessageList
        ref="messageListRef"
        :room-id="room?.id || ''"
        :direct="conversation?.kind === 'direct'"
        :unread-count="room?.unread_count || 0"
        :messages="messages"
        :favorite-message-ids="favoriteMessageIds"
        :pinned-message-ids="pinnedMessageIds"
        :can-pin="canPin"
        :read-receipts="readReceipts"
        :participants="participants"
        :current-user-id="currentUserId"
        :visible="visible"
        :history-ready="historyReady"
        :selecting="selecting"
        :selected-message-ids="selectedMessageIds"
        :loading-older="loadingOlder"
        :has-more-history="hasMoreHistory"
        :ensure-message="ensureMessage"
        @read="emit('read', $event)"
        @reply="startReply"
        @recall="recallMessage"
        @edit="startEdit"
        @forward="(message) => emit('forward', [message.message_id])"
        @favorite="(message) => emit('favorite', [message.message_id])"
        @pin="(message) => roomPinsRef?.toggle(message.message_id)"
        @toggle-select="toggleSelection"
        @load-older="emit('loadOlder')"
        @preview-image="previewImageId = $event.id"
        @view-profile="viewProfileUserId = $event"
        @poke="emit('poke', $event)"
        @retry="emit('retry', $event)"
        @reaction="(messageId, emoji, active) => emit('reaction', messageId, emoji, active)"
        @cancel-upload="emit('cancelUploadTask', $event)"
        @retry-upload="emit('retryUploadTask', $event)"
        @task="taskPanel = $event"
      />
      <TransitionGroup
        v-if="typingDrafts.length && !selecting"
        tag="div"
        class="cr-typing-strip space-y-1 px-4 py-2 sm:px-6"
        enter-active-class="transition-[opacity,transform] duration-[var(--cr-motion-normal)] [transition-timing-function:var(--cr-ease-out)] motion-reduce:transition-none"
        enter-from-class="translate-y-1 opacity-0"
        leave-active-class="transition-[opacity,transform] duration-[var(--cr-motion-fast)] [transition-timing-function:var(--cr-ease-out)] motion-reduce:transition-none"
        leave-to-class="opacity-0"
      >
        <div v-for="draft in typingDrafts" :key="draft.user_id" class="flex min-w-0 items-center gap-2 text-xs">
          <strong class="shrink-0 text-primary">{{ draft.username }} 正在输入</strong>
          <span class="truncate text-muted-color">{{ draft.content }}</span>
        </div>
      </TransitionGroup>
      <MessageSelectionBar
        v-if="selecting"
        :selected-count="selectedMessageIds.length"
        :attachment-count="selectedAttachments.length"
        :downloading="downloading"
        :download-progress="downloadProgress"
        @close="closeSelection"
        @forward="forwardSelected"
        @favorite="favoriteSelected"
        @download="downloadSelected"
        @cancel-download="emit('cancelDownload')"
      />
      <template v-else>
        <UploadStatusPanel
          :pending="pendingUploads"
          @resume="(session, file) => emit('resumeUpload', session, file)"
          @cancel="emit('cancelUpload', $event)"
        />
        <MessageComposer
          ref="composerRef"
          :key="room?.id || ''"
          v-model:replying-to="replyingTo"
          :editing-to="editingTo"
          :send-shortcut="sendShortcut"
          :max-upload-bytes="maxUploadBytes"
          :participants="participants"
          :draft-context="{ userId: currentUserId, ready: historyReady }"
          :room-id="room?.id || ''"
          :messages="messages"
          :token="token"
          :password="password"
          :ai-enabled="aiEnabled"
          :disabled="!authenticated"
          @cancel-edit="editingTo = null"
          @send="sendMessage"
          @edit="editMessage"
          @typing="emit('typing', $event)"
          @upload="uploadFiles"
        />
      </template>
    </section>
    <ChatPanelDialogs
      :files-open="filesOpen"
      :search-open="searchOpen"
      :extraction-open="extractionOpen"
      :tasks-open="Boolean(taskPanel)"
      :task-source="taskPanel === true ? null : taskPanel"
      :participants="participants"
      :room-id="room?.id || ''"
      :token="token"
      :password="password"
      :downloading="downloading"
      :download-progress="downloadProgress"
      :images="galleryImages"
      :preview-image-id="previewImageId"
      :profile-user-id="viewProfileUserId"
      :profile-room-id="conversation?.kind === 'direct' ? undefined : room?.id"
      :current-user-id="currentUserId"
      :contact="contact"
      :set-friend-remark="setFriendRemark"
      @close-files="filesOpen = false"
      @close-search="searchOpen = false"
      @close-extraction="extractionOpen = false"
      @close-tasks="taskPanel = null"
      @close-image="previewImageId = ''"
      @close-profile="viewProfileUserId = ''"
      @download="emit('download', $event)"
      @cancel-download="emit('cancelDownload')"
      @locate-message="((taskPanel = null), locateMessage($event))"
      @locate-search="locateSearchResult"
      @locate-extraction="((extractionOpen = false), locateMessage($event))"
      @remove-friend="emit('removeFriend')"
      @block-user="emit('blockUser')"
    />
  </main>
</template>
