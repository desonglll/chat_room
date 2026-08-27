<script setup lang="ts">
import {
  Compass,
  LockKeyhole,
  LogIn,
  LogOut,
  MessageCircle,
  Bookmark,
  Bot,
  PanelLeftClose,
  PanelLeftOpen,
  Plus,
  Search,
  Settings,
  UsersRound,
} from 'lucide-vue-next'
import Badge from 'primevue/badge'
import type { User } from '../types'
import AppAvatar from './AppAvatar.vue'

const props = defineProps<{
  activeSection: string
  user: User | null
  incomingRequests: number
  collapsed: boolean
}>()

const emit = defineEmits<{
  chat: []
  contacts: []
  favorites: []
  search: []
  assistant: []
  discover: []
  create: []
  authenticate: []
  profile: []
  lock: []
  settings: []
  logout: []
  toggleCollapse: []
}>()

function openContacts(): void {
  if (props.user) emit('contacts')
  else emit('authenticate')
}

function openFavorites(): void {
  if (props.user) emit('favorites')
  else emit('authenticate')
}

function openAssistant(): void {
  if (props.user) emit('assistant')
  else emit('authenticate')
}

function openSearch(): void {
  if (props.user) emit('search')
  else emit('authenticate')
}
</script>

<template>
  <nav class="cr-workspace-rail" aria-label="主导航">
    <button type="button" class="cr-rail-brand" aria-label="返回消息" title="消息" @click="emit('chat')">
      <img src="/brand/echo-gate.svg" alt="" width="36" height="36" aria-hidden="true" />
    </button>

    <div class="cr-rail-primary">
      <button
        type="button"
        class="cr-rail-action"
        :class="{ 'cr-rail-action--active': activeSection === 'assistant' }"
        :aria-current="activeSection === 'assistant' ? 'page' : undefined"
        aria-label="AI 助手"
        title="AI 助手"
        @click="openAssistant"
      >
        <Bot :size="20" aria-hidden="true" />
        <span>AI</span>
      </button>
      <button
        type="button"
        class="cr-rail-action"
        :class="{ 'cr-rail-action--active': activeSection === 'favorites' }"
        :aria-current="activeSection === 'favorites' ? 'page' : undefined"
        aria-label="收藏"
        title="收藏"
        @click="openFavorites"
      >
        <Bookmark :size="20" aria-hidden="true" />
        <span>收藏</span>
      </button>
      <button
        type="button"
        class="cr-rail-action"
        :class="{ 'cr-rail-action--active': activeSection === 'search' }"
        :aria-current="activeSection === 'search' ? 'page' : undefined"
        aria-label="搜索"
        title="搜索"
        @click="openSearch"
      >
        <Search :size="20" aria-hidden="true" />
        <span>搜索</span>
      </button>
      <button
        type="button"
        class="cr-rail-action"
        :class="{ 'cr-rail-action--active': activeSection === 'chat' }"
        :aria-current="activeSection === 'chat' ? 'page' : undefined"
        aria-label="消息"
        title="消息"
        @click="emit('chat')"
      >
        <MessageCircle :size="20" aria-hidden="true" />
        <span>消息</span>
      </button>
      <button
        type="button"
        class="cr-rail-action"
        :class="{ 'cr-rail-action--active': activeSection === 'contacts' }"
        :aria-current="activeSection === 'contacts' ? 'page' : undefined"
        aria-label="联系人"
        title="联系人"
        @click="openContacts"
      >
        <UsersRound :size="20" aria-hidden="true" />
        <span>联系人</span>
        <Badge
          v-if="incomingRequests"
          :value="incomingRequests > 99 ? '99+' : String(incomingRequests)"
          severity="danger"
          class="cr-rail-badge"
        />
      </button>
      <button
        type="button"
        class="cr-rail-action"
        :class="{ 'cr-rail-action--active': activeSection === 'discover' }"
        :aria-current="activeSection === 'discover' ? 'page' : undefined"
        aria-label="发现"
        title="发现"
        @click="emit('discover')"
      >
        <Compass :size="20" aria-hidden="true" />
        <span>发现</span>
      </button>
      <button
        type="button"
        class="cr-rail-action cr-rail-create"
        aria-label="创建群聊"
        title="创建群聊"
        @click="emit('create')"
      >
        <Plus :size="21" aria-hidden="true" />
        <span>创建</span>
      </button>
      <button
        type="button"
        class="cr-rail-action cr-rail-mobile-settings"
        :class="{ 'cr-rail-action--active': activeSection === 'settings' }"
        :aria-current="activeSection === 'settings' ? 'page' : undefined"
        aria-label="设置"
        title="设置"
        @click="emit('settings')"
      >
        <Settings :size="20" aria-hidden="true" />
        <span>设置</span>
      </button>
      <button
        v-if="user"
        type="button"
        class="cr-rail-action cr-rail-mobile-profile"
        :class="{ 'cr-rail-action--active': activeSection === 'profile' }"
        :aria-current="activeSection === 'profile' ? 'page' : undefined"
        aria-label="我的资料"
        title="我的资料"
        @click="emit('profile')"
      >
        <AppAvatar
          :avatar="user.avatar_emoji"
          :fallback="user.username"
          :color-key="user.id"
          class="size-5! text-[10px]! text-white!"
        />
        <span>我的</span>
      </button>
    </div>

    <div class="cr-rail-account">
      <button
        v-if="user"
        type="button"
        class="cr-rail-action cr-rail-avatar"
        :class="{ 'cr-rail-action--active': activeSection === 'profile' }"
        aria-label="我的资料"
        title="我的资料"
        @click="emit('profile')"
      >
        <AppAvatar :avatar="user.avatar_emoji" :fallback="user.username" :color-key="user.id" class="text-white!" />
      </button>
      <button
        v-else
        type="button"
        class="cr-rail-action"
        aria-label="登录或注册"
        title="登录或注册"
        @click="emit('authenticate')"
      >
        <LogIn :size="20" aria-hidden="true" />
      </button>
      <button
        v-if="user"
        type="button"
        class="cr-rail-action"
        aria-label="锁定界面"
        title="锁定界面"
        @click="emit('lock')"
      >
        <LockKeyhole :size="18" aria-hidden="true" />
      </button>
      <button
        type="button"
        class="cr-rail-action"
        :class="{ 'cr-rail-action--active': activeSection === 'settings' }"
        aria-label="设置"
        title="设置"
        @click="emit('settings')"
      >
        <Settings :size="19" aria-hidden="true" />
      </button>
      <button
        v-if="user"
        type="button"
        class="cr-rail-action"
        aria-label="退出登录"
        title="退出登录"
        @click="emit('logout')"
      >
        <LogOut :size="18" aria-hidden="true" />
      </button>
      <button
        type="button"
        class="cr-rail-action"
        :aria-label="collapsed ? '展开工作区' : '收起工作区'"
        :title="collapsed ? '展开工作区' : '收起工作区'"
        @click="emit('toggleCollapse')"
      >
        <PanelLeftOpen v-if="collapsed" :size="19" aria-hidden="true" />
        <PanelLeftClose v-else :size="19" aria-hidden="true" />
      </button>
    </div>
  </nav>
</template>
