<script setup lang="ts">
import { defineAsyncComponent } from 'vue'
import type { DownloadProgress } from '../attachmentDownloads'
import type { Attachment, SocialUser } from '../types'

const ChatFilesDialog = defineAsyncComponent(() => import('./ChatFilesDialog.vue'))
const ImageViewerGallery = defineAsyncComponent(() => import('./ImageViewerGallery.vue'))
const ProfileCardDialog = defineAsyncComponent(() => import('./ProfileCardDialog.vue'))
const RoomMessageSearchDialog = defineAsyncComponent(() => import('./RoomMessageSearchDialog.vue'))

defineProps<{
  filesOpen: boolean
  searchOpen: boolean
  roomId: string
  token: string
  password: string
  downloading: boolean
  downloadProgress: DownloadProgress | null
  images: Attachment[]
  previewImageId: string
  profileUserId: string
  profileRoomId?: string
  currentUserId: string
  contact: SocialUser | null
  setFriendRemark: (userId: string, remark: string) => Promise<void>
}>()

const emit = defineEmits<{
  closeFiles: []
  closeImage: []
  closeProfile: []
  closeSearch: []
  download: [attachments: Attachment[]]
  cancelDownload: []
  locateMessage: [messageId: string]
  locateSearch: [messageId: string]
  removeFriend: []
  blockUser: []
}>()
</script>

<template>
  <ChatFilesDialog
    :open="filesOpen"
    :room-id="roomId"
    :token="token"
    :password="password"
    :downloading="downloading"
    :download-progress="downloadProgress"
    @close="emit('closeFiles')"
    @download="emit('download', $event)"
    @cancel-download="emit('cancelDownload')"
    @locate-message="emit('locateMessage', $event)"
  />
  <RoomMessageSearchDialog
    :open="searchOpen"
    :room-id="roomId"
    :token="token"
    :password="password"
    @close="emit('closeSearch')"
    @locate="emit('locateSearch', $event)"
  />
  <ImageViewerGallery :images="images" :active-id="previewImageId" @close="emit('closeImage')" />
  <ProfileCardDialog
    :open="Boolean(profileUserId)"
    :user-id="profileUserId"
    :token="token"
    :room-id="profileRoomId"
    :current-user-id="currentUserId"
    :contact="contact"
    :set-remark="setFriendRemark"
    @close="emit('closeProfile')"
    @remove-friend="emit('removeFriend')"
    @block-user="emit('blockUser')"
  />
</template>
