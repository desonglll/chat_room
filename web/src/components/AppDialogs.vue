<script setup lang="ts">
import { defineAsyncComponent } from 'vue'
import Toast from 'primevue/toast'
import type {
  AuthSession,
  ChatPreferences,
  ConversationSummary,
  Room,
  RoomUpdateResult,
  SocialUser,
  User,
} from '../types'

const AuthDialog = defineAsyncComponent(() => import('./AuthDialog.vue'))
const CreateRoomDialog = defineAsyncComponent(() => import('./CreateRoomDialog.vue'))
const ForwardDialog = defineAsyncComponent(() => import('./ForwardDialog.vue'))
const JoinRoomDialog = defineAsyncComponent(() => import('./JoinRoomDialog.vue'))
const ManageRoomDialog = defineAsyncComponent(() => import('./ManageRoomDialog.vue'))
const NewConversationDialog = defineAsyncComponent(() => import('./NewConversationDialog.vue'))
const PreferencesDialog = defineAsyncComponent(() => import('./PreferencesDialog.vue'))
const VscodeDisguise = defineAsyncComponent(() => import('./VscodeDisguiseScreen.vue'))

defineProps<{
  authOpen: boolean
  createOpen: boolean
  forwardOpen: boolean
  forwardMessageIds: string[]
  forwardRooms: Room[]
  joinOpen: boolean
  manageOpen: boolean
  newConversationOpen: boolean
  preferencesOpen: boolean
  preferencesSaving: boolean
  preferences: ChatPreferences
  room: Room | null
  roomPassword: string
  token: string
  friends: SocialUser[]
  user: User | null
}>()
const emit = defineEmits<{
  authClose: []
  authenticated: [session: AuthSession]
  createClose: []
  created: [room: Room, password: string]
  forwardClose: []
  forwarded: []
  joinClose: []
  joined: [room: Room, password: string]
  manageClose: []
  updated: [result: RoomUpdateResult]
  deleted: [roomId: string]
  newConversationClose: []
  conversationOpened: [conversation: ConversationSummary]
  socialChanged: []
  createGroup: []
  preferencesClose: []
  savePreferences: [preferences: ChatPreferences]
}>()
</script>

<template>
  <AuthDialog :open="authOpen" @close="emit('authClose')" @authenticated="emit('authenticated', $event)" />
  <ForwardDialog
    :open="forwardOpen"
    :message-ids="forwardMessageIds"
    :rooms="forwardRooms"
    :token="token"
    @close="emit('forwardClose')"
    @forwarded="emit('forwarded')"
  />
  <JoinRoomDialog
    :open="joinOpen"
    :token="token"
    @close="emit('joinClose')"
    @joined="(room, password) => emit('joined', room, password)"
  />
  <CreateRoomDialog
    :open="createOpen"
    :token="token"
    @close="emit('createClose')"
    @created="(room, password) => emit('created', room, password)"
  />
  <ManageRoomDialog
    :open="manageOpen"
    :room="room"
    :credential="roomPassword"
    :token="token"
    @close="emit('manageClose')"
    @updated="emit('updated', $event)"
    @deleted="emit('deleted', $event)"
  />
  <NewConversationDialog
    :open="newConversationOpen"
    :token="token"
    :friends="friends"
    @close="emit('newConversationClose')"
    @opened="emit('conversationOpened', $event)"
    @social-changed="emit('socialChanged')"
    @create-group="emit('createGroup')"
  />
  <PreferencesDialog
    :open="preferencesOpen"
    :user="user"
    :preferences="preferences"
    :saving="preferencesSaving"
    @close="emit('preferencesClose')"
    @save="emit('savePreferences', $event)"
  />
  <VscodeDisguise :enabled="preferences.autoDisguiseEnabled" />
  <Toast position="top-right" />
</template>
