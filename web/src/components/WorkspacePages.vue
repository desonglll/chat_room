<script setup lang="ts">
import { defineAsyncComponent, type Ref } from 'vue'
import type { FavoriteForwardResult, FavoriteItem, FriendRequest, Room, SocialUser, User } from '../types'

const ContactsPage = defineAsyncComponent(() => import('./ContactsPage.vue'))
const DiscoverRooms = defineAsyncComponent(() => import('./DiscoverRooms.vue'))
const FavoritesPage = defineAsyncComponent(() => import('./FavoritesPage.vue'))
const ProfilePage = defineAsyncComponent(() => import('./ProfilePage.vue'))
const SettingsPage = defineAsyncComponent(() => import('./SettingsPage.vue'))

interface ContactsController {
  friends: Ref<SocialUser[]>
  incoming: Ref<FriendRequest[]>
  outgoing: Ref<FriendRequest[]>
  blocked: Ref<SocialUser[]>
  loading: Ref<boolean>
  error: Ref<string>
  respond: (userId: string, action: 'accept' | 'decline') => Promise<void>
  cancelRequest: (userId: string) => Promise<void>
  unblock: (userId: string) => Promise<void>
  setRemark: (userId: string, remark: string) => Promise<void>
}

interface FavoritesController {
  items: Ref<FavoriteItem[]>
  loading: Ref<boolean>
  error: Ref<string>
  create: (title: string, content: string) => Promise<FavoriteItem>
  remove: (id: string) => Promise<void>
  forward: (id: string, roomIds: string[]) => Promise<FavoriteForwardResult[]>
}

defineProps<{
  activePage: string
  user: User | null
  token: string
  contacts: ContactsController
  startChat: (userId: string) => Promise<void>
  removeFriend: (userId: string) => Promise<void>
  blockUser: (userId: string) => Promise<void>
  favorites: FavoritesController
  rooms: Room[]
  discoverLoading: boolean
  discoverJoiningId: string
  discoverError: string
  joinRoom: (room: Room) => Promise<void>
}>()
const emit = defineEmits<{
  back: []
  preferences: []
  deleted: []
  updated: [user: User]
  newChat: []
  authenticate: []
  conversationsChanged: []
  success: [message: string]
  error: [message: string]
}>()
</script>

<template>
  <ProfilePage
    v-if="activePage === 'profile' && user"
    :user="user"
    :token="token"
    @back="emit('back')"
    @updated="emit('updated', $event)"
  />
  <SettingsPage
    v-else-if="activePage === 'settings' && user"
    :user="user"
    :token="token"
    @back="emit('back')"
    @preferences="emit('preferences')"
    @deleted="emit('deleted')"
  />
  <ContactsPage
    v-else-if="activePage === 'contacts' && user"
    :friends="contacts.friends.value"
    :incoming="contacts.incoming.value"
    :outgoing="contacts.outgoing.value"
    :blocked="contacts.blocked.value"
    :loading="contacts.loading.value"
    :error="contacts.error.value"
    :start-chat="startChat"
    :respond="contacts.respond"
    :cancel-request="contacts.cancelRequest"
    :remove-friend="removeFriend"
    :block-user="blockUser"
    :unblock-user="contacts.unblock"
    :set-remark="contacts.setRemark"
    @back="emit('back')"
    @new-chat="emit('newChat')"
    @changed="emit('conversationsChanged')"
    @error="emit('error', $event)"
  />
  <FavoritesPage
    v-else-if="activePage === 'favorites' && user"
    :items="favorites.items.value"
    :rooms="rooms"
    :loading="favorites.loading.value"
    :error="favorites.error.value"
    :create="favorites.create"
    :remove="favorites.remove"
    :forward="favorites.forward"
    @back="emit('back')"
    @changed="emit('conversationsChanged')"
    @success="emit('success', $event)"
    @error="emit('error', $event)"
  />
  <DiscoverRooms
    v-else-if="activePage === 'discover'"
    :rooms="rooms"
    :user="user"
    :loading="discoverLoading"
    :joining-id="discoverJoiningId"
    :error="discoverError"
    @back="emit('back')"
    @join="joinRoom"
    @authenticate="emit('authenticate')"
  />
</template>
